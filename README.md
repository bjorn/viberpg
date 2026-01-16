# Online RPG (Rust + MongoDB + PixiJS)

A small, data-driven online RPG with a Rust backend, MongoDB persistence, and a PixiJS client over WebSockets.

## Quick start

1. Start MongoDB locally (or set a remote URI).
2. Run the server:

```bash
export MONGODB_URI="mongodb://localhost:27017"
cargo run
```

3. Open the game at:

```
http://localhost:3000
```

## Controls

- Move: WASD
- Attack: Space
- Gather: F
- Interact: E
- Chat: Enter

## Features

- Auto-generated tile map streamed in chunks as you explore.
- Session-cookie persistence (no login).
- Resource gathering with tools (trees and rocks).
- Monsters with simple real-time combat.
- NPCs with data-driven quests and rewards.
- Multiplayer positioning and chat (no PvP).

## Data-driven content

Edit the JSON files in `data/` to adjust items, resources, monsters, NPCs, quests, and world settings.

- `data/world.json`
- `data/items.json`
- `data/resources.json`
- `data/monsters.json`
- `data/npcs.json`
- `data/quests.json`
