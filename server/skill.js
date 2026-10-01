// Skill raiz pra agentes de IA: um link so (/skill.md) que o humano cola no Cursor/Claude Code/Codex.
// Copia em docs/SKILL.md (gerada: node -e "import('./server/skill.js').then(m=>process.stdout.write(m.skillText()))").
const SITE = "https://urna-mine-verine.wilianzilv.workers.dev";

export function skillText() {
    return `---
name: urna-mine-verine
description: Use when a human pastes ${SITE}/skill.md (or mentions URNA-MINE-VERINE) and wants to create a mod (creature/NPC, player avatar skin, item), connect their own web game to the Urna Game Hub, or just play. Root skill that routes to /modding.txt and /hub.txt.
---

# URNA-MINE-VERINE — root skill for AI agents

## If a human sent you here
URNA-MINE-VERINE (${SITE}) is a multiplayer voxel world that runs in the browser (Minecraft-like, Brazilian
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
- (a) MOD -> fetch ${SITE}/modding.txt and follow it completely (it has its own "If a human sent you here").
  Kinds (manifest.kind): "npc" (default: creature in the mod zone, north of the lab, with behavior),
  "avatar" (player skin with the standard humanoid rig; players pick it in the village with C -> SKIN; games get it
  through the passport), "item" (fictional transferable item; games may grant it, players spend it with confirmation).
  Always POST /api/mods/validate until ok, then create, activate, confirm GET /api/mods/<id>.
- (b) GAME -> work from the game's project folder; fetch ${SITE}/hub.txt and follow it completely (find the game,
  build, deploy to free HTTPS hosting if needed, add the SDK, register, prove ownership via
  /.well-known/urna-portal.json, activate, check GET /api/portals shows "live": true).
- (c) PLAY -> give the link and the controls below. Nothing to install.
  Play: ${SITE}  (optional: ${SITE}/?nome=SEUNOME). Controls: WASD walk, mouse look (click to capture), Space jump,
  Shift run, F fly, C character/skin menu, E inventory, T or Enter chat, H help, Esc release mouse.
  Phone: open the link in the browser landscape; left half = joystick, drag right half = look, on-screen buttons.
  Places: mod zone north of the lab (east), Game Hub corridor south of the lab; ${SITE}/?hub=<id> spawns at an arch.

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
  "Pronto: o <Nome> já tá vivo na zona de mods, do lado do laboratório: ${SITE}" or
  "Pronto: teu jogo tá no hub, no arco <Nome>: ${SITE}/?hub=<id>".

## Index
- ${SITE}/skill.md — this root skill (aliases: /SKILL.md, /skill, /llms.txt, /AGENTS.md)
- ${SITE}/modding.txt — full modding runbook: npc/avatar/item kinds, workflow, endpoints, examples
- ${SITE}/modding.json — JSON Schema of a mod package (+ limits, palette, kinds)
- ${SITE}/modding — same modding docs as HTML
- ${SITE}/hub.txt — Game Hub runbook: find/build/deploy the game, SDK, register, verify, activate, troubleshooting
- ${SITE}/hub.json — portal manifest schema
- ${SITE}/hub — Hub docs as HTML; working demo game: ${SITE}/hub/demo
- ${SITE}/sdk/urna-portal.js — browser SDK for games (connect/session, events, exit; avatar/passport/presence/grant/spend)
- ${SITE}/sdk/urna-avatar-three.js and ${SITE}/sdk/urna-avatar-canvas2d.js — render a player's avatar mod in three.js / 2D canvas
- ${SITE}/sdk/urna-hub.mjs — Node helper CLI: prepare/publish/unpublish a portal
- ${SITE}/api/passport?token=<session token> — player passport for games: avatar package, fictional wallet, items
- ${SITE}/api/mods and ${SITE}/api/portals — public lists of mods and live portals (JSON)
- ${SITE}/universe.txt — avatar/presence/items across games (if it returns 404, it is not published yet: use /modding.txt + /hub.txt)
`;
}

/// GET /skill.md, /SKILL.md, /skill, /llms.txt, /AGENTS.md. null = nao e rota da skill.
export function skill(url) {
    if (!["/skill.md", "/SKILL.md", "/skill", "/llms.txt", "/AGENTS.md"].includes(url.pathname)) return null;
    return new Response(skillText(), { headers: { "content-type": "text/markdown; charset=utf-8", "access-control-allow-origin": "*", "cache-control": "public, max-age=300" } });
}
