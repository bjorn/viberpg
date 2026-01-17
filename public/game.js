(async () => {
  const statusEl = document.getElementById('status');
  const chatLog = document.getElementById('chat-log');
  const chatInput = document.getElementById('chat-input');
  const dialogEl = document.getElementById('dialog');
  const dialogTitle = document.getElementById('dialog-title');
  const dialogText = document.getElementById('dialog-text');
  const helpEl = document.getElementById('help');
  const fullscreenButton = document.getElementById('fullscreen-toggle');
  const joystickEl = document.getElementById('touch-joystick');
  const joystickHandle = joystickEl ? joystickEl.querySelector('.stick-handle') : null;
  const actionButtons = Array.from(document.querySelectorAll('.action-btn'));

  let dialogTimer = null;
  let ws = null;
  let wsOpen = false;

  let tileSize = 32;
  let chunkSize = 32;
  let playerId = null;
  let playerState = null;
  let worldSeed = 0;
  const PLAYER_ANCHOR = { x: 0.5, y: 0.9 };
  const MONSTER_ANCHOR = { x: 0.5, y: 0.9 };
  const NPC_ANCHOR = { x: 0.5, y: 0.9 };
  const RESOURCE_ANCHOR = { x: 0.5, y: 1.0 };

  const app = new PIXI.Application();
  await app.init({
    resizeTo: window,
    backgroundColor: 0x0b0e14,
    antialias: false,
  });
  PIXI.TextureStyle.defaultOptions.scaleMode = PIXI.SCALE_MODES.NEAREST;
  app.renderer.roundPixels = true;
  document.body.appendChild(app.canvas);

  const world = new PIXI.Container();
  const tileLayer = new PIXI.Container();
  const entityLayer = new PIXI.Container();
  const overlayLayer = new PIXI.Container();
  const projectileLayer = new PIXI.Container();
  entityLayer.sortableChildren = true;
  world.addChild(tileLayer, entityLayer, projectileLayer, overlayLayer);
  app.stage.addChild(world);

  const tileAssetUrls = [
    'assets/tiles/grass.svg',
    'assets/tiles/water.svg',
    'assets/tiles/sand.svg',
    'assets/tiles/dirt.svg',
  ];
  const entityAssetUrls = [
    'assets/entities/tree.svg',
    'assets/entities/rock.svg',
    'assets/entities/player.svg',
    'assets/entities/player-back.svg',
    'assets/entities/player-side.svg',
    'assets/entities/player-alt.svg',
    'assets/entities/player-alt-back.svg',
    'assets/entities/player-alt-side.svg',
    'assets/entities/npc.svg',
    'assets/entities/slime.svg',
    'assets/entities/arrow.svg',
  ];
  await PIXI.Assets.load([...tileAssetUrls, ...entityAssetUrls]);
  let textures = null;
  let tileAtlasSize = null;

  const chunkTiles = new Map();
  const resourceSprites = new Map();
  const playerEntities = new Map();
  const monsterEntities = new Map();
  const projectileSprites = new Map();
  const npcSprites = new Map();
  const typingIndicators = new Map();

  const loadedChunks = new Set();
  const pendingChunks = new Set();

  const keys = new Set();
  const touchState = {
    active: false,
    dirX: 0,
    dirY: 0,
    attack: false,
    gather: false,
    interact: false,
    attackPulse: false,
    gatherPulse: false,
    interactPulse: false,
  };
  const pointerMoveState = {
    active: false,
    dirX: 0,
    dirY: 0,
    pointerId: null,
  };
  const INTERP_MS = 120;
  const MAX_CHAT_LINES = 60;
  const TYPING_IDLE_MS = 1800;
  let localTyping = false;
  let typingTimer = null;
  let lastTypingSent = 0;
  let lastStatusUpdate = 0;
  let joystickPointerId = null;
  let joystickCenter = { x: 0, y: 0 };
  let joystickMaxRadius = 0;

  function worldToPixels(x, y, anchor) {
    return {
      x: (x + anchor.x) * tileSize,
      y: (y + anchor.y) * tileSize,
    };
  }

  function addChat(text, className) {
    const line = document.createElement('div');
    line.className = `line ${className || ''}`.trim();
    line.textContent = text;
    const shouldStick = chatLog.scrollTop + chatLog.clientHeight >= chatLog.scrollHeight - 8;
    chatLog.appendChild(line);
    while (chatLog.children.length > MAX_CHAT_LINES) {
      chatLog.removeChild(chatLog.firstChild);
    }
    if (shouldStick) {
      chatLog.scrollTop = chatLog.scrollHeight;
    }
  }

  function setLocalTyping(nextState) {
    if (localTyping === nextState) return;
    localTyping = nextState;
    if (playerId) {
      setTypingIndicator(playerId, localTyping);
    }
    sendMessage({ type: 'typing', typing: localTyping });
    if (localTyping) {
      lastTypingSent = performance.now();
    } else {
      lastTypingSent = 0;
    }
  }

  function sendTypingPing() {
    const now = performance.now();
    if (now - lastTypingSent > 400) {
      sendMessage({ type: 'typing', typing: true });
      lastTypingSent = now;
    }
  }

  function scheduleTypingStop() {
    if (typingTimer) {
      clearTimeout(typingTimer);
    }
    typingTimer = setTimeout(() => {
      setLocalTyping(false);
    }, TYPING_IDLE_MS);
  }

  function removeTypingIndicator(id) {
    const indicator = typingIndicators.get(id);
    if (!indicator) return;
    if (indicator.container.parent) {
      indicator.container.parent.removeChild(indicator.container);
    }
    indicator.container.destroy({ children: true });
    typingIndicators.delete(id);
  }

  function setTypingIndicator(id, typing) {
    if (!typing) {
      removeTypingIndicator(id);
      return;
    }
    let indicator = typingIndicators.get(id);
    if (!indicator) {
      const container = new PIXI.Container();
      const bubble = new PIXI.Graphics();
      bubble.beginFill(0x0b0e14, 0.85);
      bubble.lineStyle(1, 0x7ad5a3, 0.8);
      bubble.drawRoundedRect(-12, -12, 24, 16, 4);
      bubble.endFill();
      const text = new PIXI.Text('...', {
        fontFamily: 'VT323',
        fontSize: 14,
        fill: 0xe8f4ea,
      });
      text.anchor.set(0.5, 0.5);
      text.y = -4;
      container.addChild(bubble, text);
      overlayLayer.addChild(container);
      indicator = { container, text };
      typingIndicators.set(id, indicator);
    }
  }

  function updateTypingIndicators(now) {
    for (const [id, indicator] of typingIndicators.entries()) {
      const entity = playerEntities.get(id);
      if (!entity) {
        continue;
      }
      indicator.container.x = entity.sprite.x;
      indicator.container.y = entity.sprite.y - tileSize * 1.35;
      const phase = Math.floor((now / 260) % 3) + 1;
      indicator.text.text = '.'.repeat(phase);
    }
  }

  function showDialog(title, text) {
    dialogTitle.textContent = title;
    dialogText.textContent = text;
    dialogEl.classList.remove('hidden');
    if (dialogTimer) {
      clearTimeout(dialogTimer);
    }
    dialogTimer = setTimeout(() => {
      dialogEl.classList.add('hidden');
    }, 5000);
  }

  function getFullscreenElement() {
    return document.fullscreenElement || document.webkitFullscreenElement;
  }

  function updateFullscreenButton() {
    if (!fullscreenButton) return;
    const active = Boolean(getFullscreenElement());
    fullscreenButton.classList.toggle('active', active);
    fullscreenButton.setAttribute('aria-pressed', String(active));
    const label = active ? 'Exit fullscreen' : 'Enter fullscreen';
    fullscreenButton.setAttribute('aria-label', label);
    fullscreenButton.setAttribute('title', label);
  }

  async function requestFullscreen() {
    const target = document.documentElement;
    if (target.requestFullscreen) {
      await target.requestFullscreen();
      return;
    }
    if (target.webkitRequestFullscreen) {
      await target.webkitRequestFullscreen();
    }
  }

  async function exitFullscreen() {
    if (document.exitFullscreen) {
      await document.exitFullscreen();
      return;
    }
    if (document.webkitExitFullscreen) {
      await document.webkitExitFullscreen();
    }
  }

  const actionButtonsByAction = new Map();
  actionButtons.forEach((button) => {
    const action = button.dataset.action;
    if (action) {
      actionButtonsByAction.set(action, button);
    }
  });

  function setActionState(action, isActive) {
    if (!(action in touchState)) return;
    touchState[action] = isActive;
    const button = actionButtonsByAction.get(action);
    if (button) {
      button.classList.toggle('active', isActive);
    }
  }

  function pulseAction(action) {
    const pulseKey = `${action}Pulse`;
    if (pulseKey in touchState) {
      touchState[pulseKey] = true;
    }
  }

  function updateJoystickVisual(dx, dy) {
    if (!joystickHandle) return;
    joystickHandle.style.transform = `translate(-50%, -50%) translate(${dx}px, ${dy}px)`;
  }

  function updateJoystickMetrics() {
    if (!joystickEl || !joystickHandle) return;
    const rect = joystickEl.getBoundingClientRect();
    const handleRadius = joystickHandle.offsetWidth / 2;
    joystickCenter = {
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    };
    joystickMaxRadius = Math.max(12, rect.width / 2 - handleRadius);
  }

  function updateJoystickFromEvent(event) {
    if (!joystickEl) return;
    const dx = event.clientX - joystickCenter.x;
    const dy = event.clientY - joystickCenter.y;
    const distance = Math.hypot(dx, dy);
    const maxRadius = joystickMaxRadius || 1;
    const deadzone = maxRadius * 0.18;
    if (distance < deadzone) {
      touchState.dirX = 0;
      touchState.dirY = 0;
      updateJoystickVisual(0, 0);
      return;
    }
    const clamped = Math.min(distance, maxRadius);
    const angle = Math.atan2(dy, dx);
    const clampedX = Math.cos(angle) * clamped;
    const clampedY = Math.sin(angle) * clamped;
    touchState.dirX = clampedX / maxRadius;
    touchState.dirY = clampedY / maxRadius;
    updateJoystickVisual(clampedX, clampedY);
  }

  function updatePointerMoveFromEvent(event) {
    const playerEntity = playerEntities.get(playerId);
    if (!playerEntity) {
      pointerMoveState.dirX = 0;
      pointerMoveState.dirY = 0;
      return;
    }
    const playerPos = worldToPixels(playerEntity.x, playerEntity.y, PLAYER_ANCHOR);
    const playerScreenX = playerPos.x + world.x;
    const playerScreenY = playerPos.y + world.y;
    const dx = event.clientX - playerScreenX;
    const dy = event.clientY - playerScreenY;
    const distance = Math.hypot(dx, dy);
    const deadzone = tileSize * 0.25;
    if (distance < deadzone) {
      pointerMoveState.dirX = 0;
      pointerMoveState.dirY = 0;
      return;
    }
    pointerMoveState.dirX = dx / distance;
    pointerMoveState.dirY = dy / distance;
  }

  function chunkKey(x, y) {
    return `${x},${y}`;
  }

  function requestChunksAround() {
    if (!wsOpen) return;
    const playerEntity = playerEntities.get(playerId);
    const px = playerEntity ? playerEntity.x : playerState?.x;
    const py = playerEntity ? playerEntity.y : playerState?.y;
    if (px == null || py == null) return;
    const cx = Math.floor(px / chunkSize);
    const cy = Math.floor(py / chunkSize);
    const needed = [];
    for (let dx = -2; dx <= 2; dx += 1) {
      for (let dy = -2; dy <= 2; dy += 1) {
        const nx = cx + dx;
        const ny = cy + dy;
        const key = chunkKey(nx, ny);
        if (!loadedChunks.has(key) && !pendingChunks.has(key)) {
          pendingChunks.add(key);
          needed.push({ x: nx, y: ny });
        }
      }
    }
    if (needed.length > 0) {
      sendMessage({ type: 'chunk_request', chunks: needed });
    }
  }

  function drawChunk(chunk) {
    ensureTextures();
    const key = chunkKey(chunk.chunk_x, chunk.chunk_y);
    pendingChunks.delete(key);
    if (!chunkTiles.has(key)) {
      const container = createTilemapLayer();
      const chunkX = chunk.chunk_x * chunkSize * tileSize;
      const chunkY = chunk.chunk_y * chunkSize * tileSize;
      container.x = chunkX;
      container.y = chunkY;
      const tiles = chunk.tiles;
      const useTilemap = typeof container.tile === 'function';
      for (let y = 0; y < chunkSize; y += 1) {
        for (let x = 0; x < chunkSize; x += 1) {
          const tileId = tiles[y * chunkSize + x];
          const texture = textures.tiles[tileId] || textures.tiles[0];
          const px = x * tileSize;
          const py = y * tileSize;
          if (useTilemap) {
            container.tile(texture, px, py);
          } else {
            const sprite = new PIXI.Sprite(texture);
            sprite.x = px;
            sprite.y = py;
            container.addChild(sprite);
          }
        }
      }
      tileLayer.addChild(container);
      chunkTiles.set(key, {
        container,
        bounds: {
          x: chunkX,
          y: chunkY,
          width: chunkSize * tileSize,
          height: chunkSize * tileSize,
        },
      });
      updateChunkVisibility();
    }
    loadedChunks.add(key);
    chunk.resources.forEach((res) => upsertResource(res));
  }

  function upsertResource(resource) {
    if (resource.hp <= 0) {
      removeResource(resource.id);
      return;
    }
    let sprite = resourceSprites.get(resource.id);
    if (!sprite) {
      const texture = textures[resource.kind] || textures.tree;
      sprite = new PIXI.Sprite(texture);
      sprite.anchor.set(0.5, 0.9);
      entityLayer.addChild(sprite);
      resourceSprites.set(resource.id, sprite);
    }
    const basePos = worldToPixels(resource.x, resource.y, RESOURCE_ANCHOR);
    sprite.x = basePos.x;
    sprite.y = basePos.y;
    sprite.zIndex = basePos.y;
  }

  function removeResource(id) {
    const sprite = resourceSprites.get(id);
    if (sprite) {
      if (sprite.parent) {
        sprite.parent.removeChild(sprite);
      }
      sprite.destroy();
      resourceSprites.delete(id);
    }
  }

  function syncPlayers(players) {
    const now = performance.now();
    const seen = new Set();
    players.forEach((player) => {
      seen.add(player.id);
      let entity = playerEntities.get(player.id);
      if (!entity) {
        const isAlt = player.id !== playerId;
        const texture = isAlt ? textures.playerAltFront : textures.playerFront;
        const sprite = new PIXI.Sprite(texture);
        sprite.anchor.set(0.5, 0.9);
        entityLayer.addChild(sprite);
        entity = createEntityState(sprite, player.x, player.y, now, {
          facing: 'down',
          isAlt,
        });
        playerEntities.set(player.id, entity);
      } else {
        updateEntityTarget(entity, player.x, player.y, now);
      }
      entity.hp = player.hp;
      if (player.id === playerId) {
        playerState = player;
      }
    });

    for (const [id, entity] of playerEntities.entries()) {
      if (!seen.has(id)) {
        removeEntity(entity);
        playerEntities.delete(id);
        removeTypingIndicator(id);
      }
    }
  }

  function syncMonsters(monsters) {
    const now = performance.now();
    const seen = new Set();
    monsters.forEach((monster) => {
      seen.add(monster.id);
      let entity = monsterEntities.get(monster.id);
      if (!entity) {
        const texture = textures[monster.kind] || textures.slime;
        const sprite = new PIXI.Sprite(texture);
        sprite.anchor.set(0.5, 0.9);
        entityLayer.addChild(sprite);
        entity = createEntityState(sprite, monster.x, monster.y, now);
        monsterEntities.set(monster.id, entity);
      } else {
        updateEntityTarget(entity, monster.x, monster.y, now);
      }
    });

    for (const [id, entity] of monsterEntities.entries()) {
      if (!seen.has(id)) {
        removeEntity(entity);
        monsterEntities.delete(id);
      }
    }
  }

  function syncProjectiles(projectiles) {
    const seen = new Set();
    projectiles.forEach((proj) => {
      seen.add(proj.id);
      if (!projectileSprites.has(proj.id)) {
        const sprite = new PIXI.Sprite(textures.arrow);
        sprite.anchor.set(0.5, 0.5);
        projectileLayer.addChild(sprite);
        projectileSprites.set(proj.id, sprite);
      }
      const sprite = projectileSprites.get(proj.id);
      sprite.x = proj.x * tileSize;
      sprite.y = proj.y * tileSize;
    });

    for (const [id, sprite] of projectileSprites.entries()) {
      if (!seen.has(id)) {
        if (sprite.parent) {
          sprite.parent.removeChild(sprite);
        }
        sprite.destroy();
        projectileSprites.delete(id);
      }
    }
  }

  function addNpc(npc) {
    if (npcSprites.has(npc.id)) return;
    const sprite = new PIXI.Sprite(textures.npc);
    sprite.anchor.set(0.5, 0.9);
    const basePos = worldToPixels(npc.x, npc.y, NPC_ANCHOR);
    sprite.x = basePos.x;
    sprite.y = basePos.y;
    entityLayer.addChild(sprite);
    sprite.zIndex = basePos.y;
    npcSprites.set(npc.id, sprite);
  }

  function updateCamera() {
    const playerEntity = playerEntities.get(playerId);
    if (!playerEntity) return;
    const playerPos = worldToPixels(playerEntity.x, playerEntity.y, PLAYER_ANCHOR);
    const targetX = app.renderer.width / 2 - playerPos.x;
    const targetY = app.renderer.height / 2 - playerPos.y;
    world.x = Math.round(targetX);
    world.y = Math.round(targetY);
    updateChunkVisibility();
  }

  function createTilemapLayer() {
    const TilemapConstructor =
      PIXI.tilemap?.CompositeTilemap ||
      PIXI.tilemap?.Tilemap ||
      PIXI.CompositeTilemap ||
      PIXI.Tilemap;
    if (TilemapConstructor) {
      return new TilemapConstructor();
    }
    return new PIXI.Container();
  }

  function ensureTextures() {
    if (!textures || tileAtlasSize !== tileSize) {
      textures = buildTextures(tileSize);
      tileAtlasSize = tileSize;
    }
  }

  function buildTileAtlas(tileSize) {
    const atlasCanvas = document.createElement('canvas');
    atlasCanvas.width = tileSize * tileAssetUrls.length;
    atlasCanvas.height = tileSize;
    const ctx = atlasCanvas.getContext('2d');
    tileAssetUrls.forEach((url, index) => {
      const texture = PIXI.Assets.get(url) || PIXI.Texture.from(url);
      const resource = texture.source?.resource || texture.baseTexture?.resource;
      const source = resource?.source || resource;
      if (!source) {
        return;
      }
      ctx.drawImage(source, index * tileSize, 0, tileSize, tileSize);
    });
    const atlasTexture = PIXI.Texture.from(atlasCanvas);
    const tiles = {};
    tileAssetUrls.forEach((_, index) => {
      tiles[index] = new PIXI.Texture({
        source: atlasTexture.source,
        frame: new PIXI.Rectangle(index * tileSize, 0, tileSize, tileSize),
      });
    });
    tiles.atlas = atlasTexture;
    return tiles;
  }

  function buildTextures(tileSize) {
    const textures = {
      tiles: buildTileAtlas(tileSize),
    };

    textures.tree = PIXI.Texture.from('assets/entities/tree.svg');
    textures.rock = PIXI.Texture.from('assets/entities/rock.svg');
    textures.playerFront = PIXI.Texture.from('assets/entities/player.svg');
    textures.playerBack = PIXI.Texture.from('assets/entities/player-back.svg');
    textures.playerSide = PIXI.Texture.from('assets/entities/player-side.svg');
    textures.playerAltFront = PIXI.Texture.from('assets/entities/player-alt.svg');
    textures.playerAltBack = PIXI.Texture.from('assets/entities/player-alt-back.svg');
    textures.playerAltSide = PIXI.Texture.from('assets/entities/player-alt-side.svg');
    textures.npc = PIXI.Texture.from('assets/entities/npc.svg');
    textures.slime = PIXI.Texture.from('assets/entities/slime.svg');
    textures.arrow = PIXI.Texture.from('assets/entities/arrow.svg');

    return textures;
  }

  function updateChunkVisibility() {
    if (!chunkTiles.size) return;
    const viewLeft = -world.x;
    const viewTop = -world.y;
    const viewRight = viewLeft + app.renderer.width;
    const viewBottom = viewTop + app.renderer.height;
    const padding = tileSize * chunkSize;
    for (const { container, bounds } of chunkTiles.values()) {
      const visible =
        bounds.x + bounds.width > viewLeft - padding &&
        bounds.x < viewRight + padding &&
        bounds.y + bounds.height > viewTop - padding &&
        bounds.y < viewBottom + padding;
      container.visible = visible;
      container.renderable = visible;
    }
  }

  function sendMessage(payload) {
    if (!wsOpen || !ws) return;
    ws.send(JSON.stringify(payload));
  }

  function createEntityState(sprite, x, y, now, options = {}) {
    return {
      sprite,
      x,
      y,
      startX: x,
      startY: y,
      targetX: x,
      targetY: y,
      startTime: now,
      hp: null,
      facing: options.facing ?? 'down',
      isAlt: options.isAlt ?? false,
      walkOffset: Math.random() * Math.PI * 2,
    };
  }

  function updateEntityTarget(entity, x, y, now) {
    const dx = x - entity.x;
    const dy = y - entity.y;
    if (dx !== 0 || dy !== 0) {
      entity.facing = getFacingFromDelta(dx, dy, entity.facing);
    }
    entity.startX = entity.x;
    entity.startY = entity.y;
    entity.targetX = x;
    entity.targetY = y;
    entity.startTime = now;
  }

  function removeEntity(entity) {
    if (entity.sprite.parent) {
      entity.sprite.parent.removeChild(entity.sprite);
    }
    entity.sprite.destroy();
  }

  function lerp(a, b, t) {
    return a + (b - a) * t;
  }

  function getFacingFromDelta(dx, dy, fallbackFacing) {
    if (dx === 0 && dy === 0) {
      return fallbackFacing;
    }
    if (Math.abs(dx) > Math.abs(dy)) {
      return dx > 0 ? 'right' : 'left';
    }
    return dy > 0 ? 'down' : 'up';
  }

  function getPlayerTexture(entity) {
    const prefix = entity.isAlt ? 'playerAlt' : 'player';
    switch (entity.facing) {
      case 'up':
        return textures[`${prefix}Back`];
      case 'left':
      case 'right':
        return textures[`${prefix}Side`];
      default:
        return textures[`${prefix}Front`];
    }
  }

  function applyPlayerFacing(entity) {
    const texture = getPlayerTexture(entity);
    if (entity.sprite.texture !== texture) {
      entity.sprite.texture = texture;
    }
    const scaleX = Math.abs(entity.sprite.scale.x || 1);
    if (entity.facing === 'left') {
      entity.sprite.scale.x = -scaleX;
    } else {
      entity.sprite.scale.x = scaleX;
    }
  }

  function applyWalkAnimation(entity, now, baseY) {
    const moveDistance = Math.hypot(entity.targetX - entity.startX, entity.targetY - entity.startY);
    const sprite = entity.sprite;
    const facingSign = entity.facing === 'left' ? -1 : 1;
    if (moveDistance > 0.01) {
      const phase = now / 70 + entity.walkOffset;
      const stride = Math.sin(phase);
      const swing = stride * 0.18;
      const bob = Math.abs(stride) * tileSize * 0.06;
      sprite.rotation = swing * 0.35;
      sprite.skew.x = swing * 0.1;
      sprite.scale.x = facingSign * (1 + stride * 0.04);
      sprite.scale.y = 1 - stride * 0.03;
      sprite.y = baseY + bob;
    } else {
      sprite.rotation = 0;
      sprite.skew.x = 0;
      sprite.scale.x = facingSign;
      sprite.scale.y = 1;
      sprite.y = baseY;
    }
  }

  function updateEntities(now) {
    const alpha = (startTime) => Math.min(1, (now - startTime) / INTERP_MS);

    for (const entity of playerEntities.values()) {
      const t = alpha(entity.startTime);
      entity.x = lerp(entity.startX, entity.targetX, t);
      entity.y = lerp(entity.startY, entity.targetY, t);
      applyPlayerFacing(entity);
      const basePos = worldToPixels(entity.x, entity.y, PLAYER_ANCHOR);
      entity.sprite.x = basePos.x;
      const baseY = basePos.y;
      applyWalkAnimation(entity, now, baseY);
      entity.sprite.zIndex = baseY;
    }

    for (const entity of monsterEntities.values()) {
      const t = alpha(entity.startTime);
      entity.x = lerp(entity.startX, entity.targetX, t);
      entity.y = lerp(entity.startY, entity.targetY, t);
      const basePos = worldToPixels(entity.x, entity.y, MONSTER_ANCHOR);
      entity.sprite.x = basePos.x;
      entity.sprite.y = basePos.y;
      entity.sprite.zIndex = basePos.y;
    }

    entityLayer.sortChildren();
  }

  function connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
    ws = new WebSocket(`${protocol}://${window.location.host}/ws`);

    ws.addEventListener('open', () => {
      wsOpen = true;
      statusEl.textContent = 'Connected. Exploring...';
    });

    ws.addEventListener('message', (event) => {
      const msg = JSON.parse(event.data);
      switch (msg.type) {
        case 'welcome': {
          playerId = msg.player.id;
          playerState = msg.player;
          tileSize = msg.world.tile_size;
          chunkSize = msg.world.chunk_size;
          worldSeed = msg.world.seed;
          ensureTextures();
          msg.npcs.forEach((npc) => addNpc(npc));
          statusEl.textContent = `HP ${msg.player.hp}`;
          syncPlayers([msg.player]);
          requestChunksAround();
          break;
        }
        case 'chunk_data': {
          drawChunk(msg);
          break;
        }
        case 'state': {
          syncPlayers(msg.players);
          syncMonsters(msg.monsters);
          syncProjectiles(msg.projectiles);
          requestChunksAround();
          break;
        }
        case 'resource_update': {
          if (msg.state === 'removed') {
            removeResource(msg.resource.id);
          } else {
            upsertResource(msg.resource);
          }
          break;
        }
        case 'chat': {
          addChat(`${msg.from}: ${msg.text}`);
          break;
        }
        case 'system': {
          addChat(msg.text, 'system');
          break;
        }
        case 'typing': {
          setTypingIndicator(msg.id, msg.typing);
          break;
        }
        case 'dialog': {
          showDialog(msg.title, msg.text);
          break;
        }
        default:
          break;
      }
    });

    ws.addEventListener('close', () => {
      wsOpen = false;
      statusEl.textContent = 'Disconnected. Reconnecting...';
      setTimeout(connect, 1000);
    });
  }

  if (helpEl && window.matchMedia('(pointer: coarse)').matches) {
    helpEl.textContent = 'Touch: drag screen or joystick to move · Tap Attack/Gather/Interact · Tap chat to type';
  }

  if (fullscreenButton) {
    updateFullscreenButton();
    fullscreenButton.addEventListener('click', async () => {
      if (getFullscreenElement()) {
        await exitFullscreen();
      } else {
        await requestFullscreen();
      }
    });
    document.addEventListener('fullscreenchange', updateFullscreenButton);
    document.addEventListener('webkitfullscreenchange', updateFullscreenButton);
  }

  if (joystickEl && joystickHandle) {
    joystickEl.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      joystickPointerId = event.pointerId;
      joystickEl.setPointerCapture(event.pointerId);
      updateJoystickMetrics();
      touchState.active = true;
      joystickEl.classList.add('is-active');
      updateJoystickFromEvent(event);
    });

    joystickEl.addEventListener('pointermove', (event) => {
      if (event.pointerId !== joystickPointerId) return;
      event.preventDefault();
      updateJoystickFromEvent(event);
    });

    const releaseJoystick = (event) => {
      if (event.pointerId !== joystickPointerId) return;
      event.preventDefault();
      joystickPointerId = null;
      touchState.active = false;
      touchState.dirX = 0;
      touchState.dirY = 0;
      joystickEl.classList.remove('is-active');
      updateJoystickVisual(0, 0);
    };

    joystickEl.addEventListener('pointerup', releaseJoystick);
    joystickEl.addEventListener('pointercancel', releaseJoystick);
  }

  if (app.canvas) {
    app.canvas.addEventListener('pointerdown', (event) => {
      if (event.pointerType === 'mouse' && event.button !== 0) return;
      if (document.activeElement === chatInput) return;
      pointerMoveState.active = true;
      pointerMoveState.pointerId = event.pointerId;
      app.canvas.setPointerCapture(event.pointerId);
      updatePointerMoveFromEvent(event);
    });

    app.canvas.addEventListener('pointermove', (event) => {
      if (!pointerMoveState.active || event.pointerId !== pointerMoveState.pointerId) return;
      updatePointerMoveFromEvent(event);
    });

    const releasePointerMove = (event) => {
      if (!pointerMoveState.active || event.pointerId !== pointerMoveState.pointerId) return;
      pointerMoveState.active = false;
      pointerMoveState.pointerId = null;
      pointerMoveState.dirX = 0;
      pointerMoveState.dirY = 0;
    };

    app.canvas.addEventListener('pointerup', releasePointerMove);
    app.canvas.addEventListener('pointercancel', releasePointerMove);
    app.canvas.addEventListener('pointerleave', releasePointerMove);
  }

  actionButtons.forEach((button) => {
    const action = button.dataset.action;
    if (!action) return;
    button.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      button.setPointerCapture(event.pointerId);
      setActionState(action, true);
      pulseAction(action);
    });
    const releaseAction = (event) => {
      event.preventDefault();
      setActionState(action, false);
    };
    button.addEventListener('pointerup', releaseAction);
    button.addEventListener('pointercancel', releaseAction);
    button.addEventListener('pointerleave', releaseAction);
  });

  function normalizeArrowKey(event) {
    if (event.code === 'ArrowUp' || event.key === 'ArrowUp' || event.key === 'Up') return 'ArrowUp';
    if (event.code === 'ArrowDown' || event.key === 'ArrowDown' || event.key === 'Down') return 'ArrowDown';
    if (event.code === 'ArrowLeft' || event.key === 'ArrowLeft' || event.key === 'Left') return 'ArrowLeft';
    if (event.code === 'ArrowRight' || event.key === 'ArrowRight' || event.key === 'Right') return 'ArrowRight';
    return null;
  }

  window.addEventListener('keydown', (event) => {
    if (document.activeElement === chatInput) return;
    keys.add(event.code);
    const arrowKey = normalizeArrowKey(event);
    if (arrowKey) {
      keys.add(arrowKey);
    }
    if (['KeyW', 'KeyA', 'KeyS', 'KeyD', 'ArrowUp', 'ArrowLeft', 'ArrowDown', 'ArrowRight', 'Space', 'KeyF', 'KeyE'].includes(event.code) || arrowKey) {
      event.preventDefault();
    }
    if (event.code === 'Enter') {
      chatInput.focus();
    }
  });

  window.addEventListener('keyup', (event) => {
    if (document.activeElement === chatInput) return;
    keys.delete(event.code);
    const arrowKey = normalizeArrowKey(event);
    if (arrowKey) {
      keys.delete(arrowKey);
    }
  });

  chatInput.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      const text = chatInput.value.trim();
      if (text) {
        sendMessage({ type: 'chat', text });
      }
      chatInput.value = '';
      setLocalTyping(false);
      if (typingTimer) {
        clearTimeout(typingTimer);
        typingTimer = null;
      }
      chatInput.blur();
    }
    event.stopPropagation();
  });

  chatInput.addEventListener('input', () => {
    if (!wsOpen) return;
    if (chatInput.value.length === 0) {
      scheduleTypingStop();
      return;
    }
    if (!localTyping) {
      setLocalTyping(true);
    } else {
      sendTypingPing();
    }
    scheduleTypingStop();
  });

  chatInput.addEventListener('blur', () => {
    setLocalTyping(false);
    if (typingTimer) {
      clearTimeout(typingTimer);
      typingTimer = null;
    }
  });

  setInterval(() => {
    if (!wsOpen) return;
    const inputLocked = document.activeElement === chatInput;
    const usingTouch = touchState.active;
    const usingPointerMove = pointerMoveState.active;
    const dirX = inputLocked
      ? 0
      : usingTouch
        ? touchState.dirX
        : usingPointerMove
          ? pointerMoveState.dirX
          : ((keys.has('KeyD') || keys.has('ArrowRight')) ? 1 : 0)
            - ((keys.has('KeyA') || keys.has('ArrowLeft')) ? 1 : 0);
    const dirY = inputLocked
      ? 0
      : usingTouch
        ? touchState.dirY
        : usingPointerMove
          ? pointerMoveState.dirY
          : ((keys.has('KeyS') || keys.has('ArrowDown')) ? 1 : 0)
            - ((keys.has('KeyW') || keys.has('ArrowUp')) ? 1 : 0);
    const attack = !inputLocked && (keys.has('Space') || touchState.attack || touchState.attackPulse);
    const gather = !inputLocked && (keys.has('KeyF') || touchState.gather || touchState.gatherPulse);
    const interact = !inputLocked && (keys.has('KeyE') || touchState.interact || touchState.interactPulse);
    sendMessage({
      type: 'input',
      dir_x: dirX,
      dir_y: dirY,
      attack,
      gather,
      interact,
    });
    touchState.attackPulse = false;
    touchState.gatherPulse = false;
    touchState.interactPulse = false;
  }, 90);

  app.ticker.add(() => {
    const now = performance.now();
    updateEntities(now);
    updateTypingIndicators(now);
    updateCamera();
    if (now - lastStatusUpdate > 200 && playerId) {
      const playerEntity = playerEntities.get(playerId);
      if (playerEntity) {
        const hp = playerEntity.hp != null ? playerEntity.hp : playerState?.hp ?? 0;
        statusEl.textContent = `HP ${hp} | ${playerEntity.x.toFixed(1)}, ${playerEntity.y.toFixed(1)}`;
      }
      lastStatusUpdate = now;
    }
  });

  fetch('/api/session')
    .then(() => connect())
    .catch(() => {
      statusEl.textContent = 'Failed to start session.';
    });
})();
