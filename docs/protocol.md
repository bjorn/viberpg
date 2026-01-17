# Protocol

This document describes the HTTP and WebSocket payloads used by the client and server. All WebSocket messages are JSON with a `type` field in `snake_case`.

## HTTP

### GET /api/session
- Purpose: ensures a session cookie (`sid`) is set before opening the WebSocket.
- Response:

```json
{
  "session_id": "<string>",
  "name": "<string>"
}
```

## WebSocket

### Endpoint
- `ws://<host>/ws` (or `wss://` for HTTPS)

### Client -> Server

#### input
```json
{
  "type": "input",
  "dir_x": 0.0,
  "dir_y": 0.0,
  "attack": false,
  "gather": false,
  "interact": false
}
```
- `dir_x`/`dir_y` are clamped to `[-1.0, 1.0]` server-side.

#### chat
```json
{
  "type": "chat",
  "text": "Hello!"
}
```
- Server trims to 160 chars.

#### set_name
```json
{
  "type": "set_name",
  "name": "Wanderer"
}
```
- Server trims to 20 chars and ignores empty names.

#### typing
```json
{
  "type": "typing",
  "typing": true
}
```

#### chunk_request
```json
{
  "type": "chunk_request",
  "chunks": [
    { "x": 0, "y": 0 },
    { "x": 1, "y": 0 }
  ]
}
```

#### ping
```json
{
  "type": "ping"
}
```

### Server -> Client

#### welcome
```json
{
  "type": "welcome",
  "player": {
    "id": "<string>",
    "name": "<string>",
    "x": 0.0,
    "y": 0.0,
    "hp": 10,
    "inventory": { "wood": 3 }
  },
  "world": {
    "seed": 123,
    "chunk_size": 32,
    "tile_size": 16,
    "spawn_x": 0.0,
    "spawn_y": 0.0
  },
  "npcs": [
    { "id": "npc_1", "name": "Elder", "x": 12.0, "y": 9.0, "dialog": "..." }
  ]
}
```

#### chunk_data
```json
{
  "type": "chunk_data",
  "chunk_x": 0,
  "chunk_y": 0,
  "tiles": [0, 0, 1, 2],
  "resources": [
    { "id": "42", "kind": "tree", "x": 10, "y": 7, "hp": 3 }
  ]
}
```

#### state
```json
{
  "type": "state",
  "players": [
    { "id": "<string>", "name": "<string>", "x": 0.0, "y": 0.0, "hp": 10 }
  ],
  "monsters": [
    { "id": 1, "kind": "slime", "x": 4.0, "y": 5.0, "hp": 6 }
  ],
  "projectiles": [
    { "id": 2, "x": 3.2, "y": 1.8 }
  ]
}
```

#### resource_update
```json
{
  "type": "resource_update",
  "resource": { "id": "42", "kind": "tree", "x": 10, "y": 7, "hp": 0 },
  "state": "removed"
}
```
- `state` is either `"removed"` or `"spawned"`.

#### chat
```json
{
  "type": "chat",
  "from": "Wanderer",
  "text": "Hello!"
}
```

#### dialog
```json
{
  "type": "dialog",
  "title": "Elder",
  "text": "Welcome to Mistwood."
}
```

#### system
```json
{
  "type": "system",
  "text": "Collected wood x2"
}
```

#### typing
```json
{
  "type": "typing",
  "id": "<string>",
  "typing": true
}
```
