use axum::{
    extract::{ws::{Message, WebSocket}, State, WebSocketUpgrade},
    http::HeaderMap,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use futures_util::{SinkExt, StreamExt};
use mongodb::{bson::doc, options::ReplaceOptions, Client, Collection};
use noise::{NoiseFn, Perlin};
use rand::Rng;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, RwLock};
use tower_http::services::ServeDir;
use tracing::{info, warn};
use uuid::Uuid;

const TICK_MS: u64 = 100;
const PLAYER_SPEED: f32 = 3.4;
const MONSTER_AGGRO_RANGE: f32 = 5.0;
const MONSTER_ATTACK_RANGE: f32 = 0.8;
const GATHER_RANGE: f32 = 1.1;
const INTERACT_RANGE: f32 = 1.2;
const SAVE_INTERVAL_MS: i64 = 5_000;
const MAX_HP: i32 = 10;
const TYPING_TIMEOUT_MS: i64 = 2500;

const TILE_GRASS: u8 = 0;
const TILE_WATER: u8 = 1;
const TILE_SAND: u8 = 2;
const TILE_DIRT: u8 = 3;

#[derive(Clone)]
struct AppState {
    state: Arc<RwLock<GameState>>,
    store: GameStore,
    data: Arc<GameData>,
    world: WorldConfig,
    noise: Arc<WorldNoise>,
}

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let world: WorldConfig = load_json("data/world.json")?;
    let data = Arc::new(load_game_data()?);
    let noise = Arc::new(WorldNoise::new(world.seed));
    let mongo_uri = std::env::var("MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let store = GameStore::new(&mongo_uri).await?;
    let state = Arc::new(RwLock::new(GameState::new()));

    let app_state = AppState {
        state: state.clone(),
        store,
        data,
        world: world.clone(),
        noise,
    };

    spawn_game_loop(app_state.clone());

    let app = Router::new()
        .route("/api/session", get(session_handler))
        .route("/ws", get(ws_handler))
        .nest_service("/", ServeDir::new("public").append_index_html_on_directories(true))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn session_handler(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> (CookieJar, Json<SessionResponse>) {
    let (sid, jar, _is_new) = ensure_session_cookie(jar);
    let doc = app_state
        .store
        .load_or_create_player(&sid, &app_state.world, &app_state.noise)
        .await
        .unwrap_or_else(|err| {
            warn!("session load failed: {}", err);
            default_player_doc(&sid, &app_state.world, &app_state.noise)
        });

    let response = SessionResponse {
        session_id: sid,
        name: doc.name,
    };
    (jar, Json(response))
}

async fn ws_handler(
    State(app_state): State<AppState>,
    ws: WebSocketUpgrade,
    headers: HeaderMap,
) -> impl IntoResponse {
    let sid = extract_session_id(&headers).unwrap_or_else(|| Uuid::new_v4().to_string());
    ws.on_upgrade(move |socket| handle_socket(socket, app_state, sid))
}

async fn handle_socket(socket: WebSocket, app_state: AppState, sid: String) {
    let (mut socket_sender, mut socket_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    {
        let mut state = app_state.state.write().await;
        state.clients.insert(sid.clone(), tx);
    }

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let payload = match serde_json::to_string(&msg) {
                Ok(text) => text,
                Err(err) => {
                    warn!("serialize message failed: {}", err);
                    continue;
                }
            };
            if socket_sender.send(Message::Text(payload)).await.is_err() {
                break;
            }
        }
    });

    let doc = app_state
        .store
        .load_or_create_player(&sid, &app_state.world, &app_state.noise)
        .await
        .unwrap_or_else(|err| {
            warn!("player load failed: {}", err);
            default_player_doc(&sid, &app_state.world, &app_state.noise)
        });

    let welcome_msg = {
        let mut state = app_state.state.write().await;
        state
            .inputs
            .entry(sid.clone())
            .or_insert(InputState::default());
        let player = state
            .players
            .entry(sid.clone())
            .or_insert_with(|| Player::from_doc(doc.clone()));
        player.sync_from_doc(&doc);
        ServerMessage::Welcome {
            player: player.self_view(),
            world: app_state.world.clone(),
            npcs: app_state
                .data
                .npcs
                .iter()
                .cloned()
                .map(NpcPublic::from)
                .collect(),
        }
    };

    send_to_player(&app_state.state, &sid, welcome_msg).await;

    while let Some(Ok(msg)) = socket_receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    handle_client_message(&app_state, &sid, client_msg).await;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    {
        let mut state = app_state.state.write().await;
        state.clients.remove(&sid);
        state.inputs.remove(&sid);
        if let Some(player) = state.players.remove(&sid) {
            let doc = player.to_doc();
            let store = app_state.store.clone();
            tokio::spawn(async move {
                let _ = store.save_player(&doc).await;
            });
        }
        if state.typing.remove(&sid).is_some() {
            broadcast_message_inline(
                &state,
                ServerMessage::Typing {
                    id: sid.clone(),
                    typing: false,
                },
            );
        }
    }

    let _ = send_task.await;
}

async fn handle_client_message(app_state: &AppState, sid: &str, msg: ClientMessage) {
    match msg {
        ClientMessage::Input {
            dir_x,
            dir_y,
            attack,
            gather,
            interact,
        } => {
            let mut state = app_state.state.write().await;
            let entry = state.inputs.entry(sid.to_string()).or_insert(InputState::default());
            entry.dir_x = dir_x.clamp(-1.0, 1.0);
            entry.dir_y = dir_y.clamp(-1.0, 1.0);
            entry.attack = attack;
            entry.gather = gather;
            entry.interact = interact;
        }
        ClientMessage::Chat { text } => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }
            let trimmed = trimmed.chars().take(160).collect::<String>();
            let (sender_name, was_typing) = {
                let mut state = app_state.state.write().await;
                let name = state
                    .players
                    .get(sid)
                    .map(|player| player.name.clone())
                    .unwrap_or_else(|| "Wanderer".to_string());
                let was_typing = state.typing.remove(sid).is_some();
                (name, was_typing)
            };
            broadcast_message(
                &app_state.state,
                ServerMessage::Chat {
                    from: sender_name,
                    text: trimmed,
                },
            )
            .await;
            if was_typing {
                broadcast_message(
                    &app_state.state,
                    ServerMessage::Typing {
                        id: sid.to_string(),
                        typing: false,
                    },
                )
                .await;
            }
        }
        ClientMessage::Typing { typing } => {
            let now_ms = now_millis();
            let mut state = app_state.state.write().await;
            let mut should_broadcast = false;
            if typing {
                let was_typing = state.typing.contains_key(sid);
                state.typing.insert(sid.to_string(), now_ms);
                if !was_typing {
                    should_broadcast = true;
                }
            } else if state.typing.remove(sid).is_some() {
                should_broadcast = true;
            }
            if should_broadcast {
                broadcast_message_inline(
                    &state,
                    ServerMessage::Typing {
                        id: sid.to_string(),
                        typing,
                    },
                );
            }
        }
        ClientMessage::ChunkRequest { chunks } => {
            handle_chunk_request(app_state, sid, chunks).await;
        }
        ClientMessage::Ping => {}
    }
}

async fn handle_chunk_request(app_state: &AppState, sid: &str, chunks: Vec<ChunkCoord>) {
    let mut state = app_state.state.write().await;
    let sender = match state.clients.get(sid) {
        Some(sender) => sender.clone(),
        None => return,
    };

    for coord in chunks {
        if !state.spawned_chunks.contains(&coord) {
            spawn_monsters_for_chunk(
                &mut state,
                app_state.world.seed,
                coord,
                &app_state.world,
                &app_state.noise,
                &app_state.data,
            );
            state.spawned_chunks.insert(coord);
        }

        let resources = state.resources.entry(coord).or_insert_with(|| {
            generate_resources(
                app_state.world.seed,
                coord,
                &app_state.world,
                &app_state.noise,
                &app_state.data,
            )
        });

        let tiles = generate_tiles(coord, &app_state.world, &app_state.noise);
        let visible_resources = resources
            .iter()
            .filter(|res| res.hp > 0)
            .map(ResourceNodePublic::from)
            .collect();

        let _ = sender.send(ServerMessage::ChunkData {
            chunk_x: coord.x,
            chunk_y: coord.y,
            tiles,
            resources: visible_resources,
        });
    }
}

fn spawn_game_loop(app_state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_MS));
        loop {
            interval.tick().await;
            let now = now_millis();
            if let Err(err) = game_tick(&app_state, now).await {
                warn!("game tick failed: {}", err);
            }
        }
    });
}

async fn game_tick(app_state: &AppState, now_ms: i64) -> AppResult<()> {
    let mut to_save = Vec::new();
    {
        let mut state = app_state.state.write().await;
        let dt = TICK_MS as f32 / 1000.0;

        let player_ids: Vec<String> = state.players.keys().cloned().collect();
        for id in player_ids {
            let input = state.inputs.get(&id).cloned().unwrap_or_default();
            if let Some(mut player) = state.players.remove(&id) {
                update_player_movement(&mut player, input, &app_state.world, &app_state.noise, dt);
                handle_player_actions(
                    &mut player,
                    input,
                    now_ms,
                    &mut state,
                    &app_state.world,
                    &app_state.noise,
                    &app_state.data,
                );
                state.players.insert(id, player);
            }
        }

        update_monsters(
            &mut state,
            now_ms,
            dt,
            &app_state.world,
            &app_state.noise,
            &app_state.data,
        );
        update_projectiles(&mut state, now_ms, dt, &app_state.data);
        update_resources(&mut state, now_ms, &app_state.data);

        let mut expired_typing = Vec::new();
        for (id, last) in state.typing.iter() {
            if now_ms - *last > TYPING_TIMEOUT_MS {
                expired_typing.push(id.clone());
            }
        }
        for id in expired_typing {
            state.typing.remove(&id);
            broadcast_message_inline(
                &state,
                ServerMessage::Typing {
                    id,
                    typing: false,
                },
            );
        }

        let state_msg = ServerMessage::State {
            players: state.players.values().map(PlayerPublic::from).collect(),
            monsters: state
                .monsters
                .values()
                .map(MonsterPublic::from)
                .collect(),
            projectiles: state
                .projectiles
                .values()
                .map(ProjectilePublic::from)
                .collect(),
        };
        for sender in state.clients.values() {
            let _ = sender.send(state_msg.clone());
        }

        for player in state.players.values_mut() {
            if now_ms - player.last_saved_ms >= SAVE_INTERVAL_MS {
                player.last_saved_ms = now_ms;
                to_save.push(player.to_doc());
            }
        }
    }

    for doc in to_save {
        let _ = app_state.store.save_player(&doc).await;
    }

    Ok(())
}

fn update_player_movement(
    player: &mut Player,
    input: InputState,
    world: &WorldConfig,
    noise: &WorldNoise,
    dt: f32,
) {
    let mut dx = input.dir_x;
    let mut dy = input.dir_y;
    let len = (dx * dx + dy * dy).sqrt();
    if len > 0.01 {
        dx /= len;
        dy /= len;
        player.face_x = dx;
        player.face_y = dy;
    }

    let next_x = player.x + dx * PLAYER_SPEED * dt;
    let next_y = player.y + dy * PLAYER_SPEED * dt;

    if can_walk(world, noise, next_x, player.y) {
        player.x = next_x;
    }
    if can_walk(world, noise, player.x, next_y) {
        player.y = next_y;
    }
}

fn handle_player_actions(
    player: &mut Player,
    input: InputState,
    now_ms: i64,
    state: &mut GameState,
    world: &WorldConfig,
    noise: &WorldNoise,
    data: &GameData,
) {
    if input.gather && now_ms - player.last_gather_ms >= 400 {
        let mut messages = Vec::new();
        let mut resource_update: Option<(ResourceNodePublic, String)> = None;
        let mut did_gather = false;

        {
            if let Some((resource, def)) = find_nearby_resource(player, state, data) {
                did_gather = true;
                let tool_power = best_tool_power(&player.inventory, data, &def.tool);
                if let Some(power) = tool_power {
                    resource.hp -= power;
                    if resource.hp <= 0 {
                        resource.hp = 0;
                        resource.respawn_at_ms = Some(now_ms + def.respawn_ms);
                        for drop in &def.drops {
                            add_item(&mut player.inventory, &drop.id, drop.count);
                            messages.push(format!("Collected {} x{}", drop.id, drop.count));
                        }
                        resource_update = Some((
                            ResourceNodePublic::from(resource.clone()),
                            "removed".to_string(),
                        ));
                    } else {
                        messages.push(format!("Hit {} ({})", def.name, resource.hp));
                    }
                } else {
                    messages.push(format!("You need a {}", def.tool));
                }
            }
        }

        if did_gather {
            player.last_gather_ms = now_ms;
        }
        for text in messages {
            send_system_message(state, &player.id, text);
        }
        if let Some((resource, state_label)) = resource_update {
            broadcast_message_inline(
                state,
                ServerMessage::ResourceUpdate {
                    resource,
                    state: state_label,
                },
            );
        }
    }

    if input.attack {
        if let Some(weapon) = best_melee_weapon(&player.inventory, data) {
            if now_ms - player.last_attack_ms >= weapon.cooldown_ms {
                if attack_monster_melee(player, state, &weapon, data) {
                    player.last_attack_ms = now_ms;
                } else if try_ranged_attack(player, state, data, now_ms) {
                    player.last_attack_ms = now_ms;
                }
            }
        } else if try_ranged_attack(player, state, data, now_ms) {
            player.last_attack_ms = now_ms;
        }
    }

    if input.interact && now_ms - player.last_interact_ms >= 500 {
        if let Some(npc) = find_nearby_npc(player, data) {
            handle_npc_interaction(player, npc, state, data);
            player.last_interact_ms = now_ms;
        }
    }

    if player.hp <= 0 {
        player.hp = MAX_HP;
        let (spawn_x, spawn_y) = safe_spawn(world, noise);
        player.x = spawn_x;
        player.y = spawn_y;
        send_system_message(state, &player.id, "You wake up by the campfire.".to_string());
    }
}

fn update_monsters(
    state: &mut GameState,
    now_ms: i64,
    dt: f32,
    world: &WorldConfig,
    noise: &WorldNoise,
    data: &GameData,
) {
    let player_positions: Vec<(String, f32, f32)> = state
        .players
        .values()
        .map(|p| (p.id.clone(), p.x, p.y))
        .collect();

    let mut damage_events: Vec<(String, i32, String)> = Vec::new();
    for monster in state.monsters.values_mut() {
        let def = match data.monsters.get(&monster.kind) {
            Some(def) => def,
            None => continue,
        };
        let mut target = None;
        let mut nearest_dist = f32::MAX;
        for (id, px, py) in &player_positions {
            let dist = distance(*px, *py, monster.x, monster.y);
            if dist < nearest_dist {
                nearest_dist = dist;
                target = Some(id.clone());
            }
        }

        if let Some(target_id) = target {
            if nearest_dist <= MONSTER_AGGRO_RANGE {
                monster.target = Some(target_id.clone());
                let (tx, ty) = player_positions
                    .iter()
                    .find(|(id, _, _)| id == &target_id)
                    .map(|(_, x, y)| (*x, *y))
                    .unwrap_or((monster.x, monster.y));
                move_towards(monster, tx, ty, def.speed, dt, world, noise);

                if nearest_dist <= MONSTER_ATTACK_RANGE
                    && now_ms - monster.last_attack_ms >= 800
                {
                    damage_events.push((target_id.clone(), def.damage, def.name.clone()));
                    monster.last_attack_ms = now_ms;
                }
            } else {
                wander(monster, now_ms, def.speed, dt, world, noise);
            }
        } else {
            wander(monster, now_ms, def.speed, dt, world, noise);
        }
    }

    for (player_id, damage, monster_name) in damage_events {
        let message = if let Some(player) = state.players.get_mut(&player_id) {
            player.hp -= damage;
            Some((player.id.clone(), format!("{} hits you ({})", monster_name, player.hp.max(0))))
        } else {
            None
        };
        if let Some((target_id, text)) = message {
            send_system_message(state, &target_id, text);
        }
    }
}

fn update_projectiles(state: &mut GameState, _now_ms: i64, dt: f32, data: &GameData) {
    let mut to_remove = Vec::new();
    let mut killed = HashSet::new();

    {
        let (monsters, projectiles) = (&mut state.monsters, &mut state.projectiles);
        for (id, projectile) in projectiles.iter_mut() {
            projectile.x += projectile.vx * dt;
            projectile.y += projectile.vy * dt;
            projectile.ttl_ms -= (dt * 1000.0) as i64;
            if projectile.ttl_ms <= 0 {
                to_remove.push(*id);
                continue;
            }

            let mut hit = None;
            for (monster_id, monster) in monsters.iter_mut() {
                let dist = distance(projectile.x, projectile.y, monster.x, monster.y);
                if dist < 0.5 {
                    monster.hp -= projectile.damage;
                    hit = Some(*monster_id);
                    break;
                }
            }

            if let Some(monster_id) = hit {
                to_remove.push(*id);
                if let Some(monster) = monsters.get(&monster_id) {
                    if monster.hp <= 0 {
                        killed.insert(monster_id);
                    }
                }
            }
        }

        for id in &to_remove {
            projectiles.remove(id);
        }
    }

    for monster_id in killed {
        handle_monster_death(state, monster_id, data, None);
    }
}

fn update_resources(state: &mut GameState, now_ms: i64, data: &GameData) {
    let mut respawned = Vec::new();
    for resources in state.resources.values_mut() {
        for res in resources.iter_mut() {
            if res.hp <= 0 {
                if let Some(respawn_at) = res.respawn_at_ms {
                    if now_ms >= respawn_at {
                        if let Some(def) = data.resources.get(&res.kind) {
                            res.hp = def.hp;
                            res.respawn_at_ms = None;
                            respawned.push(ResourceNodePublic::from(res.clone()));
                        }
                    }
                }
            }
        }
    }

    if !respawned.is_empty() {
        for res in respawned {
            broadcast_message_inline(
                state,
                ServerMessage::ResourceUpdate {
                    resource: res,
                    state: "spawned".to_string(),
                },
            );
        }
    }
}

fn attack_monster_melee(
    player: &mut Player,
    state: &mut GameState,
    weapon: &WeaponStats,
    data: &GameData,
) -> bool {
    let mut target_id = None;
    let mut nearest_dist = f32::MAX;
    for (id, monster) in state.monsters.iter() {
        let dist = distance(player.x, player.y, monster.x, monster.y);
        if dist < weapon.range && dist < nearest_dist {
            nearest_dist = dist;
            target_id = Some(*id);
        }
    }

    if let Some(monster_id) = target_id {
        let mut message = None;
        let mut killed = false;
        if let Some(monster) = state.monsters.get_mut(&monster_id) {
            monster.hp -= weapon.damage;
            message = Some(format!("Hit {} ({})", monster.kind, monster.hp.max(0)));
            if monster.hp <= 0 {
                killed = true;
            }
        }
        if let Some(text) = message {
            send_system_message(state, &player.id, text);
        }
        if killed {
            handle_monster_death(state, monster_id, data, Some(player));
        }
        return true;
    }
    false
}

fn try_ranged_attack(
    player: &mut Player,
    state: &mut GameState,
    data: &GameData,
    now_ms: i64,
) -> bool {
    let (weapon, ammo_id) = match best_ranged_weapon(&player.inventory, data) {
        Some(result) => result,
        None => return false,
    };

    if now_ms - player.last_attack_ms < weapon.cooldown_ms {
        return false;
    }

    if !consume_item(&mut player.inventory, &ammo_id, 1) {
        send_system_message(
            state,
            &player.id,
            format!("Out of {}", ammo_id),
        );
        return false;
    }

    let speed = weapon.projectile_speed.unwrap_or(7.0);
    let dir_len = (player.face_x * player.face_x + player.face_y * player.face_y).sqrt();
    let (dir_x, dir_y) = if dir_len > 0.01 {
        (player.face_x / dir_len, player.face_y / dir_len)
    } else {
        (1.0, 0.0)
    };

    let proj_id = state.next_id();
    state.projectiles.insert(
        proj_id,
        Projectile {
            id: proj_id,
            x: player.x + dir_x * 0.6,
            y: player.y + dir_y * 0.6,
            vx: dir_x * speed,
            vy: dir_y * speed,
            ttl_ms: 1200,
            damage: weapon.damage,
        },
    );

    true
}

fn handle_monster_death(
    state: &mut GameState,
    monster_id: u64,
    data: &GameData,
    mut award_to: Option<&mut Player>,
) {
    if let Some(monster) = state.monsters.remove(&monster_id) {
        if let Some(def) = data.monsters.get(&monster.kind) {
            if let Some(drop) = &def.drop {
                let message = format!("Picked up {} x{}", drop.id, drop.count);
                if let Some(player) = award_to.as_deref_mut() {
                    add_item(&mut player.inventory, &drop.id, drop.count);
                    let player_id = player.id.clone();
                    send_system_message(state, &player_id, message);
                } else {
                    let mut awarded_to = None;
                    for player in state.players.values_mut() {
                        let dist = distance(player.x, player.y, monster.x, monster.y);
                        if dist < 2.0 {
                            add_item(&mut player.inventory, &drop.id, drop.count);
                            awarded_to = Some(player.id.clone());
                            break;
                        }
                    }
                    if let Some(player_id) = awarded_to {
                        send_system_message(state, &player_id, message);
                    }
                }
            }
        }
    }
}

fn handle_npc_interaction(player: &mut Player, npc: &NpcDef, state: &mut GameState, data: &GameData) {
    if let Some(quest) = data.quests_by_npc.get(&npc.id) {
        if player.completed_quests.contains(&quest.id) {
            send_dialog(state, &player.id, &npc.name, "Thanks again for your help.");
            return;
        }

        if has_items(&player.inventory, &quest.requires) {
            remove_items(&mut player.inventory, &quest.requires);
            for reward in &quest.rewards {
                add_item(&mut player.inventory, &reward.id, reward.count);
            }
            player.completed_quests.insert(quest.id.clone());
            send_dialog(
                state,
                &player.id,
                &npc.name,
                &format!("Quest complete! {}", quest.description),
            );
            for reward in &quest.rewards {
                send_system_message(
                    state,
                    &player.id,
                    format!("Reward: {} x{}", reward.id, reward.count),
                );
            }
        } else {
            let mut needs = Vec::new();
            for req in &quest.requires {
                let have = player.inventory.get(&req.id).copied().unwrap_or(0);
                needs.push(format!("{} {}/{}", req.id, have, req.count));
            }
            send_dialog(
                state,
                &player.id,
                &npc.name,
                &format!("{}\nNeeds: {}", quest.description, needs.join(", ")),
            );
        }
    } else {
        send_dialog(state, &player.id, &npc.name, &npc.dialog);
    }
}

fn send_dialog(state: &mut GameState, player_id: &str, title: &str, text: &str) {
    if let Some(sender) = state.clients.get(player_id) {
        let _ = sender.send(ServerMessage::Dialog {
            title: title.to_string(),
            text: text.to_string(),
        });
    }
}

fn send_system_message(state: &mut GameState, player_id: &str, text: String) {
    if let Some(sender) = state.clients.get(player_id) {
        let _ = sender.send(ServerMessage::System { text });
    }
}

fn find_nearby_resource<'a>(
    player: &Player,
    state: &'a mut GameState,
    data: &'a GameData,
) -> Option<(&'a mut ResourceNode, &'a ResourceDef)> {
    for resources in state.resources.values_mut() {
        for res in resources.iter_mut() {
            if res.hp <= 0 {
                continue;
            }
            let dist = distance(player.x, player.y, res.x as f32 + 0.5, res.y as f32 + 0.5);
            if dist <= GATHER_RANGE {
                if let Some(def) = data.resources.get(&res.kind) {
                    return Some((res, def));
                }
            }
        }
    }
    None
}

fn find_nearby_npc<'a>(player: &Player, data: &'a GameData) -> Option<&'a NpcDef> {
    for npc in &data.npcs {
        let dist = distance(player.x, player.y, npc.x, npc.y);
        if dist <= INTERACT_RANGE {
            return Some(npc);
        }
    }
    None
}

fn best_tool_power(
    inventory: &HashMap<String, i32>,
    data: &GameData,
    tool: &str,
) -> Option<i32> {
    let mut best = None;
    for (item_id, count) in inventory {
        if *count <= 0 {
            continue;
        }
        if let Some(def) = data.items.get(item_id) {
            if def.tool.as_deref() == Some(tool) {
                let power = def.power.unwrap_or(1);
                if best.map_or(true, |current| power > current) {
                    best = Some(power);
                }
            }
        }
    }
    best
}

fn best_melee_weapon(inventory: &HashMap<String, i32>, data: &GameData) -> Option<WeaponStats> {
    let mut best: Option<WeaponStats> = None;
    for (item_id, count) in inventory {
        if *count <= 0 {
            continue;
        }
        if let Some(def) = data.items.get(item_id) {
            if let Some(weapon) = &def.weapon {
                if weapon.kind == "melee" {
                    if best
                        .as_ref()
                        .map_or(true, |current| weapon.damage > current.damage)
                    {
                        best = Some(weapon.clone());
                    }
                }
            }
        }
    }
    best
}

fn best_ranged_weapon(
    inventory: &HashMap<String, i32>,
    data: &GameData,
) -> Option<(WeaponStats, String)> {
    let mut best: Option<(WeaponStats, String)> = None;
    for (item_id, count) in inventory {
        if *count <= 0 {
            continue;
        }
        if let Some(def) = data.items.get(item_id) {
            if let Some(weapon) = &def.weapon {
                if weapon.kind == "ranged" {
                    let ammo_id = def.ammo_for.clone().unwrap_or_else(|| "arrow".to_string());
                    if best
                        .as_ref()
                        .map_or(true, |current| weapon.damage > current.0.damage)
                    {
                        best = Some((weapon.clone(), ammo_id));
                    }
                }
            }
        }
    }
    best
}

fn has_items(inventory: &HashMap<String, i32>, items: &[ItemStack]) -> bool {
    items.iter().all(|item| inventory.get(&item.id).copied().unwrap_or(0) >= item.count)
}

fn remove_items(inventory: &mut HashMap<String, i32>, items: &[ItemStack]) -> bool {
    if !has_items(inventory, items) {
        return false;
    }
    for item in items {
        consume_item(inventory, &item.id, item.count);
    }
    true
}

fn add_item(inventory: &mut HashMap<String, i32>, item_id: &str, count: i32) {
    let entry = inventory.entry(item_id.to_string()).or_insert(0);
    *entry += count;
}

fn consume_item(inventory: &mut HashMap<String, i32>, item_id: &str, count: i32) -> bool {
    let entry = inventory.entry(item_id.to_string()).or_insert(0);
    if *entry < count {
        return false;
    }
    *entry -= count;
    true
}

fn move_towards(
    monster: &mut Monster,
    tx: f32,
    ty: f32,
    speed: f32,
    dt: f32,
    world: &WorldConfig,
    noise: &WorldNoise,
) {
    let dx = tx - monster.x;
    let dy = ty - monster.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len > 0.01 {
        let vx = dx / len * speed * dt;
        let vy = dy / len * speed * dt;
        let next_x = monster.x + vx;
        let next_y = monster.y + vy;
        if can_walk(world, noise, next_x, monster.y) {
            monster.x = next_x;
        }
        if can_walk(world, noise, monster.x, next_y) {
            monster.y = next_y;
        }
    }
}

fn wander(
    monster: &mut Monster,
    now_ms: i64,
    speed: f32,
    dt: f32,
    world: &WorldConfig,
    noise: &WorldNoise,
) {
    if now_ms >= monster.wander_until_ms {
        let mut rng = rand::thread_rng();
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        monster.wander_dir = (angle.cos(), angle.sin());
        monster.wander_until_ms = now_ms + 1200;
    }
    let (dx, dy) = monster.wander_dir;
    let next_x = monster.x + dx * speed * 0.4 * dt;
    let next_y = monster.y + dy * speed * 0.4 * dt;
    if can_walk(world, noise, next_x, monster.y) {
        monster.x = next_x;
    }
    if can_walk(world, noise, monster.x, next_y) {
        monster.y = next_y;
    }
}

fn distance(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    (dx * dx + dy * dy).sqrt()
}

fn can_walk(_world: &WorldConfig, noise: &WorldNoise, x: f32, y: f32) -> bool {
    tile_at(noise, x.floor() as i32, y.floor() as i32) != TILE_WATER
}

fn generate_tiles(coord: ChunkCoord, world: &WorldConfig, noise: &WorldNoise) -> Vec<u8> {
    let chunk_size = world.chunk_size;
    let mut tiles = Vec::with_capacity((chunk_size * chunk_size) as usize);
    for y in 0..chunk_size {
        for x in 0..chunk_size {
            let wx = coord.x * chunk_size + x;
            let wy = coord.y * chunk_size + y;
            tiles.push(tile_at(noise, wx, wy));
        }
    }
    tiles
}

fn generate_resources(
    seed: u64,
    coord: ChunkCoord,
    world: &WorldConfig,
    noise: &WorldNoise,
    data: &GameData,
) -> Vec<ResourceNode> {
    let chunk_size = world.chunk_size;
    let mut resources = Vec::new();
    for y in 0..chunk_size {
        for x in 0..chunk_size {
            let wx = coord.x * chunk_size + x;
            let wy = coord.y * chunk_size + y;
            let tile = tile_at(noise, wx, wy);
            if tile == TILE_WATER {
                continue;
            }
            let elevation = noise.elevation(wx as f32, wy as f32);
            let moisture = noise.moisture(wx as f32, wy as f32);
            let tree_density = noise.tree_density(wx as f32, wy as f32);
            let rock_density = noise.rock_density(wx as f32, wy as f32);

            let mut kind = None;
            if tile == TILE_GRASS || tile == TILE_DIRT {
                let tree_score = tree_density + moisture * 0.25;
                let tree_roll = noise_hash01(seed, wx, wy);
                if tree_score > 0.25 && tree_roll < (tree_score * 0.45 + 0.1) {
                    kind = Some("tree");
                }
            }

            if kind.is_none() && tile != TILE_SAND {
                let rock_score = rock_density + elevation * 0.2;
                let rock_roll = noise_hash01(seed.wrapping_add(991), wx, wy);
                if rock_score > 0.48 && rock_roll < (rock_score * 0.4 + 0.05) {
                    kind = Some("rock");
                }
            }

            if let Some(kind) = kind {
                if let Some(def) = data.resources.get(kind) {
                    let id = hash_u64(
                        seed ^ (wx as u64).wrapping_mul(0x9E3779B97F4A7C15)
                            ^ (wy as u64).wrapping_mul(0xC2B2AE3D27D4EB4F),
                    );
                    resources.push(ResourceNode {
                        id,
                        kind: kind.to_string(),
                        x: wx,
                        y: wy,
                        hp: def.hp,
                        respawn_at_ms: None,
                    });
                }
            }
        }
    }
    resources
}

fn spawn_monsters_for_chunk(
    state: &mut GameState,
    seed: u64,
    coord: ChunkCoord,
    world: &WorldConfig,
    noise: &WorldNoise,
    data: &GameData,
) {
    let chunk_size = world.chunk_size;
    let base = hash_u64(seed ^ (coord.x as u64).wrapping_mul(0xD1B54A32) ^ coord.y as u64);
    let count = (base % 3) as i32;
    for i in 0..count {
        let local_seed = hash_u64(base.wrapping_add(i as u64));
        let lx = (local_seed % chunk_size as u64) as i32;
        let ly = ((local_seed >> 8) % chunk_size as u64) as i32;
        let wx = coord.x * chunk_size + lx;
        let wy = coord.y * chunk_size + ly;
        if tile_at(noise, wx, wy) == TILE_WATER {
            continue;
        }
        let monster_id = state.next_id();
        state.monsters.insert(
            monster_id,
            Monster {
                id: monster_id,
                kind: "slime".to_string(),
                x: wx as f32 + 0.5,
                y: wy as f32 + 0.5,
                hp: data
                    .monsters
                    .get("slime")
                    .map(|m| m.hp)
                    .unwrap_or(6),
                target: None,
                wander_dir: (0.0, 0.0),
                wander_until_ms: 0,
                last_attack_ms: 0,
            },
        );
    }
}

fn tile_at(noise: &WorldNoise, x: i32, y: i32) -> u8 {
    let elevation = noise.elevation(x as f32, y as f32);
    let moisture = noise.moisture(x as f32, y as f32);
    let soil = noise.soil(x as f32, y as f32);
    let river = noise.river(x as f32, y as f32).abs();

    let water_level = -0.18;
    let shore_level = -0.08;
    let river_mask = river < 0.06 && elevation < 0.35;

    if elevation < water_level || river_mask {
        TILE_WATER
    } else if elevation < shore_level {
        TILE_SAND
    } else if moisture < -0.55 && elevation < 0.4 {
        TILE_SAND
    } else if soil > 0.45 && moisture > -0.2 {
        TILE_DIRT
    } else {
        TILE_GRASS
    }
}

fn noise_hash01(seed: u64, x: i32, y: i32) -> f32 {
    let value = hash_u64(
        seed ^ (x as u64).wrapping_mul(0x9E3779B97F4A7C15)
            ^ (y as u64).wrapping_mul(0xC2B2AE3D27D4EB4F),
    );
    (value % 10_000) as f32 / 10_000.0
}

fn hash_u64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn load_json<T: DeserializeOwned>(path: &str) -> AppResult<T> {
    let data = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

fn load_game_data() -> AppResult<GameData> {
    let items: Vec<ItemDef> = load_json("data/items.json")?;
    let resources: Vec<ResourceDef> = load_json("data/resources.json")?;
    let monsters: Vec<MonsterDef> = load_json("data/monsters.json")?;
    let quests: Vec<QuestDef> = load_json("data/quests.json")?;
    let npcs: Vec<NpcDef> = load_json("data/npcs.json")?;

    Ok(GameData::new(items, resources, monsters, quests, npcs))
}

fn ensure_session_cookie(jar: CookieJar) -> (String, CookieJar, bool) {
    if let Some(cookie) = jar.get("sid") {
        (cookie.value().to_string(), jar, false)
    } else {
        let sid = Uuid::new_v4().to_string();
        let cookie = Cookie::build(("sid", sid.clone()))
            .path("/")
            .http_only(true)
            .same_site(SameSite::Lax)
            .build();
        (sid, jar.add(cookie), true)
    }
}

fn extract_session_id(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    cookie_header
        .split(';')
        .find_map(|cookie| {
            let mut parts = cookie.trim().splitn(2, '=');
            let name = parts.next()?;
            let value = parts.next()?;
            if name == "sid" {
                Some(value.to_string())
            } else {
                None
            }
        })
}

fn default_player_doc(id: &str, world: &WorldConfig, noise: &WorldNoise) -> PlayerDoc {
    let mut inventory = HashMap::new();
    inventory.insert("basic_axe".to_string(), 1);
    inventory.insert("basic_pick".to_string(), 1);
    inventory.insert("rusty_sword".to_string(), 1);
    let (spawn_x, spawn_y) = safe_spawn(world, noise);
    PlayerDoc {
        id: id.to_string(),
        name: random_name(),
        x: spawn_x,
        y: spawn_y,
        hp: MAX_HP,
        inventory,
        completed_quests: Vec::new(),
    }
}

fn random_name() -> String {
    let mut rng = rand::thread_rng();
    format!("Adventurer{}", rng.gen_range(1000..9999))
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as i64
}

fn safe_spawn(world: &WorldConfig, noise: &WorldNoise) -> (f32, f32) {
    let base_x = world.spawn_x.round() as i32;
    let base_y = world.spawn_y.round() as i32;
    if tile_at(noise, base_x, base_y) != TILE_WATER {
        return (world.spawn_x, world.spawn_y);
    }

    for radius in 1..=8 {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let x = base_x + dx;
                let y = base_y + dy;
                if tile_at(noise, x, y) != TILE_WATER {
                    return (x as f32 + 0.5, y as f32 + 0.5);
                }
            }
        }
    }

    (world.spawn_x, world.spawn_y)
}

async fn send_to_player(state: &Arc<RwLock<GameState>>, player_id: &str, msg: ServerMessage) {
    let state = state.read().await;
    if let Some(sender) = state.clients.get(player_id) {
        let _ = sender.send(msg);
    }
}

async fn broadcast_message(state: &Arc<RwLock<GameState>>, msg: ServerMessage) {
    let state = state.read().await;
    for sender in state.clients.values() {
        let _ = sender.send(msg.clone());
    }
}

fn broadcast_message_inline(state: &GameState, msg: ServerMessage) {
    for sender in state.clients.values() {
        let _ = sender.send(msg.clone());
    }
}

#[derive(Clone)]
struct GameStore {
    players: Collection<PlayerDoc>,
}

impl GameStore {
    async fn new(uri: &str) -> AppResult<Self> {
        let client = Client::with_uri_str(uri).await?;
        let db = client.database("onlinerpg");
        Ok(Self {
            players: db.collection::<PlayerDoc>("players"),
        })
    }

    async fn load_player(&self, id: &str) -> AppResult<Option<PlayerDoc>> {
        Ok(self.players.find_one(doc! { "_id": id }, None).await?)
    }

    async fn load_or_create_player(
        &self,
        id: &str,
        world: &WorldConfig,
        noise: &WorldNoise,
    ) -> AppResult<PlayerDoc> {
        if let Some(doc) = self.load_player(id).await? {
            Ok(doc)
        } else {
            let doc = default_player_doc(id, world, noise);
            let _ = self.save_player(&doc).await;
            Ok(doc)
        }
    }

    async fn save_player(&self, doc: &PlayerDoc) -> AppResult<()> {
        let opts = ReplaceOptions::builder().upsert(true).build();
        self.players
            .replace_one(doc! { "_id": &doc.id }, doc, opts)
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerDoc {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    x: f32,
    y: f32,
    hp: i32,
    inventory: HashMap<String, i32>,
    completed_quests: Vec<String>,
}

#[derive(Clone)]
struct GameState {
    players: HashMap<String, Player>,
    inputs: HashMap<String, InputState>,
    monsters: HashMap<u64, Monster>,
    projectiles: HashMap<u64, Projectile>,
    resources: HashMap<ChunkCoord, Vec<ResourceNode>>,
    spawned_chunks: HashSet<ChunkCoord>,
    clients: HashMap<String, mpsc::UnboundedSender<ServerMessage>>,
    typing: HashMap<String, i64>,
    next_entity_id: u64,
}

impl GameState {
    fn new() -> Self {
        Self {
            players: HashMap::new(),
            inputs: HashMap::new(),
            monsters: HashMap::new(),
            projectiles: HashMap::new(),
            resources: HashMap::new(),
            spawned_chunks: HashSet::new(),
            clients: HashMap::new(),
            typing: HashMap::new(),
            next_entity_id: 1,
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }
}

#[derive(Debug, Clone)]
struct Player {
    id: String,
    name: String,
    x: f32,
    y: f32,
    hp: i32,
    face_x: f32,
    face_y: f32,
    inventory: HashMap<String, i32>,
    completed_quests: HashSet<String>,
    last_attack_ms: i64,
    last_gather_ms: i64,
    last_interact_ms: i64,
    last_saved_ms: i64,
}

impl Player {
    fn from_doc(doc: PlayerDoc) -> Self {
        Self {
            id: doc.id,
            name: doc.name,
            x: doc.x,
            y: doc.y,
            hp: doc.hp,
            face_x: 1.0,
            face_y: 0.0,
            inventory: doc.inventory,
            completed_quests: doc.completed_quests.into_iter().collect(),
            last_attack_ms: 0,
            last_gather_ms: 0,
            last_interact_ms: 0,
            last_saved_ms: now_millis(),
        }
    }

    fn sync_from_doc(&mut self, doc: &PlayerDoc) {
        self.name = doc.name.clone();
        self.x = doc.x;
        self.y = doc.y;
        self.hp = doc.hp;
        self.inventory = doc.inventory.clone();
        self.completed_quests = doc.completed_quests.iter().cloned().collect();
    }

    fn to_doc(&self) -> PlayerDoc {
        PlayerDoc {
            id: self.id.clone(),
            name: self.name.clone(),
            x: self.x,
            y: self.y,
            hp: self.hp,
            inventory: self.inventory.clone(),
            completed_quests: self.completed_quests.iter().cloned().collect(),
        }
    }

    fn self_view(&self) -> PlayerSelf {
        PlayerSelf {
            id: self.id.clone(),
            name: self.name.clone(),
            x: self.x,
            y: self.y,
            hp: self.hp,
            inventory: self.inventory.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct InputState {
    dir_x: f32,
    dir_y: f32,
    attack: bool,
    gather: bool,
    interact: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            dir_x: 0.0,
            dir_y: 0.0,
            attack: false,
            gather: false,
            interact: false,
        }
    }
}

#[derive(Debug, Clone)]
struct Monster {
    id: u64,
    kind: String,
    x: f32,
    y: f32,
    hp: i32,
    target: Option<String>,
    wander_dir: (f32, f32),
    wander_until_ms: i64,
    last_attack_ms: i64,
}

#[derive(Debug, Clone)]
struct Projectile {
    id: u64,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    ttl_ms: i64,
    damage: i32,
}

#[derive(Debug, Clone)]
struct ResourceNode {
    id: u64,
    kind: String,
    x: i32,
    y: i32,
    hp: i32,
    respawn_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
struct ChunkCoord {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WeaponStats {
    kind: String,
    damage: i32,
    range: f32,
    cooldown_ms: i64,
    projectile_speed: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ItemDef {
    id: String,
    name: String,
    kind: String,
    tool: Option<String>,
    power: Option<i32>,
    weapon: Option<WeaponStats>,
    ammo_for: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResourceDef {
    id: String,
    name: String,
    tool: String,
    hp: i32,
    respawn_ms: i64,
    drops: Vec<ItemStack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MonsterDef {
    id: String,
    name: String,
    hp: i32,
    speed: f32,
    damage: i32,
    drop: Option<ItemStack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuestDef {
    id: String,
    npc_id: String,
    name: String,
    description: String,
    requires: Vec<ItemStack>,
    rewards: Vec<ItemStack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NpcDef {
    id: String,
    name: String,
    x: f32,
    y: f32,
    dialog: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorldConfig {
    seed: u64,
    chunk_size: i32,
    tile_size: i32,
    spawn_x: f32,
    spawn_y: f32,
}

struct WorldNoise {
    elevation: Perlin,
    moisture: Perlin,
    soil: Perlin,
    river: Perlin,
    tree: Perlin,
    rock: Perlin,
}

impl WorldNoise {
    fn new(seed: u64) -> Self {
        let base = seed as u32;
        Self {
            elevation: Perlin::new(base.wrapping_add(11)),
            moisture: Perlin::new(base.wrapping_add(23)),
            soil: Perlin::new(base.wrapping_add(37)),
            river: Perlin::new(base.wrapping_add(41)),
            tree: Perlin::new(base.wrapping_add(59)),
            rock: Perlin::new(base.wrapping_add(71)),
        }
    }

    fn elevation(&self, x: f32, y: f32) -> f32 {
        self.fbm(&self.elevation, x, y, 0.008, 4)
    }

    fn moisture(&self, x: f32, y: f32) -> f32 {
        self.fbm(&self.moisture, x, y, 0.01, 3)
    }

    fn soil(&self, x: f32, y: f32) -> f32 {
        self.fbm(&self.soil, x, y, 0.02, 2)
    }

    fn river(&self, x: f32, y: f32) -> f32 {
        self.fbm(&self.river, x, y, 0.02, 2).abs()
    }

    fn tree_density(&self, x: f32, y: f32) -> f32 {
        self.fbm(&self.tree, x, y, 0.045, 3)
    }

    fn rock_density(&self, x: f32, y: f32) -> f32 {
        self.fbm(&self.rock, x, y, 0.06, 2)
    }

    fn fbm(&self, perlin: &Perlin, x: f32, y: f32, base_freq: f64, octaves: i32) -> f32 {
        let mut freq = base_freq;
        let mut amp = 0.5;
        let mut sum = 0.0;
        let mut max = 0.0;
        for _ in 0..octaves {
            let value = perlin.get([x as f64 * freq, y as f64 * freq]) as f32;
            sum += value * amp;
            max += amp;
            freq *= 2.0;
            amp *= 0.5;
        }
        if max > 0.0 {
            sum / max
        } else {
            sum
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ItemStack {
    id: String,
    count: i32,
}

#[derive(Clone)]
struct GameData {
    items: HashMap<String, ItemDef>,
    resources: HashMap<String, ResourceDef>,
    monsters: HashMap<String, MonsterDef>,
    quests_by_npc: HashMap<String, QuestDef>,
    npcs: Vec<NpcDef>,
}

impl GameData {
    fn new(
        items: Vec<ItemDef>,
        resources: Vec<ResourceDef>,
        monsters: Vec<MonsterDef>,
        quests: Vec<QuestDef>,
        npcs: Vec<NpcDef>,
    ) -> Self {
        let items_map = items.into_iter().map(|item| (item.id.clone(), item)).collect();
        let resources_map = resources
            .into_iter()
            .map(|res| (res.id.clone(), res))
            .collect();
        let monsters_map = monsters
            .into_iter()
            .map(|monster| (monster.id.clone(), monster))
            .collect();
        let quests_by_npc = quests
            .into_iter()
            .map(|quest| (quest.npc_id.clone(), quest))
            .collect();
        Self {
            items: items_map,
            resources: resources_map,
            monsters: monsters_map,
            quests_by_npc,
            npcs,
        }
    }
}

#[derive(Clone, Serialize)]
struct PlayerPublic {
    id: String,
    name: String,
    x: f32,
    y: f32,
    hp: i32,
}

impl From<&Player> for PlayerPublic {
    fn from(player: &Player) -> Self {
        Self {
            id: player.id.clone(),
            name: player.name.clone(),
            x: player.x,
            y: player.y,
            hp: player.hp,
        }
    }
}

#[derive(Clone, Serialize)]
struct PlayerSelf {
    id: String,
    name: String,
    x: f32,
    y: f32,
    hp: i32,
    inventory: HashMap<String, i32>,
}

#[derive(Clone, Serialize)]
struct MonsterPublic {
    id: u64,
    kind: String,
    x: f32,
    y: f32,
    hp: i32,
}

impl From<&Monster> for MonsterPublic {
    fn from(monster: &Monster) -> Self {
        Self {
            id: monster.id,
            kind: monster.kind.clone(),
            x: monster.x,
            y: monster.y,
            hp: monster.hp,
        }
    }
}

#[derive(Clone, Serialize)]
struct ProjectilePublic {
    id: u64,
    x: f32,
    y: f32,
}

impl From<&Projectile> for ProjectilePublic {
    fn from(projectile: &Projectile) -> Self {
        Self {
            id: projectile.id,
            x: projectile.x,
            y: projectile.y,
        }
    }
}

#[derive(Clone, Serialize)]
struct ResourceNodePublic {
    id: String,
    kind: String,
    x: i32,
    y: i32,
    hp: i32,
}

impl From<ResourceNode> for ResourceNodePublic {
    fn from(node: ResourceNode) -> Self {
        Self {
            id: node.id.to_string(),
            kind: node.kind,
            x: node.x,
            y: node.y,
            hp: node.hp,
        }
    }
}

impl From<&ResourceNode> for ResourceNodePublic {
    fn from(node: &ResourceNode) -> Self {
        Self {
            id: node.id.to_string(),
            kind: node.kind.clone(),
            x: node.x,
            y: node.y,
            hp: node.hp,
        }
    }
}

#[derive(Clone, Serialize)]
struct NpcPublic {
    id: String,
    name: String,
    x: f32,
    y: f32,
    dialog: String,
}

impl From<NpcDef> for NpcPublic {
    fn from(npc: NpcDef) -> Self {
        Self {
            id: npc.id,
            name: npc.name,
            x: npc.x,
            y: npc.y,
            dialog: npc.dialog,
        }
    }
}

impl From<&NpcDef> for NpcPublic {
    fn from(npc: &NpcDef) -> Self {
        Self {
            id: npc.id.clone(),
            name: npc.name.clone(),
            x: npc.x,
            y: npc.y,
            dialog: npc.dialog.clone(),
        }
    }
}

#[derive(Serialize)]
struct SessionResponse {
    session_id: String,
    name: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage {
    Welcome {
        player: PlayerSelf,
        world: WorldConfig,
        npcs: Vec<NpcPublic>,
    },
    ChunkData {
        chunk_x: i32,
        chunk_y: i32,
        tiles: Vec<u8>,
        resources: Vec<ResourceNodePublic>,
    },
    State {
        players: Vec<PlayerPublic>,
        monsters: Vec<MonsterPublic>,
        projectiles: Vec<ProjectilePublic>,
    },
    ResourceUpdate {
        resource: ResourceNodePublic,
        state: String,
    },
    Chat {
        from: String,
        text: String,
    },
    Dialog {
        title: String,
        text: String,
    },
    System {
        text: String,
    },
    Typing {
        id: String,
        typing: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Input {
        dir_x: f32,
        dir_y: f32,
        attack: bool,
        gather: bool,
        interact: bool,
    },
    Chat {
        text: String,
    },
    Typing {
        typing: bool,
    },
    ChunkRequest {
        chunks: Vec<ChunkCoord>,
    },
    Ping,
}
