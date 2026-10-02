---
name: urna-mine-verine
description: Use when a human pastes https://urna-mine-verine.wilianzilv.workers.dev/skill.md (or mentions URNA-MINE-VERINE) and wants to create a mod (creature/NPC, player avatar skin, item), connect their own web game to the Urna Game Hub, or just play. Root skill that routes to /modding.txt and /hub.txt.
---

# URNA-MINE-VERINE — root skill for AI agents

## If a human sent you here
URNA-MINE-VERINE (https://urna-mine-verine.wilianzilv.workers.dev) is a multiplayer voxel world that runs in the browser (Minecraft-like, Brazilian
political satire) and is an OPEN PLATFORM: any AI agent can, over plain HTTP, with no human coding:
- add MODS that go live for every player instantly: creatures/NPCs, avatar skins the player wears in the village
  and in every connected game, and items (all declarative JSON data, never code);
- connect an external web game as a glowing portal arch in the Game Hub corridor: players walk in, play it
  full-screen with their name/avatar, earn FICTIONAL coins, and come back.

The human is probably NOT technical. You do all the work end to end. Never ask technical questions
(ids, JSON, versions, frameworks, colors): decide yourself.

## FIRST ACTION (do this before anything else)
Greet the human in ONE short line in their language (detect it from their message; default pt-BR) and ask ONE
question with options. pt-BR template:

> Oi! Eu vou fazer tudo pra ti no URNA-MINE-VERINE. O que tu quer?
> (a) criar um MOD: uma criatura/NPC, uma skin/avatar pro teu player (que viaja entre os jogos) ou um item
> (b) conectar um JOGO teu ao Hub (vira um portal no corredor; teu jogo recebe o avatar, presença e moedas dos players)
> (c) só jogar / entender como funciona

If their first message already makes the choice obvious, skip the question and say what you will do.
Then ask ONLY what you truly need, in plain words, one question at a time, e.g. "Como é a criatura?",
"Que nome aparece em cima dela?", "Em que pasta tá teu jogo?" (only if you cannot find it yourself).

## Routing
- (a) MOD -> fetch https://urna-mine-verine.wilianzilv.workers.dev/modding.txt and follow it completely (it has its own "If a human sent you here").
  Kinds (manifest.kind): "npc" (default: creature in the mod zone, north of the lab, with behavior),
  "avatar" (player skin with the standard humanoid rig; players pick it in the village with C -> SKIN; games get it
  through the passport), "item" (fictional transferable item; games may grant it, players spend it with confirmation).
  Always POST /api/mods/validate until ok, then create, activate, confirm GET /api/mods/<id>.
- (b) GAME -> work from the game's project folder; fetch https://urna-mine-verine.wilianzilv.workers.dev/hub.txt and follow it completely (find the game,
  build, deploy to free HTTPS hosting if needed, add the SDK, register, prove ownership via
  /.well-known/urna-portal.json, activate, check GET /api/portals shows "live": true).
- (c) PLAY -> give the link and the controls below. Nothing to install.
  Play: https://urna-mine-verine.wilianzilv.workers.dev  (optional: https://urna-mine-verine.wilianzilv.workers.dev/?nome=SEUNOME). Controls: WASD walk, mouse look (click to capture), Space jump,
  Shift run, F fly, C character/skin menu, E inventory, T or Enter chat, H help, Esc release mouse.
  Characters (C, keys 1-8): Steve survival/creative, skater, bandit, portal gun, ENCANADOR, WOLVERINE, BOMBADINHO
  (key 8: grid bombs that blast in a cross; F/click drop, Q/E/wheel type: normal, fogo, perfurante, remota (G), linha, gosma).
  Phone: open the link in the browser landscape; left half = joystick, drag right half = look, on-screen buttons.
  Places: mod zone north of the lab (east), Game Hub corridor south of the lab; https://urna-mine-verine.wilianzilv.workers.dev/?hub=<id> spawns at an arch.
  Avenida dos Poderes (north of the plaza): CONGRESSO (west; vote laws that change the game: /leis, /lei <id>),
  BOLSA (east; fictional stocks: /bolsa, /investir T n, /vender T n, /carteira). South: TV URNA NEWS (/tv,
  /noticia, /manchete texto n), BANCO CENTRAL (/poupar n, /sacar n, /ranking), TERMINAL (/destinos, /viajar <dest>).
  All coins are fictional; every place has an in-world panel, chat commands work from anywhere.
  New here? /tour (visit the 5 places, light beacons guide you, pays fictional coins). Extras: /momento (TV),
  /bolsa mods (mod stocks), /criadores (creator impact ranking). B saves the last 10 s as a local video clip.
  /concurso + /votarmod n (weekly MOD DA SEMANA vote), /passaporte (persistent stamps, also in /api/passport).
  ESCOLA DE AGENTES (south of the houses; /escola: the 4-step mod lesson), /roadmap + /proposta n (vote what the
  devs build next), /campeonato (weekly arena championship on TV), /diario (daily streak, fictional coins).

## Shared rules (all paths)
- Creator token: get it once (POST /api/mods/register {"name":...}; shown ONCE). Save it to
  $HOME/.urna-creator-token (if it must live in a project: a gitignored file, never inside a deployed folder).
  NEVER print it in chat, logs, commits or public files. Reuse the SAME token for mods and for the Hub.
  If that file exists, reuse it; lost/leaked -> POST /api/mods/token/rotate.
- Limits (exact numbers in each doc): mod JSON <= 256 KB; npc <= 32 parts / 400 boxes, avatar <= 16 parts / 160 boxes,
  item <= 48 boxes; active npc 12 server-wide / 3 per creator; <= 20 mods per creator; <= 6 portals per creator;
  register 5/hour per IP; writes rate-limited (HTTP 429 = wait and retry). Out-of-range numbers are clamped (warnings).
- Moderation: every public text (names, phrases, descriptions, events) must have NO links, no "@"/contacts,
  no real money/pix/crypto, no keys/passwords/tokens, no hate or sexual content. Satire is fine. AI moderation may reject.
- Coins are FICTIONAL game coins only. Never promise or mention real money, prizes or crypto.
- Updating later: change it, bump the version (semver must increase; versions are immutable), upload, activate.
  Rollback/deactivate/unpublish endpoints exist in each doc.
- Finish by telling the human the result in ONE sentence in their language + the play link, e.g.
  "Pronto: o <Nome> já tá vivo na zona de mods, do lado do laboratório: https://urna-mine-verine.wilianzilv.workers.dev" or
  "Pronto: teu jogo tá no hub, no arco <Nome>: https://urna-mine-verine.wilianzilv.workers.dev/?hub=<id>".

## Index
- https://urna-mine-verine.wilianzilv.workers.dev/skill.md — this root skill (aliases: /SKILL.md, /skill, /llms.txt, /AGENTS.md)
- https://urna-mine-verine.wilianzilv.workers.dev/modding.txt — full modding runbook: npc/avatar/item kinds, workflow, endpoints, examples
- https://urna-mine-verine.wilianzilv.workers.dev/modding.json — JSON Schema of a mod package (+ limits, palette, kinds)
- https://urna-mine-verine.wilianzilv.workers.dev/modding — same modding docs as HTML
- https://urna-mine-verine.wilianzilv.workers.dev/hub.txt — Game Hub runbook: find/build/deploy the game, SDK, register, verify, activate, troubleshooting
- https://urna-mine-verine.wilianzilv.workers.dev/hub.json — portal manifest schema
- https://urna-mine-verine.wilianzilv.workers.dev/hub — Hub docs as HTML; working demo game: https://urna-mine-verine.wilianzilv.workers.dev/hub/demo
- https://urna-mine-verine.wilianzilv.workers.dev/hub-templates/ — copy-and-go game templates (canvas2d, three.js) with the SDK wired + manifest generator
- https://urna-mine-verine.wilianzilv.workers.dev/api/world — live public world state (laws, stocks, TV, rich list) as JSON; widgets to embed: https://urna-mine-verine.wilianzilv.workers.dev/embed/
- https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-portal.js — browser SDK for games (connect/session, events, exit; avatar/passport/presence/grant/spend); add data-auto to the tag and other Urna players show up in a three.js game with zero code (any portal already gets a presence overlay from the Urna page; see "Presenca sem codigo" in /hub.txt)
- https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-avatar-three.js and https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-avatar-canvas2d.js — render a player's avatar mod in three.js / 2D canvas
- https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-hub.mjs — Node helper CLI: prepare/publish/unpublish a portal
- https://urna-mine-verine.wilianzilv.workers.dev/api/passport?token=<session token> — player passport for games: avatar package, fictional wallet, items
- https://urna-mine-verine.wilianzilv.workers.dev/api/mods and https://urna-mine-verine.wilianzilv.workers.dev/api/portals — public lists of mods and live portals (JSON)
- https://urna-mine-verine.wilianzilv.workers.dev/universe.txt — avatar/presence/items across games