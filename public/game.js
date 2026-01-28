(async () => {
  const statusTextEl = document.getElementById('status-text');
  const statusHeartsEl = document.getElementById('status-hearts');
  const statusCoordsEl = document.getElementById('status-coords');
  const chatLog = document.getElementById('chat-log');
  const chatInput = document.getElementById('chat-input');
  const nameInput = document.getElementById('name-input');
  const nameSave = document.getElementById('name-save');
  const inventoryList = document.getElementById('inventory-list');
  const inventoryPanel = document.getElementById('inventory');
  const buildMenu = document.getElementById('build-menu');
  const hudEl = document.getElementById('hud');
  const buildStatus = document.getElementById('build-status');
  const buildButtons = Array.from(document.querySelectorAll('.build-btn'));
  const panelButtons = Array.from(document.querySelectorAll('.panel-btn'));
  const uiScaleDown = document.getElementById('ui-scale-down');
  const uiScaleUp = document.getElementById('ui-scale-up');
  const uiScaleValue = document.getElementById('ui-scale-value');
  const dialogEl = document.getElementById('dialog');
  const dialogTitle = document.getElementById('dialog-title');
  const dialogText = document.getElementById('dialog-text');
  const helpEl = document.getElementById('help');
  const fullscreenButton = document.getElementById('fullscreen-toggle');
  const musicToggle = document.getElementById('music-toggle');
  const sfxToggle = document.getElementById('sfx-toggle');
  const joystickEl = document.getElementById('touch-joystick');
  const joystickHandle = joystickEl ? joystickEl.querySelector('.stick-handle') : null;
  const actionButtons = Array.from(document.querySelectorAll('.action-btn'));
  const buildMenuBaseTop = buildMenu
    ? Number.parseFloat(window.getComputedStyle(buildMenu).top) || buildMenu.offsetTop
    : 0;
  const translations = {
    en: {
      statusConnecting: 'Connecting...',
      statusConnected: 'Connected. Exploring...',
      statusDisconnected: 'Disconnected. Reconnecting...',
      statusSessionFailed: 'Failed to start session.',
      panelInventory: 'Inventory',
      panelBuild: 'Build',
      panelChat: 'Chat',
      toggleInventory: 'Toggle inventory',
      toggleBuild: 'Toggle build menu',
      toggleChat: 'Toggle chat',
      uiScale: 'UI scale',
      uiScaleDown: 'Scale UI down',
      uiScaleUp: 'Scale UI up',
      fullscreenEnter: 'Enter fullscreen',
      fullscreenExit: 'Exit fullscreen',
      namePlaceholder: 'Name',
      nameAria: 'Player name',
      nameSet: 'Set',
      nameSetAria: 'Set name',
      chatPlaceholder: 'Say something...',
      buildStatusSelect: 'Select a build option.',
      buildStatusDemolish: 'Click a structure twice to remove it.',
      buildStatusPlace: 'Click the map to place.',
      buildStatusNotConnected: 'Not connected.',
      buildStatusDemolishConfirm: 'Click again to confirm demolition.',
      buildStatusDemolishRequested: 'Demolition requested.',
      buildStatusPlacementRequested: 'Placement requested.',
      buildStatusCraftRequested: 'Crafting requested.',
      buildOptionCraftAxe: 'Wooden Axe (4 wood)',
      buildOptionCraftPick: 'Wooden Pickaxe (4 wood)',
      buildOptionCraftArrows: 'Arrows x6 (1 wood + 1 stone)',
      buildOptionHut: 'Wood Hut (20 wood)',
      buildOptionHouse: 'Stone House (50 stone)',
      buildOptionBridgeWood: 'Wood Bridge (10 wood)',
      buildOptionBridgeStone: 'Stone Bridge (20 stone)',
      buildOptionPath: 'Path (shovel)',
      buildOptionRoad: 'Road (2 stone + shovel)',
      buildOptionBoat: 'Boat (10 wood)',
      buildOptionDemolish: 'Demolish',
      actionAttack: 'Attack',
      actionGather: 'Gather',
      actionInteract: 'Interact',
      helpTouch: 'Touch: drag screen or joystick to move · Tap Attack/Gather/Interact · Tap chat to type',
      inventoryEmpty: 'Empty',
      inventoryEat: 'Click to eat',
      musicToggle: 'Toggle music',
      sfxToggle: 'Toggle sound effects',
      hpLabel: 'HP',
    },
    de: {
      statusConnecting: 'Verbinde...',
      statusConnected: 'Verbunden. Auf Erkundung...',
      statusDisconnected: 'Getrennt. Verbinde neu...',
      statusSessionFailed: 'Sitzung konnte nicht gestartet werden.',
      panelInventory: 'Inventar',
      panelBuild: 'Bauen',
      panelChat: 'Chat',
      toggleInventory: 'Inventar ein-/ausblenden',
      toggleBuild: 'Bau-Menü ein-/ausblenden',
      toggleChat: 'Chat ein-/ausblenden',
      uiScale: 'UI-Skalierung',
      uiScaleDown: 'UI verkleinern',
      uiScaleUp: 'UI vergrößern',
      fullscreenEnter: 'Vollbild aktivieren',
      fullscreenExit: 'Vollbild verlassen',
      namePlaceholder: 'Name',
      nameAria: 'Spielername',
      nameSet: 'Setzen',
      nameSetAria: 'Name setzen',
      chatPlaceholder: 'Sag etwas...',
      buildStatusSelect: 'Bauoption wählen.',
      buildStatusDemolish: 'Gebäude doppelt anklicken zum Entfernen.',
      buildStatusPlace: 'Karte zum Platzieren anklicken.',
      buildStatusNotConnected: 'Nicht verbunden.',
      buildStatusDemolishConfirm: 'Nochmal klicken zum Bestätigen.',
      buildStatusDemolishRequested: 'Abriss angefragt.',
      buildStatusPlacementRequested: 'Platzierung angefragt.',
      buildStatusCraftRequested: 'Herstellung angefragt.',
      buildOptionCraftAxe: 'Holzaxt (4 Holz)',
      buildOptionCraftPick: 'Holzspitzhacke (4 Holz)',
      buildOptionCraftArrows: 'Pfeile x6 (1 Holz + 1 Stein)',
      buildOptionHut: 'Holzhütte (20 Holz)',
      buildOptionHouse: 'Steinhaus (50 Stein)',
      buildOptionBridgeWood: 'Holzbrücke (10 Holz)',
      buildOptionBridgeStone: 'Steinbrücke (20 Stein)',
      buildOptionPath: 'Pfad (Schaufel)',
      buildOptionRoad: 'Straße (2 Stein + Schaufel)',
      buildOptionBoat: 'Boot (10 Holz)',
      buildOptionDemolish: 'Abriss',
      actionAttack: 'Angriff',
      actionGather: 'Sammeln',
      actionInteract: 'Interagieren',
      helpTouch: 'Touch: Bildschirm oder Joystick ziehen zum Laufen · Angriff/Sammeln/Interagieren tippen · Chat zum Tippen antippen',
      inventoryEmpty: 'Leer',
      inventoryEat: 'Klicken zum Essen',
      musicToggle: 'Musik umschalten',
      sfxToggle: 'Soundeffekte umschalten',
      hpLabel: 'HP',
    },
  };
  const locale =
    (navigator.languages && navigator.languages[0]) || navigator.language || 'en';
  const language = locale.toLowerCase().startsWith('de') ? 'de' : 'en';
  const strings = translations[language] || translations.en;
  const t = (key) => strings[key] || translations.en[key] || key;
  document.documentElement.lang = language;

  let dialogTimer = null;
  let ws = null;
  let wsOpen = false;

  const MAX_HEARTS = 10;
  let tileSize = 32;
  let chunkSize = 32;
  let playerId = null;
  let playerState = null;
  let localInBoat = false;
  let worldSeed = 0;
  const PLAYER_ANCHOR = { x: 0.5, y: 0.9 };
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
  const structureLayer = new PIXI.Container();
  const entityLayer = new PIXI.Container();
  const overlayLayer = new PIXI.Container();
  const projectileLayer = new PIXI.Container();
  entityLayer.sortableChildren = true;
  structureLayer.sortableChildren = true;
  overlayLayer.sortableChildren = true;
  world.addChild(tileLayer, structureLayer, entityLayer, projectileLayer, overlayLayer);
  app.stage.addChild(world);

  const tileAssetUrls = [
    'assets/tiles/grass.svg',
    'assets/tiles/water.svg',
    'assets/tiles/sand.svg',
    'assets/tiles/dirt.svg',
    'assets/tiles/grass-flowers.svg',
  ];
  const TILE_WATER = 1;
  const entityAssetUrls = [
    'assets/entities/tree-1.svg',
    'assets/entities/tree-2.svg',
    'assets/entities/tree-3.svg',
    'assets/entities/tree-apple-1.svg',
    'assets/entities/tree-apple-2.svg',
    'assets/entities/tree-apple-3.svg',
    'assets/entities/tree-pine-1.svg',
    'assets/entities/tree-pine-2.svg',
    'assets/entities/tree-pine-3.svg',
    'assets/entities/palm-1.svg',
    'assets/entities/palm-2.svg',
    'assets/entities/palm-3.svg',
    'assets/entities/rock.svg',
    'assets/entities/rock-small.svg',
    'assets/entities/rock-medium.svg',
    'assets/entities/rock-large.svg',
    'assets/entities/hut-wood.svg',
    'assets/entities/house-stone.svg',
    'assets/entities/bridge-wood.svg',
    'assets/entities/bridge-stone.svg',
    'assets/entities/path.svg',
    'assets/entities/road.svg',
    'assets/entities/boat.svg',
    'assets/entities/tent.svg',
    'assets/entities/campfire.svg',
    'assets/entities/player.svg',
    'assets/entities/player-back.svg',
    'assets/entities/player-side.svg',
    'assets/entities/player-alt.svg',
    'assets/entities/player-alt-back.svg',
    'assets/entities/player-alt-side.svg',
    'assets/entities/npc.svg',
    'assets/entities/slime.svg',
    'assets/entities/boar.svg',
    'assets/entities/rabbit.svg',
    'assets/entities/arrow.svg',
  ];
  await PIXI.Assets.load([...tileAssetUrls, ...entityAssetUrls]);
  let textures = null;
  let tileAtlasSize = null;

  const chunkTiles = new Map();
  const chunkResources = new Map();
  const chunkStructures = new Map();
  const resourceSprites = new Map();
  const structureSprites = new Map();
  const structureTiles = new Map();
  const playerEntities = new Map();
  const monsterEntities = new Map();
  const projectileSprites = new Map();
  const boatEntities = new Map();
  const npcSprites = new Map();
  const landmarkSprites = new Map();
  const typingIndicators = new Map();
  const treeKinds = new Set(['tree', 'apple_tree', 'pine_tree', 'palm_tree']);
  function resourceTextureFor(kind, size = 1) {
    const level = Math.max(1, Math.min(3, size || 1));
    switch (kind) {
      case 'tree':
        return textures[`tree${level}`] || textures.tree1;
      case 'apple_tree':
        return textures[`apple_tree${level}`] || textures.apple_tree1;
      case 'pine_tree':
        return textures[`pine_tree${level}`] || textures.pine_tree1;
      case 'palm_tree':
        return textures[`palm_tree${level}`] || textures.palm_tree1;
      case 'rock': {
        if (level >= 3) return textures.rockLarge || textures.rock;
        if (level === 2) return textures.rockMedium || textures.rock;
        return textures.rockSmall || textures.rock;
      }
      default:
        return textures[kind];
    }
  }

  const loadedChunks = new Set();
  const pendingChunks = new Set();
  const CHUNK_REQUEST_RADIUS = 2;
  const CHUNK_KEEP_RADIUS = 3;

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
  const INPUT_SEND_INTERVAL_MS = 90;
  const PLAYER_SPEED = 3.4;
  const CORRECTION_DISTANCE = 2;
  const MAX_CHAT_LINES = 60;
  const TYPING_IDLE_MS = 1800;
  const MAX_NAME_CHARS = 20;
  const GATHER_RANGE = 1.1;
  const INTERACT_RANGE = 1.2;
  let localTyping = false;
  let typingTimer = null;
  let lastTypingSent = 0;
  let lastStatusUpdate = 0;
  let lastChunkRequest = 0;
  let lastChunkPrune = 0;
  let joystickPointerId = null;
  let joystickCenter = { x: 0, y: 0 };
  let joystickMaxRadius = 0;
  let lastKnownName = '';
  let nameStyle = null;
  let pendingName = null;
  let buildMode = null;
  const craftKinds = new Set(['craft_basic_axe', 'craft_basic_pick', 'craft_arrows']);
  let pendingDemolish = null;
  let buildPreviewSprite = null;
  let buildPreviewKind = null;
  let lastPointerTile = null;
  let uiScale = 1;
  let inputSeq = 0;
  let lastAckSeq = 0;
  const pendingInputs = [];
  let localPrediction = null;
  let lastInputDir = { x: 0, y: 0 };
  let localRenderOffset = { x: 0, y: 0 };
  let lastStatusHp = null;
  let lastStatusCoords = null;

  function setStatusText(text) {
    if (!statusTextEl) {
      return;
    }
    if (statusTextEl.textContent !== text) {
      statusTextEl.textContent = text;
    }
  }

  function renderStatusHearts(hp, maxHearts = MAX_HEARTS) {
    if (!statusHeartsEl) {
      return;
    }
    const safeHp = Math.max(0, Math.min(hp, maxHearts));
    const label = `${t('hpLabel')} ${safeHp}/${maxHearts}`;
    if (lastStatusHp === safeHp && statusHeartsEl.childElementCount === maxHearts) {
      statusHeartsEl.setAttribute('aria-label', label);
      statusHeartsEl.title = label;
      return;
    }
    while (statusHeartsEl.firstChild) {
      statusHeartsEl.removeChild(statusHeartsEl.firstChild);
    }
    for (let i = 1; i <= maxHearts; i += 1) {
      const heart = document.createElement('span');
      heart.className = i <= safeHp ? 'heart' : 'heart empty';
      heart.textContent = '♥';
      statusHeartsEl.appendChild(heart);
    }
    statusHeartsEl.setAttribute('aria-label', label);
    statusHeartsEl.title = label;
    lastStatusHp = safeHp;
  }

  function setStatusCoords(text) {
    if (!statusCoordsEl) {
      return;
    }
    if (lastStatusCoords !== text) {
      statusCoordsEl.textContent = text;
      lastStatusCoords = text;
    }
  }

  const audioSettingsKey = 'audio-settings';
  const defaultAudioSettings = {
    musicEnabled: true,
    sfxEnabled: true,
    musicVolume: 0.25,
    sfxVolume: 0.7,
  };
  let audioSettings = { ...defaultAudioSettings };
  try {
    const raw = localStorage.getItem(audioSettingsKey);
    if (raw) {
      const parsed = JSON.parse(raw);
      audioSettings = { ...audioSettings, ...parsed };
    }
  } catch (err) {
    console.warn('Audio settings load failed', err);
  }

  let audioContext = null;
  let masterGain = null;
  let musicGain = null;
  let sfxGain = null;
  let currentMusicSource = null;
  const audioBuffers = new Map();
  let audioUnlocked = false;
  let currentMidiPart = null;
  let currentMidiSynths = [];
  const midiCache = new Map();

  function saveAudioSettings() {
    try {
      localStorage.setItem(audioSettingsKey, JSON.stringify(audioSettings));
    } catch (err) {
      console.warn('Audio settings save failed', err);
    }
  }

  function ensureAudioContext() {
    if (!audioContext) {
      const AudioContextCtor = window.AudioContext || window.webkitAudioContext;
      if (!AudioContextCtor) {
        console.warn('Web Audio not supported');
        return null;
      }
      audioContext = new AudioContextCtor();
      masterGain = audioContext.createGain();
      musicGain = audioContext.createGain();
      sfxGain = audioContext.createGain();
      musicGain.connect(masterGain);
      sfxGain.connect(masterGain);
      masterGain.connect(audioContext.destination);
      applyAudioSettings();
    }
    return audioContext;
  }

  function applyAudioSettings() {
    if (!masterGain || !musicGain || !sfxGain) return;
    const musicOn = Boolean(audioSettings.musicEnabled);
    const sfxOn = Boolean(audioSettings.sfxEnabled);
    masterGain.gain.value = 1;
    musicGain.gain.value = musicOn ? Math.max(0, audioSettings.musicVolume ?? 0.25) : 0;
    sfxGain.gain.value = sfxOn ? Math.max(0, audioSettings.sfxVolume ?? 0.7) : 0;
    updateMidiVolume();
    setToggleButtonState(
      musicToggle,
      musicOn,
      `${t('musicToggle')} (On)`,
      `${t('musicToggle')} (Off)`
    );
    setToggleButtonState(
      sfxToggle,
      sfxOn,
      `${t('sfxToggle')} (On)`,
      `${t('sfxToggle')} (Off)`
    );
  }

  function setToggleButtonState(button, enabled, labelOn, labelOff) {
    if (!button) return;
    button.classList.toggle('active', enabled);
    button.setAttribute('aria-pressed', String(enabled));
    button.setAttribute('aria-label', enabled ? labelOn : labelOff);
    button.setAttribute('title', enabled ? labelOn : labelOff);
  }

  async function unlockAudioContext() {
    if (audioUnlocked) return;
    const ctx = ensureAudioContext();
    if (!ctx) return;
    if (ctx.state === 'suspended') {
      await ctx.resume();
    }
    audioUnlocked = true;
  }

  async function loadAudioBuffer(url) {
    if (audioBuffers.has(url)) {
      return audioBuffers.get(url);
    }
    const ctx = ensureAudioContext();
    if (!ctx) return null;
    const response = await fetch(url);
    const data = await response.arrayBuffer();
    const buffer = await ctx.decodeAudioData(data);
    audioBuffers.set(url, buffer);
    return buffer;
  }

  async function playMusic(url, { loop = true } = {}) {
    const ctx = ensureAudioContext();
    if (!ctx) return;
    const buffer = await loadAudioBuffer(url);
    if (!buffer) return;
    if (currentMusicSource) {
      try {
        currentMusicSource.stop();
      } catch (err) {
        console.warn('Music stop failed', err);
      }
      currentMusicSource.disconnect();
    }
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.loop = loop;
    source.connect(musicGain);
    source.start(0);
    currentMusicSource = source;
  }

  function stopMusic() {
    if (!currentMusicSource) return;
    try {
      currentMusicSource.stop();
    } catch (err) {
      console.warn('Music stop failed', err);
    }
    currentMusicSource.disconnect();
    currentMusicSource = null;
  }

  async function playSfx(url, { volume = 1 } = {}) {
    const ctx = ensureAudioContext();
    if (!ctx) return;
    const buffer = await loadAudioBuffer(url);
    if (!buffer) return;
    const source = ctx.createBufferSource();
    const gainNode = ctx.createGain();
    gainNode.gain.value = Math.max(0, volume);
    source.buffer = buffer;
    source.connect(gainNode);
    gainNode.connect(sfxGain);
    source.start(0);
  }

  async function loadMidi(url) {
    if (midiCache.has(url)) {
      return midiCache.get(url);
    }
    try {
      const midi = await Midi.fromUrl(url);
      midiCache.set(url, midi);
      return midi;
    } catch (err) {
      console.error('MIDI load failed:', err);
      return null;
    }
  }

  async function playMidi(url, { loop = true } = {}) {
    ensureAudioContext();
    if (!Tone || !Midi) {
      console.warn('Tone.js or @tonejs/midi not loaded');
      return;
    }
    await Tone.start();
    audioUnlocked = true;

    stopMidi();

    const midi = await loadMidi(url);
    if (!midi) return;

    const synths = [];
    midi.tracks.forEach((track) => {
      const synth = new Tone.PolySynth(Tone.Synth, {
        maxPolyphony: 8,
        oscillator: { type: 'triangle' },
        envelope: { attack: 0.005, decay: 0.1, sustain: 0.4, release: 0.3 },
      });
      const gain = new Tone.Gain(audioSettings.musicVolume ?? 0.25);
      synth.connect(gain);
      gain.connect(Tone.getDestination());
      synths.push({ synth, gain, track });
    });
    currentMidiSynths = synths;

    const part = new Tone.Part((time, note) => {
      const synthEntry = synths[note.trackIndex];
      if (synthEntry) {
        synthEntry.synth.triggerAttackRelease(
          note.name,
          note.duration,
          time,
          note.velocity
        );
      }
    }, []);

    midi.tracks.forEach((track, trackIndex) => {
      track.notes.forEach((note) => {
        part.add({
          time: note.time,
          trackIndex,
          name: note.name,
          duration: note.duration,
          velocity: note.velocity,
        });
      });
    });

    part.loop = loop;
    part.loopEnd = midi.duration;
    part.start(0);
    Tone.getTransport().start();
    currentMidiPart = part;
  }

  function stopMidi() {
    if (currentMidiPart) {
      currentMidiPart.stop();
      currentMidiPart.dispose();
      currentMidiPart = null;
    }
    if (currentMidiSynths.length > 0) {
      currentMidiSynths.forEach(({ synth, gain }) => {
        synth.releaseAll();
        synth.dispose();
        gain.dispose();
      });
      currentMidiSynths = [];
    }
    if (Tone && Tone.getTransport) {
      Tone.getTransport().stop();
      Tone.getTransport().position = 0;
    }
  }

  function updateMidiVolume() {
    if (currentMidiSynths.length > 0) {
      const vol = audioSettings.musicEnabled ? (audioSettings.musicVolume ?? 0.25) : 0;
      currentMidiSynths.forEach(({ gain }) => {
        gain.gain.rampTo(vol, 0.1);
      });
    }
  }

  function applyLocale() {
    setStatusText(t('statusConnecting'));
    renderStatusHearts(lastStatusHp ?? 0);
    const inventoryTitle = inventoryPanel?.querySelector('.panel-title');
    if (inventoryTitle) {
      inventoryTitle.textContent = t('panelInventory');
    }
    const buildTitle = buildMenu?.querySelector('.panel-title');
    if (buildTitle) {
      buildTitle.textContent = t('panelBuild');
    }
    const chatTitle = document.querySelector('#chat .panel-title');
    if (chatTitle) {
      chatTitle.textContent = t('panelChat');
    }
    const inventoryToggle = document.querySelector('[data-panel="inventory"]');
    if (inventoryToggle) {
      inventoryToggle.setAttribute('aria-label', t('toggleInventory'));
      inventoryToggle.setAttribute('title', t('toggleInventory'));
    }
    const buildToggle = document.querySelector('[data-panel="build-menu"]');
    if (buildToggle) {
      buildToggle.setAttribute('aria-label', t('toggleBuild'));
      buildToggle.setAttribute('title', t('toggleBuild'));
    }
    const chatToggle = document.querySelector('[data-panel="chat"]');
    if (chatToggle) {
      chatToggle.setAttribute('aria-label', t('toggleChat'));
      chatToggle.setAttribute('title', t('toggleChat'));
    }
    const uiScalePanel = document.getElementById('ui-scale-control');
    if (uiScalePanel) {
      uiScalePanel.setAttribute('aria-label', t('uiScale'));
    }
    if (uiScaleDown) {
      uiScaleDown.setAttribute('aria-label', t('uiScaleDown'));
      uiScaleDown.setAttribute('title', t('uiScaleDown'));
    }
    if (uiScaleUp) {
      uiScaleUp.setAttribute('aria-label', t('uiScaleUp'));
      uiScaleUp.setAttribute('title', t('uiScaleUp'));
    }
    if (musicToggle) {
      const label = t('musicToggle');
      musicToggle.textContent = label;
      musicToggle.setAttribute('aria-label', label);
      musicToggle.setAttribute('title', label);
    }
    if (sfxToggle) {
      const label = t('sfxToggle');
      sfxToggle.textContent = label;
      sfxToggle.setAttribute('aria-label', label);
      sfxToggle.setAttribute('title', label);
    }
    if (fullscreenButton) {
      updateFullscreenButton();
    }
    if (nameInput) {
      nameInput.placeholder = t('namePlaceholder');
      nameInput.setAttribute('aria-label', t('nameAria'));
    }
    if (nameSave) {
      nameSave.textContent = t('nameSet');
      nameSave.setAttribute('aria-label', t('nameSetAria'));
    }
    if (chatInput) {
      chatInput.placeholder = t('chatPlaceholder');
    }
    if (buildStatus) {
      buildStatus.textContent = t('buildStatusSelect');
    }
    const buildLabels = {
      craft_basic_axe: t('buildOptionCraftAxe'),
      craft_basic_pick: t('buildOptionCraftPick'),
      craft_arrows: t('buildOptionCraftArrows'),
      hut_wood: t('buildOptionHut'),
      house_stone: t('buildOptionHouse'),
      bridge_wood: t('buildOptionBridgeWood'),
      bridge_stone: t('buildOptionBridgeStone'),
      path: t('buildOptionPath'),
      road: t('buildOptionRoad'),
      boat: t('buildOptionBoat'),
      demolish: t('buildOptionDemolish'),
    };
    buildButtons.forEach((button) => {
      const label = buildLabels[button.dataset.build];
      if (label) {
        button.textContent = label;
      }
    });
    actionButtons.forEach((button) => {
      const label = button.querySelector('.action-label');
      if (!label) return;
      switch (button.dataset.action) {
        case 'attack':
          label.textContent = t('actionAttack');
          break;
        case 'gather':
          label.textContent = t('actionGather');
          break;
        case 'interact':
          label.textContent = t('actionInteract');
          break;
        default:
          break;
      }
    });
  }

  function worldToPixels(x, y) {
    return {
      x: x * tileSize,
      y: y * tileSize,
    };
  }

  function tileToPixels(x, y, anchor) {
    return {
      x: (x + anchor.x) * tileSize,
      y: (y + anchor.y) * tileSize,
    };
  }

  function normalizeNameInput(value) {
    return value.trim().replace(/\s+/g, ' ').slice(0, MAX_NAME_CHARS);
  }

  function refreshNameStyle() {
    const fontSize = Math.max(12, Math.round(tileSize * 0.5));
    nameStyle = new PIXI.TextStyle({
      fontFamily: 'VT323',
      fontSize,
      fill: 0xe8f4ea,
      stroke: 0x0b0e14,
      strokeThickness: 3,
    });
    for (const entity of playerEntities.values()) {
      if (entity.label) {
        entity.label.style = nameStyle;
      }
    }
  }

  function syncLocalName(name) {
    if (!nameInput) return;
    if (pendingName && name !== pendingName) {
      return;
    }
    if (pendingName && name === pendingName) {
      pendingName = null;
    }
    lastKnownName = name || '';
    if (document.activeElement !== nameInput) {
      nameInput.value = lastKnownName;
    }
  }

  function ensurePlayerLabel(entity, name) {
    const safeName = name || 'Wanderer';
    if (!nameStyle) {
      refreshNameStyle();
    }
    if (!entity.label) {
      const label = new PIXI.Text(safeName, nameStyle);
      label.anchor.set(0.5, 0);
      overlayLayer.addChild(label);
      entity.label = label;
      entity.name = safeName;
      return;
    }
    if (entity.name !== safeName) {
      entity.name = safeName;
      entity.label.text = safeName;
    }
  }

  function submitNameChange() {
    if (!nameInput) return;
    const normalized = normalizeNameInput(nameInput.value);
    if (!normalized) {
      if (lastKnownName) {
        nameInput.value = lastKnownName;
      }
      return;
    }
    nameInput.value = normalized;
    if (normalized === lastKnownName) return;
    lastKnownName = normalized;
    pendingName = normalized;
    if (wsOpen) {
      sendMessage({ type: 'set_name', name: normalized });
    }
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

  function renderInventory(items) {
    if (!inventoryList) return;
    const previousScrollTop = inventoryList.scrollTop;
    const shouldStick = inventoryList.scrollTop + inventoryList.clientHeight >= inventoryList.scrollHeight - 8;
    while (inventoryList.firstChild) {
      inventoryList.removeChild(inventoryList.firstChild);
    }
    if (!items || items.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'empty';
      empty.textContent = t('inventoryEmpty');
      inventoryList.appendChild(empty);
      return;
    }
    items.forEach((item) => {
      const row = document.createElement('div');
      row.className = 'inventory-item';
      const nameEl = document.createElement('span');
      nameEl.textContent = item.name;
      const countEl = document.createElement('span');
      countEl.className = 'count';
      countEl.textContent = `x${item.count}`;
      row.appendChild(nameEl);
      row.appendChild(countEl);
      if (item.heal) {
        row.classList.add('is-usable');
        row.setAttribute('role', 'button');
        row.setAttribute('tabindex', '0');
        row.title = t('inventoryEat');
        row.addEventListener('click', () => {
          sendMessage({ type: 'use_item', id: item.id });
        });
      }
      inventoryList.appendChild(row);
    });
    if (shouldStick) {
      inventoryList.scrollTop = inventoryList.scrollHeight;
    } else {
      inventoryList.scrollTop = Math.min(previousScrollTop, inventoryList.scrollHeight);
    }
  }

  function setBuildStatus(text) {
    if (!buildStatus) return;
    buildStatus.textContent = text;
  }

  function isCraftKind(kind) {
    return craftKinds.has(kind);
  }

  function getPlayerTile() {
    if (!playerId) return null;
    const playerEntity = playerEntities.get(playerId);
    const px = playerEntity ? playerEntity.x : playerState?.x;
    const py = playerEntity ? playerEntity.y : playerState?.y;
    if (!Number.isFinite(px) || !Number.isFinite(py)) return null;
    return {
      x: Math.floor(px),
      y: Math.floor(py),
    };
  }

  function requestCraft(kind) {
    if (!wsOpen) {
      setBuildStatus(t('buildStatusNotConnected'));
      return;
    }
    const tile = getPlayerTile();
    if (!tile) {
      setBuildStatus(t('buildStatusNotConnected'));
      return;
    }
    sendMessage({ type: 'build', kind, x: tile.x, y: tile.y });
    setBuildStatus(t('buildStatusCraftRequested'));
  }

  function setBuildMode(mode) {
    buildMode = buildMode === mode ? null : mode;
    pendingDemolish = null;
    buildButtons.forEach((button) => {
      button.classList.toggle('active', button.dataset.build === buildMode);
    });
    if (!buildMode) {
      setBuildStatus(t('buildStatusSelect'));
      clearBuildPreview();
    } else if (buildMode === 'demolish') {
      setBuildStatus(t('buildStatusDemolish'));
      clearBuildPreview();
    } else if (isCraftKind(buildMode)) {
      setBuildStatus(t('buildStatusCraftRequested'));
      clearBuildPreview();
    } else {
      setBuildStatus(t('buildStatusPlace'));
      updateBuildPreview(lastPointerTile);
    }
  }

  function ensureBuildPreview(kind) {
    if (!kind || kind === 'demolish' || isCraftKind(kind)) return;
    ensureTextures();
    const textureKey = baseStructureKind(kind).replace(/_(h|v)$/, '');
    const texture = textures[textureKey];
    if (!texture) return;
    if (!buildPreviewSprite) {
      buildPreviewSprite = new PIXI.Sprite(texture);
      buildPreviewSprite.alpha = 0.45;
      buildPreviewSprite.zIndex = 10_000;
      overlayLayer.addChild(buildPreviewSprite);
    }
    if (buildPreviewKind !== kind) {
      buildPreviewSprite.texture = texture;
      buildPreviewKind = kind;
    }
    const isGround = groundStructureKinds.has(kind) || kind.startsWith('bridge_');
    const isBridge = kind.startsWith('bridge_');
    const footprint = structureFootprints.get(kind) || structureFootprints.get(baseStructureKind(kind));
    if (isGround || footprint) {
      if (isBridge) {
        buildPreviewSprite.anchor.set(0.5, 0.5);
      } else if (footprint) {
        buildPreviewSprite.anchor.set(0, 0);
      } else {
        buildPreviewSprite.anchor.set(0, 0);
      }
    } else {
      buildPreviewSprite.anchor.set(0.5, 0.9);
    }
  }

  function clearBuildPreview() {
    if (buildPreviewSprite) {
      buildPreviewSprite.visible = false;
    }
  }

  function updateBuildPreview(tile) {
    if (!tile || !buildMode || buildMode === 'demolish' || isCraftKind(buildMode)) {
      clearBuildPreview();
      return;
    }
    ensureBuildPreview(buildMode);
    if (!buildPreviewSprite) return;
    const isGround = groundStructureKinds.has(buildMode) || buildMode.startsWith('bridge_');
    const isBridge = buildMode.startsWith('bridge_');
    const footprint =
      structureFootprints.get(buildMode) || structureFootprints.get(baseStructureKind(buildMode));
    if (isGround || footprint) {
      if (isBridge) {
        buildPreviewSprite.x = (tile.x + 0.5) * tileSize;
        buildPreviewSprite.y = (tile.y + 0.5) * tileSize;
        buildPreviewSprite.rotation = 0;
      } else if (footprint) {
        buildPreviewSprite.x = tile.x * tileSize;
        buildPreviewSprite.y = tile.y * tileSize;
        buildPreviewSprite.rotation = 0;
        buildPreviewSprite.width = tileSize * footprint.width;
        buildPreviewSprite.height = tileSize * footprint.height;
      } else {
        buildPreviewSprite.x = tile.x * tileSize;
        buildPreviewSprite.y = tile.y * tileSize;
        buildPreviewSprite.rotation = 0;
      }
      if (footprint) {
        buildPreviewSprite.zIndex = (tile.y + footprint.height) * tileSize + 10;
      } else {
        buildPreviewSprite.zIndex = tile.y * tileSize + 10;
      }
    } else {
      const basePos = tileToPixels(tile.x, tile.y, PLAYER_ANCHOR);
      buildPreviewSprite.x = basePos.x;
      buildPreviewSprite.y = basePos.y;
      buildPreviewSprite.zIndex = basePos.y + 10;
    }
    buildPreviewSprite.visible = true;
  }

  function screenToTile(event) {
    const worldX = (event.clientX - world.x) / tileSize;
    const worldY = (event.clientY - world.y) / tileSize;
    return {
      x: Math.floor(worldX),
      y: Math.floor(worldY),
    };
  }

  function handlePointerPreview(event) {
    if (!buildMode || buildMode === 'demolish') return;
    lastPointerTile = screenToTile(event);
    updateBuildPreview(lastPointerTile);
  }

  function handleBuildClick(event) {
    if (!buildMode) return false;
    if (!wsOpen) {
      setBuildStatus(t('buildStatusNotConnected'));
      return true;
    }
    if (isCraftKind(buildMode)) {
      requestCraft(buildMode);
      return true;
    }
    const tile = screenToTile(event);
    if (buildMode === 'demolish') {
      const now = performance.now();
      if (
        !pendingDemolish ||
        pendingDemolish.x !== tile.x ||
        pendingDemolish.y !== tile.y ||
        now > pendingDemolish.expires
      ) {
        pendingDemolish = {
          x: tile.x,
          y: tile.y,
          expires: now + 1200,
        };
        setBuildStatus(t('buildStatusDemolishConfirm'));
        return true;
      }
      sendMessage({ type: 'demolish', x: tile.x, y: tile.y });
      pendingDemolish = null;
      setBuildStatus(t('buildStatusDemolishRequested'));
      return true;
    }
    sendMessage({ type: 'build', kind: buildMode, x: tile.x, y: tile.y });
    setBuildStatus(t('buildStatusPlacementRequested'));
    return true;
  }

  function setupPanelControls(panelId) {
    const panel = document.getElementById(panelId);
    if (!panel) return;
    const state = { collapsed: panel.classList.contains('collapsed') };
    panelButtons
      .filter((button) => button.dataset.panel === panelId)
      .forEach((button) => {
        button.addEventListener('click', () => {
          const action = button.dataset.action;
          if (action === 'toggle') {
            state.collapsed = !state.collapsed;
            panel.classList.toggle('collapsed', state.collapsed);
            if (panelId === 'inventory') {
              updateBuildMenuPosition();
            }
            return;
          }
        });
      });
  }

  function updateBuildMenuPosition() {
    if (!inventoryPanel || !buildMenu || !hudEl) return;
    if (!Number.isFinite(buildMenuBaseTop)) return;
    const inventoryRect = inventoryPanel.getBoundingClientRect();
    const hudRect = hudEl.getBoundingClientRect();
    const gap = 12;
    const nextTop = Math.round(inventoryRect.bottom - hudRect.top + gap);
    buildMenu.style.top = `${nextTop}px`;
  }

  function setUiScale(value) {
    uiScale = Math.max(0.7, Math.min(1.3, value));
    document.getElementById('hud')?.style.setProperty('--ui-scale', uiScale);
    if (uiScaleValue) {
      uiScaleValue.textContent = `${Math.round(uiScale * 100)}%`;
    }
    updateBuildMenuPosition();
    try {
      localStorage.setItem('ui-scale', uiScale.toString());
    } catch (err) {
      console.warn('UI scale save failed', err);
    }
  }

  if (uiScaleDown) {
    uiScaleDown.addEventListener('click', () => {
      setUiScale(+(uiScale - 0.1).toFixed(2));
    });
  }

  if (uiScaleUp) {
    uiScaleUp.addEventListener('click', () => {
      setUiScale(+(uiScale + 0.1).toFixed(2));
    });
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
      indicator.container.zIndex = entity.sprite.y + tileSize;
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
    const label = active ? t('fullscreenExit') : t('fullscreenEnter');
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
    playSfxForAction(action);
  }

  function playSfxForAction(action) {
    if (!audioSettings.sfxEnabled) return;
    switch (action) {
      case 'attack':
        playSfx('assets/sfx/attack.ogg', { volume: 0.6 }).catch(() => {});
        break;
      case 'gather':
        playSfx('assets/sfx/gather.ogg', { volume: 0.5 }).catch(() => {});
        break;
      case 'interact':
        playSfx('assets/sfx/interact.ogg', { volume: 0.5 }).catch(() => {});
        break;
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
    const playerPos = worldToPixels(playerEntity.x, playerEntity.y);
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

  function tileKey(x, y) {
    return `${x},${y}`;
  }

  function chunkKeyForTile(x, y) {
    const cx = Math.floor(x / chunkSize);
    const cy = Math.floor(y / chunkSize);
    return chunkKey(cx, cy);
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
    for (let dx = -CHUNK_REQUEST_RADIUS; dx <= CHUNK_REQUEST_RADIUS; dx += 1) {
      for (let dy = -CHUNK_REQUEST_RADIUS; dy <= CHUNK_REQUEST_RADIUS; dy += 1) {
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
    if (!shouldKeepChunk(chunk.chunk_x, chunk.chunk_y)) {
      if (loadedChunks.has(key)) {
        unloadChunk(key);
      }
      return;
    }
    const existing = chunkTiles.get(key);
    const container = existing?.container || createTilemapLayer();
    if (typeof container.clear === 'function') {
      container.clear();
    } else if (typeof container.removeChildren === 'function') {
      container.removeChildren();
    }
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
    if (!existing) {
      tileLayer.addChild(container);
      chunkTiles.set(key, {
        container,
        bounds: {
          x: chunkX,
          y: chunkY,
          width: chunkSize * tileSize,
          height: chunkSize * tileSize,
        },
        tiles: chunk.tiles,
      });
    } else {
      existing.tiles = chunk.tiles;
    }
    loadedChunks.add(key);
    replaceChunkResources(key, chunk.resources);
    replaceChunkStructures(key, chunk.structures);
  }

  function shouldKeepChunk(cx, cy) {
    const playerEntity = playerEntities.get(playerId);
    const px = playerEntity ? playerEntity.x : playerState?.x;
    const py = playerEntity ? playerEntity.y : playerState?.y;
    if (px == null || py == null) return true;
    const centerX = Math.floor(px / chunkSize);
    const centerY = Math.floor(py / chunkSize);
    return Math.abs(cx - centerX) <= CHUNK_KEEP_RADIUS && Math.abs(cy - centerY) <= CHUNK_KEEP_RADIUS;
  }

  function unloadChunk(key) {
    const entry = chunkTiles.get(key);
    if (entry) {
      if (entry.container.parent) {
        entry.container.parent.removeChild(entry.container);
      }
      if (typeof entry.container.destroy === 'function') {
        entry.container.destroy({ children: true });
      }
      chunkTiles.delete(key);
    }
    loadedChunks.delete(key);
    pendingChunks.delete(key);
    const resources = chunkResources.get(key);
    if (resources) {
      for (const id of resources) {
        removeResource(id);
      }
      chunkResources.delete(key);
    }
    const structures = chunkStructures.get(key);
    if (structures) {
      for (const structure of structures.values()) {
        removeStructure(structure);
      }
      chunkStructures.delete(key);
    }
  }

  function replaceChunkResources(key, resources) {
    const next = new Set();
    for (const resource of resources) {
      next.add(resource.id);
      upsertResource(resource);
    }
    const existing = chunkResources.get(key);
    if (existing) {
      for (const id of existing) {
        if (!next.has(id)) {
          removeResource(id);
        }
      }
    }
    if (next.size > 0) {
      chunkResources.set(key, next);
    } else {
      chunkResources.delete(key);
    }
  }

  function replaceChunkStructures(key, structures) {
    const next = new Map();
    for (const structure of structures) {
      const sKey = structureKey(structure);
      next.set(sKey, structure);
      upsertStructure(structure);
    }
    const existing = chunkStructures.get(key);
    if (existing) {
      for (const [sKey, structure] of existing) {
        if (!next.has(sKey)) {
          removeStructure(structure);
        }
      }
    }
    if (next.size > 0) {
      chunkStructures.set(key, next);
    } else {
      chunkStructures.delete(key);
    }
  }

  function addResourceToChunk(key, id) {
    let set = chunkResources.get(key);
    if (!set) {
      set = new Set();
      chunkResources.set(key, set);
    }
    set.add(id);
  }

  function removeResourceFromChunk(key, id) {
    const set = chunkResources.get(key);
    if (!set) return;
    set.delete(id);
    if (!set.size) {
      chunkResources.delete(key);
    }
  }

  function addStructureToChunk(key, structure) {
    let map = chunkStructures.get(key);
    if (!map) {
      map = new Map();
      chunkStructures.set(key, map);
    }
    map.set(structureKey(structure), structure);
  }

  function removeStructureFromChunk(key, structure) {
    const map = chunkStructures.get(key);
    if (!map) return;
    map.delete(structureKey(structure));
    if (!map.size) {
      chunkStructures.delete(key);
    }
  }

  function pruneChunksAround() {
    const playerEntity = playerEntities.get(playerId);
    const px = playerEntity ? playerEntity.x : playerState?.x;
    const py = playerEntity ? playerEntity.y : playerState?.y;
    if (px == null || py == null) return;
    const cx = Math.floor(px / chunkSize);
    const cy = Math.floor(py / chunkSize);
    for (const key of Array.from(loadedChunks)) {
      const [x, y] = key.split(',').map(Number);
      if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
      if (Math.abs(x - cx) > CHUNK_KEEP_RADIUS || Math.abs(y - cy) > CHUNK_KEEP_RADIUS) {
        unloadChunk(key);
      }
    }
    for (const key of Array.from(pendingChunks)) {
      const [x, y] = key.split(',').map(Number);
      if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
      if (Math.abs(x - cx) > CHUNK_KEEP_RADIUS || Math.abs(y - cy) > CHUNK_KEEP_RADIUS) {
        pendingChunks.delete(key);
      }
    }
  }

  function getTileIdAt(tileX, tileY) {
    const cx = Math.floor(tileX / chunkSize);
    const cy = Math.floor(tileY / chunkSize);
    const key = chunkKey(cx, cy);
    const chunk = chunkTiles.get(key);
    if (!chunk || !chunk.tiles) return null;
    const localX = tileX - cx * chunkSize;
    const localY = tileY - cy * chunkSize;
    if (localX < 0 || localY < 0 || localX >= chunkSize || localY >= chunkSize) {
      return null;
    }
    return chunk.tiles[localY * chunkSize + localX];
  }

  function canWalkLocal(x, y) {
    const tileX = Math.floor(x);
    const tileY = Math.floor(y);
    const structureKind = structureTiles.get(tileKey(tileX, tileY));
    if (structureKind) {
      if (structureKind.startsWith('bridge_')) {
        return true;
      }
      if (blockingStructureKinds.has(structureKind)) {
        return false;
      }
    }
    const tileId = getTileIdAt(tileX, tileY);
    if (tileId == null) return true;
    if (tileId === TILE_WATER) {
      return localInBoat;
    }
    return true;
  }

  function upsertResource(resource) {
    if (resource.hp <= 0) {
      removeResource(resource.id);
      return;
    }
    let entry = resourceSprites.get(resource.id);
    if (!entry) {
      const texture = resourceTextureFor(resource.kind, resource.size);
      const sprite = new PIXI.Sprite(texture);
      sprite.anchor.set(0.5, 1);
      entityLayer.addChild(sprite);
      entry = { sprite, x: resource.x, y: resource.y };
      resourceSprites.set(resource.id, entry);
    }
    entry.x = resource.x;
    entry.y = resource.y;
    const texture = resourceTextureFor(resource.kind, resource.size);
    if (texture && entry.sprite.texture !== texture) {
      entry.sprite.texture = texture;
    }
    entry.sprite.scale.set(1, 1);
    const basePos = tileToPixels(resource.x, resource.y, RESOURCE_ANCHOR);
    entry.sprite.x = basePos.x;
    entry.sprite.y = basePos.y;
    entry.sprite.zIndex = basePos.y;
  }

  function removeResource(id) {
    const entry = resourceSprites.get(id);
    if (entry) {
      if (entry.sprite.parent) {
        entry.sprite.parent.removeChild(entry.sprite);
      }
      entry.sprite.destroy();
      resourceSprites.delete(id);
    }
  }

  const groundStructureKinds = new Set([
    'path',
    'road',
    'bridge_wood',
    'bridge_wood_h',
    'bridge_wood_v',
    'bridge_stone',
    'bridge_stone_h',
    'bridge_stone_v',
    'hut_wood_root',
    'house_stone_root',
  ]);
  const renderlessStructureKinds = new Set([
    'hut_wood_fill',
    'house_stone_fill',
    'hut_wood_block',
    'hut_wood_top',
    'house_stone_block',
    'house_stone_top',
  ]);
  const blockingStructureKinds = new Set([
    'hut_wood',
    'hut_wood_root',
    'hut_wood_block',
    'house_stone',
    'house_stone_root',
    'house_stone_block',
  ]);
  const structureFootprints = new Map([
    ['hut_wood_root', { width: 2, height: 2 }],
    ['house_stone_root', { width: 3, height: 3 }],
  ]);

  function baseStructureKind(kind) {
    return kind.replace(/_(root|block|top|fill)$/, '');
  }

  function structureKey(structure) {
    return `${structure.id}:${structure.x}:${structure.y}`;
  }

  function recordStructureTile(structure) {
    structureTiles.set(tileKey(structure.x, structure.y), structure.kind);
  }

  function removeStructureTile(structure) {
    structureTiles.delete(tileKey(structure.x, structure.y));
  }

  function upsertStructure(structure) {
    const key = structureKey(structure);
    recordStructureTile(structure);
    if (renderlessStructureKinds.has(structure.kind)) {
      return;
    }
    const baseKind = baseStructureKind(structure.kind);
    let entry = structureSprites.get(key);
    if (!entry) {
      const textureKey = baseKind.replace(/_(h|v)$/, '');
      const texture = textures[textureKey];
      if (!texture) return;
      const sprite = new PIXI.Sprite(texture);
      const isGround = groundStructureKinds.has(structure.kind) || groundStructureKinds.has(baseKind);
      const isBridge = structure.kind.startsWith('bridge_');
      const footprint =
        structureFootprints.get(structure.kind) || structureFootprints.get(baseKind);
      if (isGround || footprint) {
        if (isBridge) {
          sprite.anchor.set(0.5, 0.5);
        } else if (footprint) {
          sprite.anchor.set(0, 0);
        } else {
          sprite.anchor.set(0, 0);
        }
        structureLayer.addChild(sprite);
      } else {
        sprite.anchor.set(0.5, 0.9);
        entityLayer.addChild(sprite);
      }
      entry = {
        sprite,
        isGround,
        isBridge,
        id: structure.id,
        kind: structure.kind,
        tileX: structure.x,
        tileY: structure.y,
      };
      if (structure.kind === 'boat') {
        entry.sprite.anchor.set(0.5, 0.5);
      }
      structureSprites.set(key, entry);
    }
    entry.id = structure.id;
    entry.kind = structure.kind;
    entry.tileX = structure.x;
    entry.tileY = structure.y;
    entry.sprite.scale.set(1, 1);
    const footprint =
      structureFootprints.get(structure.kind) || structureFootprints.get(baseKind);
    if (entry.isGround || footprint) {
      if (entry.isBridge) {
        entry.sprite.x = (structure.x + 0.5) * tileSize;
        entry.sprite.y = (structure.y + 0.5) * tileSize;
        entry.sprite.rotation = structure.kind.endsWith('_v') ? Math.PI / 2 : 0;
      } else if (footprint) {
        entry.sprite.x = structure.x * tileSize;
        entry.sprite.y = structure.y * tileSize;
        entry.sprite.rotation = 0;
        entry.sprite.width = tileSize * footprint.width;
        entry.sprite.height = tileSize * footprint.height;
      } else {
        entry.sprite.x = structure.x * tileSize;
        entry.sprite.y = structure.y * tileSize;
        entry.sprite.rotation = 0;
      }
      if (footprint) {
        entry.sprite.zIndex = (structure.y + footprint.height) * tileSize;
      } else {
        entry.sprite.zIndex = structure.y * tileSize;
      }
    } else {
      const basePos = tileToPixels(structure.x, structure.y, PLAYER_ANCHOR);
      entry.sprite.x = basePos.x;
      entry.sprite.y = basePos.y;
      entry.sprite.zIndex = basePos.y;
    }
  }

  function removeStructure(structure) {
    const key = structureKey(structure);
    removeStructureTile(structure);
    if (renderlessStructureKinds.has(structure.kind)) {
      return;
    }
    const entry = structureSprites.get(key);
    if (!entry) return;
    if (entry.sprite.parent) {
      entry.sprite.parent.removeChild(entry.sprite);
    }
    entry.sprite.destroy();
    structureSprites.delete(key);
  }

  function syncStructures(structures) {
    if (!structures) return;
    structures.forEach((structure) => upsertStructure(structure));
  }

  function syncPlayers(players, clearMissing = true) {
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
          inBoat: Boolean(player.in_boat),
          boatId: player.boat_id ?? null,
        });
        playerEntities.set(player.id, entity);
      }
      if (player.id === playerId) {
        const prevInBoat = localInBoat;
        playerState = player;
        localInBoat = Boolean(player.in_boat);
        entity.inBoat = localInBoat;
        entity.boatId = player.boat_id ?? null;
        syncLocalName(player.name);
        if (!localPrediction) {
          localPrediction = { x: player.x, y: player.y };
        }
        if (prevInBoat !== localInBoat && localPrediction) {
          localPrediction.x = player.x;
          localPrediction.y = player.y;
          localRenderOffset.x = 0;
          localRenderOffset.y = 0;
        }
        if (player.last_input_seq != null) {
          reconcileLocalPlayer(entity, player, now);
        } else {
          localPrediction.x = player.x;
          localPrediction.y = player.y;
        }
      } else if (entity) {
        entity.inBoat = Boolean(player.in_boat);
        entity.boatId = player.boat_id ?? null;
        updateEntityTarget(entity, player.x, player.y, now);
      }
      ensurePlayerLabel(entity, player.name);
      entity.hp = player.hp;
    });

    if (clearMissing) {
      for (const [id, entity] of playerEntities.entries()) {
        if (!seen.has(id)) {
          removeEntity(entity);
          playerEntities.delete(id);
          removeTypingIndicator(id);
        }
      }
    }
  }

  function syncMonsters(monsters, clearMissing = true) {
    const now = performance.now();
    const seen = new Set();
    monsters.forEach((monster) => {
      seen.add(monster.id);
      let entity = monsterEntities.get(monster.id);
      const texture = textures[monster.kind] || textures.slime;
      if (!entity) {
        const sprite = new PIXI.Sprite(texture);
        sprite.anchor.set(0.5, 0.9);
        entityLayer.addChild(sprite);
        entity = createEntityState(sprite, monster.x, monster.y, now, { kind: monster.kind });
        monsterEntities.set(monster.id, entity);
      } else {
        updateEntityTarget(entity, monster.x, monster.y, now);
        entity.kind = monster.kind;
        if (entity.sprite.texture !== texture) {
          entity.sprite.texture = texture;
        }
      }
    });

    if (clearMissing) {
      for (const [id, entity] of monsterEntities.entries()) {
        if (!seen.has(id)) {
          removeEntity(entity);
          monsterEntities.delete(id);
        }
      }
    }
  }

  function syncBoats(boats, clearMissing = true) {
    const seen = new Set();
    boats.forEach((boat) => {
      seen.add(boat.id);
      let entry = boatEntities.get(boat.id);
      if (!entry) {
        const sprite = new PIXI.Sprite(textures.boat);
        sprite.anchor.set(0.5, 0.5);
        entityLayer.addChild(sprite);
        entry = { sprite, x: boat.x, y: boat.y };
        boatEntities.set(boat.id, entry);
      }
      entry.x = boat.x;
      entry.y = boat.y;
      const basePos = worldToPixels(boat.x + 0.18, boat.y - 0.2);
      entry.sprite.x = basePos.x;
      entry.sprite.y = basePos.y;
      entry.sprite.zIndex = basePos.y + tileSize * 0.35;
    });
    if (clearMissing) {
      for (const [id, entry] of boatEntities.entries()) {
        if (!seen.has(id)) {
          if (entry.sprite.parent) {
            entry.sprite.parent.removeChild(entry.sprite);
          }
          entry.sprite.destroy();
          boatEntities.delete(id);
        }
      }
    }
  }

  function removeBoats(ids) {
    ids.forEach((id) => {
      const entry = boatEntities.get(id);
      if (!entry) return;
      if (entry.sprite.parent) {
        entry.sprite.parent.removeChild(entry.sprite);
      }
      entry.sprite.destroy();
      boatEntities.delete(id);
    });
  }

  function syncProjectiles(projectiles, clearMissing = true) {
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

    if (clearMissing) {
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
  }

  function removePlayers(ids) {
    if (!ids) return;
    ids.forEach((id) => {
      const entity = playerEntities.get(id);
      if (!entity) return;
      removeEntity(entity);
      playerEntities.delete(id);
      removeTypingIndicator(id);
    });
  }

  function removeMonsters(ids) {
    if (!ids) return;
    ids.forEach((id) => {
      const entity = monsterEntities.get(id);
      if (!entity) return;
      removeEntity(entity);
      monsterEntities.delete(id);
    });
  }

  function removeProjectiles(ids) {
    if (!ids) return;
    ids.forEach((id) => {
      const sprite = projectileSprites.get(id);
      if (!sprite) return;
      if (sprite.parent) {
        sprite.parent.removeChild(sprite);
      }
      sprite.destroy();
      projectileSprites.delete(id);
    });
  }

  function addNpc(npc) {
    if (npcSprites.has(npc.id)) return;
    const sprite = new PIXI.Sprite(textures.npc);
    sprite.anchor.set(0.5, 0.9);
    const basePos = worldToPixels(npc.x, npc.y);
    sprite.x = basePos.x;
    sprite.y = basePos.y;
    entityLayer.addChild(sprite);
    sprite.zIndex = basePos.y;
    npcSprites.set(npc.id, { sprite, x: npc.x, y: npc.y });
  }

  function addLandmark(id, textureKey, tileX, tileY, anchor = PLAYER_ANCHOR) {
    if (landmarkSprites.has(id)) return;
    const texture = textures[textureKey];
    if (!texture) return;
    const sprite = new PIXI.Sprite(texture);
    sprite.anchor.set(anchor.x, anchor.y);
    const basePos = tileToPixels(tileX, tileY, anchor);
    sprite.x = basePos.x;
    sprite.y = basePos.y;
    sprite.zIndex = basePos.y;
    entityLayer.addChild(sprite);
    landmarkSprites.set(id, sprite);
  }

  function addCampfireAndTent(world) {
    if (!world) return;
    const baseX = Math.round(world.spawn_x);
    const baseY = Math.round(world.spawn_y);
    addLandmark('campfire', 'campfire', baseX, baseY, PLAYER_ANCHOR);
    addLandmark('tent', 'tent', baseX + 1, baseY, PLAYER_ANCHOR);
  }

  function updateCamera() {
    const playerEntity = playerEntities.get(playerId);
    if (!playerEntity) return;
    const playerPos = worldToPixels(playerEntity.x, playerEntity.y);
    const targetX = app.renderer.width / 2 - playerPos.x;
    const targetY = app.renderer.height / 2 - playerPos.y;
    world.x = Math.round(targetX);
    world.y = Math.round(targetY);
    updateChunkVisibility();
  }

  function chunkKey(x, y) {
    return `${x},${y}`;
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
    if (!ctx) return {};
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

    textures.tree1 = PIXI.Texture.from('assets/entities/tree-1.svg');
    textures.tree2 = PIXI.Texture.from('assets/entities/tree-2.svg');
    textures.tree3 = PIXI.Texture.from('assets/entities/tree-3.svg');
    textures.apple_tree1 = PIXI.Texture.from('assets/entities/tree-apple-1.svg');
    textures.apple_tree2 = PIXI.Texture.from('assets/entities/tree-apple-2.svg');
    textures.apple_tree3 = PIXI.Texture.from('assets/entities/tree-apple-3.svg');
    textures.pine_tree1 = PIXI.Texture.from('assets/entities/tree-pine-1.svg');
    textures.pine_tree2 = PIXI.Texture.from('assets/entities/tree-pine-2.svg');
    textures.pine_tree3 = PIXI.Texture.from('assets/entities/tree-pine-3.svg');
    textures.palm_tree1 = PIXI.Texture.from('assets/entities/palm-1.svg');
    textures.palm_tree2 = PIXI.Texture.from('assets/entities/palm-2.svg');
    textures.palm_tree3 = PIXI.Texture.from('assets/entities/palm-3.svg');
    textures.rock = PIXI.Texture.from('assets/entities/rock.svg');
    textures.rockSmall = PIXI.Texture.from('assets/entities/rock-small.svg');
    textures.rockMedium = PIXI.Texture.from('assets/entities/rock-medium.svg');
    textures.rockLarge = PIXI.Texture.from('assets/entities/rock-large.svg');
    textures.hut_wood = PIXI.Texture.from('assets/entities/hut-wood.svg');
    textures.house_stone = PIXI.Texture.from('assets/entities/house-stone.svg');
    textures.bridge_wood = PIXI.Texture.from('assets/entities/bridge-wood.svg');
    textures.bridge_stone = PIXI.Texture.from('assets/entities/bridge-stone.svg');
    textures.path = PIXI.Texture.from('assets/entities/path.svg');
    textures.road = PIXI.Texture.from('assets/entities/road.svg');
    textures.boat = PIXI.Texture.from('assets/entities/boat.svg');
    textures.tent = PIXI.Texture.from('assets/entities/tent.svg');
    textures.campfire = PIXI.Texture.from('assets/entities/campfire.svg');
    textures.playerFront = PIXI.Texture.from('assets/entities/player.svg');
    textures.playerBack = PIXI.Texture.from('assets/entities/player-back.svg');
    textures.playerSide = PIXI.Texture.from('assets/entities/player-side.svg');
    textures.playerAltFront = PIXI.Texture.from('assets/entities/player-alt.svg');
    textures.playerAltBack = PIXI.Texture.from('assets/entities/player-alt-back.svg');
    textures.playerAltSide = PIXI.Texture.from('assets/entities/player-alt-side.svg');
    textures.npc = PIXI.Texture.from('assets/entities/npc.svg');
    textures.slime = PIXI.Texture.from('assets/entities/slime.svg');
    textures.boar = PIXI.Texture.from('assets/entities/boar.svg');
    textures.rabbit = PIXI.Texture.from('assets/entities/rabbit.svg');
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

  function isTextInputFocused() {
    return document.activeElement === chatInput || document.activeElement === nameInput;
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
      inBoat: options.inBoat ?? false,
      boatId: options.boatId ?? null,
      walkOffset: Math.random() * Math.PI * 2,
      label: options.label ?? null,
      name: options.name ?? null,
      kind: options.kind ?? null,
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

  function normalizeDirection(dirX, dirY) {
    const length = Math.hypot(dirX, dirY);
    if (length < 0.01) {
      return { x: 0, y: 0, length: 0 };
    }
    return { x: dirX / length, y: dirY / length, length };
  }

  function applyPredictionStep(position, dirX, dirY, dt) {
    const norm = normalizeDirection(dirX, dirY);
    if (!norm.length) return;
    const nextX = position.x + norm.x * PLAYER_SPEED * dt;
    const nextY = position.y + norm.y * PLAYER_SPEED * dt;
    if (canWalkLocal(nextX, position.y)) {
      position.x = nextX;
    }
    if (canWalkLocal(position.x, nextY)) {
      position.y = nextY;
    }
  }

  function reconcileLocalPlayer(entity, player, now) {
    if (!localPrediction) {
      localPrediction = { x: player.x, y: player.y };
      localRenderOffset = { x: 0, y: 0 };
    }
    const renderX = localPrediction.x + localRenderOffset.x;
    const renderY = localPrediction.y + localRenderOffset.y;
    if (player.last_input_seq == null) {
      localPrediction.x = player.x;
      localPrediction.y = player.y;
      localRenderOffset.x = renderX - localPrediction.x;
      localRenderOffset.y = renderY - localPrediction.y;
      return;
    }
    const ack = player.last_input_seq;
    if (ack < lastAckSeq) {
      return;
    }
    lastAckSeq = ack;
    while (pendingInputs.length && pendingInputs[0].seq <= ack) {
      pendingInputs.shift();
    }
    const serverPrediction = { x: player.x, y: player.y };
    const dt = INPUT_SEND_INTERVAL_MS / 1000;
    for (const input of pendingInputs) {
      applyPredictionStep(serverPrediction, input.dirX, input.dirY, dt);
    }
    const dx = serverPrediction.x - localPrediction.x;
    const dy = serverPrediction.y - localPrediction.y;
    const distance = Math.hypot(dx, dy);
    if (distance > CORRECTION_DISTANCE) {
      localPrediction.x = serverPrediction.x;
      localPrediction.y = serverPrediction.y;
      localRenderOffset.x = renderX - localPrediction.x;
      localRenderOffset.y = renderY - localPrediction.y;
    }
  }

  function updateLocalPrediction(dt) {
    if (!wsOpen || !playerId || !localPrediction) return;
    applyPredictionStep(localPrediction, lastInputDir.x, lastInputDir.y, dt);
    const decay = Math.min(1, dt * 10);
    localRenderOffset.x = lerp(localRenderOffset.x, 0, decay);
    localRenderOffset.y = lerp(localRenderOffset.y, 0, decay);
  }

  function removeEntity(entity) {
    if (entity.sprite.parent) {
      entity.sprite.parent.removeChild(entity.sprite);
    }
    entity.sprite.destroy();
    if (entity.label) {
      if (entity.label.parent) {
        entity.label.parent.removeChild(entity.label);
      }
      entity.label.destroy();
    }
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

  function applyWalkAnimation(entity, now, baseY, moveDistanceOverride = null) {
    const moveDistance =
      moveDistanceOverride == null
        ? Math.hypot(entity.targetX - entity.startX, entity.targetY - entity.startY)
        : moveDistanceOverride;
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

  function applyRabbitHopAnimation(entity, now, baseY) {
    const moveDistance = Math.hypot(entity.targetX - entity.startX, entity.targetY - entity.startY);
    const sprite = entity.sprite;
    if (moveDistance > 0.01) {
      const phase = now / 120 + entity.walkOffset;
      const hop = Math.max(0, Math.sin(phase));
      const lift = hop * tileSize * 0.12;
      sprite.scale.x = 1 - hop * 0.06;
      sprite.scale.y = 1 + hop * 0.08;
      sprite.rotation = 0;
      sprite.skew.x = 0;
      sprite.y = baseY - lift;
    } else {
      sprite.scale.x = 1;
      sprite.scale.y = 1;
      sprite.rotation = 0;
      sprite.skew.x = 0;
      sprite.y = baseY;
    }
  }

  function setActionAvailability(action, available) {
    const button = actionButtonsByAction.get(action);
    if (!button) return;
    button.classList.toggle('is-disabled', !available);
    button.setAttribute('aria-disabled', String(!available));
  }

  function updateActionAvailability() {
    const playerEntity = playerEntities.get(playerId);
    if (!playerEntity) {
      setActionAvailability('gather', false);
      setActionAvailability('interact', false);
      return;
    }
    const px = playerEntity.x;
    const py = playerEntity.y;

    let canGather = false;
    for (const entry of resourceSprites.values()) {
      const dx = entry.x + 0.5 - px;
      const dy = entry.y + 0.5 - py;
      if (Math.hypot(dx, dy) <= GATHER_RANGE) {
        canGather = true;
        break;
      }
    }
    if (!canGather) {
      const tileId = getTileIdAt(Math.floor(px), Math.floor(py));
      if (tileId === TILE_WATER) {
        canGather = true;
      }
    }

    let canInteract = false;
    for (const entry of npcSprites.values()) {
      const dx = entry.x - px;
      const dy = entry.y - py;
      if (Math.hypot(dx, dy) <= INTERACT_RANGE) {
        canInteract = true;
        break;
      }
    }
    if (!canInteract) {
      for (const entry of boatEntities.values()) {
        const dx = entry.x - px;
        const dy = entry.y - py;
        if (Math.hypot(dx, dy) <= INTERACT_RANGE) {
          canInteract = true;
          break;
        }
      }
    }

    setActionAvailability('gather', canGather);
    setActionAvailability('interact', canInteract);
  }

  function updateEntities(now) {
    const alpha = (startTime) => Math.min(1, (now - startTime) / INTERP_MS);
    const localEntity = playerId ? playerEntities.get(playerId) : null;

    for (const entity of playerEntities.values()) {
      const isLocal = localEntity && entity === localEntity && localPrediction;
      if (isLocal) {
        const renderX = localPrediction.x + localRenderOffset.x;
        const renderY = localPrediction.y + localRenderOffset.y;
        const prevX = entity.x;
        const prevY = entity.y;
        entity.x = renderX;
        entity.y = renderY;
        const dx = entity.x - prevX;
        const dy = entity.y - prevY;
        if (entity.inBoat) {
          entity.facing = 'down';
        } else if (dx !== 0 || dy !== 0) {
          entity.facing = getFacingFromDelta(dx, dy, entity.facing);
        }
        applyPlayerFacing(entity);
        const basePos = worldToPixels(entity.x, entity.y);
        entity.sprite.x = basePos.x;
        const baseY = basePos.y;
        const moveDistance = Math.hypot(dx, dy);
        if (entity.inBoat) {
          entity.sprite.rotation = 0;
          entity.sprite.skew.x = 0;
          entity.sprite.scale.x = 1;
          entity.sprite.scale.y = 1;
          entity.sprite.y = baseY;
          entity.sprite.zIndex = baseY - tileSize * 0.25;
        } else {
          applyWalkAnimation(entity, now, baseY, moveDistance);
          entity.sprite.zIndex = baseY;
        }
        if (entity.label) {
          entity.label.x = basePos.x;
          entity.label.y = baseY + tileSize * 0.2;
          entity.label.zIndex = baseY + tileSize * 0.2;
        }
        continue;
      }
      const t = alpha(entity.startTime);
      entity.x = lerp(entity.startX, entity.targetX, t);
      entity.y = lerp(entity.startY, entity.targetY, t);
      if (entity.inBoat) {
        entity.facing = 'down';
      }
      applyPlayerFacing(entity);
      const basePos = worldToPixels(entity.x, entity.y);
      entity.sprite.x = basePos.x;
      const baseY = basePos.y;
      if (entity.inBoat) {
        entity.sprite.rotation = 0;
        entity.sprite.skew.x = 0;
        entity.sprite.scale.x = 1;
        entity.sprite.scale.y = 1;
        entity.sprite.y = baseY;
        entity.sprite.zIndex = baseY - tileSize * 0.25;
      } else {
        applyWalkAnimation(entity, now, baseY);
        entity.sprite.zIndex = baseY;
      }
      if (entity.label) {
        entity.label.x = basePos.x;
        entity.label.y = baseY + tileSize * 0.2;
        entity.label.zIndex = baseY + tileSize * 0.2;
      }
    }

    for (const entity of playerEntities.values()) {
      if (!entity.inBoat || entity.boatId == null) {
        continue;
      }
      const boatEntry = boatEntities.get(entity.boatId);
      if (!boatEntry) {
        continue;
      }
      const boatX = entity.x + 0.18;
      const boatY = entity.y - 0.2;
      const basePos = worldToPixels(boatX, boatY);
      boatEntry.sprite.x = basePos.x;
      boatEntry.sprite.y = basePos.y;
      boatEntry.sprite.zIndex = basePos.y + tileSize * 0.35;
    }

    for (const entity of monsterEntities.values()) {
      const t = alpha(entity.startTime);
      entity.x = lerp(entity.startX, entity.targetX, t);
      entity.y = lerp(entity.startY, entity.targetY, t);
      const basePos = worldToPixels(entity.x, entity.y);
      entity.sprite.x = basePos.x;
      const baseY = basePos.y;
      if (entity.kind === 'rabbit') {
        applyRabbitHopAnimation(entity, now, baseY);
      } else {
        entity.sprite.y = baseY;
      }
      entity.sprite.zIndex = baseY;
    }

    structureLayer.sortChildren();
    entityLayer.sortChildren();
  }

  function connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
    ws = new WebSocket(`${protocol}://${window.location.host}/ws`);

    ws.addEventListener('open', () => {
      wsOpen = true;
      setStatusText(t('statusConnected'));
      sendMessage({ type: 'locale', language });
      if (pendingName) {
        sendMessage({ type: 'set_name', name: pendingName });
      }
    });

    ws.addEventListener('message', (event) => {
      const msg = JSON.parse(event.data);
      switch (msg.type) {
        case 'welcome': {
          playerId = msg.player.id;
          playerState = msg.player;
          localInBoat = Boolean(msg.player.in_boat);
          localPrediction = { x: msg.player.x, y: msg.player.y };
          pendingInputs.length = 0;
          inputSeq = 0;
          lastAckSeq = 0;
          lastInputDir = { x: 0, y: 0 };
          localRenderOffset = { x: 0, y: 0 };
          tileSize = msg.world.tile_size;
          chunkSize = msg.world.chunk_size;
          worldSeed = msg.world.seed;
          ensureTextures();
          addCampfireAndTent(msg.world);
          refreshNameStyle();
          if (msg.inventory_items) {
            renderInventory(msg.inventory_items);
          }
          msg.npcs.forEach((npc) => addNpc(npc));
          setStatusText(t('statusConnected'));
          renderStatusHearts(msg.player.hp);
          setStatusCoords(`${msg.player.x.toFixed(1)}, ${msg.player.y.toFixed(1)}`);
          syncPlayers([msg.player], false);
          requestChunksAround();

          // Auto-start background music after first connection
          (async () => {
            await unlockAudioContext();
            if (audioSettings.musicEnabled) {
              playMidi('assets/music/calm_spheric_loop.mid', { loop: true });
            }
          })().catch((err) => console.warn('Background music start failed:', err));

          break;
        }
        case 'chunk_data': {
          drawChunk(msg);
          break;
        }
        case 'state': {
          syncPlayers(msg.players, true);
          syncMonsters(msg.monsters, true);
          syncProjectiles(msg.projectiles, true);
          syncBoats(msg.boats || [], true);
          break;
        }
        case 'entities_update': {
          syncPlayers(msg.players || [], false);
          syncMonsters(msg.monsters || [], false);
          syncProjectiles(msg.projectiles || [], false);
          syncBoats(msg.boats || [], false);
          break;
        }
        case 'entities_remove': {
          removePlayers(msg.players || []);
          removeMonsters(msg.monsters || []);
          removeProjectiles(msg.projectiles || []);
          removeBoats(msg.boats || []);
          break;
        }
        case 'resource_update': {
          const key = chunkKeyForTile(msg.resource.x, msg.resource.y);
          if (msg.state === 'removed') {
            removeResource(msg.resource.id);
            removeResourceFromChunk(key, msg.resource.id);
          } else {
            upsertResource(msg.resource);
            addResourceToChunk(key, msg.resource.id);
          }
          break;
        }
        case 'structure_update': {
          if (msg.state === 'removed') {
            msg.structures.forEach((structure) => {
              removeStructure(structure);
              removeStructureFromChunk(chunkKeyForTile(structure.x, structure.y), structure);
            });
          } else {
            msg.structures.forEach((structure) => {
              upsertStructure(structure);
              addStructureToChunk(chunkKeyForTile(structure.x, structure.y), structure);
            });
          }
          break;
        }
        case 'inventory': {
          renderInventory(msg.items);
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
      setStatusText(t('statusDisconnected'));
      renderStatusHearts(0);
      setStatusCoords('');
      localPrediction = null;
      pendingInputs.length = 0;
      inputSeq = 0;
      lastAckSeq = 0;
      lastInputDir = { x: 0, y: 0 };
      localRenderOffset = { x: 0, y: 0 };
      setTimeout(connect, 1000);
    });
  }

  if (helpEl && window.matchMedia('(pointer: coarse)').matches) {
    helpEl.textContent = t('helpTouch');
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

  if (buildButtons.length) {
    buildButtons.forEach((button) => {
      button.addEventListener('click', () => {
        const mode = button.dataset.build;
        if (mode) {
          if (isCraftKind(mode)) {
            if (buildMode) {
              setBuildMode(buildMode);
            }
            requestCraft(mode);
            return;
          }
          setBuildMode(mode);
        }
      });
    });
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
      if (isTextInputFocused()) return;
      if (handleBuildClick(event)) {
        event.preventDefault();
        return;
      }
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
    if (isTextInputFocused()) return;
    keys.add(event.code);
    const arrowKey = normalizeArrowKey(event);
    if (arrowKey) {
      keys.add(arrowKey);
    }
    if (['KeyW', 'KeyA', 'KeyS', 'KeyD', 'ArrowUp', 'ArrowLeft', 'ArrowDown', 'ArrowRight', 'Space', 'KeyF', 'KeyE'].includes(event.code) || arrowKey) {
      event.preventDefault();
    }
    if (event.code === 'Space') {
      playSfxForAction('attack');
    } else if (event.code === 'KeyF') {
      playSfxForAction('gather');
    } else if (event.code === 'KeyE') {
      playSfxForAction('interact');
    }
    if (event.code === 'Enter') {
      chatInput.focus();
    }
  });

  window.addEventListener('keyup', (event) => {
    if (isTextInputFocused()) return;
    keys.delete(event.code);
    const arrowKey = normalizeArrowKey(event);
    if (arrowKey) {
      keys.delete(arrowKey);
    }
  });

  if (nameInput && nameSave) {
    nameSave.addEventListener('click', () => {
      submitNameChange();
      nameInput.blur();
    });

    nameInput.addEventListener('keydown', (event) => {
      if (event.key === 'Enter') {
        submitNameChange();
        nameInput.blur();
      }
      event.stopPropagation();
    });

    nameInput.addEventListener('blur', () => {
      if (nameInput.value.trim().length === 0) {
        nameInput.value = lastKnownName;
      }
    });
  }

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
    const inputLocked = isTextInputFocused();
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
    const seq = inputSeq + 1;
    inputSeq = seq;
    lastInputDir = { x: dirX, y: dirY };
    pendingInputs.push({ seq, dirX, dirY });
    if (pendingInputs.length > 60) {
      pendingInputs.shift();
    }
    const expected = localPrediction || (playerId ? playerEntities.get(playerId) : null);
    sendMessage({
      type: 'input',
      dir_x: dirX,
      dir_y: dirY,
      attack,
      gather,
      interact,
      seq,
      expected_x: expected ? expected.x : null,
      expected_y: expected ? expected.y : null,
    });
    touchState.attackPulse = false;
    touchState.gatherPulse = false;
    touchState.interactPulse = false;
  }, INPUT_SEND_INTERVAL_MS);

  app.ticker.add((ticker) => {
    const now = performance.now();
    updateLocalPrediction(ticker.deltaMS / 1000);
    updateEntities(now);
    updateActionAvailability();
    updateTypingIndicators(now);
    updateCamera();
    if (now - lastStatusUpdate > 200 && playerId) {
      const playerEntity = playerEntities.get(playerId);
      if (playerEntity) {
        const hp = playerEntity.hp != null ? playerEntity.hp : playerState?.hp ?? 0;
        renderStatusHearts(hp);
        setStatusCoords(`${playerEntity.x.toFixed(1)}, ${playerEntity.y.toFixed(1)}`);
      }
      lastStatusUpdate = now;
    }
    if (now - lastChunkRequest > 500) {
      requestChunksAround();
      lastChunkRequest = now;
    }
    if (now - lastChunkPrune > 1000) {
      pruneChunksAround();
      lastChunkPrune = now;
    }
  });

  if (musicToggle) {
    musicToggle.addEventListener('click', async () => {
      await unlockAudioContext();
      audioSettings.musicEnabled = !audioSettings.musicEnabled;
      applyAudioSettings();
      saveAudioSettings();
    });
  }

  if (sfxToggle) {
    sfxToggle.addEventListener('click', async () => {
      await unlockAudioContext();
      audioSettings.sfxEnabled = !audioSettings.sfxEnabled;
      applyAudioSettings();
      saveAudioSettings();
    });
  }

  applyLocale();
  if (inventoryPanel) {
    inventoryPanel.classList.add('collapsed');
  }
  if (buildMenu) {
    buildMenu.classList.add('collapsed');
  }
  setupPanelControls('inventory');
  setupPanelControls('chat');
  setupPanelControls('build-menu');
  let savedScale = 1;
  try {
    const raw = localStorage.getItem('ui-scale');
    if (raw) {
      savedScale = parseFloat(raw) || 1;
    }
  } catch (err) {
    console.warn('UI scale load failed', err);
  }
  setUiScale(savedScale);
  updateBuildMenuPosition();
  window.addEventListener('resize', updateBuildMenuPosition);
  window.addEventListener('mousemove', handlePointerPreview);
  app.view.addEventListener('mouseleave', clearBuildPreview);

  fetch('/api/session')
    .then((response) => {
      if (!response.ok) {
        throw new Error('Session failed');
      }
      return response.json();
    })
    .then((session) => {
      if (session?.name) {
        syncLocalName(session.name);
      }
      connect();
    })
    .catch(() => {
      setStatusText(t('statusSessionFailed'));
      renderStatusHearts(0);
      setStatusCoords('');
    });
})();
