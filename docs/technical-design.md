# Technical Design

This project is a lightweight client/server game with a Rust backend and a PixiJS frontend. The goal is to stay simple, data-driven, and easy to extend.

## Architecture

- **Client:** PixiJS in `public/` renders the world, entities, and UI.
- **Server:** Rust (Axum + WebSockets) simulates the world and broadcasts state.
- **Database:** MongoDB stores player documents keyed by session ID.

## Client/server flow

- The client requests `/api/session` to obtain a session cookie.
- A WebSocket connection is opened at `/ws`.
- The server sends `welcome` with player data, world config, and NPCs.
- The client requests chunks around the player (`chunk_request`).
- The server streams `chunk_data` with tiles and resources.
- The server broadcasts `state` ticks with players, monsters, and projectiles.
- Typing notifications are broadcast so clients can show chat bubbles above players.

## Persistence

- Players are stored in MongoDB collection `onlinerpg.players`.
- Each player document stores name, position, HP, inventory, and completed quests.
- The server saves periodically and on disconnect.
- No login required; a session cookie (`sid`) is the identity key.

## Data-driven content

The server loads JSON files from `data/` on startup:

- `data/world.json`: world seed, chunk size, tile size, spawn.
- `data/items.json`: items, tools, weapons, ammo.
- `data/resources.json`: resource nodes and drops.
- `data/monsters.json`: monster stats and drops.
- `data/npcs.json`: NPC locations and dialog.
- `data/quests.json`: quest requirements and rewards.

Adjusting these files changes behavior without code changes.
