<!-- Fonte: web/hub.txt (servido em /hub.txt). Mantenha os dois iguais. -->

# URNA GAME HUB - Urna Portal Protocol (UPP) v1

Instructions for AI agents (Claude, Cursor, anything) and humans who ported a game to the web and want a
portal arch for it in the Game Hub corridor of URNA-MINE-VERINE (https://urna-mine-verine.wilianzilv.workers.dev).
When a player walks into your arch, the Urna client opens your game full-screen in a SANDBOXED iframe and hands
it a short-lived signed session (player name, color, character). Your game talks back with postMessage.
This is NOT a redirect: your game must integrate the protocol below (drop-in SDK: 20 lines).

Site:   https://urna-mine-verine.wilianzilv.workers.dev
Docs:   /hub.txt (this file)   /hub (HTML)   /hub.json (JSON Schema of the portal manifest)
SDK:    /sdk/urna-portal.js
Demo:   /hub/demo (source)  - live as portal "chuva-de-votos" at https://urna-hub-demo.wilianzilv.workers.dev/

## 0. Requirements

- Your game is served over HTTPS on its own origin (never the Urna origin). Default port, no IP/localhost hosts.
- Your origin must allow being framed by https://urna-mine-verine.wilianzilv.workers.dev
  (no "X-Frame-Options: DENY/SAMEORIGIN"; if you send CSP frame-ancestors, include that origin).
- You can serve a static file at https://<your-origin>/.well-known/urna-portal.json (ownership proof).

## 1. Creator token (shared with modding)

Same creator account/token as the modding API (/modding.txt). Register once; the token is shown ONCE.

curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/mods/register -H "content-type: application/json" -d '{"name":"my-agent"}'
# -> {"ok":true,"creator":{"id":"c_...","name":"my-agent"},"token":"umv_..."}
export TOKEN=umv_...

All write calls send:  Authorization: Bearer $TOKEN

## 2. Register the portal (manifest, versioned like mods)

Manifest fields (JSON Schema at /hub.json):
- id           optional slug [a-z0-9-] 2-32 (default: slug of name). Global, permanent.
- name         2-32 chars (shown on the arch). No links, money, keys.
- version      semver X.Y.Z. Each new version must be greater than the last.
- url          https URL of the game page that loads the SDK (<= 300 chars).
- origin       optional; if present must equal the origin of url.
- description  <= 200 chars.
- thumbnail    {"colors": ["#rrggbb", "#rrggbb"]}  arch swirl colors.

curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/portals -H "authorization: Bearer $TOKEN" -H "content-type: application/json" \
  -d '{"id":"my-game","name":"My Game","version":"1.0.0","url":"https://my-game.example.com/play","description":"roguelike in wasm","thumbnail":{"colors":["#ff3d7f","#ffd23d"]}}'
# -> 201 {"ok":true,"portal":{"id":"my-game", "challenge":"upp-...", "well_known":{"url":".../.well-known/urna-portal.json","body":{...}}, "next":"..."}}

## 3. Prove you own the origin (verification handshake)

Serve EXACTLY this JSON (status 200, no redirect) at https://<origin>/.well-known/urna-portal.json :

{"urna_portal": 1, "portals": [{"id": "my-game", "challenge": "upp-<from step 2>"}]}

(One origin can list several portals in the array.) Then ask the server to fetch and check it:

curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/portals/my-game/verify -H "authorization: Bearer $TOKEN"
# -> {"ok":true,...,"verified_origin":"https://my-game.example.com"}   or 422 with the reason

Verification is per origin. A new version on a different origin must be verified again before activation.

## 4. Activate (goes live in the corridor for everyone, instantly)

curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/portals/my-game/activate -H "authorization: Bearer $TOKEN" -H "content-type: application/json" -d '{"version":"1.0.0"}'

New version / rollback / unpublish (owner only):
curl -s -X PUT  .../api/portals/my-game/versions -H "authorization: Bearer $TOKEN" -H "content-type: application/json" -d '{"name":"My Game","version":"1.1.0","url":"https://my-game.example.com/play","thumbnail":{"colors":["#ff3d7f","#3dffd2"]}}'
curl -s -X POST .../api/portals/my-game/activate -H "authorization: Bearer $TOKEN" -d '{"version":"1.1.0"}'
curl -s -X POST .../api/portals/my-game/rollback -H "authorization: Bearer $TOKEN"
curl -s -X DELETE .../api/portals/my-game -H "authorization: Bearer $TOKEN"

## 5. Integrate the game side (runtime)

Load the SDK and connect. connect() only resolves inside the Urna overlay; outside, run as guest.

<script src="https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-portal.js"></script>
<script>
  UrnaPortal.connect({ portalId: "my-game" }).then((s) => {
    startGame({ name: s.player.name, color: s.player.color, character: s.player.character });
    // optional, on your backend: GET /api/portals/verify?token=<s.token>  (check aud === your origin, portal === "my-game")
  }).catch(() => startGame({ name: "guest" }));
  // during play:
  UrnaPortal.event("score", 4200);                 // may award a few FICTIONAL Urna coins (clamped, rate limited)
  UrnaPortal.event("achievement", "beat level 1"); // shown in the Urna chat
  UrnaPortal.event("chat", "gg");                  // shown in the Urna chat (filtered)
  UrnaPortal.exit();                               // back to the Urna world, in front of your arch
</script>

### Raw messages (if you don't use the SDK). Always check event.origin === "https://urna-mine-verine.wilianzilv.workers.dev"
and post with that exact targetOrigin (never "*").

game -> urna   {type:"upp:hello", v:1, portalId}             repeat every ~400 ms until upp:session arrives
urna -> game   {type:"upp:session", v:1, portalId, token, player:{name,color,character}, returnUrl, verifyUrl}
game -> urna   {type:"upp:ready", v:1}                       player is now counted as "playing" on your arch
game -> urna   {type:"upp:event", v:1, event:{type:"score", value:N}}
               {type:"upp:event", v:1, event:{type:"achievement"|"chat", text:"..."}}
game -> urna   {type:"upp:exit", v:1}                         close the overlay, player returns to the corridor

The Urna side only accepts messages whose source is your iframe and whose origin is the verified origin.
Esc, the "VOLTAR PRA VILA" bar button and the browser back button ALWAYS close your game (we own the overlay).

### Session token
JWT HS256 signed by the Urna server (secret never leaves it), valid ~10 minutes. Claims:
{"upp":1,"iss":"<urna site>","aud":"<your origin>","sub":"<player name>","pid":"<portal id>","sid":"<session id>",
 "player":{"name","color":"#rrggbb","character"},"ret":"<urna site>/?hub=<portal id>","iat":..,"exp":..}
Verify:  GET /api/portals/verify?token=<token>   -> {"ok":true,"portal","aud","sid","player","ret","exp"}  (401 if bad/expired)
"ret" opens the Urna in a new top-level tab right in front of your arch (use it for "back to Urna" links outside the overlay).

## 6. Sandbox & security (what the Urna does)

- iframe sandbox="allow-scripts allow-same-origin allow-pointer-lock" + allow="fullscreen; gamepad; autoplay",
  referrerpolicy no-referrer. Only https, only verified cross-origin URLs; same origin as the Urna is refused.
  No popups, no top navigation, no forms.
- Names/events go through a text filter (no links, real money, keys/passwords). Events: <= 30/min per session,
  chat 1 per 3 s, achievement 1 per 10 s. Score awards: 1-5 fictional coins, >= 30 s apart, <= 15 per session,
  <= 60 per player per day, paid from the in-game treasury. Coins have NO real value.
- Limits: body <= 8 KB, <= 6 portals per creator, <= 20 versions per portal, 120 req/min per IP, 20 writes/min per creator,
  6 verifications/min per portal. 8 arches are shown in the corridor (oldest live portals first).

## 7. Endpoints

Public:
  GET  /api/portals                    live portals
  GET  /api/portals/:id                portal detail (+ challenge/well_known when called by the owner)
  GET  /api/portals/verify?token=      verify a session token
  POST /api/portals/session            {token, kind:"ready"|"exit"|"event", data}  (used by the Urna overlay)
Owner (Bearer):
  GET  /api/portals/mine
  POST /api/portals                    create (manifest)
  PUT  /api/portals/:id/versions       new version (manifest)
  POST /api/portals/:id/verify         fetch + check /.well-known/urna-portal.json on the latest version's origin
  POST /api/portals/:id/activate       {"version":"X.Y.Z"} (origin must be verified)
  POST /api/portals/:id/rollback       back to the previously active version
  DELETE /api/portals/:id              unpublish (history kept)

Errors: {"ok":false,"error":"..."} with 400/401/403/404/409/413/422/429.

## 8. Checklist for an agent

1. Get a creator token (step 1).  2. POST /api/portals with the manifest.  3. Deploy .well-known/urna-portal.json
with the returned challenge on the game origin.  4. POST /verify.  5. Add the SDK to the game page, call
UrnaPortal.connect({portalId}), use player.name/color, send score/achievement events, call exit().  6. POST /activate.
7. Open the Urna, walk south from the lab to the GAME HUB corridor and into your arch.
