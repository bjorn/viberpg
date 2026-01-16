(() => {
  const statusEl = document.getElementById('status');
  const chatLog = document.getElementById('chat-log');
  const chatInput = document.getElementById('chat-input');
  const dialogEl = document.getElementById('dialog');
  const dialogTitle = document.getElementById('dialog-title');
  const dialogText = document.getElementById('dialog-text');

  let dialogTimer = null;
  let ws = null;
  let wsOpen = false;

  let tileSize = 16;
  let chunkSize = 32;
  let playerId = null;
  let playerState = null;
  let worldSeed = 0;

  const app = new PIXI.Application({
    resizeTo: window,
    backgroundColor: 0x0b0e14,
    antialias: false,
  });
  PIXI.settings.SCALE_MODE = PIXI.SCALE_MODES.NEAREST;
  app.renderer.roundPixels = true;
  document.body.appendChild(app.view);

  const world = new PIXI.Container();
  const tileLayer = new PIXI.Container();
  const resourceLayer = new PIXI.Container();
  const entityLayer = new PIXI.Container();
  const projectileLayer = new PIXI.Container();
  world.addChild(tileLayer, resourceLayer, entityLayer, projectileLayer);
  app.stage.addChild(world);

  const textures = buildTextures();

  const chunkTiles = new Map();
  const resourceSprites = new Map();
  const playerEntities = new Map();
  const monsterEntities = new Map();
  const projectileSprites = new Map();
  const npcSprites = new Map();

  const loadedChunks = new Set();
  const pendingChunks = new Set();

  const keys = new Set();
  const INTERP_MS = 120;
  const MAX_CHAT_LINES = 60;
  let lastStatusUpdate = 0;

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
    const key = chunkKey(chunk.chunk_x, chunk.chunk_y);
    pendingChunks.delete(key);
    if (!chunkTiles.has(key)) {
      const container = new PIXI.Container();
      container.x = chunk.chunk_x * chunkSize * tileSize;
      container.y = chunk.chunk_y * chunkSize * tileSize;
      const tiles = chunk.tiles;
      for (let y = 0; y < chunkSize; y += 1) {
        for (let x = 0; x < chunkSize; x += 1) {
          const tileId = tiles[y * chunkSize + x];
          const texture = textures.tiles[tileId] || textures.tiles[0];
          const sprite = new PIXI.Sprite(texture);
          sprite.x = x * tileSize;
          sprite.y = y * tileSize;
          container.addChild(sprite);
        }
      }
      tileLayer.addChild(container);
      chunkTiles.set(key, container);
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
      resourceLayer.addChild(sprite);
      resourceSprites.set(resource.id, sprite);
    }
    sprite.x = (resource.x + 0.5) * tileSize;
    sprite.y = (resource.y + 1.0) * tileSize;
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
        const texture = player.id === playerId ? textures.player : textures.playerAlt;
        const sprite = new PIXI.Sprite(texture);
        sprite.anchor.set(0.5, 0.9);
        entityLayer.addChild(sprite);
        entity = createEntityState(sprite, player.x, player.y, now);
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
    sprite.x = (npc.x + 0.5) * tileSize;
    sprite.y = (npc.y + 0.9) * tileSize;
    entityLayer.addChild(sprite);
    npcSprites.set(npc.id, sprite);
  }

  function updateCamera() {
    const playerEntity = playerEntities.get(playerId);
    if (!playerEntity) return;
    const targetX = app.renderer.width / 2 - playerEntity.x * tileSize;
    const targetY = app.renderer.height / 2 - playerEntity.y * tileSize;
    world.x = Math.round(targetX);
    world.y = Math.round(targetY);
  }

  function buildTextures() {
    const makeTexture = (draw) => {
      const canvas = document.createElement('canvas');
      canvas.width = 16;
      canvas.height = 16;
      const ctx = canvas.getContext('2d');
      ctx.imageSmoothingEnabled = false;
      draw(ctx);
      return PIXI.Texture.from(canvas);
    };

    const textures = {
      tiles: {},
    };

    textures.tiles[0] = makeTexture((ctx) => {
      ctx.fillStyle = '#4b8c3f';
      ctx.fillRect(0, 0, 16, 16);
      ctx.fillStyle = '#3f7b33';
      ctx.fillRect(2, 2, 2, 2);
      ctx.fillRect(10, 5, 2, 2);
      ctx.fillRect(6, 11, 2, 2);
    });

    textures.tiles[1] = makeTexture((ctx) => {
      ctx.fillStyle = '#2a5faa';
      ctx.fillRect(0, 0, 16, 16);
      ctx.fillStyle = '#3a79c7';
      ctx.fillRect(1, 4, 6, 2);
      ctx.fillRect(8, 9, 6, 2);
    });

    textures.tiles[2] = makeTexture((ctx) => {
      ctx.fillStyle = '#c2a768';
      ctx.fillRect(0, 0, 16, 16);
      ctx.fillStyle = '#b89752';
      ctx.fillRect(4, 4, 3, 3);
      ctx.fillRect(9, 9, 3, 3);
    });

    textures.tiles[3] = makeTexture((ctx) => {
      ctx.fillStyle = '#7a5a3a';
      ctx.fillRect(0, 0, 16, 16);
      ctx.fillStyle = '#6a4c2f';
      ctx.fillRect(3, 3, 2, 2);
      ctx.fillRect(10, 6, 2, 2);
    });

    textures.tree = makeTexture((ctx) => {
      ctx.fillStyle = '#2d6b39';
      ctx.fillRect(3, 2, 10, 7);
      ctx.fillStyle = '#3f8d4a';
      ctx.fillRect(4, 3, 8, 5);
      ctx.fillStyle = '#3b2a1c';
      ctx.fillRect(7, 9, 2, 5);
    });

    textures.rock = makeTexture((ctx) => {
      ctx.fillStyle = '#70757d';
      ctx.fillRect(4, 7, 8, 6);
      ctx.fillStyle = '#8b9199';
      ctx.fillRect(6, 8, 4, 2);
    });

    textures.player = makeTexture((ctx) => {
      ctx.fillStyle = '#e3b98d';
      ctx.fillRect(6, 3, 4, 4);
      ctx.fillStyle = '#28427c';
      ctx.fillRect(5, 7, 6, 6);
      ctx.fillStyle = '#1f2b4a';
      ctx.fillRect(6, 11, 4, 3);
    });

    textures.playerAlt = makeTexture((ctx) => {
      ctx.fillStyle = '#d9a86c';
      ctx.fillRect(6, 3, 4, 4);
      ctx.fillStyle = '#7b3a2c';
      ctx.fillRect(5, 7, 6, 6);
      ctx.fillStyle = '#5a2a1e';
      ctx.fillRect(6, 11, 4, 3);
    });

    textures.npc = makeTexture((ctx) => {
      ctx.fillStyle = '#d6c18e';
      ctx.fillRect(6, 3, 4, 4);
      ctx.fillStyle = '#4b6b2e';
      ctx.fillRect(5, 7, 6, 6);
      ctx.fillStyle = '#2e4a1a';
      ctx.fillRect(6, 11, 4, 3);
    });

    textures.slime = makeTexture((ctx) => {
      ctx.fillStyle = '#3dbd7d';
      ctx.fillRect(4, 7, 8, 6);
      ctx.fillStyle = '#2a8a5c';
      ctx.fillRect(5, 8, 6, 3);
      ctx.fillStyle = '#0b2b1c';
      ctx.fillRect(6, 9, 1, 1);
      ctx.fillRect(9, 9, 1, 1);
    });

    textures.arrow = makeTexture((ctx) => {
      ctx.fillStyle = '#e9d6a4';
      ctx.fillRect(7, 2, 2, 12);
      ctx.fillStyle = '#8a6d3b';
      ctx.fillRect(7, 9, 2, 4);
    });

    return textures;
  }

  function sendMessage(payload) {
    if (!wsOpen || !ws) return;
    ws.send(JSON.stringify(payload));
  }

  function createEntityState(sprite, x, y, now) {
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
    };
  }

  function updateEntityTarget(entity, x, y, now) {
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

  function updateEntities(now) {
    const alpha = (startTime) => Math.min(1, (now - startTime) / INTERP_MS);

    for (const entity of playerEntities.values()) {
      const t = alpha(entity.startTime);
      entity.x = lerp(entity.startX, entity.targetX, t);
      entity.y = lerp(entity.startY, entity.targetY, t);
      entity.sprite.x = (entity.x + 0.5) * tileSize;
      entity.sprite.y = (entity.y + 0.9) * tileSize;
    }

    for (const entity of monsterEntities.values()) {
      const t = alpha(entity.startTime);
      entity.x = lerp(entity.startX, entity.targetX, t);
      entity.y = lerp(entity.startY, entity.targetY, t);
      entity.sprite.x = (entity.x + 0.5) * tileSize;
      entity.sprite.y = (entity.y + 0.9) * tileSize;
    }
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

  window.addEventListener('keydown', (event) => {
    if (document.activeElement === chatInput) return;
    keys.add(event.code);
    if (['KeyW', 'KeyA', 'KeyS', 'KeyD', 'Space', 'KeyF', 'KeyE'].includes(event.code)) {
      event.preventDefault();
    }
    if (event.code === 'Enter') {
      chatInput.focus();
    }
  });

  window.addEventListener('keyup', (event) => {
    if (document.activeElement === chatInput) return;
    keys.delete(event.code);
  });

  chatInput.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      const text = chatInput.value.trim();
      if (text) {
        sendMessage({ type: 'chat', text });
      }
      chatInput.value = '';
      chatInput.blur();
    }
    event.stopPropagation();
  });

  setInterval(() => {
    if (!wsOpen) return;
    if (document.activeElement === chatInput) return;
    const dirX = (keys.has('KeyD') ? 1 : 0) - (keys.has('KeyA') ? 1 : 0);
    const dirY = (keys.has('KeyS') ? 1 : 0) - (keys.has('KeyW') ? 1 : 0);
    const attack = keys.has('Space');
    const gather = keys.has('KeyF');
    const interact = keys.has('KeyE');
    sendMessage({
      type: 'input',
      dir_x: dirX,
      dir_y: dirY,
      attack,
      gather,
      interact,
    });
  }, 90);

  app.ticker.add(() => {
    const now = performance.now();
    updateEntities(now);
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
