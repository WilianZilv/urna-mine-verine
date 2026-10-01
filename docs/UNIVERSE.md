# URNA UNIVERSE — player mods, passport, presence, items across games

Goal: one identity that travels. A player picks an avatar mod in the village, walks into a portal, and the
external game renders the same avatar, sees other village players live, and can pay/charge fictional coins and
mod items — all validated by our server, never trusting the game.

## 1. Gap analysis (state before this work)

| Area | What existed | Gap |
| --- | --- | --- |
| Mod schema (`server/mods.js`) | Strict declarative JSON: manifest, voxel `model.parts` (absolute pivots), keyframe `animations` (idle/walk/attack/death/roar/jump/fly), `behavior` primitives, synth `sounds`, showcase `items`/`blocks`. Versioned, immutable, creator tokens (SHA-256), AI text moderation, rate limits. | Only one kind (NPC). No player skins, no standalone items. `behavior` mandatory. Active limit shared by everything. |
| Mod runtime (`src/mods.rs`) | Parses NPC packages from WS `{t:"mod"}` and simulates them in the mod zone. | Nothing applies a mod to a *player*. |
| Hub (`server/hub.js`) | UPP v1: verified portals, sandboxed iframe, HMAC session JWT (10 min) with `{name,color,character}`, score/achievement/chat events, coin reward from score (15/session, 60/day). | Token has no avatar. No public profile endpoint. No multiplayer between players inside a game. No way for a game to give/take items or coins explicitly; no idempotency. |
| SDK (`web/sdk/urna-portal.js`) | `connect`, `event`, `exit`, `verify`. Never throws. | No avatar, presence, grant, spend. No renderers for engines. |
| Economy (`server/economy.js`) | Fictional wallets by player name, public ledger, treasury (`cofre`). | No per-player inventory of transferable items. |
| Client (`src/main.rs`, `inventory.rs`) | Character menu (C), remote players drawn as generic humanoids, Steve inventory (blocks/tools). | No wardrobe, no avatar rendering, no "items from other worlds". |

## 2. Target design (implemented)

### Mod kinds (`manifest.kind`)
- `"npc"` (default, old packages unchanged — King Kong is npc).
- `"avatar"`: player skin. Required parts `head, body, arm_l, arm_r, leg_l, leg_r` (optional `tail, wing_l, wing_r, extra*`...).
  Animations `idle, walk, run, jump, attack, fly, death, emote1..emote4`. `avatar: {scale, emotes}`; the server
  computes `avatar.height` and shrinks `scale` so the rendered body is 1.2..2.4 blocks tall and <= 1.6 wide
  (gameplay hitbox never changes: avatars are cosmetic). <= 16 parts, <= 160 boxes, coords +-4.
- `"item"`: transferable item. `item: {color, color2, pattern, stack, daily_cap}` + optional tiny `model` (<= 48 boxes).
- Same registry, versions, auth, moderation. Separate active limits per kind (npc 12, avatar 40, item 60).

### Wardrobe (village)
- C menu shows a SKIN row with every active avatar. Choice → WS `{t:"uv_set", mod}` → stored in DO
  (`uv:av:<player>`), auto-follows the mod's active version.
- Server stamps `av:"id@version"` into every relayed `p` message (client cannot fake it). Clients fetch
  packages once via `{t:"uv_get"}` and cache them; Steve/creative/portal-gun bodies render the avatar.

### Passport
- Portal session token gains `player.avatar` (ref). `GET /api/passport?token=` →
  `{player, color, character, avatar_ref, avatar:<validated package>, wallet:{coins}, inventory:[...], limits}`.

### Presence
- `wss://…/api/portals/:id/presence?token=` per-portal relay room inside the DO. Server relays only:
  <= 12 msgs/s per socket (excess dropped), state <= 512 bytes, <= 32 players per portal, 1 socket per session.

### Items & coins across games
- `POST /api/universe/grant {token, coins|item, n, reason, key}` — request validated server-side: coins <= 60/day
  per player per portal (paid by the treasury), items only from the portal's allowlist (owner registers via
  `PUT /api/portals/:id/items`; only active item mods by the same creator are approved), item `daily_cap`,
  idempotency `key` (replays return the first result), ledger entry "veio do jogo X".
- `UrnaPortal.spend()` → `upp:spend` postMessage → confirm dialog owned by OUR page → our page calls
  `POST /api/universe/spend` with a confirm token the iframe never sees. Without it, spend is 403.
- Steve inventory (E) shows "ITENS DE OUTROS MUNDOS".

### SDK & renderers (`/sdk/`)
- `urna-portal.js`: `connect()` (session incl. avatar), `avatar()`, `passport()`, `presence.join/update/on/leave`,
  `grant()`, `spend()`.
- `urna-avatar-three.js`: `UrnaAvatarThree.build(pkg, THREE)` → `{group, update(dt, state)}`.
- `urna-avatar-canvas2d.js`: `UrnaAvatar2D.create(pkg, {view:"side"|"front"|"top"})` → cached offscreen frames,
  `draw(ctx, x, y, h, state, t)`.
- Generic: the package itself is the mesh description (parts → boxes, pivot, parent, keyframes). Godot/Unity notes
  in `/universe.txt`.

## 3. Plan / status (all done, live)
1. Gap analysis (this file).
2. Schema kinds + validation + docs/examples (`examples/mods/astronauta.json`, `examples/mods/voto-dourado.json`).
3. `server/universe.js` (avatars, bag, passport, presence, grant, spend, allowlist) + hooks in worker/hub.
4. SDK + renderers + overlay confirm (`web/universe.js`).
5. Rust: `src/universe.rs` (wardrobe, avatar rendering, bag UI) + small hooks in `main.rs`.
6. Demo `/hub/demo` (+ worker `urna-hub-demo`): avatar via canvas2d, presence ghosts, `grant()` reward.
7. Verified live: `node tools/verify_universe.mjs` (36 checks: upload/activate kinds, allowlist, skin stamp +
   forgery strip, passport, presence join/update/leave/rate limit, grant budget/idempotency/cap/allowlist,
   spend confirm, receipt) and `node tools/verify_universe_cdp.mjs` (muted headless: wardrobe click → server
   stamps skin, remote skins render, demo draws the passport avatar).

## 4. Not in v1 (next)
- Cross-creator item allowlists (needs a human moderation queue).
- Emote trigger key in the village (emotes are rendered by SDK renderers today).
- Persisting the package cache in IndexedDB (memory cache per session today).
