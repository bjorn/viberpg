# AGENTS

## Project overview
- Small online RPG: Rust server simulates the world and broadcasts state over WebSockets; PixiJS client renders and sends input.
- Data-driven content lives in JSON under `data/` and is loaded on startup.

## Stack
- Server: Rust + Axum (`src/main.rs`), MongoDB persistence.
- Client: vanilla JS + PixiJS (no build step), assets in `public/`.
- CDN deps (loaded in `public/index.html`): `pixi.js@7.4.3`, `pixijs-joystick@1.1.1`.

## Run / dev
- Start MongoDB: `docker compose up`.
- Run server: `MONGODB_URI=mongodb://localhost:27017 PORT=3000 cargo run`.
- Open `http://localhost:3000` (server serves `public/`).

## Key files
- `src/main.rs`: main server, game loop, WebSocket protocol, persistence.
- `public/game.js`: client render loop, entity sync, input handling.
- `public/index.html`: loads PixiJS + joystick lib, HUD markup.
- `public/styles.css`: HUD layout, touch UI styling.
- `docs/protocol.md`: HTTP/WebSocket message schema reference.
- `data/*.json`: world, items, resources, monsters, NPCs, quests.

## Client/server flow (quick refresher)
- Client fetches `/api/session` to set session cookie.
- Client opens WebSocket at `/ws`.
- Server sends `welcome`, then streams `chunk_data`, broadcasts `state` ticks.
- Client sends `input` (dir_x/dir_y + attack/gather/interact) about every 90ms.
- See `docs/protocol.md` for full message schemas.

## Mobile controls
- Touch joystick uses `pixijs-joystick` and is rendered in a Pixi UI layer.
- Action buttons are DOM elements in `#touch-actions` and appear for coarse pointers.
- Touch movement updates `touchState.dirX/dirY`; input loop merges touch + keyboard.

## Persistence + gameplay notes
- MongoDB collection: `onlinerpg.players` (by session cookie).
- HP regen is applied server-side each tick (see `PLAYER_REGEN_INTERVAL_MS` in `src/main.rs`).

## Tests
- No automated tests or linting configured.
