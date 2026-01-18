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
use rand::{seq::SliceRandom, Rng};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
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
const ENTITY_FOOT_OFFSET_X: f32 = 0.5;
const ENTITY_FOOT_OFFSET_Y: f32 = 0.9;
const SAVE_INTERVAL_MS: i64 = 5_000;
const MAX_HP: i32 = 10;
const PLAYER_REGEN_INTERVAL_MS: i64 = 5_000;
const TYPING_TIMEOUT_MS: i64 = 2500;
const CHUNK_KEEP_RADIUS: i32 = 3;
const ENTITY_VISIBILITY_RADIUS: i32 = 2;
const EXPECTED_POS_CORRECTION_RANGE: f32 = 1.5;
const EXPECTED_POS_CORRECTION_WEIGHT: f32 = 0.35;
const CHUNK_TTL_MS: i64 = 60_000;
const MAX_NAME_CHARS: usize = 20;
const PLAYER_COORD_VERSION: i32 = 1;
const TREE_GROW_INTERVAL_MS: i64 = 30_000;
const TREE_MAX_SIZE: i32 = 3;
const ROCK_MAX_SIZE: i32 = 3;
const COMMUNITY_AREA_SIZE: i32 = 10;
const COMMUNITY_CREATE_WOOD: i32 = 200;
const COMMUNITY_CREATE_STONE: i32 = 10;
const COMMUNITY_APPROVAL_TIMEOUT_MS: i64 = 60_000;
const CASTLE_SIZE: i32 = 8;
const CHURCH_SIZE: i32 = 5;
const SILO_SIZE: i32 = 3;
const CASTLE_MEMBER_COUNT: usize = 10;
const CHURCH_MEMBER_COUNT: usize = 3;
const SILO_MEMBER_COUNT: usize = 2;

const TILE_GRASS: u8 = 0;
const TILE_WATER: u8 = 1;
const TILE_SAND: u8 = 2;
const TILE_DIRT: u8 = 3;
const TILE_FLOWER: u8 = 4;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Language {
    En,
    De,
}

fn language_from_tag(tag: &str) -> Language {
    let lowered = tag.trim().to_ascii_lowercase();
    if lowered.starts_with("de") {
        Language::De
    } else {
        Language::En
    }
}

fn language_from_headers(headers: &HeaderMap) -> Language {
    let value = headers
        .get("accept-language")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    for part in value.split(',') {
        let token = part.split(';').next().unwrap_or("").trim();
        if token.is_empty() {
            continue;
        }
        return language_from_tag(token);
    }
    Language::En
}

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
    {
        let structures = store.load_structures().await?;
        let communities = store.load_communities().await?;
        let storages = store.load_storages().await?;
        let mut state_guard = state.write().await;
        let mut max_id = 0;
        for doc in structures {
            let id = doc.id as u64;
            max_id = max_id.max(id);
            let tile = StructureTile {
                id,
                kind: doc.kind,
                x: doc.x,
                y: doc.y,
                owner_id: doc.owner_id,
            };
            state_guard
                .structure_tiles
                .insert(TileCoord { x: tile.x, y: tile.y }, tile);
        }
        if max_id >= state_guard.next_structure_id {
            state_guard.next_structure_id = max_id + 1;
        }
        for doc in communities {
            let mut member_set = HashSet::new();
            for member in doc.members {
                member_set.insert(member);
            }
            let community = Community {
                id: doc.id.clone(),
                members: member_set,
                areas: doc.areas.clone(),
            };
            for area in &doc.areas {
                insert_community_area_tiles(&mut state_guard, &doc.id, *area);
            }
            state_guard.communities.insert(doc.id, community);
        }
        for doc in storages {
            state_guard.storages.insert(
                doc.id as u64,
                Storage {
                    id: doc.id as u64,
                    community_id: doc.community_id,
                    inventory: doc.inventory,
                },
            );
        }
    }

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

    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
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
    let language = language_from_headers(&headers);
    ws.on_upgrade(move |socket| handle_socket(socket, app_state, sid, language))
}

async fn handle_socket(socket: WebSocket, app_state: AppState, sid: String, language: Language) {
    let (mut socket_sender, mut socket_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    {
        let mut state = app_state.state.write().await;
        state.clients.insert(sid.clone(), tx);
        state
            .visibility
            .insert(sid.clone(), VisibilityState::default());
        state.locales.insert(sid.clone(), language);
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
        let lang = player_language(&state, &sid);
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
                .map(|npc| NpcPublic {
                    id: npc.id.clone(),
                    name: localize_npc_name(npc, lang),
                    x: npc.x,
                    y: npc.y,
                    dialog: localize_npc_dialog(npc, lang),
                })
                .collect(),
            inventory_items: build_inventory_items(&player.inventory, app_state.data.as_ref(), lang),
        }
    };

    send_to_player(&app_state.state, &sid, welcome_msg).await;
    let inventory_msg = ServerMessage::Inventory {
        items: build_inventory_items(&doc.inventory, app_state.data.as_ref(), language),
    };
    send_to_player(&app_state.state, &sid, inventory_msg).await;

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
        state.visibility.remove(&sid);
        state.locales.remove(&sid);
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
            seq,
            expected_x,
            expected_y,
        } => {
            let mut state = app_state.state.write().await;
            let entry = state.inputs.entry(sid.to_string()).or_insert(InputState::default());
            entry.dir_x = dir_x.clamp(-1.0, 1.0);
            entry.dir_y = dir_y.clamp(-1.0, 1.0);
            entry.attack = attack;
            entry.gather = gather;
            entry.interact = interact;
            entry.seq = seq;
            entry.expected_x = expected_x;
            entry.expected_y = expected_y;
        }
        ClientMessage::Chat { text } => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }
            let trimmed = trimmed.chars().take(160).collect::<String>();
            let (sender_name, sender_id, sender_community_id, sender_pos, was_typing) = {
                let mut state = app_state.state.write().await;
                let player = state.players.get(sid);
                let name = player
                    .map(|player| player.name.clone())
                    .unwrap_or_else(|| "Wanderer".to_string());
                let community_id = player.and_then(|player| player.community_id.clone());
                let pos = player.map(|player| (player.x, player.y));
                let was_typing = state.typing.remove(sid).is_some();
                (name, sid.to_string(), community_id, pos, was_typing)
            };
            let mut state = app_state.state.write().await;
            let lang = player_language(&state, &sender_id);
            if let Some((sx, sy)) = sender_pos {
                let sender_chunk = chunk_coord_for_position(sx, sy, app_state.world.chunk_size);
                for (player_id, sender) in state.clients.iter() {
                    let player = match state.players.get(player_id) {
                        Some(player) => player,
                        None => continue,
                    };
                    let recipient_chunk =
                        chunk_coord_for_position(player.x, player.y, app_state.world.chunk_size);
                    if !chunk_in_radius(sender_chunk, recipient_chunk, ENTITY_VISIBILITY_RADIUS) {
                        continue;
                    }
                    if let Some(comm_id) = sender_community_id.as_ref() {
                        if player.community_id.as_ref() == Some(comm_id) {
                            continue;
                        }
                    }
                    let _ = sender.send(ServerMessage::Chat {
                        from: sender_name.clone(),
                        text: trimmed.clone(),
                        channel: "local".to_string(),
                        community_member: false,
                    });
                }
            }
            if let Some(community_id) = sender_community_id.as_ref() {
                if let Some(community) = state.communities.get(community_id) {
                    for member_id in &community.members {
                        if let Some(sender) = state.clients.get(member_id) {
                            let _ = sender.send(ServerMessage::Chat {
                                from: sender_name.clone(),
                                text: trimmed.clone(),
                                channel: "community".to_string(),
                                community_member: true,
                            });
                        }
                    }
                } else {
                    send_system_message(
                        &mut state,
                        &sender_id,
                        message_community_missing(lang).to_string(),
                    );
                }
            }
            if was_typing {
                broadcast_message_inline(
                    &state,
                    ServerMessage::Typing {
                        id: sid.to_string(),
                        typing: false,
                    },
                );
            }
        }
        ClientMessage::CommunityApproval { request_id, approve } => {
            handle_community_approval(app_state, sid, &request_id, approve).await;
        }
        ClientMessage::CommunityLeave => {
            handle_community_leave(app_state, sid).await;
        }
        ClientMessage::SetName { name } => {
            let normalized = match normalize_player_name(&name) {
                Some(normalized) => normalized,
                None => return,
            };
            let doc = {
                let mut state = app_state.state.write().await;
                let player = match state.players.get_mut(sid) {
                    Some(player) => player,
                    None => return,
                };
                if player.name == normalized {
                    None
                } else {
                    player.name = normalized.clone();
                    Some(player.to_doc())
                }
            };
            if let Some(doc) = doc {
                let store = app_state.store.clone();
                tokio::spawn(async move {
                    let _ = store.save_player(&doc).await;
                });
            }
        }
        ClientMessage::UseItem { id } => {
            let heal_amount = match app_state.data.items.get(&id).and_then(|def| def.heal) {
                Some(amount) if amount > 0 => amount,
                _ => return,
            };
            let mut state = app_state.state.write().await;
            let lang = player_language(&state, sid);
            let item_name = localize_item_name(app_state.data.as_ref(), &id, lang);
            let (items, player_id, message) = {
                let player = match state.players.get_mut(sid) {
                    Some(player) => player,
                    None => return,
                };
                if player.hp >= MAX_HP {
                    return;
                }
                if !consume_item(&mut player.inventory, &id, 1) {
                    return;
                }
                let hp_before = player.hp;
                player.hp = (player.hp + heal_amount).min(MAX_HP);
                player.last_inventory_hash = inventory_hash(&player.inventory);
                let items =
                    build_inventory_items(&player.inventory, app_state.data.as_ref(), lang);
                let player_id = player.id.clone();
                let message = if id == "apple" {
                    message_eat_apple(lang)
                } else {
                    message_eat_item(&item_name, player.hp > hp_before, lang)
                };
                (items, player_id, message)
            };
            if let Some(sender) = state.clients.get(sid) {
                let _ = sender.send(ServerMessage::Inventory { items });
            }
            send_system_message(&mut state, &player_id, message);
        }
        ClientMessage::StorageDeposit {
            storage_id,
            item_id,
            count,
        } => {
            handle_storage_deposit(app_state, sid, storage_id, &item_id, count).await;
        }
        ClientMessage::StorageWithdraw {
            storage_id,
            item_id,
            count,
        } => {
            handle_storage_withdraw(app_state, sid, storage_id, &item_id, count).await;
        }
        ClientMessage::Build { kind, x, y } => {
            handle_build_request(app_state, sid, kind, x, y).await;
        }
        ClientMessage::Demolish { x, y } => {
            handle_demolish_request(app_state, sid, x, y).await;
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
        ClientMessage::Locale { language } => {
            let mut state = app_state.state.write().await;
            let lang = language_from_tag(&language);
            state.locales.insert(sid.to_string(), lang);
            if let Some(player) = state.players.get(sid) {
                let items =
                    build_inventory_items(&player.inventory, app_state.data.as_ref(), lang);
                if let Some(sender) = state.clients.get(sid) {
                    let _ = sender.send(ServerMessage::Inventory { items });
                }
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
    let now_ms = now_millis();

    for coord in chunks {
        state.chunk_last_access.insert(coord, now_ms);
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

        if !state.resources.contains_key(&coord) {
            let generated = generate_resources(
                app_state.world.seed,
                now_ms,
                coord,
                &app_state.world,
                &app_state.noise,
                &app_state.data,
                &state.structure_tiles,
            );
            state.resources.insert(coord, generated);
        }

        let tiles = generate_tiles(coord, &app_state.world, &app_state.noise);
        let visible_resources = match state.resources.get(&coord) {
            Some(resources) => resources
                .iter()
                .filter(|res| res.hp > 0)
                .map(ResourceNodePublic::from)
                .collect(),
            None => Vec::new(),
        };
        let structures = state
            .structure_tiles
            .values()
            .filter(|structure| {
                chunk_coord_for_tile(structure.x, structure.y, app_state.world.chunk_size) == coord
            })
            .map(StructurePublic::from)
            .collect();

        let _ = sender.send(ServerMessage::ChunkData {
            chunk_x: coord.x,
            chunk_y: coord.y,
            tiles,
            resources: visible_resources,
            structures,
        });
    }
}

async fn handle_build_request(app_state: &AppState, sid: &str, kind: String, x: i32, y: i32) {
    if kind.as_str() == "community" {
        handle_community_build_request(app_state, sid, x, y).await;
        return;
    }
    if matches!(kind.as_str(), "castle" | "silo" | "church") {
        handle_group_build_request(app_state, sid, &kind, x, y).await;
        return;
    }

    let mut state = app_state.state.write().await;
    let (player_id, inventory_snapshot, player_community_id) = match state.players.get(sid) {
        Some(player) => (
            player.id.clone(),
            player.inventory.clone(),
            player.community_id.clone(),
        ),
        None => return,
    };

    let mut tiles = Vec::new();
    let mut placements: Vec<(TileCoord, String)> = Vec::new();
    let mut cost = Vec::new();
    let mut require_shovel = false;
    let mut requires_land = true;
    let lang = player_language(&state, &player_id);

    let build_kind = kind.as_str();
    match build_kind {
        "hut_wood" => {
            cost.push(ItemStack {
                id: "wood".to_string(),
                count: 20,
            });
            let width = 2;
            let height = 2;
            let base_y = y - (height - 1);
            for dy in 0..height {
                for dx in 0..width {
                    let coord = TileCoord {
                        x: x + dx,
                        y: base_y + dy,
                    };
                    tiles.push(coord);
                    let kind = if dy == height - 1 && dx == 0 {
                        "hut_wood_root"
                    } else if dy == height - 1 {
                        "hut_wood_block"
                    } else {
                        "hut_wood_top"
                    };
                    placements.push((coord, kind.to_string()));
                }
            }
        }
        "house_stone" => {
            cost.push(ItemStack {
                id: "stone".to_string(),
                count: 50,
            });
            let width = 3;
            let height = 3;
            let base_y = y - (height - 1);
            for dy in 0..height {
                for dx in 0..width {
                    let coord = TileCoord {
                        x: x + dx,
                        y: base_y + dy,
                    };
                    tiles.push(coord);
                    let kind = if dy == height - 1 && dx == 0 {
                        "house_stone_root"
                    } else if dy == height - 1 {
                        "house_stone_block"
                    } else {
                        "house_stone_top"
                    };
                    placements.push((coord, kind.to_string()));
                }
            }
        }
        "bridge_wood" => {
            cost.push(ItemStack {
                id: "wood".to_string(),
                count: 10,
            });
            requires_land = false;
            match find_bridge_span(&app_state.noise, x, y) {
                Some(span) => tiles = span,
                None => {
                    send_system_message(
                        &mut state,
                        &player_id,
                        message_bridge_span_error(lang).to_string(),
                    );
                    return;
                }
            }
        }
        "bridge_stone" => {
            cost.push(ItemStack {
                id: "stone".to_string(),
                count: 20,
            });
            requires_land = false;
            match find_bridge_span(&app_state.noise, x, y) {
                Some(span) => tiles = span,
                None => {
                    send_system_message(
                        &mut state,
                        &player_id,
                        message_bridge_span_error(lang).to_string(),
                    );
                    return;
                }
            }
        }
        "path" => {
            require_shovel = true;
            let coord = TileCoord { x, y };
            tiles.push(coord);
            placements.push((coord, "path".to_string()));
        }
        "road" => {
            require_shovel = true;
            cost.push(ItemStack {
                id: "stone".to_string(),
                count: 2,
            });
            let coord = TileCoord { x, y };
            tiles.push(coord);
            placements.push((coord, "road".to_string()));
        }
        _ => {
            send_system_message(
                &mut state,
                &player_id,
                message_unknown_build(lang).to_string(),
            );
            return;
        }
    };

    let mut structure_kind = build_kind.to_string();
    if build_kind == "bridge_wood" || build_kind == "bridge_stone" {
        let is_vertical = tiles
            .first()
            .map(|first| tiles.iter().all(|tile| tile.x == first.x))
            .unwrap_or(false);
        if build_kind == "bridge_wood" {
            structure_kind = if is_vertical {
                "bridge_wood_v".to_string()
            } else {
                "bridge_wood_h".to_string()
            };
        } else {
            structure_kind = if is_vertical {
                "bridge_stone_v".to_string()
            } else {
                "bridge_stone_h".to_string()
            };
        }
    }

    if require_shovel && !has_tool(&inventory_snapshot, app_state.data.as_ref(), "shovel") {
        send_system_message(
            &mut state,
            &player_id,
            message_need_shovel(lang).to_string(),
        );
        return;
    }

    if !cost.is_empty() && !has_items(&inventory_snapshot, &cost) {
        send_system_message(
            &mut state,
            &player_id,
            message_not_enough_materials(lang).to_string(),
        );
        return;
    }

    for tile in &tiles {
        if let Some(owner) = community_tile_owner(&state, tile.x, tile.y) {
            if player_community_id.as_ref() != Some(owner) {
                send_system_message(
                    &mut state,
                    &player_id,
                    message_community_members_only(lang).to_string(),
                );
                return;
            }
        }
        if state
            .structure_tiles
            .contains_key(&TileCoord { x: tile.x, y: tile.y })
        {
            send_system_message(
                &mut state,
                &player_id,
                message_spot_occupied(lang).to_string(),
            );
            return;
        }
        if resource_at_tile(&state, tile.x, tile.y) {
            send_system_message(
                &mut state,
                &player_id,
                message_clear_resource(lang).to_string(),
            );
            return;
        }
        if requires_land && tile_at(&app_state.noise, tile.x, tile.y) == TILE_WATER {
            send_system_message(
                &mut state,
                &player_id,
                message_build_on_land(lang).to_string(),
            );
            return;
        }
    }

    if placements.is_empty() {
        for tile in &tiles {
            placements.push((TileCoord { x: tile.x, y: tile.y }, structure_kind.clone()));
        }
    }

    let mut inventory_items = None;
    if !cost.is_empty() {
        let mut removal_failed = false;
        let items = {
            let player = match state.players.get_mut(sid) {
                Some(player) => player,
                None => return,
            };
            if !remove_items(&mut player.inventory, &cost) {
                removal_failed = true;
                Vec::new()
            } else {
                player.last_inventory_hash = inventory_hash(&player.inventory);
                build_inventory_items(&player.inventory, app_state.data.as_ref(), lang)
            }
        };
        if removal_failed {
            send_system_message(
                &mut state,
                &player_id,
                message_not_enough_materials(lang).to_string(),
            );
            return;
        }
        inventory_items = Some(items);
    }

    let structure_id = state.next_structure_id();
    let mut new_tiles = Vec::new();
    for (tile, kind) in placements {
        let structure = StructureTile {
            id: structure_id,
            kind,
            x: tile.x,
            y: tile.y,
            owner_id: player_id.clone(),
        };
        state
            .structure_tiles
            .insert(TileCoord { x: tile.x, y: tile.y }, structure.clone());
        new_tiles.push(structure);
    }

    if let Some(items) = inventory_items {
        if let Some(sender) = state.clients.get(sid) {
            let _ = sender.send(ServerMessage::Inventory { items });
        }
    }

    let structures_public: Vec<StructurePublic> = new_tiles.iter().map(StructurePublic::from).collect();
    let mut chunks = HashSet::new();
    for structure in &structures_public {
        chunks.insert(chunk_coord_for_tile(
            structure.x,
            structure.y,
            app_state.world.chunk_size,
        ));
    }
    send_to_players_in_chunks(
        &state,
        app_state.world.chunk_size,
        &chunks,
        ServerMessage::StructureUpdate {
            structures: structures_public,
            state: "added".to_string(),
        },
    );
    send_system_message(
        &mut state,
        &player_id,
        message_build_success(lang, build_kind),
    );

    let docs: Vec<StructureDoc> = new_tiles
        .into_iter()
        .map(|tile| StructureDoc {
            id: structure_id as i64,
            kind: tile.kind,
            x: tile.x,
            y: tile.y,
            owner_id: tile.owner_id,
        })
        .collect();
    let store = app_state.store.clone();
    tokio::spawn(async move {
        let _ = store.insert_structures(&docs).await;
    });
}

async fn handle_demolish_request(app_state: &AppState, sid: &str, x: i32, y: i32) {
    let mut state = app_state.state.write().await;
    let player_id = match state.players.get(sid) {
        Some(player) => player.id.clone(),
        None => return,
    };
    let lang = player_language(&state, &player_id);
    let tile = TileCoord { x, y };
    let structure = match state.structure_tiles.get(&tile) {
        Some(structure) => structure.clone(),
        None => {
            send_system_message(&mut state, &player_id, message_nothing_to_remove(lang).to_string());
            return;
        }
    };
    if structure.owner_id != player_id {
        send_system_message(
            &mut state,
            &player_id,
            message_remove_own_only(lang).to_string(),
        );
        return;
    }

    let target_id = structure.id;
    let mut removed = Vec::new();
    state
        .structure_tiles
        .retain(|_, structure| {
            if structure.id == target_id {
                removed.push(StructurePublic {
                    id: structure.id,
                    kind: structure.kind.clone(),
                    x: structure.x,
                    y: structure.y,
                    owner_id: structure.owner_id.clone(),
                });
                false
            } else {
                true
            }
        });

    if removed.is_empty() {
        return;
    }

    let mut chunks = HashSet::new();
    for structure in &removed {
        chunks.insert(chunk_coord_for_tile(
            structure.x,
            structure.y,
            app_state.world.chunk_size,
        ));
    }
    send_to_players_in_chunks(
        &state,
        app_state.world.chunk_size,
        &chunks,
        ServerMessage::StructureUpdate {
            structures: removed,
            state: "removed".to_string(),
        },
    );
    send_system_message(
        &mut state,
        &player_id,
        message_structure_removed(lang).to_string(),
    );

    let store = app_state.store.clone();
    tokio::spawn(async move {
        let _ = store.delete_structure_group(target_id as i64).await;
    });
}

async fn handle_community_approval(
    app_state: &AppState,
    sid: &str,
    request_id: &str,
    approve: bool,
) {
    let (pending, lang) = {
        let mut state = app_state.state.write().await;
        let lang = player_language(&state, sid);
        let pending = match state.pending_approvals.get_mut(request_id) {
            Some(pending) => pending,
            None => return,
        };
        if !pending.required.contains(sid) {
            return;
        }
        if !approve {
            let pending = state.pending_approvals.remove(request_id).unwrap();
            notify_approval_cancelled(&mut state, &pending, lang);
            return;
        }
        pending.approvals.insert(sid.to_string());
        if pending.approvals.len() != pending.required.len() {
            return;
        }
        let pending = state.pending_approvals.remove(request_id).unwrap();
        (pending, lang)
    };

    match pending.kind {
        ApprovalKind::Join {
            community_id,
            target_id,
            area,
            requester_id,
        } => {
            handle_join_approval_complete(
                app_state,
                &community_id,
                &target_id,
                area,
                &requester_id,
                lang,
            )
            .await;
        }
        ApprovalKind::Build {
            community_id,
            build_kind,
            x,
            y,
            requester_id,
            participants,
        } => {
            handle_build_approval_complete(
                app_state,
                &community_id,
                &build_kind,
                x,
                y,
                &requester_id,
                &participants,
                lang,
            )
            .await;
        }
    }
}

async fn handle_community_leave(app_state: &AppState, sid: &str) {
    let mut state = app_state.state.write().await;
    let lang = player_language(&state, sid);
    let (player_id, community_id_opt) = match state.players.get(sid) {
        Some(player) => (player.id.clone(), player.community_id.clone()),
        None => return,
    };
    let community_id = match community_id_opt {
        Some(id) => id,
        None => {
            send_system_message(
                &mut state,
                &player_id,
                message_community_not_member(lang).to_string(),
            );
            return;
        }
    };
    if let Some(player) = state.players.get_mut(sid) {
        player.community_id = None;
    }

    if let Some(community) = state.communities.get_mut(&community_id) {
        community.members.remove(&player_id);
    }
    send_system_message(
        &mut state,
        &player_id,
        message_community_left(lang).to_string(),
    );

    let player_doc = state.players.get(sid).map(|player| player.to_doc());
    let mut delete_community = false;
    let community_doc = if let Some(community) = state.communities.get(&community_id) {
        if community.members.is_empty() {
            delete_community = true;
            None
        } else {
            Some(CommunityDoc {
                id: community.id.clone(),
                members: community.members.iter().cloned().collect(),
                areas: community.areas.clone(),
            })
        }
    } else {
        None
    };
    if delete_community {
        remove_community_from_state(&mut state, &community_id);
    }
    drop(state);

    let store = app_state.store.clone();
    if let Some(doc) = player_doc {
        let store = store.clone();
        tokio::spawn(async move {
            let _ = store.save_player(&doc).await;
        });
    }
    if delete_community {
        let store = store.clone();
        let community_id = community_id.clone();
        tokio::spawn(async move {
            let _ = store.delete_community(&community_id).await;
            let _ = store
                .delete_structures_by_owner_and_kinds(
                    &community_id,
                    &[
                        "community_fence",
                        "community_float",
                        "community_gate",
                        "community_well",
                    ],
                )
                .await;
            let _ = store.delete_storages_by_community(&community_id).await;
        });
    } else if let Some(doc) = community_doc {
        let store = store.clone();
        tokio::spawn(async move {
            let _ = store.save_community(&doc).await;
        });
    }
}

async fn handle_storage_deposit(
    app_state: &AppState,
    sid: &str,
    storage_id: u64,
    item_id: &str,
    count: i32,
) {
    if count <= 0 {
        return;
    }
    let mut state = app_state.state.write().await;
    let lang = player_language(&state, sid);
    let (player_id, player_pos, community_id, has_item) = match state.players.get(sid) {
        Some(player) => (
            player.id.clone(),
            (player.x, player.y),
            player.community_id.clone(),
            player.inventory.get(item_id).copied().unwrap_or(0) >= count,
        ),
        None => return,
    };
    let community_id = match community_id {
        Some(id) => id,
        None => {
            send_system_message(
                &mut state,
                &player_id,
                message_community_not_member(lang).to_string(),
            );
            return;
        }
    };
    if !player_near_storage_at(&state, player_pos.0, player_pos.1, storage_id) {
        return;
    }
    if !has_item {
        send_system_message(
            &mut state,
            &player_id,
            message_not_enough_materials(lang).to_string(),
        );
        return;
    }

    let (player_doc, player_items) = {
        let player = match state.players.get_mut(sid) {
            Some(player) => player,
            None => return,
        };
        consume_item(&mut player.inventory, item_id, count);
        player.last_inventory_hash = inventory_hash(&player.inventory);
        let player_items =
            build_inventory_items(&player.inventory, app_state.data.as_ref(), lang);
        (player.to_doc(), player_items)
    };

    let (storage_items, storage_doc) = {
        let storage = match state.storages.get_mut(&storage_id) {
            Some(storage) => storage,
            None => return,
        };
        if storage.community_id != community_id {
            return;
        }
        add_item(&mut storage.inventory, item_id, count);
        let storage_items =
            build_inventory_items(&storage.inventory, app_state.data.as_ref(), lang);
        let storage_doc = StorageDoc {
            id: storage.id as i64,
            community_id: storage.community_id.clone(),
            inventory: storage.inventory.clone(),
        };
        (storage_items, storage_doc)
    };

    if let Some(sender) = state.clients.get(sid) {
        let _ = sender.send(ServerMessage::Inventory { items: player_items });
        let _ = sender.send(ServerMessage::Storage {
            storage_id,
            items: storage_items.clone(),
        });
    }

    let store = app_state.store.clone();
    tokio::spawn(async move {
        let _ = store.save_player(&player_doc).await;
        let _ = store.save_storage(&storage_doc).await;
    });
}

async fn handle_storage_withdraw(
    app_state: &AppState,
    sid: &str,
    storage_id: u64,
    item_id: &str,
    count: i32,
) {
    if count <= 0 {
        return;
    }
    let mut state = app_state.state.write().await;
    let lang = player_language(&state, sid);
    let (player_id, player_pos, community_id) = match state.players.get(sid) {
        Some(player) => (
            player.id.clone(),
            (player.x, player.y),
            player.community_id.clone(),
        ),
        None => return,
    };
    let community_id = match community_id {
        Some(id) => id,
        None => {
            send_system_message(
                &mut state,
                &player_id,
                message_community_not_member(lang).to_string(),
            );
            return;
        }
    };
    if !player_near_storage_at(&state, player_pos.0, player_pos.1, storage_id) {
        return;
    }
    let storage_snapshot = match state.storages.get(&storage_id) {
        Some(storage) => storage.clone(),
        None => return,
    };
    if storage_snapshot.community_id != community_id {
        return;
    }
    if storage_snapshot.inventory.get(item_id).copied().unwrap_or(0) < count {
        return;
    }

    let (storage_items, storage_doc) = {
        let storage = match state.storages.get_mut(&storage_id) {
            Some(storage) => storage,
            None => return,
        };
        consume_item(&mut storage.inventory, item_id, count);
        let storage_items =
            build_inventory_items(&storage.inventory, app_state.data.as_ref(), lang);
        let storage_doc = StorageDoc {
            id: storage.id as i64,
            community_id: storage.community_id.clone(),
            inventory: storage.inventory.clone(),
        };
        (storage_items, storage_doc)
    };
    let (player_doc, player_items) = {
        let player = match state.players.get_mut(sid) {
            Some(player) => player,
            None => return,
        };
        add_item(&mut player.inventory, item_id, count);
        player.last_inventory_hash = inventory_hash(&player.inventory);
        let player_items =
            build_inventory_items(&player.inventory, app_state.data.as_ref(), lang);
        (player.to_doc(), player_items)
    };
    if let Some(sender) = state.clients.get(sid) {
        let _ = sender.send(ServerMessage::Inventory { items: player_items });
        let _ = sender.send(ServerMessage::Storage {
            storage_id,
            items: storage_items.clone(),
        });
    }

    let store = app_state.store.clone();
    tokio::spawn(async move {
        let _ = store.save_player(&player_doc).await;
        let _ = store.save_storage(&storage_doc).await;
    });
}

fn handle_storage_interaction(player: &mut Player, state: &mut GameState, app_state: &AppState) -> bool {
    let lang = player_language(state, &player.id);
    let (storage_id, storage_community) = match find_nearby_storage(player, state) {
        Some(result) => result,
        None => return false,
    };
    if player.community_id.as_ref() != Some(&storage_community) {
        send_system_message(state, &player.id, message_community_members_only(lang).to_string());
        return true;
    }
    if let Some(storage) = state.storages.get(&storage_id) {
        let items = build_inventory_items(&storage.inventory, app_state.data.as_ref(), lang);
        if let Some(sender) = state.clients.get(&player.id) {
            let _ = sender.send(ServerMessage::Storage {
                storage_id,
                items,
            });
        }
        return true;
    }
    false
}

fn handle_community_interaction(
    player: &mut Player,
    state: &mut GameState,
    app_state: &AppState,
    now_ms: i64,
) -> bool {
    let lang = player_language(state, &player.id);
    let player_tile = player_tile(player);
    let area = community_area_origin(player_tile.x, player_tile.y);

    if player.community_id.is_none() {
        if let Some(community_id) = find_adjacent_community_request(state, &area) {
            if community_area_overlaps(state, area) {
                send_system_message(
                    state,
                    &player.id,
                    message_community_area_overlap(lang).to_string(),
                );
                return true;
            }
            if !community_area_is_clear(state, area) {
                send_system_message(
                    state,
                    &player.id,
                    message_community_area_blocked(lang).to_string(),
                );
                return true;
            }
            if pending_join_request_exists(state, &community_id, &player.id) {
                return true;
            }
            start_join_request(
                state,
                app_state,
                &community_id,
                &player.id,
                &player.id,
                area,
                now_ms,
            );
            send_system_message(
                state,
                &player.id,
                message_community_join_sent(lang).to_string(),
            );
            return true;
        }
    }

    if let Some(community_id) = player.community_id.clone() {
        if let Some((target_id, target_area)) = find_nearby_join_target(player, state) {
            if !community_area_adjacent(
                state
                    .communities
                    .get(&community_id)
                    .expect("community missing"),
                target_area,
            ) {
                send_system_message(
                    state,
                    &player.id,
                    message_community_area_adjacent(lang).to_string(),
                );
                return true;
            }
            if community_area_overlaps(state, target_area) {
                send_system_message(
                    state,
                    &player.id,
                    message_community_area_overlap(lang).to_string(),
                );
                return true;
            }
            if !community_area_is_clear(state, target_area) {
                send_system_message(
                    state,
                    &player.id,
                    message_community_area_blocked(lang).to_string(),
                );
                return true;
            }
            if pending_join_request_exists(state, &community_id, &target_id) {
                return true;
            }
            start_join_request(
                state,
                app_state,
                &community_id,
                &target_id,
                &player.id,
                target_area,
                now_ms,
            );
            send_system_message(
                state,
                &player.id,
                message_community_invite_sent(lang).to_string(),
            );
            return true;
        }
    }
    false
}

fn notify_approval_cancelled(state: &mut GameState, pending: &PendingApproval, lang: Language) {
    match &pending.kind {
        ApprovalKind::Join { requester_id, target_id, .. } => {
            send_system_message(
                state,
                requester_id,
                message_community_request_declined(lang).to_string(),
            );
            send_system_message(
                state,
                target_id,
                message_community_request_declined(lang).to_string(),
            );
        }
        ApprovalKind::Build { requester_id, .. } => {
            send_system_message(
                state,
                requester_id,
                message_build_request_declined(lang).to_string(),
            );
        }
    }
}

fn notify_approval_timeout(state: &mut GameState, pending: &PendingApproval) {
    match &pending.kind {
        ApprovalKind::Join {
            requester_id,
            target_id,
            ..
        } => {
            let lang = player_language(state, requester_id);
            send_system_message(
                state,
                requester_id,
                message_request_timed_out(lang).to_string(),
            );
            let lang = player_language(state, target_id);
            send_system_message(state, target_id, message_request_timed_out(lang).to_string());
        }
        ApprovalKind::Build { requester_id, .. } => {
            let lang = player_language(state, requester_id);
            send_system_message(state, requester_id, message_request_timed_out(lang).to_string());
        }
    }
}

async fn handle_community_build_request(app_state: &AppState, sid: &str, x: i32, y: i32) {
    let mut state = app_state.state.write().await;
    let (player_id, community_id) = match state.players.get(sid) {
        Some(player) => (player.id.clone(), player.community_id.clone()),
        None => return,
    };
    let lang = player_language(&state, &player_id);
    if community_id.is_some() {
        send_system_message(
            &mut state,
            &player_id,
            message_community_already_member(lang).to_string(),
        );
        return;
    }
    if tile_at(&app_state.noise, x, y) == TILE_WATER {
        send_system_message(
            &mut state,
            &player_id,
            message_build_on_land(lang).to_string(),
        );
        return;
    }

    let area = community_area_origin(x, y);
    if community_area_overlaps(&state, area) {
        send_system_message(
            &mut state,
            &player_id,
            message_community_area_overlap(lang).to_string(),
        );
        return;
    }
    if !community_area_is_clear(&state, area) {
        send_system_message(
            &mut state,
            &player_id,
            message_community_area_blocked(lang).to_string(),
        );
        return;
    }
    if resource_at_tile(&state, x, y) {
        send_system_message(
            &mut state,
            &player_id,
            message_clear_resource(lang).to_string(),
        );
        return;
    }

    let cost = [
        ItemStack {
            id: "wood".to_string(),
            count: COMMUNITY_CREATE_WOOD,
        },
        ItemStack {
            id: "stone".to_string(),
            count: COMMUNITY_CREATE_STONE,
        },
    ];
    let has_materials = match state.players.get(sid) {
        Some(player) => has_items(&player.inventory, &cost),
        None => return,
    };
    if !has_materials {
        send_system_message(
            &mut state,
            &player_id,
            message_not_enough_materials(lang).to_string(),
        );
        return;
    }
    {
        let player = match state.players.get_mut(sid) {
            Some(player) => player,
            None => return,
        };
        if !remove_items(&mut player.inventory, &cost) {
            send_system_message(
                &mut state,
                &player_id,
                message_not_enough_materials(lang).to_string(),
            );
            return;
        }
        player.last_inventory_hash = inventory_hash(&player.inventory);
    }

    let community_id = Uuid::new_v4().to_string();
    let mut members = HashSet::new();
    members.insert(player_id.clone());
    let community = Community {
        id: community_id.clone(),
        members,
        areas: vec![area],
    };
    insert_community_area_tiles(&mut state, &community_id, area);
    state.communities.insert(community_id.clone(), community);
    if let Some(player) = state.players.get_mut(sid) {
        player.community_id = Some(community_id.clone());
    }

    let mut added_structures = Vec::new();
    let well_id = state.next_structure_id();
    let well = StructureTile {
        id: well_id,
        kind: "community_well".to_string(),
        x,
        y,
        owner_id: community_id.clone(),
    };
    state
        .structure_tiles
        .insert(TileCoord { x, y }, well.clone());
    added_structures.push(well);

    let mut fence_structures = rebuild_community_fences(&mut state, app_state, &community_id);
    added_structures.append(&mut fence_structures);

    if let Some(player) = state.players.get(&player_id) {
        let items = build_inventory_items(&player.inventory, app_state.data.as_ref(), lang);
        if let Some(sender) = state.clients.get(&player_id) {
            let _ = sender.send(ServerMessage::Inventory { items });
        }
    }

    let mut chunks = HashSet::new();
    for structure in &added_structures {
        chunks.insert(chunk_coord_for_tile(
            structure.x,
            structure.y,
            app_state.world.chunk_size,
        ));
    }
    let public_structures: Vec<StructurePublic> =
        added_structures.iter().map(StructurePublic::from).collect();
    send_to_players_in_chunks(
        &mut state,
        app_state.world.chunk_size,
        &chunks,
        ServerMessage::StructureUpdate {
            structures: public_structures.clone(),
            state: "added".to_string(),
        },
    );
    send_system_message(
        &mut state,
        &player_id,
        message_community_created(lang).to_string(),
    );

    let store = app_state.store.clone();
    let community_doc = CommunityDoc {
        id: community_id.clone(),
        members: vec![player_id.clone()],
        areas: vec![area],
    };
    let player_doc = state.players.get(&player_id).map(|player| player.to_doc());
    let docs: Vec<StructureDoc> = added_structures
        .iter()
        .map(|tile| StructureDoc {
            id: tile.id as i64,
            kind: tile.kind.clone(),
            x: tile.x,
            y: tile.y,
            owner_id: tile.owner_id.clone(),
        })
        .collect();
    tokio::spawn(async move {
        if let Some(doc) = player_doc {
            let _ = store.save_player(&doc).await;
        }
        let _ = store.save_community(&community_doc).await;
        let _ = store.insert_structures(&docs).await;
    });
}

async fn handle_group_build_request(
    app_state: &AppState,
    sid: &str,
    build_kind: &str,
    x: i32,
    y: i32,
) {
    let mut state = app_state.state.write().await;
    let (player_id, player_name, player_pos, community_id) = match state.players.get(sid) {
        Some(player) => (
            player.id.clone(),
            player.name.clone(),
            (player.x, player.y),
            player.community_id.clone(),
        ),
        None => return,
    };
    let lang = player_language(&state, &player_id);
    let community_id = match community_id {
        Some(id) => id,
        None => {
            send_system_message(
                &mut state,
                &player_id,
                message_community_not_member(lang).to_string(),
            );
            return;
        }
    };
    if !position_in_community_area(&state, player_pos.0, player_pos.1, &community_id) {
        send_system_message(
            &mut state,
            &player_id,
            message_community_build_in_area(lang).to_string(),
        );
        return;
    }

    let (required_count, cost) = match build_kind {
        "castle" => (
            CASTLE_MEMBER_COUNT,
            vec![
                ItemStack {
                    id: "stone".to_string(),
                    count: 1000,
                },
                ItemStack {
                    id: "wood".to_string(),
                    count: 50,
                },
            ],
        ),
        "silo" => (
            SILO_MEMBER_COUNT,
            vec![ItemStack {
                id: "wood".to_string(),
                count: 15,
            }],
        ),
        "church" => (
            CHURCH_MEMBER_COUNT,
            vec![
                ItemStack {
                    id: "stone".to_string(),
                    count: 300,
                },
                ItemStack {
                    id: "wood".to_string(),
                    count: 70,
                },
            ],
        ),
        _ => return,
    };

    let community_members: Vec<String> = match state.communities.get(&community_id) {
        Some(community) => community.members.iter().cloned().collect(),
        None => return,
    };
    let mut candidates: Vec<String> = community_members
        .iter()
        .filter(|id| state.clients.contains_key(*id))
        .filter(|id| {
            if let Some(member) = state.players.get(*id) {
                player_in_community_area(&state, member, &community_id)
            } else {
                false
            }
        })
        .cloned()
        .collect();
    if !candidates.contains(&player_id) {
        candidates.push(player_id.clone());
    }
    if candidates.len() < required_count {
        send_system_message(
            &mut state,
            &player_id,
            message_build_need_members(lang).to_string(),
        );
        return;
    }

    candidates.sort();
    let mut participants = Vec::new();
    for id in candidates {
        participants.push(id);
        if participants.len() == required_count {
            break;
        }
    }

    let (tiles, _placements) = match group_build_layout(build_kind, x, y) {
        Some(result) => result,
        None => return,
    };
    for tile in &tiles {
        if state
            .structure_tiles
            .contains_key(&TileCoord { x: tile.x, y: tile.y })
        {
            send_system_message(
                &mut state,
                &player_id,
                message_spot_occupied(lang).to_string(),
            );
            return;
        }
        if resource_at_tile(&state, tile.x, tile.y) {
            send_system_message(
                &mut state,
                &player_id,
                message_clear_resource(lang).to_string(),
            );
            return;
        }
        if tile_at(&app_state.noise, tile.x, tile.y) == TILE_WATER {
            send_system_message(
                &mut state,
                &player_id,
                message_build_on_land(lang).to_string(),
            );
            return;
        }
        if community_tile_owner(&state, tile.x, tile.y)
            .map(|id| id.as_str())
            != Some(&community_id)
        {
            send_system_message(
                &mut state,
                &player_id,
                message_community_build_in_area(lang).to_string(),
            );
            return;
        }
    }

    let request_id = Uuid::new_v4().to_string();
    let pending = PendingApproval {
        kind: ApprovalKind::Build {
            community_id: community_id.clone(),
            build_kind: build_kind.to_string(),
            x,
            y,
            requester_id: player_id.clone(),
            participants: participants.clone(),
        },
        approvals: HashSet::new(),
        required: participants.iter().cloned().collect(),
        created_ms: now_millis(),
    };
    state.pending_approvals.insert(request_id.clone(), pending);

    let requester_name = player_name.clone();
    for participant_id in &participants {
        if let Some(sender) = state.clients.get(participant_id) {
            let participant_lang = player_language(&state, participant_id);
            let (title, text, accept, decline) = approval_build_text(
                build_kind,
                &requester_name,
                &cost,
                participant_lang,
            );
            let _ = sender.send(ServerMessage::ApprovalRequest {
                request_id: request_id.clone(),
                title,
                text,
                accept,
                decline,
            });
        }
    }
    send_system_message(
        &mut state,
        &player_id,
        message_build_request_sent(lang).to_string(),
    );
}

async fn handle_join_approval_complete(
    app_state: &AppState,
    community_id: &str,
    target_id: &str,
    area: CommunityArea,
    requester_id: &str,
    lang: Language,
) {
    let mut state = app_state.state.write().await;
    let community_snapshot = match state.communities.get(community_id) {
        Some(community) => Community {
            id: community.id.clone(),
            members: community.members.clone(),
            areas: community.areas.clone(),
        },
        None => {
            send_system_message(
                &mut state,
                requester_id,
                message_community_missing(lang).to_string(),
            );
            return;
        }
    };
    if !community_snapshot.members.contains(requester_id) && requester_id != target_id {
        return;
    }
    if !community_area_adjacent(&community_snapshot, area) {
        send_system_message(
            &mut state,
            requester_id,
            message_community_area_adjacent(lang).to_string(),
        );
        return;
    }
    if community_area_overlaps(&state, area) || !community_area_is_clear(&state, area) {
        send_system_message(
            &mut state,
            requester_id,
            message_community_area_blocked(lang).to_string(),
        );
        return;
    }
    if let Some(community) = state.communities.get_mut(community_id) {
        community.members.insert(target_id.to_string());
        community.areas.push(area);
    }
    insert_community_area_tiles(&mut state, community_id, area);

    let joined_player_id = if let Some(player) = state.players.get_mut(target_id) {
        player.community_id = Some(community_id.to_string());
        Some(player.id.clone())
    } else {
        None
    };
    if let Some(player_id) = joined_player_id {
        send_system_message(
            &mut state,
            &player_id,
            message_community_joined(lang).to_string(),
        );
    }

    let removed = remove_community_fences(&mut state, community_id);
    let added = rebuild_community_fences(&mut state, app_state, community_id);
    if !removed.is_empty() || !added.is_empty() {
        let mut chunks = HashSet::new();
        for structure in removed.iter().chain(added.iter()) {
            chunks.insert(chunk_coord_for_tile(
                structure.x,
                structure.y,
                app_state.world.chunk_size,
            ));
        }
        if !removed.is_empty() {
            let removed_public: Vec<StructurePublic> =
                removed.iter().map(StructurePublic::from).collect();
            send_to_players_in_chunks(
                &state,
                app_state.world.chunk_size,
                &chunks,
                ServerMessage::StructureUpdate {
                    structures: removed_public,
                    state: "removed".to_string(),
                },
            );
        }
        if !added.is_empty() {
            let added_public: Vec<StructurePublic> =
                added.iter().map(StructurePublic::from).collect();
            send_to_players_in_chunks(
                &state,
                app_state.world.chunk_size,
                &chunks,
                ServerMessage::StructureUpdate {
                    structures: added_public,
                    state: "added".to_string(),
                },
            );
        }
    }

    let community_doc = state
        .communities
        .get(community_id)
        .map(|community| CommunityDoc {
            id: community.id.clone(),
            members: community.members.iter().cloned().collect(),
            areas: community.areas.clone(),
        })
        .unwrap();
    let target_doc = state.players.get(target_id).map(|player| player.to_doc());
    let store = app_state.store.clone();
    let fence_docs: Vec<StructureDoc> = added
        .iter()
        .map(|tile| StructureDoc {
            id: tile.id as i64,
            kind: tile.kind.clone(),
            x: tile.x,
            y: tile.y,
            owner_id: tile.owner_id.clone(),
        })
        .collect();
    let community_id = community_id.to_string();
    tokio::spawn(async move {
        let _ = store.save_community(&community_doc).await;
        if let Some(doc) = target_doc {
            let _ = store.save_player(&doc).await;
        }
        let _ = store
            .delete_structures_by_owner_and_kinds(
                &community_id,
                &["community_fence", "community_float", "community_gate"],
            )
            .await;
        let _ = store.insert_structures(&fence_docs).await;
    });
}

async fn handle_build_approval_complete(
    app_state: &AppState,
    community_id: &str,
    build_kind: &str,
    x: i32,
    y: i32,
    requester_id: &str,
    participants: &[String],
    lang: Language,
) {
    let mut state = app_state.state.write().await;
    let community_has_requester = match state.communities.get(community_id) {
        Some(community) => community.members.contains(requester_id),
        None => {
            send_system_message(
                &mut state,
                requester_id,
                message_community_missing(lang).to_string(),
            );
            return;
        }
    };
    if !community_has_requester {
        return;
    }

    let (tiles, placements) = match group_build_layout(build_kind, x, y) {
        Some(result) => result,
        None => return,
    };

    for tile in &tiles {
        if state
            .structure_tiles
            .contains_key(&TileCoord { x: tile.x, y: tile.y })
        {
            send_system_message(&mut state, requester_id, message_spot_occupied(lang).to_string());
            return;
        }
        if resource_at_tile(&state, tile.x, tile.y) {
            send_system_message(&mut state, requester_id, message_clear_resource(lang).to_string());
            return;
        }
        if tile_at(&app_state.noise, tile.x, tile.y) == TILE_WATER {
            send_system_message(&mut state, requester_id, message_build_on_land(lang).to_string());
            return;
        }
        if community_tile_owner(&state, tile.x, tile.y)
            .map(|id| id.as_str())
            != Some(community_id)
        {
            send_system_message(
                &mut state,
                requester_id,
                message_community_build_in_area(lang).to_string(),
            );
            return;
        }
    }

    let (required_count, cost) = match build_kind {
        "castle" => (
            CASTLE_MEMBER_COUNT,
            vec![
                ItemStack {
                    id: "stone".to_string(),
                    count: 1000,
                },
                ItemStack {
                    id: "wood".to_string(),
                    count: 50,
                },
            ],
        ),
        "silo" => (
            SILO_MEMBER_COUNT,
            vec![ItemStack {
                id: "wood".to_string(),
                count: 15,
            }],
        ),
        "church" => (
            CHURCH_MEMBER_COUNT,
            vec![
                ItemStack {
                    id: "stone".to_string(),
                    count: 300,
                },
                ItemStack {
                    id: "wood".to_string(),
                    count: 70,
                },
            ],
        ),
        _ => return,
    };

    if participants.len() < required_count {
        send_system_message(
            &mut state,
            requester_id,
            message_build_need_members(lang).to_string(),
        );
        return;
    }

    for participant_id in participants {
        let participant = match state.players.get(participant_id) {
            Some(player) => player,
            None => {
                send_system_message(
                    &mut state,
                    requester_id,
                    message_build_request_declined(lang).to_string(),
                );
                return;
            }
        };
        if !player_in_community_area(&state, participant, community_id) {
            send_system_message(
                &mut state,
                requester_id,
                message_build_need_members(lang).to_string(),
            );
            return;
        }
        if !has_items(&participant.inventory, &cost) {
            send_system_message(&mut state, requester_id, message_not_enough_materials(lang).to_string());
            return;
        }
    }

    for participant_id in participants {
        if let Some(player) = state.players.get_mut(participant_id) {
            remove_items(&mut player.inventory, &cost);
            player.last_inventory_hash = inventory_hash(&player.inventory);
            let items = build_inventory_items(&player.inventory, app_state.data.as_ref(), lang);
            if let Some(sender) = state.clients.get(participant_id) {
                let _ = sender.send(ServerMessage::Inventory { items });
            }
        }
    }

    let structure_id = state.next_structure_id();
    let mut new_tiles = Vec::new();
    for (tile, kind) in placements {
        let structure = StructureTile {
            id: structure_id,
            kind,
            x: tile.x,
            y: tile.y,
            owner_id: requester_id.to_string(),
        };
        state
            .structure_tiles
            .insert(TileCoord { x: tile.x, y: tile.y }, structure.clone());
        new_tiles.push(structure);
    }

    let structures_public: Vec<StructurePublic> = new_tiles.iter().map(StructurePublic::from).collect();
    let mut chunks = HashSet::new();
    for structure in &structures_public {
        chunks.insert(chunk_coord_for_tile(
            structure.x,
            structure.y,
            app_state.world.chunk_size,
        ));
    }
    send_to_players_in_chunks(
        &state,
        app_state.world.chunk_size,
        &chunks,
        ServerMessage::StructureUpdate {
            structures: structures_public,
            state: "added".to_string(),
        },
    );
    send_system_message(&mut state, requester_id, message_build_success(lang, build_kind));

    if build_kind == "silo" {
        state.storages.insert(
            structure_id,
            Storage {
                id: structure_id,
                community_id: community_id.to_string(),
                inventory: HashMap::new(),
            },
        );
    }

    let docs: Vec<StructureDoc> = new_tiles
        .into_iter()
        .map(|tile| StructureDoc {
            id: structure_id as i64,
            kind: tile.kind,
            x: tile.x,
            y: tile.y,
            owner_id: tile.owner_id,
        })
        .collect();
    let store = app_state.store.clone();
    let storage_doc = if build_kind == "silo" {
        Some(StorageDoc {
            id: structure_id as i64,
            community_id: community_id.to_string(),
            inventory: HashMap::new(),
        })
    } else {
        None
    };
    tokio::spawn(async move {
        let _ = store.insert_structures(&docs).await;
        if let Some(doc) = storage_doc {
            let _ = store.save_storage(&doc).await;
        }
    });
}

fn group_build_layout(build_kind: &str, x: i32, y: i32) -> Option<(Vec<TileCoord>, Vec<(TileCoord, String)>)> {
    let (width, height, root_kind, fill_kind) = match build_kind {
        "castle" => (CASTLE_SIZE, CASTLE_SIZE, "castle_root", "castle_fill"),
        "silo" => (SILO_SIZE, SILO_SIZE, "silo_root", "silo_fill"),
        "church" => (CHURCH_SIZE, CHURCH_SIZE, "church_root", "church_fill"),
        _ => return None,
    };
    let mut tiles = Vec::new();
    let mut placements = Vec::new();
    let base_y = y - (height - 1);
    for dy in 0..height {
        for dx in 0..width {
            let coord = TileCoord {
                x: x + dx,
                y: base_y + dy,
            };
            tiles.push(coord);
            let kind = if dy == height - 1 && dx == 0 {
                root_kind
            } else {
                fill_kind
            };
            placements.push((coord, kind.to_string()));
        }
    }
    Some((tiles, placements))
}

fn find_nearby_storage(player: &Player, state: &GameState) -> Option<(u64, String)> {
    for structure in state.structure_tiles.values() {
        if structure.kind != "silo_root" {
            continue;
        }
        let (sx, sy) = tile_anchor_position(structure.x, structure.y);
        if distance(player.x, player.y, sx, sy) <= INTERACT_RANGE {
            if let Some(storage) = state.storages.get(&structure.id) {
                return Some((structure.id, storage.community_id.clone()));
            }
        }
    }
    None
}

fn player_near_storage_at(state: &GameState, x: f32, y: f32, storage_id: u64) -> bool {
    for structure in state.structure_tiles.values() {
        if structure.kind == "silo_root" && structure.id == storage_id {
            let (sx, sy) = tile_anchor_position(structure.x, structure.y);
            return distance(x, y, sx, sy) <= INTERACT_RANGE;
        }
    }
    false
}

fn pending_join_request_exists(state: &GameState, community_id: &str, target_id: &str) -> bool {
    state.pending_approvals.values().any(|pending| match &pending.kind {
        ApprovalKind::Join {
            community_id: existing,
            target_id: existing_target,
            ..
        } => existing == community_id && existing_target == target_id,
        _ => false,
    })
}

fn find_adjacent_community_request(
    state: &GameState,
    area: &CommunityArea,
) -> Option<String> {
    for (community_id, community) in state.communities.iter() {
        if community_area_adjacent(community, *area) {
            return Some(community_id.clone());
        }
    }
    None
}

fn find_nearby_join_target(player: &Player, state: &GameState) -> Option<(String, CommunityArea)> {
    for other in state.players.values() {
        if other.id == player.id {
            continue;
        }
        if other.community_id.is_some() {
            continue;
        }
        if distance(player.x, player.y, other.x, other.y) > INTERACT_RANGE {
            continue;
        }
        let tile = player_tile(other);
        return Some((other.id.clone(), community_area_origin(tile.x, tile.y)));
    }
    None
}

fn start_join_request(
    state: &mut GameState,
    app_state: &AppState,
    community_id: &str,
    target_id: &str,
    requester_id: &str,
    area: CommunityArea,
    now_ms: i64,
) {
    let community = match state.communities.get(community_id) {
        Some(community) => community,
        None => return,
    };
    let mut required = HashSet::new();
    for member_id in &community.members {
        if state.clients.contains_key(member_id) {
            required.insert(member_id.clone());
        }
    }
    if required.is_empty() {
        drop(required);
        let app_state = app_state.clone();
        let community_id = community_id.to_string();
        let target_id = target_id.to_string();
        let requester_id = requester_id.to_string();
        tokio::spawn(async move {
            handle_join_approval_complete(
                &app_state,
                &community_id,
                &target_id,
                area,
                &requester_id,
                Language::En,
            )
            .await;
        });
        return;
    }
    let request_id = Uuid::new_v4().to_string();
    let pending = PendingApproval {
        kind: ApprovalKind::Join {
            community_id: community_id.to_string(),
            target_id: target_id.to_string(),
            area,
            requester_id: requester_id.to_string(),
        },
        approvals: HashSet::new(),
        required: required.clone(),
        created_ms: now_ms,
    };
    state.pending_approvals.insert(request_id.clone(), pending);
    let requester_name = state
        .players
        .get(requester_id)
        .map(|player| player.name.clone())
        .unwrap_or_else(|| "Wanderer".to_string());
    let target_name = state
        .players
        .get(target_id)
        .map(|player| player.name.clone())
        .unwrap_or_else(|| "Wanderer".to_string());
    for member_id in required {
        if let Some(sender) = state.clients.get(&member_id) {
            let lang = player_language(state, &member_id);
            let (title, text, accept, decline) = approval_join_text(&requester_name, &target_name, lang);
            let _ = sender.send(ServerMessage::ApprovalRequest {
                request_id: request_id.clone(),
                title,
                text,
                accept,
                decline,
            });
        }
    }
}

fn remove_community_from_state(state: &mut GameState, community_id: &str) {
    state.communities.remove(community_id);
    state
        .community_tiles
        .retain(|_, owner| owner != community_id);
    state.structure_tiles.retain(|_, structure| {
        if structure.owner_id != community_id {
            return true;
        }
        !matches!(
            structure.kind.as_str(),
            "community_fence" | "community_float" | "community_gate" | "community_well"
        )
    });
    state
        .storages
        .retain(|_, storage| storage.community_id != community_id);
}

fn remove_community_fences(state: &mut GameState, community_id: &str) -> Vec<StructureTile> {
    let mut removed = Vec::new();
    state.structure_tiles.retain(|_, structure| {
        if structure.owner_id == community_id
            && matches!(
                structure.kind.as_str(),
                "community_fence" | "community_float" | "community_gate"
            )
        {
            removed.push(structure.clone());
            return false;
        }
        true
    });
    removed
}

fn rebuild_community_fences(
    state: &mut GameState,
    app_state: &AppState,
    community_id: &str,
) -> Vec<StructureTile> {
    let mut community_tiles = HashSet::new();
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for (tile, owner) in &state.community_tiles {
        if owner == community_id {
            community_tiles.insert(*tile);
            min_x = min_x.min(tile.x);
            max_x = max_x.max(tile.x);
            min_y = min_y.min(tile.y);
            max_y = max_y.max(tile.y);
        }
    }
    let mut added = Vec::new();
    let center_x = (min_x + max_x) / 2;
    let center_y = (min_y + max_y) / 2;
    let mut boundary_tiles = Vec::new();
    for tile in &community_tiles {
        let neighbors = [
            TileCoord {
                x: tile.x + 1,
                y: tile.y,
            },
            TileCoord {
                x: tile.x - 1,
                y: tile.y,
            },
            TileCoord {
                x: tile.x,
                y: tile.y + 1,
            },
            TileCoord {
                x: tile.x,
                y: tile.y - 1,
            },
        ];
        if neighbors.iter().all(|neighbor| community_tiles.contains(neighbor)) {
            continue;
        }
        boundary_tiles.push(*tile);
    }

    let mut gate_tile: Option<TileCoord> = None;
    let mut best_score = i32::MAX;
    for tile in &boundary_tiles {
        if tile_at(&app_state.noise, tile.x, tile.y) == TILE_WATER {
            continue;
        }
        let score = (tile.x - center_x).abs() + (tile.y - center_y).abs();
        if score < best_score {
            best_score = score;
            gate_tile = Some(*tile);
        }
    }
    if gate_tile.is_none() {
        gate_tile = boundary_tiles.first().copied();
    }

    for tile in boundary_tiles {
        let kind = if Some(tile) == gate_tile {
            "community_gate"
        } else if tile_at(&app_state.noise, tile.x, tile.y) == TILE_WATER {
            "community_float"
        } else {
            "community_fence"
        };
        let structure = StructureTile {
            id: state.next_structure_id(),
            kind: kind.to_string(),
            x: tile.x,
            y: tile.y,
            owner_id: community_id.to_string(),
        };
        state
            .structure_tiles
            .insert(TileCoord { x: tile.x, y: tile.y }, structure.clone());
        added.push(structure);
    }
    added
}

fn approval_join_text(
    requester_name: &str,
    target_name: &str,
    lang: Language,
) -> (String, String, String, String) {
    match lang {
        Language::De => (
            "Beitrittsanfrage".to_string(),
            format!("{} möchte {} zur Gemeinschaft hinzufügen.", requester_name, target_name),
            "Zustimmen".to_string(),
            "Ablehnen".to_string(),
        ),
        Language::En => (
            "Join request".to_string(),
            format!("{} wants to add {} to the community.", requester_name, target_name),
            "Approve".to_string(),
            "Decline".to_string(),
        ),
    }
}

fn chunk_coord_for_position(x: f32, y: f32, chunk_size: i32) -> ChunkCoord {
    let size = chunk_size as f32;
    ChunkCoord {
        x: (x / size).floor() as i32,
        y: (y / size).floor() as i32,
    }
}

fn chunk_coord_for_tile(x: i32, y: i32, chunk_size: i32) -> ChunkCoord {
    let size = chunk_size as f32;
    ChunkCoord {
        x: (x as f32 / size).floor() as i32,
        y: (y as f32 / size).floor() as i32,
    }
}

fn chunk_in_radius(center: ChunkCoord, coord: ChunkCoord, radius: i32) -> bool {
    (coord.x - center.x).abs() <= radius && (coord.y - center.y).abs() <= radius
}

fn send_to_players_in_chunk(
    state: &GameState,
    chunk_size: i32,
    coord: ChunkCoord,
    msg: ServerMessage,
) {
    for (player_id, sender) in state.clients.iter() {
        if let Some(player) = state.players.get(player_id) {
            let center = chunk_coord_for_position(player.x, player.y, chunk_size);
            if chunk_in_radius(center, coord, ENTITY_VISIBILITY_RADIUS) {
                let _ = sender.send(msg.clone());
            }
        }
    }
}

fn send_to_players_in_chunks(
    state: &GameState,
    chunk_size: i32,
    chunks: &HashSet<ChunkCoord>,
    msg: ServerMessage,
) {
    for (player_id, sender) in state.clients.iter() {
        if let Some(player) = state.players.get(player_id) {
            let center = chunk_coord_for_position(player.x, player.y, chunk_size);
            if chunks
                .iter()
                .any(|coord| chunk_in_radius(center, *coord, ENTITY_VISIBILITY_RADIUS))
            {
                let _ = sender.send(msg.clone());
            }
        }
    }
}

fn collect_active_chunks(state: &GameState, chunk_size: i32) -> HashSet<ChunkCoord> {
    let mut keep = HashSet::new();
    for player in state.players.values() {
        let center = chunk_coord_for_position(player.x, player.y, chunk_size);
        for dx in -CHUNK_KEEP_RADIUS..=CHUNK_KEEP_RADIUS {
            for dy in -CHUNK_KEEP_RADIUS..=CHUNK_KEEP_RADIUS {
                keep.insert(ChunkCoord {
                    x: center.x + dx,
                    y: center.y + dy,
                });
            }
        }
    }
    keep
}

fn prune_chunks(state: &mut GameState, now_ms: i64, chunk_size: i32) {
    let keep = collect_active_chunks(state, chunk_size);
    for coord in &keep {
        state.chunk_last_access.insert(*coord, now_ms);
    }

    let mut expired = Vec::new();
    for (coord, last_access) in state.chunk_last_access.iter() {
        if keep.contains(coord) {
            continue;
        }
        if now_ms - *last_access > CHUNK_TTL_MS {
            expired.push(*coord);
        }
    }

    if expired.is_empty() {
        return;
    }

    let expired_set: HashSet<ChunkCoord> = expired.iter().copied().collect();
    for coord in &expired {
        state.chunk_last_access.remove(coord);
        state.resources.remove(coord);
        state.spawned_chunks.remove(coord);
    }

    state.monsters.retain(|_, monster| {
        !expired_set.contains(&chunk_coord_for_position(monster.x, monster.y, chunk_size))
    });
    state.projectiles.retain(|_, projectile| {
        !expired_set.contains(&chunk_coord_for_position(projectile.x, projectile.y, chunk_size))
    });
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
                let prev_inventory_hash = player.last_inventory_hash;
                update_player_movement(
                    &mut player,
                    input,
                    &state,
                    &app_state.noise,
                    dt,
                );
                handle_player_actions(
                    &mut player,
                    input,
                    now_ms,
                    &mut state,
                    app_state,
                );
                apply_player_regen(&mut player, now_ms);
                let next_inventory_hash = inventory_hash(&player.inventory);
                if next_inventory_hash != prev_inventory_hash {
                    player.last_inventory_hash = next_inventory_hash;
                    if let Some(sender) = state.clients.get(&id) {
                        let lang = player_language(&state, &id);
                        let items =
                            build_inventory_items(&player.inventory, app_state.data.as_ref(), lang);
                        let _ = sender.send(ServerMessage::Inventory { items });
                    }
                }
                state.players.insert(id, player);
            }
        }

        update_monsters(
            &mut state,
            now_ms,
            dt,
            &app_state.noise,
            &app_state.data,
        );
        update_projectiles(&mut state, now_ms, dt, &app_state.data);
        update_resources(&mut state, now_ms, &app_state.data, app_state.world.chunk_size);
        prune_chunks(&mut state, now_ms, app_state.world.chunk_size);

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

        let mut expired_approvals = Vec::new();
        for (id, pending) in state.pending_approvals.iter() {
            if now_ms - pending.created_ms > COMMUNITY_APPROVAL_TIMEOUT_MS {
                expired_approvals.push(id.clone());
            }
        }
        for id in expired_approvals {
            if let Some(pending) = state.pending_approvals.remove(&id) {
                notify_approval_timeout(&mut state, &pending);
            }
        }

        let chunk_size = app_state.world.chunk_size;
        let mut players_by_chunk: HashMap<ChunkCoord, Vec<PlayerPublic>> = HashMap::new();
        let mut monsters_by_chunk: HashMap<ChunkCoord, Vec<MonsterPublic>> = HashMap::new();
        let mut projectiles_by_chunk: HashMap<ChunkCoord, Vec<ProjectilePublic>> = HashMap::new();

        for player in state.players.values() {
            let coord = chunk_coord_for_position(player.x, player.y, chunk_size);
            players_by_chunk
                .entry(coord)
                .or_default()
                .push(PlayerPublic::from(player));
        }
        for monster in state.monsters.values() {
            let coord = chunk_coord_for_position(monster.x, monster.y, chunk_size);
            monsters_by_chunk
                .entry(coord)
                .or_default()
                .push(MonsterPublic::from(monster));
        }
        for projectile in state.projectiles.values() {
            let coord = chunk_coord_for_position(projectile.x, projectile.y, chunk_size);
            projectiles_by_chunk
                .entry(coord)
                .or_default()
                .push(ProjectilePublic::from(projectile));
        }

        let client_entries: Vec<(String, mpsc::UnboundedSender<ServerMessage>)> = state
            .clients
            .iter()
            .map(|(id, sender)| (id.clone(), sender.clone()))
            .collect();

        for (client_id, sender) in client_entries {
            let player = match state.players.get(&client_id) {
                Some(player) => player,
                None => continue,
            };
            let center = chunk_coord_for_position(player.x, player.y, chunk_size);
            let mut visible_players = Vec::new();
            let mut visible_monsters = Vec::new();
            let mut visible_projectiles = Vec::new();
            let mut visible_player_ids = HashSet::new();
            let mut visible_monster_ids = HashSet::new();
            let mut visible_projectile_ids = HashSet::new();

            for dx in -ENTITY_VISIBILITY_RADIUS..=ENTITY_VISIBILITY_RADIUS {
                for dy in -ENTITY_VISIBILITY_RADIUS..=ENTITY_VISIBILITY_RADIUS {
                    let coord = ChunkCoord {
                        x: center.x + dx,
                        y: center.y + dy,
                    };
                    if let Some(players) = players_by_chunk.get(&coord) {
                        for player_public in players {
                            let mut entry = player_public.clone();
                            if entry.id == client_id {
                                entry.last_input_seq = Some(player.last_input_seq);
                            }
                            if visible_player_ids.insert(entry.id.clone()) {
                                visible_players.push(entry);
                            }
                        }
                    }
                    if let Some(monsters) = monsters_by_chunk.get(&coord) {
                        for monster_public in monsters {
                            if visible_monster_ids.insert(monster_public.id) {
                                visible_monsters.push(monster_public.clone());
                            }
                        }
                    }
                    if let Some(projectiles) = projectiles_by_chunk.get(&coord) {
                        for projectile_public in projectiles {
                            if visible_projectile_ids.insert(projectile_public.id) {
                                visible_projectiles.push(projectile_public.clone());
                            }
                        }
                    }
                }
            }

            let visibility = state
                .visibility
                .entry(client_id.clone())
                .or_insert_with(VisibilityState::default);
            let removed_players: Vec<String> = visibility
                .players
                .difference(&visible_player_ids)
                .cloned()
                .collect();
            let removed_monsters: Vec<u64> = visibility
                .monsters
                .difference(&visible_monster_ids)
                .cloned()
                .collect();
            let removed_projectiles: Vec<u64> = visibility
                .projectiles
                .difference(&visible_projectile_ids)
                .cloned()
                .collect();

            if !removed_players.is_empty()
                || !removed_monsters.is_empty()
                || !removed_projectiles.is_empty()
            {
                let _ = sender.send(ServerMessage::EntitiesRemove {
                    players: removed_players,
                    monsters: removed_monsters,
                    projectiles: removed_projectiles,
                });
            }

            visibility.players = visible_player_ids;
            visibility.monsters = visible_monster_ids;
            visibility.projectiles = visible_projectile_ids;

            let _ = sender.send(ServerMessage::EntitiesUpdate {
                players: visible_players,
                monsters: visible_monsters,
                projectiles: visible_projectiles,
            });
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
    state: &GameState,
    noise: &WorldNoise,
    dt: f32,
) {
    player.last_input_seq = input.seq;
    let mut dx = input.dir_x;
    let mut dy = input.dir_y;
    let input_len = (dx * dx + dy * dy).sqrt();
    if input_len > 0.01 {
        dx /= input_len;
        dy /= input_len;
        if let (Some(expected_x), Some(expected_y)) = (input.expected_x, input.expected_y) {
            let corr_x = expected_x - player.x;
            let corr_y = expected_y - player.y;
            let corr_dist = (corr_x * corr_x + corr_y * corr_y).sqrt();
            if corr_dist > 0.01 && corr_dist <= EXPECTED_POS_CORRECTION_RANGE {
                let corr_weight =
                    (corr_dist / EXPECTED_POS_CORRECTION_RANGE) * EXPECTED_POS_CORRECTION_WEIGHT;
                dx += (corr_x / corr_dist) * corr_weight;
                dy += (corr_y / corr_dist) * corr_weight;
                let combined_len = (dx * dx + dy * dy).sqrt();
                if combined_len > 0.01 {
                    dx /= combined_len;
                    dy /= combined_len;
                }
            }
        }
    }
    if (dx * dx + dy * dy).sqrt() > 0.01 {
        player.face_x = dx;
        player.face_y = dy;
    }

    let next_x = player.x + dx * PLAYER_SPEED * dt;
    let next_y = player.y + dy * PLAYER_SPEED * dt;

    if can_walk_player(state, noise, player, next_x, player.y) {
        player.x = next_x;
    }
    if can_walk_player(state, noise, player, player.x, next_y) {
        player.y = next_y;
    }
}

fn apply_player_regen(player: &mut Player, now_ms: i64) {
    if player.hp >= MAX_HP {
        player.last_regen_ms = now_ms;
        return;
    }

    if now_ms - player.last_regen_ms >= PLAYER_REGEN_INTERVAL_MS {
        player.hp = (player.hp + 1).min(MAX_HP);
        player.last_regen_ms = now_ms;
    }
}

fn handle_player_actions(
    player: &mut Player,
    input: InputState,
    now_ms: i64,
    state: &mut GameState,
    app_state: &AppState,
) {
    if input.gather && now_ms - player.last_gather_ms >= 400 {
        let lang = player_language(state, &player.id);
        let mut messages = Vec::new();
        let mut resource_update: Option<(ResourceNodePublic, String)> = None;
        let mut did_gather = false;

        {
            if let Some((resource, def, community_owner)) =
                find_nearby_resource(player, state, app_state.data.as_ref())
            {
                if let Some(community_id) = community_owner.as_ref() {
                    if player.community_id.as_ref() != Some(community_id) {
                        messages.push(message_community_members_only(lang).to_string());
                    } else {
                        did_gather = true;
                        let tool_power = best_tool_power(&player.inventory, app_state.data.as_ref(), &def.tool);
                        if let Some(mut power) = tool_power {
                            if resource.kind == "rock" {
                                power = (power as f32 / resource.size.max(1) as f32).ceil() as i32;
                            }
                            resource.hp -= power;
                            if resource.hp <= 0 {
                                resource.hp = 0;
                                resource.respawn_at_ms = Some(now_ms + def.respawn_ms);
                                let yield_multiplier = resource.size.max(1);
                                for drop in &def.drops {
                                    let count = drop.count * yield_multiplier;
                                    add_item(&mut player.inventory, &drop.id, count);
                                    let item_name =
                                        localize_item_name(app_state.data.as_ref(), &drop.id, lang);
                                    messages.push(message_collected(&item_name, count, lang));
                                }
                                resource_update = Some((
                                    ResourceNodePublic::from(resource.clone()),
                                    "removed".to_string(),
                                ));
                            } else {
                                let resource_name =
                                    localize_resource_name(app_state.data.as_ref(), &resource.kind, lang);
                                messages.push(message_hit_resource(&resource_name, resource.hp, lang));
                            }
                        } else {
                            let tool_name = localize_tool_name(&def.tool, lang);
                            messages.push(message_need_tool(&tool_name, lang));
                        }
                    }
                } else {
                    did_gather = true;
                    let tool_power = best_tool_power(&player.inventory, app_state.data.as_ref(), &def.tool);
                    if let Some(mut power) = tool_power {
                        if resource.kind == "rock" {
                            power = (power as f32 / resource.size.max(1) as f32).ceil() as i32;
                        }
                        resource.hp -= power;
                        if resource.hp <= 0 {
                            resource.hp = 0;
                            resource.respawn_at_ms = Some(now_ms + def.respawn_ms);
                            let yield_multiplier = resource.size.max(1);
                            for drop in &def.drops {
                                let count = drop.count * yield_multiplier;
                                add_item(&mut player.inventory, &drop.id, count);
                                let item_name =
                                    localize_item_name(app_state.data.as_ref(), &drop.id, lang);
                                messages.push(message_collected(&item_name, count, lang));
                            }
                            resource_update = Some((
                                ResourceNodePublic::from(resource.clone()),
                                "removed".to_string(),
                            ));
                        } else {
                            let resource_name =
                                localize_resource_name(app_state.data.as_ref(), &resource.kind, lang);
                            messages.push(message_hit_resource(&resource_name, resource.hp, lang));
                        }
                    } else {
                        let tool_name = localize_tool_name(&def.tool, lang);
                        messages.push(message_need_tool(&tool_name, lang));
                    }
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
            let chunk = chunk_coord_for_tile(resource.x, resource.y, app_state.world.chunk_size);
            if let Some(sender) = state.clients.get(&player.id) {
                let _ = sender.send(ServerMessage::ResourceUpdate {
                    resource: resource.clone(),
                    state: state_label.clone(),
                });
            }
            send_to_players_in_chunk(
                state,
                app_state.world.chunk_size,
                chunk,
                ServerMessage::ResourceUpdate {
                    resource,
                    state: state_label,
                },
            );
        }
    }

    if input.attack {
        if let Some(weapon) = best_melee_weapon(&player.inventory, app_state.data.as_ref()) {
            if now_ms - player.last_attack_ms >= weapon.cooldown_ms {
                if attack_monster_melee(player, state, &weapon, app_state.data.as_ref()) {
                    player.last_attack_ms = now_ms;
                } else if try_ranged_attack(player, state, app_state.data.as_ref(), now_ms) {
                    player.last_attack_ms = now_ms;
                }
            }
        } else if try_ranged_attack(player, state, app_state.data.as_ref(), now_ms) {
            player.last_attack_ms = now_ms;
        }
    }

    if input.interact && now_ms - player.last_interact_ms >= 500 {
        if let Some(npc) = find_nearby_npc(player, app_state.data.as_ref()) {
            handle_npc_interaction(player, npc, state, app_state.data.as_ref());
            player.last_interact_ms = now_ms;
        } else if handle_storage_interaction(player, state, app_state) {
            player.last_interact_ms = now_ms;
        } else if handle_community_interaction(player, state, app_state, now_ms) {
            player.last_interact_ms = now_ms;
        }
    }

    if player.hp <= 0 {
        player.hp = MAX_HP;
        let (spawn_x, spawn_y) = spawn_near_campfire(&app_state.world, &app_state.noise);
        player.x = spawn_x;
        player.y = spawn_y;
        let lang = player_language(state, &player.id);
        send_system_message(state, &player.id, message_wake_up(lang).to_string());
    }
}

fn update_monsters(
    state: &mut GameState,
    now_ms: i64,
    dt: f32,
    noise: &WorldNoise,
    data: &GameData,
) {
    let structure_tiles = &state.structure_tiles;
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
                move_towards(
                    monster,
                    tx,
                    ty,
                    def.speed,
                    dt,
                    structure_tiles,
                    noise,
                );

                if nearest_dist <= MONSTER_ATTACK_RANGE
                    && now_ms - monster.last_attack_ms >= 800
                {
                    damage_events.push((target_id.clone(), def.damage, monster.kind.clone()));
                    monster.last_attack_ms = now_ms;
                }
            } else {
                wander(
                    monster,
                    now_ms,
                    def.speed,
                    dt,
                    structure_tiles,
                    noise,
                );
            }
        } else {
            wander(
                monster,
                now_ms,
                def.speed,
                dt,
                structure_tiles,
                noise,
            );
        }
    }

    for (player_id, damage, monster_id) in damage_events {
        let lang = player_language(state, &player_id);
        let monster_name = localize_monster_name(data, &monster_id, lang);
        let message = if let Some(player) = state.players.get_mut(&player_id) {
            player.hp -= damage;
            Some((
                player.id.clone(),
                message_monster_hits_you(&monster_name, player.hp.max(0), lang),
            ))
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

fn update_resources(state: &mut GameState, now_ms: i64, data: &GameData, chunk_size: i32) {
    let mut respawned = Vec::new();
    let mut grown = Vec::new();
    for resources in state.resources.values_mut() {
        for res in resources.iter_mut() {
            if res.hp <= 0 {
                if let Some(respawn_at) = res.respawn_at_ms {
                    if now_ms >= respawn_at {
                        if let Some(def) = data.resources.get(&res.kind) {
                            if state
                                .structure_tiles
                                .contains_key(&TileCoord { x: res.x, y: res.y })
                            {
                                res.respawn_at_ms = Some(now_ms + def.respawn_ms);
                                continue;
                            }
                            res.hp = def.hp;
                            res.respawn_at_ms = None;
                            if is_tree_kind(&res.kind) {
                                let (s, next) = tree_spawn_state(now_ms as u64, res.x, res.y, now_ms);
                                res.size = s;
                                res.next_growth_ms = next;
                            } else if res.kind == "rock" {
                                let size_roll = noise_hash01(now_ms as u64, res.x, res.y);
                                if size_roll > 0.8 {
                                    res.size = 3.min(ROCK_MAX_SIZE);
                                } else if size_roll > 0.5 {
                                    res.size = 2.min(ROCK_MAX_SIZE);
                                } else {
                                    res.size = 1;
                                }
                                res.next_growth_ms = None;
                            } else {
                                res.size = 1;
                                res.next_growth_ms = None;
                            }
                            respawned.push(ResourceNodePublic::from(res.clone()));
                        }
                    }
                }
            } else if is_tree_kind(&res.kind) {
                if res.size < TREE_MAX_SIZE {
                    if let Some(next_growth) = res.next_growth_ms {
                        if now_ms >= next_growth {
                            res.size += 1;
                            res.next_growth_ms = if res.size < TREE_MAX_SIZE {
                                let jitter = noise_hash01(now_ms as u64, res.x, res.y);
                                let delay =
                                    (TREE_GROW_INTERVAL_MS as f32 * (0.4 + jitter * 1.2)) as i64;
                                Some(now_ms + delay)
                            } else {
                                None
                            };
                            grown.push(ResourceNodePublic::from(res.clone()));
                        }
                    }
                } else {
                    res.next_growth_ms = None;
                }
            }
        }
    }

    if !respawned.is_empty() {
        for res in respawned {
            let chunk = chunk_coord_for_tile(res.x, res.y, chunk_size);
            send_to_players_in_chunk(
                state,
                chunk_size,
                chunk,
                ServerMessage::ResourceUpdate {
                    resource: res,
                    state: "spawned".to_string(),
                },
            );
        }
    }

    if !grown.is_empty() {
        for res in grown {
            let chunk = chunk_coord_for_tile(res.x, res.y, chunk_size);
            send_to_players_in_chunk(
                state,
                chunk_size,
                chunk,
                ServerMessage::ResourceUpdate {
                    resource: res,
                    state: "grown".to_string(),
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
    let lang = player_language(state, &player.id);
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
            let monster_name = localize_monster_name(data, &monster.kind, lang);
            message = Some(message_hit_monster(&monster_name, monster.hp.max(0), lang));
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
        let lang = player_language(state, &player.id);
        let item_name = localize_item_name(data, &ammo_id, lang);
        send_system_message(state, &player.id, message_out_of(&item_name, lang));
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
                if let Some(player) = award_to.as_deref_mut() {
                    add_item(&mut player.inventory, &drop.id, drop.count);
                    let player_id = player.id.clone();
                    let lang = player_language(state, &player_id);
                    let item_name = localize_item_name(data, &drop.id, lang);
                    send_system_message(
                        state,
                        &player_id,
                        message_picked_up(&item_name, drop.count, lang),
                    );
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
                        let lang = player_language(state, &player_id);
                        let item_name = localize_item_name(data, &drop.id, lang);
                        send_system_message(
                            state,
                            &player_id,
                            message_picked_up(&item_name, drop.count, lang),
                        );
                    }
                }
            }
        }
    }
}

fn handle_npc_interaction(player: &mut Player, npc: &NpcDef, state: &mut GameState, data: &GameData) {
    let lang = player_language(state, &player.id);
    let npc_name = localize_npc_name(npc, lang);
    let npc_dialog = localize_npc_dialog(npc, lang);
    if let Some(quest) = data.quests_by_npc.get(&npc.id) {
        if player.completed_quests.contains(&quest.id) {
            send_dialog(state, &player.id, &npc_name, &message_thanks_again(lang));
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
                &npc_name,
                &message_quest_complete(
                    &localize_quest_name(quest, lang),
                    &localize_quest_description(quest, lang),
                    lang,
                ),
            );
            for reward in &quest.rewards {
                let lang = player_language(state, &player.id);
                let item_name = localize_item_name(data, &reward.id, lang);
                send_system_message(
                    state,
                    &player.id,
                    message_reward(&item_name, reward.count, lang),
                );
            }
        } else {
            let mut needs = Vec::new();
            for req in &quest.requires {
                let have = player.inventory.get(&req.id).copied().unwrap_or(0);
                let lang = player_language(state, &player.id);
                let item_name = localize_item_name(data, &req.id, lang);
                needs.push(format!("{} {}/{}", item_name, have, req.count));
            }
            send_dialog(
                state,
                &player.id,
                &npc_name,
                &message_quest_needs(
                    &localize_quest_name(quest, lang),
                    &localize_quest_description(quest, lang),
                    &needs.join(", "),
                    lang,
                ),
            );
        }
    } else {
        send_dialog(state, &player.id, &npc_name, &npc_dialog);
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
) -> Option<(&'a mut ResourceNode, &'a ResourceDef, Option<String>)> {
    for resources in state.resources.values_mut() {
        for res in resources.iter_mut() {
            if res.hp <= 0 {
                continue;
            }
            let dist = distance(player.x, player.y, res.x as f32 + 0.5, res.y as f32 + 0.5);
            if dist <= GATHER_RANGE {
                if let Some(def) = data.resources.get(&res.kind) {
                    let owner = state
                        .community_tiles
                        .get(&TileCoord { x: res.x, y: res.y })
                        .cloned();
                    return Some((res, def, owner));
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

fn inventory_hash(inventory: &HashMap<String, i32>) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut entries: Vec<_> = inventory.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    let mut hasher = DefaultHasher::new();
    for (id, count) in entries {
        id.hash(&mut hasher);
        count.hash(&mut hasher);
    }
    hasher.finish()
}

fn build_inventory_items(
    inventory: &HashMap<String, i32>,
    data: &GameData,
    lang: Language,
) -> Vec<InventoryItem> {
    let mut items = Vec::new();
    for (id, count) in inventory {
        if *count <= 0 {
            continue;
        }
        let (name, heal) = match data.items.get(id) {
            Some(def) => (localize_item_name(data, &def.id, lang), def.heal),
            None => (id.clone(), None),
        };
        items.push(InventoryItem {
            id: id.clone(),
            name,
            count: *count,
            heal,
        });
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items
}

fn player_language(state: &GameState, player_id: &str) -> Language {
    state.locales.get(player_id).copied().unwrap_or(Language::En)
}

fn localize_item_name(data: &GameData, item_id: &str, lang: Language) -> String {
    if lang != Language::De {
        return data
            .items
            .get(item_id)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| item_id.to_string());
    }
    let localized = match item_id {
        "wood" => "Holz",
        "apple" => "Apfel",
        "stone" => "Stein",
        "boar_leg" => "Wildschweinkeule",
        "slime_core" => "Schleimkern",
        "arrow" => "Pfeil",
        "basic_axe" => "Einfache Axt",
        "fine_axe" => "Gute Axt",
        "basic_pick" => "Einfache Spitzhacke",
        "basic_shovel" => "Schaufel",
        "rusty_sword" => "Rostiges Schwert",
        "iron_sword" => "Eisenschwert",
        "bow" => "Bogen",
        _ => return data
            .items
            .get(item_id)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| item_id.to_string()),
    };
    localized.to_string()
}

fn localize_resource_name(data: &GameData, resource_id: &str, lang: Language) -> String {
    if lang != Language::De {
        return data
            .resources
            .get(resource_id)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| resource_id.to_string());
    }
    let localized = match resource_id {
        "tree" => "Baum",
        "apple_tree" => "Apfelbaum",
        "pine_tree" => "Kiefer",
        "palm_tree" => "Palme",
        "rock" => "Fels",
        _ => return data
            .resources
            .get(resource_id)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| resource_id.to_string()),
    };
    localized.to_string()
}

fn localize_monster_name(data: &GameData, monster_id: &str, lang: Language) -> String {
    if lang != Language::De {
        return data
            .monsters
            .get(monster_id)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| monster_id.to_string());
    }
    let localized = match monster_id {
        "boar" => "Wildschwein",
        _ => return data
            .monsters
            .get(monster_id)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| monster_id.to_string()),
    };
    localized.to_string()
}

fn localize_tool_name(tool: &str, lang: Language) -> String {
    if lang != Language::De {
        return tool.to_string();
    }
    let localized = match tool {
        "axe" => "Axt",
        "pick" => "Spitzhacke",
        "shovel" => "Schaufel",
        _ => tool,
    };
    localized.to_string()
}

fn localize_npc_name(npc: &NpcDef, lang: Language) -> String {
    if lang != Language::De {
        return npc.name.clone();
    }
    let localized = match npc.id.as_str() {
        "npc_logger" => "Edda die Holzfällerin",
        "npc_hunter" => "Bram der Jäger",
        _ => return npc.name.clone(),
    };
    localized.to_string()
}

fn localize_npc_dialog(npc: &NpcDef, lang: Language) -> String {
    if lang != Language::De {
        return npc.dialog.clone();
    }
    let localized = match npc.id.as_str() {
        "npc_logger" => "Pfadwege halten uns in Bewegung. Bring mir Holz und Stein, dann baue ich eine Schaufel.",
        "npc_hunter" => "Wildschweine streifen durchs Dickicht. Bring mir ihre Keulen.",
        _ => return npc.dialog.clone(),
    };
    localized.to_string()
}

fn localize_quest_name(quest: &QuestDef, lang: Language) -> String {
    if lang != Language::De {
        return quest.name.clone();
    }
    let localized = match quest.id.as_str() {
        "quest_lumber" => "Holz für das Lager",
        "quest_shovel" => "Pfadbereiter",
        "quest_hunter" => "Wildschweinkeulen",
        _ => return quest.name.clone(),
    };
    localized.to_string()
}

fn localize_quest_description(quest: &QuestDef, lang: Language) -> String {
    if lang != Language::De {
        return quest.description.clone();
    }
    let localized = match quest.id.as_str() {
        "quest_lumber" => "Bring 5 Holz zu Edda.",
        "quest_shovel" => "Bring 6 Holz und 4 Stein, damit Edda eine Schaufel bauen kann.",
        "quest_hunter" => "Bring 3 Wildschweinkeulen zu Bram.",
        _ => return quest.description.clone(),
    };
    localized.to_string()
}

fn message_build_success(lang: Language, kind: &str) -> String {
    match lang {
        Language::De => match kind {
            "hut_wood" => "Du baust eine Holzhütte.".to_string(),
            "house_stone" => "Du baust ein Steinhaus.".to_string(),
            "bridge_wood" => "Du baust eine Holzbrücke.".to_string(),
            "bridge_stone" => "Du baust eine Steinbrücke.".to_string(),
            "path" => "Du legst einen Pfad an.".to_string(),
            "road" => "Du baust eine Straße.".to_string(),
            "castle" => "Ihr baut ein Schloss.".to_string(),
            "silo" => "Ihr baut ein Lagerhaus.".to_string(),
            "church" => "Ihr baut ein Gemeinschaftsgebäude.".to_string(),
            _ => "Unbekannte Bauoption.".to_string(),
        },
        Language::En => match kind {
            "hut_wood" => "You build a wooden hut.".to_string(),
            "house_stone" => "You build a stone house.".to_string(),
            "bridge_wood" => "You build a wooden bridge.".to_string(),
            "bridge_stone" => "You build a stone bridge.".to_string(),
            "path" => "You lay down a path.".to_string(),
            "road" => "You build a road.".to_string(),
            "castle" => "You build a castle together.".to_string(),
            "silo" => "You build a storage silo together.".to_string(),
            "church" => "You build a community building together.".to_string(),
            _ => "Unknown build option.".to_string(),
        },
    }
}

fn message_bridge_span_error(lang: Language) -> &'static str {
    match lang {
        Language::De => {
            "Brücken müssen 1-4 Wasserfelder überspannen und an beiden Enden Land haben."
        }
        Language::En => "Bridges must span 1-4 water tiles with land on both ends.",
    }
}

fn message_need_shovel(lang: Language) -> &'static str {
    match lang {
        Language::De => "Du brauchst eine Schaufel.",
        Language::En => "You need a shovel.",
    }
}

fn message_not_enough_materials(lang: Language) -> &'static str {
    match lang {
        Language::De => "Du hast nicht genug Materialien.",
        Language::En => "You don't have enough materials.",
    }
}

fn message_spot_occupied(lang: Language) -> &'static str {
    match lang {
        Language::De => "Der Platz ist bereits belegt.",
        Language::En => "That spot is already occupied.",
    }
}

fn message_clear_resource(lang: Language) -> &'static str {
    match lang {
        Language::De => "Entferne zuerst die Ressource.",
        Language::En => "Clear the resource first.",
    }
}

fn message_build_on_land(lang: Language) -> &'static str {
    match lang {
        Language::De => "Das kannst du nur an Land bauen.",
        Language::En => "You can only build that on land.",
    }
}

fn message_unknown_build(lang: Language) -> &'static str {
    match lang {
        Language::De => "Unbekannte Bauoption.",
        Language::En => "Unknown build option.",
    }
}

fn message_nothing_to_remove(lang: Language) -> &'static str {
    match lang {
        Language::De => "Nichts zum Entfernen.",
        Language::En => "Nothing to remove.",
    }
}

fn message_remove_own_only(lang: Language) -> &'static str {
    match lang {
        Language::De => "Du kannst nur deine eigenen Gebäude entfernen.",
        Language::En => "You can only remove your own buildings.",
    }
}

fn message_structure_removed(lang: Language) -> &'static str {
    match lang {
        Language::De => "Gebäude entfernt.",
        Language::En => "Structure removed.",
    }
}

fn message_wake_up(lang: Language) -> &'static str {
    match lang {
        Language::De => "Du wachst am Lagerfeuer auf.",
        Language::En => "You wake up by the campfire.",
    }
}

fn message_collected(item_name: &str, count: i32, lang: Language) -> String {
    match lang {
        Language::De => format!("Gesammelt: {} x{}", item_name, count),
        Language::En => format!("Collected {} x{}", item_name, count),
    }
}

fn message_hit_resource(resource_name: &str, hp: i32, lang: Language) -> String {
    match lang {
        Language::De => format!("Getroffen: {} ({})", resource_name, hp),
        Language::En => format!("Hit {} ({})", resource_name, hp),
    }
}

fn message_need_tool(tool_name: &str, lang: Language) -> String {
    match lang {
        Language::De => format!("Du brauchst eine {}.", tool_name),
        Language::En => format!("You need a {}.", tool_name),
    }
}

fn message_hit_monster(monster_name: &str, hp: i32, lang: Language) -> String {
    match lang {
        Language::De => format!("Getroffen: {} ({})", monster_name, hp),
        Language::En => format!("Hit {} ({})", monster_name, hp),
    }
}

fn message_monster_hits_you(monster_name: &str, hp: i32, lang: Language) -> String {
    match lang {
        Language::De => format!("{} trifft dich ({})", monster_name, hp),
        Language::En => format!("{} hits you ({})", monster_name, hp),
    }
}

fn message_out_of(item_name: &str, lang: Language) -> String {
    match lang {
        Language::De => format!("Keine {} mehr.", item_name),
        Language::En => format!("Out of {}", item_name),
    }
}

fn message_picked_up(item_name: &str, count: i32, lang: Language) -> String {
    match lang {
        Language::De => format!("Aufgehoben: {} x{}", item_name, count),
        Language::En => format!("Picked up {} x{}", item_name, count),
    }
}

fn message_reward(item_name: &str, count: i32, lang: Language) -> String {
    match lang {
        Language::De => format!("Belohnung: {} x{}", item_name, count),
        Language::En => format!("Reward: {} x{}", item_name, count),
    }
}

fn message_eat_apple(lang: Language) -> String {
    match lang {
        Language::De => "Du isst einen Apfel. Ein Apfel am Tag hält den Doktor fern."
            .to_string(),
        Language::En => {
            "You eat an apple. An apple a day keeps the doctor away.".to_string()
        }
    }
}

fn message_eat_item(item_name: &str, improved: bool, lang: Language) -> String {
    match (lang, improved) {
        (Language::De, true) => format!("Du isst {} und fühlst dich besser.", item_name),
        (Language::De, false) => format!("Du isst {}.", item_name),
        (Language::En, true) => format!("You eat {} and feel better.", item_name),
        (Language::En, false) => format!("You eat {}.", item_name),
    }
}

fn message_thanks_again(lang: Language) -> String {
    match lang {
        Language::De => "Danke nochmal für deine Hilfe.".to_string(),
        Language::En => "Thanks again for your help.".to_string(),
    }
}

fn message_quest_complete(name: &str, description: &str, lang: Language) -> String {
    match lang {
        Language::De => format!("Quest abgeschlossen: {}. {}", name, description),
        Language::En => format!("Quest complete: {}. {}", name, description),
    }
}

fn message_quest_needs(name: &str, description: &str, needs: &str, lang: Language) -> String {
    match lang {
        Language::De => format!("Quest: {}\n{}\nBenötigt: {}", name, description, needs),
        Language::En => format!("Quest: {}\n{}\nNeeds: {}", name, description, needs),
    }
}

fn message_community_missing(lang: Language) -> &'static str {
    match lang {
        Language::De => "Diese Gemeinschaft existiert nicht mehr.",
        Language::En => "That community no longer exists.",
    }
}

fn message_community_members_only(lang: Language) -> &'static str {
    match lang {
        Language::De => "Nur Gemeinschaftsmitglieder dürfen hier handeln.",
        Language::En => "Only community members can do that here.",
    }
}

fn message_community_not_member(lang: Language) -> &'static str {
    match lang {
        Language::De => "Du bist in keiner Gemeinschaft.",
        Language::En => "You are not in a community.",
    }
}

fn message_community_already_member(lang: Language) -> &'static str {
    match lang {
        Language::De => "Du bist bereits in einer Gemeinschaft.",
        Language::En => "You are already in a community.",
    }
}

fn message_community_created(lang: Language) -> &'static str {
    match lang {
        Language::De => "Gemeinschaft gegründet.",
        Language::En => "Community founded.",
    }
}

fn message_community_left(lang: Language) -> &'static str {
    match lang {
        Language::De => "Du hast die Gemeinschaft verlassen.",
        Language::En => "You left the community.",
    }
}

fn message_community_joined(lang: Language) -> &'static str {
    match lang {
        Language::De => "Du bist der Gemeinschaft beigetreten.",
        Language::En => "You joined the community.",
    }
}

fn message_community_join_sent(lang: Language) -> &'static str {
    match lang {
        Language::De => "Beitrittsanfrage gesendet.",
        Language::En => "Join request sent.",
    }
}

fn message_community_invite_sent(lang: Language) -> &'static str {
    match lang {
        Language::De => "Einladung gesendet.",
        Language::En => "Invitation sent.",
    }
}

fn message_community_area_blocked(lang: Language) -> &'static str {
    match lang {
        Language::De => "Der Bereich ist bereits bebaut.",
        Language::En => "That area is already built over.",
    }
}

fn message_community_area_overlap(lang: Language) -> &'static str {
    match lang {
        Language::De => "Der Bereich überschneidet sich mit einer Gemeinschaft.",
        Language::En => "That area overlaps another community.",
    }
}

fn message_community_area_adjacent(lang: Language) -> &'static str {
    match lang {
        Language::De => "Der Bereich muss direkt an die Gemeinschaft grenzen.",
        Language::En => "The area must touch the community boundary.",
    }
}

fn message_community_request_declined(lang: Language) -> &'static str {
    match lang {
        Language::De => "Anfrage abgelehnt.",
        Language::En => "Request declined.",
    }
}

fn message_request_timed_out(lang: Language) -> &'static str {
    match lang {
        Language::De => "Anfrage abgelaufen.",
        Language::En => "Request timed out.",
    }
}

fn message_build_request_sent(lang: Language) -> &'static str {
    match lang {
        Language::De => "Bauanfrage gesendet.",
        Language::En => "Build request sent.",
    }
}

fn message_build_request_declined(lang: Language) -> &'static str {
    match lang {
        Language::De => "Bauanfrage abgelehnt.",
        Language::En => "Build request declined.",
    }
}

fn message_build_need_members(lang: Language) -> &'static str {
    match lang {
        Language::De => "Nicht genug Gemeinschaftsmitglieder vor Ort.",
        Language::En => "Not enough community members are present.",
    }
}

fn message_community_build_in_area(lang: Language) -> &'static str {
    match lang {
        Language::De => "Das muss im Gemeinschaftsbereich gebaut werden.",
        Language::En => "That must be built inside the community area.",
    }
}

fn approval_build_text(
    build_kind: &str,
    requester_name: &str,
    cost: &[ItemStack],
    lang: Language,
) -> (String, String, String, String) {
    let (kind_name, title) = match (lang, build_kind) {
        (Language::De, "castle") => ("Schloss", "Bauanfrage"),
        (Language::De, "silo") => ("Lagerhaus", "Bauanfrage"),
        (Language::De, "church") => ("Gemeinschaftsgebäude", "Bauanfrage"),
        (Language::En, "castle") => ("Castle", "Build request"),
        (Language::En, "silo") => ("Silo", "Build request"),
        (Language::En, "church") => ("Community building", "Build request"),
        _ => ("Build", "Build request"),
    };
    let costs = cost
        .iter()
        .map(|stack| format!("{} x{}", stack.id, stack.count))
        .collect::<Vec<_>>()
        .join(", ");
    match lang {
        Language::De => (
            title.to_string(),
            format!("{} möchte ein {} bauen. Kosten pro Mitglied: {}.", requester_name, kind_name, costs),
            "Zustimmen".to_string(),
            "Ablehnen".to_string(),
        ),
        Language::En => (
            title.to_string(),
            format!("{} wants to build a {}. Cost per member: {}.", requester_name, kind_name, costs),
            "Approve".to_string(),
            "Decline".to_string(),
        ),
    }
}

fn has_tool(inventory: &HashMap<String, i32>, data: &GameData, tool: &str) -> bool {
    best_tool_power(inventory, data, tool).is_some()
}

fn resource_at_tile(state: &GameState, x: i32, y: i32) -> bool {
    for resources in state.resources.values() {
        for resource in resources {
            if resource.hp > 0 && resource.x == x && resource.y == y {
                return true;
            }
        }
    }
    false
}

fn bridge_span_along_axis(
    noise: &WorldNoise,
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
) -> Option<Vec<TileCoord>> {
    if tile_at(noise, x, y) != TILE_WATER {
        return None;
    }
    let mut left_tiles = Vec::new();
    let mut cx = x;
    let mut cy = y;
    let mut left_land = None;
    for _ in 0..4 {
        let nx = cx - dx;
        let ny = cy - dy;
        if tile_at(noise, nx, ny) == TILE_WATER {
            left_tiles.push(TileCoord { x: nx, y: ny });
            cx = nx;
            cy = ny;
        } else {
            left_land = Some(TileCoord { x: nx, y: ny });
            break;
        }
    }
    if left_land.is_none() {
        return None;
    }

    let mut right_tiles = Vec::new();
    cx = x;
    cy = y;
    let mut right_land = None;
    for _ in 0..4 {
        let nx = cx + dx;
        let ny = cy + dy;
        if tile_at(noise, nx, ny) == TILE_WATER {
            right_tiles.push(TileCoord { x: nx, y: ny });
            cx = nx;
            cy = ny;
        } else {
            right_land = Some(TileCoord { x: nx, y: ny });
            break;
        }
    }
    if right_land.is_none() {
        return None;
    }

    let total_len = 1 + left_tiles.len() + right_tiles.len();
    if total_len == 0 || total_len > 4 {
        return None;
    }

    left_tiles.reverse();
    let mut tiles = left_tiles;
    tiles.push(TileCoord { x, y });
    tiles.extend(right_tiles);
    Some(tiles)
}

fn find_bridge_span(noise: &WorldNoise, x: i32, y: i32) -> Option<Vec<TileCoord>> {
    let horizontal = bridge_span_along_axis(noise, x, y, 1, 0);
    let vertical = bridge_span_along_axis(noise, x, y, 0, 1);
    match (horizontal, vertical) {
        (Some(h), Some(v)) => {
            if h.len() >= v.len() {
                Some(h)
            } else {
                Some(v)
            }
        }
        (Some(h), None) => Some(h),
        (None, Some(v)) => Some(v),
        (None, None) => None,
    }
}

fn move_towards(
    monster: &mut Monster,
    tx: f32,
    ty: f32,
    speed: f32,
    dt: f32,
    structure_tiles: &HashMap<TileCoord, StructureTile>,
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
        if can_walk(structure_tiles, noise, next_x, monster.y) {
            monster.x = next_x;
        }
        if can_walk(structure_tiles, noise, monster.x, next_y) {
            monster.y = next_y;
        }
    }
}

fn wander(
    monster: &mut Monster,
    now_ms: i64,
    speed: f32,
    dt: f32,
    structure_tiles: &HashMap<TileCoord, StructureTile>,
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
    if can_walk(structure_tiles, noise, next_x, monster.y) {
        monster.x = next_x;
    }
    if can_walk(structure_tiles, noise, monster.x, next_y) {
        monster.y = next_y;
    }
}

fn distance(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    (dx * dx + dy * dy).sqrt()
}

fn entity_foot_tile(x: f32, y: f32) -> (i32, i32) {
    (x.floor() as i32, y.floor() as i32)
}

fn tile_anchor_position(x: i32, y: i32) -> (f32, f32) {
    (x as f32 + ENTITY_FOOT_OFFSET_X, y as f32 + ENTITY_FOOT_OFFSET_Y)
}

fn community_area_origin(center_x: i32, center_y: i32) -> CommunityArea {
    let half = COMMUNITY_AREA_SIZE / 2;
    CommunityArea {
        x: center_x - half,
        y: center_y - half,
    }
}

fn community_area_tiles(area: CommunityArea) -> Vec<TileCoord> {
    let mut tiles = Vec::with_capacity((COMMUNITY_AREA_SIZE * COMMUNITY_AREA_SIZE) as usize);
    for dy in 0..COMMUNITY_AREA_SIZE {
        for dx in 0..COMMUNITY_AREA_SIZE {
            tiles.push(TileCoord {
                x: area.x + dx,
                y: area.y + dy,
            });
        }
    }
    tiles
}

fn insert_community_area_tiles(state: &mut GameState, community_id: &str, area: CommunityArea) {
    for tile in community_area_tiles(area) {
        state
            .community_tiles
            .insert(TileCoord { x: tile.x, y: tile.y }, community_id.to_string());
    }
}

fn community_tile_owner(state: &GameState, x: i32, y: i32) -> Option<&String> {
    state.community_tiles.get(&TileCoord { x, y })
}

fn community_area_is_clear(state: &GameState, area: CommunityArea) -> bool {
    for tile in community_area_tiles(area) {
        if state
            .structure_tiles
            .contains_key(&TileCoord { x: tile.x, y: tile.y })
        {
            return false;
        }
    }
    true
}

fn community_area_overlaps(state: &GameState, area: CommunityArea) -> bool {
    for tile in community_area_tiles(area) {
        if state
            .community_tiles
            .contains_key(&TileCoord { x: tile.x, y: tile.y })
        {
            return true;
        }
    }
    false
}

fn community_area_adjacent(community: &Community, area: CommunityArea) -> bool {
    for existing in &community.areas {
        let same_row = existing.y == area.y;
        let same_col = existing.x == area.x;
        if same_row && (existing.x + COMMUNITY_AREA_SIZE == area.x || area.x + COMMUNITY_AREA_SIZE == existing.x) {
            return true;
        }
        if same_col && (existing.y + COMMUNITY_AREA_SIZE == area.y || area.y + COMMUNITY_AREA_SIZE == existing.y) {
            return true;
        }
    }
    false
}

fn player_tile(player: &Player) -> TileCoord {
    let (x, y) = entity_foot_tile(player.x, player.y);
    TileCoord { x, y }
}

fn player_in_community_area(state: &GameState, player: &Player, community_id: &str) -> bool {
    let tile = player_tile(player);
    community_tile_owner(state, tile.x, tile.y).map(|id| id == community_id).unwrap_or(false)
}

fn position_in_community_area(state: &GameState, x: f32, y: f32, community_id: &str) -> bool {
    let (tile_x, tile_y) = entity_foot_tile(x, y);
    community_tile_owner(state, tile_x, tile_y)
        .map(|id| id == community_id)
        .unwrap_or(false)
}

fn can_walk(
    structure_tiles: &HashMap<TileCoord, StructureTile>,
    noise: &WorldNoise,
    x: f32,
    y: f32,
) -> bool {
    let (tile_x, tile_y) = entity_foot_tile(x, y);
    if let Some(structure) = structure_tiles.get(&TileCoord { x: tile_x, y: tile_y }) {
        if structure.kind == "community_float" {
            return true;
        }
        if structure.kind == "community_well" {
            return false;
        }
        if matches!(structure.kind.as_str(), "community_fence" | "community_gate") {
            return false;
        }
        if matches!(
            structure.kind.as_str(),
            "hut_wood"
                | "hut_wood_root"
                | "hut_wood_block"
                | "house_stone"
                | "house_stone_root"
                | "house_stone_block"
                | "castle_root"
                | "castle_fill"
                | "silo_root"
                | "silo_fill"
                | "church_root"
                | "church_fill"
        ) {
            return false;
        }
        if structure.kind.starts_with("bridge_") {
            return true;
        }
    }
    tile_at(noise, tile_x, tile_y) != TILE_WATER
}

fn can_walk_player(state: &GameState, noise: &WorldNoise, player: &Player, x: f32, y: f32) -> bool {
    let (tile_x, tile_y) = entity_foot_tile(x, y);
    if let Some(structure) = state.structure_tiles.get(&TileCoord { x: tile_x, y: tile_y }) {
        if structure.kind == "community_gate" {
            return player.community_id.as_ref() == Some(&structure.owner_id);
        }
    }
    if let Some(community_id) = community_tile_owner(state, tile_x, tile_y) {
        if player.community_id.as_ref() != Some(community_id) {
            return false;
        }
    }
    can_walk(&state.structure_tiles, noise, x, y)
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

fn is_tree_kind(kind: &str) -> bool {
    matches!(kind, "tree" | "apple_tree" | "pine_tree" | "palm_tree")
}

fn tree_spawn_state(seed: u64, x: i32, y: i32, now_ms: i64) -> (i32, Option<i64>) {
    let roll = noise_hash01(seed.wrapping_add(5555), x, y);
    let size = if roll > 0.7 {
        TREE_MAX_SIZE
    } else if roll > 0.4 {
        2
    } else {
        1
    };
    let next_growth_ms = if size < TREE_MAX_SIZE {
        let jitter = noise_hash01(seed.wrapping_add(9999), x, y);
        let delay = (TREE_GROW_INTERVAL_MS as f32 * (0.4 + jitter * 1.2)) as i64;
        Some(now_ms + delay)
    } else {
        None
    };
    (size, next_growth_ms)
}

fn generate_resources(
    seed: u64,
    now_ms: i64,
    coord: ChunkCoord,
    world: &WorldConfig,
    noise: &WorldNoise,
    data: &GameData,
    structure_tiles: &HashMap<TileCoord, StructureTile>,
) -> Vec<ResourceNode> {
    let chunk_size = world.chunk_size;
    let campfire_x = world.spawn_x.round() as i32;
    let campfire_y = world.spawn_y.round() as i32;
    let mut resources = Vec::new();
    for y in 0..chunk_size {
        for x in 0..chunk_size {
            let wx = coord.x * chunk_size + x;
            let wy = coord.y * chunk_size + y;
            let tile = tile_at(noise, wx, wy);
            if tile == TILE_WATER {
                continue;
            }
            if (wx == campfire_x && wy == campfire_y) || (wx == campfire_x + 1 && wy == campfire_y) {
                continue;
            }
            if structure_tiles.contains_key(&TileCoord { x: wx, y: wy }) {
                continue;
            }
            let elevation = noise.elevation(wx as f32, wy as f32);
            let moisture = noise.moisture(wx as f32, wy as f32);
            let tree_density = noise.tree_density(wx as f32, wy as f32);
            let rock_density = noise.rock_density(wx as f32, wy as f32);

            let is_grass = tile == TILE_GRASS || tile == TILE_FLOWER;
            let is_dirt = tile == TILE_DIRT;
            let is_sand = tile == TILE_SAND;

            let mut kind = None;
            if is_grass {
                let tree_score = tree_density + moisture * 0.25;
                let tree_roll = noise_hash01(seed, wx, wy);
                if tree_score > 0.25 && tree_roll < (tree_score * 0.45 + 0.1) {
                    let apple_roll = noise_hash01(seed.wrapping_add(1337), wx, wy);
                    if apple_roll > 0.82 {
                        kind = Some("apple_tree");
                    } else {
                        kind = Some("tree");
                    }
                }
            }

            if kind.is_none() && is_dirt {
                let tree_score = tree_density + moisture * 0.2;
                let tree_roll = noise_hash01(seed.wrapping_add(77), wx, wy);
                if tree_score > 0.2 && tree_roll < (tree_score * 0.65 + 0.12) {
                    kind = Some("pine_tree");
                }
            }

            if kind.is_none() && is_sand {
                let palm_score = tree_density + moisture * 0.2;
                let palm_roll = noise_hash01(seed.wrapping_add(4242), wx, wy);
                if palm_score > 0.15 && palm_roll < (palm_score * 0.35 + 0.05) {
                    kind = Some("palm_tree");
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
                    let mut size = 1;
                    let mut next_growth_ms = None;
                    if is_tree_kind(kind) {
                        let (s, next) = tree_spawn_state(seed, wx, wy, now_ms);
                        size = s;
                        next_growth_ms = next;
                    } else if kind == "rock" {
                        let size_roll = noise_hash01(seed.wrapping_add(2024), wx, wy);
                        if size_roll > 0.8 {
                            size = 3.min(ROCK_MAX_SIZE);
                        } else if size_roll > 0.5 {
                            size = 2.min(ROCK_MAX_SIZE);
                        }
                    }
                    resources.push(ResourceNode {
                        id,
                        kind: kind.to_string(),
                        x: wx,
                        y: wy,
                        hp: def.hp,
                        respawn_at_ms: None,
                        size,
                        next_growth_ms,
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
        let (spawn_x, spawn_y) = tile_anchor_position(wx, wy);
        state.monsters.insert(
            monster_id,
            Monster {
                id: monster_id,
                kind: "boar".to_string(),
                x: spawn_x,
                y: spawn_y,
                hp: data
                    .monsters
                    .get("boar")
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
        let flower_score = noise.flower_density(x as f32, y as f32) + moisture * 0.2;
        if flower_score > 0.35 {
            TILE_FLOWER
        } else {
            TILE_GRASS
        }
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
    let npcs = npcs
        .into_iter()
        .map(|mut npc| {
            npc.x += ENTITY_FOOT_OFFSET_X;
            npc.y += ENTITY_FOOT_OFFSET_Y;
            npc
        })
        .collect();

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
    inventory.insert("basic_shovel".to_string(), 1);
    inventory.insert("rusty_sword".to_string(), 1);
    let (spawn_x, spawn_y) = spawn_near_campfire(world, noise);
    PlayerDoc {
        id: id.to_string(),
        name: random_name(),
        x: spawn_x,
        y: spawn_y,
        hp: MAX_HP,
        inventory,
        completed_quests: Vec::new(),
        community_id: None,
        coord_version: PLAYER_COORD_VERSION,
    }
}

fn random_name() -> String {
    let mut rng = rand::thread_rng();
    format!("Adventurer{}", rng.gen_range(1000..9999))
}

fn normalize_player_name(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let cleaned: String = trimmed.chars().filter(|ch| !ch.is_control()).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return None;
    }
    Some(cleaned.chars().take(MAX_NAME_CHARS).collect())
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
        return tile_anchor_position(base_x, base_y);
    }

    for radius in 1..=8 {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let x = base_x + dx;
                let y = base_y + dy;
                if tile_at(noise, x, y) != TILE_WATER {
                    return tile_anchor_position(x, y);
                }
            }
        }
    }

    (world.spawn_x, world.spawn_y)
}

fn spawn_near_campfire(world: &WorldConfig, noise: &WorldNoise) -> (f32, f32) {
    let base_x = world.spawn_x.round() as i32;
    let base_y = world.spawn_y.round() as i32;
    let mut offsets = Vec::new();
    for dx in -2..=2 {
        for dy in -2..=2 {
            if dx == 0 && dy == 0 {
                continue;
            }
            offsets.push((dx, dy));
        }
    }
    let mut rng = rand::thread_rng();
    offsets.shuffle(&mut rng);
    for (dx, dy) in offsets {
        let x = base_x + dx;
        let y = base_y + dy;
        if tile_at(noise, x, y) != TILE_WATER {
            return tile_anchor_position(x, y);
        }
    }
    safe_spawn(world, noise)
}

async fn send_to_player(state: &Arc<RwLock<GameState>>, player_id: &str, msg: ServerMessage) {
    let state = state.read().await;
    if let Some(sender) = state.clients.get(player_id) {
        let _ = sender.send(msg);
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
    structures: Collection<StructureDoc>,
    communities: Collection<CommunityDoc>,
    storages: Collection<StorageDoc>,
}

impl GameStore {
    async fn new(uri: &str) -> AppResult<Self> {
        let client = Client::with_uri_str(uri).await?;
        let db = client.database("onlinerpg");
        Ok(Self {
            players: db.collection::<PlayerDoc>("players"),
            structures: db.collection::<StructureDoc>("structures"),
            communities: db.collection::<CommunityDoc>("communities"),
            storages: db.collection::<StorageDoc>("storages"),
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

    async fn load_structures(&self) -> AppResult<Vec<StructureDoc>> {
        let mut cursor = self.structures.find(doc! {}, None).await?;
        let mut docs = Vec::new();
        while let Some(result) = cursor.next().await {
            docs.push(result?);
        }
        Ok(docs)
    }

    async fn load_communities(&self) -> AppResult<Vec<CommunityDoc>> {
        let mut cursor = self.communities.find(doc! {}, None).await?;
        let mut docs = Vec::new();
        while let Some(result) = cursor.next().await {
            docs.push(result?);
        }
        Ok(docs)
    }

    async fn save_community(&self, doc: &CommunityDoc) -> AppResult<()> {
        let opts = ReplaceOptions::builder().upsert(true).build();
        self.communities
            .replace_one(doc! { "_id": &doc.id }, doc, opts)
            .await?;
        Ok(())
    }

    async fn delete_community(&self, id: &str) -> AppResult<()> {
        self.communities.delete_one(doc! { "_id": id }, None).await?;
        Ok(())
    }

    async fn insert_structures(&self, structures: &[StructureDoc]) -> AppResult<()> {
        if structures.is_empty() {
            return Ok(());
        }
        self.structures.insert_many(structures, None).await?;
        Ok(())
    }

    async fn load_storages(&self) -> AppResult<Vec<StorageDoc>> {
        let mut cursor = self.storages.find(doc! {}, None).await?;
        let mut docs = Vec::new();
        while let Some(result) = cursor.next().await {
            docs.push(result?);
        }
        Ok(docs)
    }

    async fn save_storage(&self, doc: &StorageDoc) -> AppResult<()> {
        let opts = ReplaceOptions::builder().upsert(true).build();
        self.storages
            .replace_one(doc! { "id": doc.id }, doc, opts)
            .await?;
        Ok(())
    }

    async fn delete_structure_group(&self, id: i64) -> AppResult<()> {
        self.structures.delete_many(doc! { "id": id }, None).await?;
        Ok(())
    }

    async fn delete_structures_by_owner_and_kinds(
        &self,
        owner_id: &str,
        kinds: &[&str],
    ) -> AppResult<()> {
        self.structures
            .delete_many(
                doc! {
                    "owner_id": owner_id,
                    "kind": { "$in": kinds }
                },
                None,
            )
            .await?;
        Ok(())
    }

    async fn delete_storages_by_community(&self, community_id: &str) -> AppResult<()> {
        self.storages
            .delete_many(doc! { "community_id": community_id }, None)
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
    #[serde(default)]
    community_id: Option<String>,
    #[serde(default)]
    coord_version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommunityDoc {
    #[serde(rename = "_id")]
    id: String,
    members: Vec<String>,
    areas: Vec<CommunityArea>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StorageDoc {
    id: i64,
    community_id: String,
    inventory: HashMap<String, i32>,
}

#[derive(Clone)]
struct GameState {
    players: HashMap<String, Player>,
    inputs: HashMap<String, InputState>,
    monsters: HashMap<u64, Monster>,
    projectiles: HashMap<u64, Projectile>,
    resources: HashMap<ChunkCoord, Vec<ResourceNode>>,
    structure_tiles: HashMap<TileCoord, StructureTile>,
    communities: HashMap<String, Community>,
    community_tiles: HashMap<TileCoord, String>,
    storages: HashMap<u64, Storage>,
    pending_approvals: HashMap<String, PendingApproval>,
    spawned_chunks: HashSet<ChunkCoord>,
    chunk_last_access: HashMap<ChunkCoord, i64>,
    clients: HashMap<String, mpsc::UnboundedSender<ServerMessage>>,
    typing: HashMap<String, i64>,
    visibility: HashMap<String, VisibilityState>,
    locales: HashMap<String, Language>,
    next_entity_id: u64,
    next_structure_id: u64,
}

impl GameState {
    fn new() -> Self {
        Self {
            players: HashMap::new(),
            inputs: HashMap::new(),
            monsters: HashMap::new(),
            projectiles: HashMap::new(),
            resources: HashMap::new(),
            structure_tiles: HashMap::new(),
            communities: HashMap::new(),
            community_tiles: HashMap::new(),
            storages: HashMap::new(),
            pending_approvals: HashMap::new(),
            spawned_chunks: HashSet::new(),
            chunk_last_access: HashMap::new(),
            clients: HashMap::new(),
            typing: HashMap::new(),
            visibility: HashMap::new(),
            locales: HashMap::new(),
            next_entity_id: 1,
            next_structure_id: 1,
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }

    fn next_structure_id(&mut self) -> u64 {
        let id = self.next_structure_id;
        self.next_structure_id += 1;
        id
    }
}

#[derive(Default, Clone)]
struct VisibilityState {
    players: HashSet<String>,
    monsters: HashSet<u64>,
    projectiles: HashSet<u64>,
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
    community_id: Option<String>,
    last_attack_ms: i64,
    last_gather_ms: i64,
    last_interact_ms: i64,
    last_regen_ms: i64,
    last_saved_ms: i64,
    last_inventory_hash: u64,
    last_input_seq: u32,
}

impl Player {
    fn from_doc(doc: PlayerDoc) -> Self {
        let inventory_hash = inventory_hash(&doc.inventory);
        let (x, y) = player_position_from_doc(&doc);
        Self {
            id: doc.id,
            name: doc.name,
            x,
            y,
            hp: doc.hp,
            face_x: 1.0,
            face_y: 0.0,
            inventory: doc.inventory,
            completed_quests: doc.completed_quests.into_iter().collect(),
            community_id: doc.community_id,
            last_attack_ms: 0,
            last_gather_ms: 0,
            last_interact_ms: 0,
            last_regen_ms: now_millis(),
            last_saved_ms: now_millis(),
            last_inventory_hash: inventory_hash,
            last_input_seq: 0,
        }
    }

    fn sync_from_doc(&mut self, doc: &PlayerDoc) {
        let (x, y) = player_position_from_doc(doc);
        self.name = doc.name.clone();
        self.x = x;
        self.y = y;
        self.hp = doc.hp;
        self.inventory = doc.inventory.clone();
        self.completed_quests = doc.completed_quests.iter().cloned().collect();
        self.community_id = doc.community_id.clone();
        self.last_inventory_hash = inventory_hash(&self.inventory);
        self.last_input_seq = 0;
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
            community_id: self.community_id.clone(),
            coord_version: PLAYER_COORD_VERSION,
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
            community_id: self.community_id.clone(),
        }
    }
}

fn player_position_from_doc(doc: &PlayerDoc) -> (f32, f32) {
    if doc.coord_version >= PLAYER_COORD_VERSION {
        (doc.x, doc.y)
    } else {
        (doc.x + ENTITY_FOOT_OFFSET_X, doc.y + ENTITY_FOOT_OFFSET_Y)
    }
}

#[derive(Debug, Clone, Copy)]
struct InputState {
    dir_x: f32,
    dir_y: f32,
    attack: bool,
    gather: bool,
    interact: bool,
    seq: u32,
    expected_x: Option<f32>,
    expected_y: Option<f32>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            dir_x: 0.0,
            dir_y: 0.0,
            attack: false,
            gather: false,
            interact: false,
            seq: 0,
            expected_x: None,
            expected_y: None,
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
    size: i32,
    next_growth_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
struct ChunkCoord {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct TileCoord {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone)]
struct StructureTile {
    id: u64,
    kind: String,
    x: i32,
    y: i32,
    owner_id: String,
}

#[derive(Clone, Serialize)]
struct StructurePublic {
    id: u64,
    kind: String,
    x: i32,
    y: i32,
    owner_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StructureDoc {
    id: i64,
    kind: String,
    x: i32,
    y: i32,
    owner_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct CommunityArea {
    x: i32,
    y: i32,
}

#[derive(Clone)]
struct Community {
    id: String,
    members: HashSet<String>,
    areas: Vec<CommunityArea>,
}

#[derive(Debug, Clone)]
struct Storage {
    id: u64,
    community_id: String,
    inventory: HashMap<String, i32>,
}

#[derive(Debug, Clone)]
enum ApprovalKind {
    Join {
        community_id: String,
        target_id: String,
        area: CommunityArea,
        requester_id: String,
    },
    Build {
        community_id: String,
        build_kind: String,
        x: i32,
        y: i32,
        requester_id: String,
        participants: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct PendingApproval {
    kind: ApprovalKind,
    approvals: HashSet<String>,
    required: HashSet<String>,
    created_ms: i64,
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
    heal: Option<i32>,
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
    flowers: Perlin,
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
            flowers: Perlin::new(base.wrapping_add(83)),
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

    fn flower_density(&self, x: f32, y: f32) -> f32 {
        self.fbm(&self.flowers, x, y, 0.08, 3)
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

#[derive(Clone, Serialize)]
struct InventoryItem {
    id: String,
    name: String,
    count: i32,
    heal: Option<i32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    community_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_input_seq: Option<u32>,
}

impl From<&Player> for PlayerPublic {
    fn from(player: &Player) -> Self {
        Self {
            id: player.id.clone(),
            name: player.name.clone(),
            x: player.x,
            y: player.y,
            hp: player.hp,
            community_id: player.community_id.clone(),
            last_input_seq: None,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    community_id: Option<String>,
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
    size: i32,
}

impl From<ResourceNode> for ResourceNodePublic {
    fn from(node: ResourceNode) -> Self {
        Self {
            id: node.id.to_string(),
            kind: node.kind,
            x: node.x,
            y: node.y,
            hp: node.hp,
            size: node.size,
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
            size: node.size,
        }
    }
}

impl From<&StructureTile> for StructurePublic {
    fn from(structure: &StructureTile) -> Self {
        Self {
            id: structure.id,
            kind: structure.kind.clone(),
            x: structure.x,
            y: structure.y,
            owner_id: structure.owner_id.clone(),
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
        inventory_items: Vec<InventoryItem>,
    },
    ChunkData {
        chunk_x: i32,
        chunk_y: i32,
        tiles: Vec<u8>,
        resources: Vec<ResourceNodePublic>,
        structures: Vec<StructurePublic>,
    },
    EntitiesUpdate {
        players: Vec<PlayerPublic>,
        monsters: Vec<MonsterPublic>,
        projectiles: Vec<ProjectilePublic>,
    },
    EntitiesRemove {
        players: Vec<String>,
        monsters: Vec<u64>,
        projectiles: Vec<u64>,
    },
    ResourceUpdate {
        resource: ResourceNodePublic,
        state: String,
    },
    StructureUpdate {
        structures: Vec<StructurePublic>,
        state: String,
    },
    Inventory {
        items: Vec<InventoryItem>,
    },
    Chat {
        from: String,
        text: String,
        channel: String,
        community_member: bool,
    },
    ApprovalRequest {
        request_id: String,
        title: String,
        text: String,
        accept: String,
        decline: String,
    },
    Storage {
        storage_id: u64,
        items: Vec<InventoryItem>,
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
        seq: u32,
        expected_x: Option<f32>,
        expected_y: Option<f32>,
    },
    Chat {
        text: String,
    },
    CommunityApproval {
        request_id: String,
        approve: bool,
    },
    CommunityLeave,
    SetName {
        name: String,
    },
    UseItem {
        id: String,
    },
    StorageDeposit {
        storage_id: u64,
        item_id: String,
        count: i32,
    },
    StorageWithdraw {
        storage_id: u64,
        item_id: String,
        count: i32,
    },
    Build {
        kind: String,
        x: i32,
        y: i32,
    },
    Demolish {
        x: i32,
        y: i32,
    },
    Typing {
        typing: bool,
    },
    Locale {
        language: String,
    },
    ChunkRequest {
        chunks: Vec<ChunkCoord>,
    },
    Ping,
}
