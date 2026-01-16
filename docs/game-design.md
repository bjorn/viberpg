# Game Design

Mistwood Vale is a small, session-based online RPG focused on exploration and light co-op play. It keeps the loop simple so the core systems are easy to reason about and iterate on.

## Core features

- Auto-generated world streamed in chunks as players explore.
- Gathering loop with tools (trees with axes, rocks with pickaxes).
- Real-time combat against monsters (no PvP).
- NPCs with simple quest hand-ins and item rewards.
- Visible multiplayer presence, chat, and typing bubbles.
- Session cookie persistence (no login required).

## Player loop

1. Explore the world to find resources and monsters.
2. Gather wood/stone, defeat slimes for drops.
3. Visit NPCs to complete quests and earn better gear.
4. Chat and explore with other players.

## Shortcomings and opportunities

- Combat is basic: no hitboxes, stamina, dodge, or enemy variety.
- AI is minimal: monsters only wander or chase the nearest player.
- World variation is limited: only a few tile types and one monster.
- No crafting or economy system yet.
- UI is functional but sparse (no minimap, no quest log).
- Multiplayer has no proximity filtering or region-based optimization.
