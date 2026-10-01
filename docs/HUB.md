<!-- Fonte: web/hub.txt (servido em /hub.txt). Mantenha os dois iguais. -->

# URNA GAME HUB - agent runbook (Urna Portal Protocol, UPP v1)

Site: https://urna-mine-verine.wilianzilv.workers.dev   (browser voxel game, multiplayer)
This file: /hub.txt   HTML: /hub   Manifest schema: /hub.json   SDK: /sdk/urna-portal.js   Helper CLI: /sdk/urna-hub.mjs
Working example: https://urna-hub-demo.wilianzilv.workers.dev/ (portal "chuva-de-votos", source at /hub/demo)

## If a human sent you here

The human said something like: "Le https://urna-mine-verine.wilianzilv.workers.dev/hub.txt e coloca meu jogo no hub",
probably from inside their game's project folder. GOAL: their web game becomes a glowing portal arch in the Game
Hub corridor of the Urna world; players walk into the arch and play it full-screen, then come back.

You do ALL the work, end to end, autonomously: find the game, build it, deploy it to free HTTPS hosting if needed,
add the tiny SDK, register, prove ownership, activate, and check it is live. Do NOT ask the human technical
questions (frameworks, ports, JSON, versions, colors, ids: decide yourself). Only ask for things you truly
cannot get yourself, phrased simply, e.g. "Abri uma janela de login da Cloudflare, pode clicar em Permitir?".
Never commit or deploy the creator token. When finished, say ONE sentence (see the end of this file).

Overview of what you will do:
  A. find the game + its build   B. know/choose its HTTPS URL   C. add the SDK   D. register the portal
  E. put .well-known/urna-portal.json at the site root   F. deploy   G. verify + activate   H. check + tell the human

## A. Find the game in the current project

Look at the files (package.json, *.html, project.godot, ProjectSettings/, Assets/, CMakeLists/Makefile, README).

| Detected                              | Build command                          | Folder that gets deployed (DIR)      | Static files folder (copied as-is to DIR root) |
|---------------------------------------|----------------------------------------|--------------------------------------|------------------------------------------------|
| plain HTML/canvas/Phaser via <script> | none                                   | folder with index.html               | same folder                                    |
| Vite (React/Vue/Svelte/Phaser+Vite)   | npm install; npm run build             | dist/ (vite.config build.outDir)     | public/                                        |
| Create React App                      | npm install; npm run build             | build/                               | public/                                        |
| Next.js                               | set output:"export" in next.config; npm run build | out/                      | public/                                        |
| Parcel / webpack / other bundler      | npm run build                          | dist/ or as configured               | static/ or public/ if configured, else copy into DIR after build |
| Godot 4 web export                    | godot --headless --export-release "Web" build/web/index.html | build/web/      | none: copy into DIR after every export          |
| Unity WebGL                           | Unity build (WebGL)                    | the build folder (index.html, Build/) | none: copy into DIR after every build           |
| Emscripten (C/C++/Rust)               | emcc ... -o build/index.html / make    | build/                               | copy into DIR after build                       |

Engine notes (these break inside our iframe otherwise):
- Godot 4: in the Web export preset turn OFF "Thread Support" (Godot 4.3+). Threads need cross-origin isolation
  (SharedArrayBuffer), which is not available inside our iframe. The main file must be index.html.
- Unity WebGL: Player Settings > Publishing: Compression Format = Disabled, or enable "Decompression Fallback"
  (free static hosts do not send Content-Encoding for .br/.gz).
- Emscripten: build WITHOUT -pthread / USE_PTHREADS (same SharedArrayBuffer reason).
- Vite: keep base "/" (or "./"); DIR must contain index.html at its root.
- The game must load only https:// assets (no http://, no localhost).

## B. Is it already deployed on HTTPS?

Search for an existing public URL: package.json "homepage", CNAME file, netlify.toml, vercel.json / .vercel/,
wrangler.toml (pages_build_output_dir), .github/workflows/*pages*, README links, `git remote -v` + GitHub Pages.
Use it ONLY if you can also redeploy there (you will add a file at the site root). Requirements for the URL:
- https, its own origin (scheme+host), you control the ROOT of that origin (https://host/.well-known/...).
  So: GitHub *project* pages (user.github.io/repo/) do NOT work unless it is the user site repo
  "<user>.github.io" or has a custom domain. itch.io / newgrounds / CrazyGames hosting do NOT work (shared host).
- the site must allow being framed by https://urna-mine-verine.wilianzilv.workers.dev (see headers below).
If there is no usable URL, deploy (step F). Easiest free options, in this order:

1) Cloudflare Pages (recommended; URL = https://<project>.pages.dev/)
   npx wrangler login                                   # opens the browser once; ask the human to click "Allow"
   npx wrangler pages project create <project> --production-branch main
   npx wrangler pages deploy <DIR> --project-name <project> --branch main
   (pick <project> = slug of the game name; if taken, add a suffix. Re-run only the last line to redeploy.)
2) Netlify (URL = https://<name>.netlify.app/)
   npx netlify-cli login
   npx netlify-cli sites:create --name <name>
   npx netlify-cli deploy --prod --dir <DIR> --site <name>
3) Vercel (URL = https://<project>.vercel.app/)
   npx vercel login
   npx vercel deploy <DIR> --prod --yes               # prints the production URL
4) GitHub Pages - only for a "<user>.github.io" repo or a custom domain (see above). Put an empty file named
   .nojekyll in DIR (otherwise Jekyll hides the .well-known folder), push DIR to the branch Pages serves, enable:
   gh api repos/<user>/<user>.github.io/pages -X POST -f "source[branch]=main" -f "source[path]=/"

Headers: by default these hosts allow framing. If the project sets X-Frame-Options or CSP frame-ancestors, make it allow us:
- Cloudflare Pages / Netlify: file DIR/_headers (put it in the static files folder so the build copies it):
    /*
      Content-Security-Policy: frame-ancestors 'self' https://urna-mine-verine.wilianzilv.workers.dev
  and remove any "X-Frame-Options: DENY/SAMEORIGIN" line.
- Vercel: vercel.json in the project root:
    {"headers":[{"source":"/(.*)","headers":[{"key":"Content-Security-Policy","value":"frame-ancestors 'self' https://urna-mine-verine.wilianzilv.workers.dev"}]}]}
- GitHub Pages: cannot set headers, but sends none that block framing.

## C. Add the SDK to the game (it must keep working standalone)

Pick the portal id now: slug of the game name, [a-z0-9-], 2-32 chars (e.g. "space-goose"). Add to index.html <head>
(for Vite/CRA/Next: the root index.html / _document; Godot: Export > Web > HTML > Head Include; Unity: the
WebGL template's index.html or the built index.html):

  <script src="https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-portal.js" data-portal-id="space-goose"></script>

SDK behavior (guaranteed): never throws, never blocks. Outside our iframe it resolves null immediately
(state "standalone"); inside another site's iframe it resolves null after ~8 s. Start the game right away as a
guest and apply the player name/color when the session arrives. Events/exit without a session do nothing.

API: UrnaPortal.connect({portalId}) -> Promise<session|null>   session = {player:{name,color,character}, token, returnUrl, portalId}
     UrnaPortal.state  "connecting" | "connected" | "standalone"      UrnaPortal.playerJson() -> '{"name":..}' or 'null'
     UrnaPortal.event("score", number) | ("achievement", "text") | ("chat", "text")      UrnaPortal.exit()

Plain JS / canvas:
  UrnaPortal.connect({ portalId: "space-goose" }).then((s) => { if (s) { playerName = s.player.name; playerColor = s.player.color; } });
  // on game over:      UrnaPortal.event("score", score);
  // on a milestone:    UrnaPortal.event("achievement", "boss defeated");
  // a "back" button:   UrnaPortal.exit();

React:
  useEffect(() => { window.UrnaPortal?.connect({ portalId: "space-goose" }).then((s) => s && setPlayer(s.player)); }, []);
  // elsewhere: window.UrnaPortal?.event("score", score)

Phaser 3 (in your first scene's create()):
  window.UrnaPortal?.connect({ portalId: "space-goose" }).then((s) => { if (s) this.registry.set("player", s.player); });
  // game over: window.UrnaPortal?.event("score", this.score);

Godot 4 (GDScript, autoload "Urna.gd"):
  extends Node
  var player = null
  func _process(_d):
      if player == null and OS.has_feature("web"):
          var j = str(JavaScriptBridge.eval("window.UrnaPortal ? UrnaPortal.playerJson() : 'null'", true))
          if j != "null": player = JSON.parse_string(j)   # {"name","color","character"}
  func event(type: String, value) -> void:
      if OS.has_feature("web"):
          JavaScriptBridge.eval("window.UrnaPortal && UrnaPortal.event(%s, %s)" % [JSON.stringify(type), JSON.stringify(value)], true)
  func exit_to_urna() -> void:
      if OS.has_feature("web"): JavaScriptBridge.eval("window.UrnaPortal && UrnaPortal.exit()", true)

Unity WebGL: Assets/Plugins/WebGL/UrnaPortal.jslib
  mergeInto(LibraryManager.library, {
    UrnaPlayerJson: function () { var s = window.UrnaPortal ? UrnaPortal.playerJson() : "null"; var n = lengthBytesUTF8(s) + 1; var b = _malloc(n); stringToUTF8(s, b, n); return b; },
    UrnaEvent: function (type, value, text) { if (window.UrnaPortal) UrnaPortal.event(UTF8ToString(type), text ? UTF8ToString(text) : value); },
    UrnaExit: function () { if (window.UrnaPortal) UrnaPortal.exit(); }
  });
and C#:
  #if UNITY_WEBGL && !UNITY_EDITOR
  [System.Runtime.InteropServices.DllImport("__Internal")] static extern string UrnaPlayerJson();
  [System.Runtime.InteropServices.DllImport("__Internal")] static extern void UrnaEvent(string type, double value, string text);
  [System.Runtime.InteropServices.DllImport("__Internal")] static extern void UrnaExit();
  #endif
  // poll UrnaPlayerJson() until != "null"; UrnaEvent("score", score, null); UrnaEvent("achievement", 0, "boss defeated");

## D + E + G. Register, prove ownership, activate

You need a creator token (same account as /modding.txt). Store it in $HOME/.urna-creator-token, never inside DIR,
the project, or git. (If you must keep it in the project, name it .urna-portal-token, add it to .gitignore, and make
sure DIR is a build subfolder that does not contain it.)

### Option 1 - helper script (recommended, Node 18+, no dependencies)
  curl -sO https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-hub.mjs          # keep it OUTSIDE DIR (e.g. %TEMP% or /tmp)
  node urna-hub.mjs prepare --id space-goose --name "Space Goose" --url https://space-goose.pages.dev/ --dir <STATIC_OR_DIR> --colors "#ff3d7f,#ffd23d" --desc "goose shooter"
      -> registers a creator if needed (token saved to ~/.urna-creator-token), registers the portal (or a new version
         if --version is new), writes <STATIC_OR_DIR>/.well-known/urna-portal.json
  (build if needed and deploy DIR - step F - so https://space-goose.pages.dev/.well-known/urna-portal.json is online)
  node urna-hub.mjs publish --id space-goose          -> verifies the origin, activates, confirms it is live
  Later updates: change the game, redeploy; new URL/name/colors -> prepare with a higher --version, then publish.
  Take it down: node urna-hub.mjs unpublish --id space-goose

Where to point --dir so the file ends up at the SITE ROOT: Vite/CRA/Next -> public/ ; plain HTML -> the folder with
index.html ; Godot/Unity/Emscripten -> the export/build folder (re-run prepare or copy the file after every export).
Check after deploy: curl -s https://<host>/.well-known/urna-portal.json must print the JSON (no redirect, HTTP 200).

### Option 2 - curl (bash; in PowerShell use curl.exe and single-line commands)
  S=https://urna-mine-verine.wilianzilv.workers.dev
  curl -s -X POST $S/api/mods/register -H "content-type: application/json" -d '{"name":"space-goose-dev"}'
      # -> {"token":"umv_..."}  shown ONCE: save it to ~/.urna-creator-token  (409 = name taken: add digits)
  TOKEN=$(cat ~/.urna-creator-token)
  curl -s -X POST $S/api/portals -H "authorization: Bearer $TOKEN" -H "content-type: application/json" \
    -d '{"id":"space-goose","name":"Space Goose","version":"1.0.0","url":"https://space-goose.pages.dev/","description":"goose shooter","thumbnail":{"colors":["#ff3d7f","#ffd23d"]}}'
      # -> 201 {"portal":{"challenge":"upp-...","well_known":{"url":"...","body":{...}}}}
  # write portal.well_known.body EXACTLY to <STATIC_OR_DIR>/.well-known/urna-portal.json :
  #   {"urna_portal":1,"portals":[{"id":"space-goose","challenge":"upp-..."}]}
  # deploy, then:
  curl -s -X POST $S/api/portals/space-goose/verify -H "authorization: Bearer $TOKEN"
  curl -s -X POST $S/api/portals/space-goose/activate -H "authorization: Bearer $TOKEN" -H "content-type: application/json" -d '{"version":"1.0.0"}'

Manifest rules (/hub.json): id slug 2-32 (only on create, permanent, first come first served); name 2-32 chars;
version semver X.Y.Z, each new version greater; url https, default port, no IP/localhost, not our origin, <= 300 chars;
optional origin must equal url's origin; description <= 200; thumbnail.colors = up to 2 "#rrggbb" (arch swirl).
Text (name, description, events) must not contain links, "@", real money/crypto, keys/passwords.

## H. Check it is live

  curl -s https://urna-mine-verine.wilianzilv.workers.dev/api/portals      -> your portal with "live": true
Optional browser check: open https://urna-mine-verine.wilianzilv.workers.dev/?hub=<id> - you spawn in front of your
arch (Game Hub corridor, south of the lab); walk backwards (S) into it; the game opens with the player's name.
Esc / "VOLTAR PRA VILA" / browser back always return to the world.

## Troubleshooting

| Symptom                                | Cause / fix |
|----------------------------------------|-------------|
| verify: "HTTP 404"                     | File not at the origin ROOT: must be https://<host>/.well-known/urna-portal.json. Put it in the static folder (public/) or the export folder; GitHub Pages needs .nojekyll; project pages under /repo/ cannot work. |
| verify: "HTTP 301/302/308"             | Redirects are not followed: use the final https URL (with/without www exactly as served) in "url". |
| verify: "sem {id, challenge}" / old JSON | Stale deploy or CDN cache: redeploy, wait ~30 s, check with curl; the id/challenge must match GET /api/portals/<id> (owner). |
| verify: "nao e JSON"                   | Serve the raw JSON (no HTML wrapper, no BOM, no comments). |
| "origem ... nao verificada" on activate | The new version moved to another origin: verify again, then activate. |
| 401 / 403                              | Missing/wrong Bearer token, or the portal id belongs to another creator (pick another id). |
| 409 on version                         | Version must be greater than the last one. |
| 429                                    | Rate limit: wait a minute (writes 20/min, verify 6/min per portal, 120 req/min per IP). |
| arch opens but iframe is blank/refused | X-Frame-Options DENY/SAMEORIGIN or CSP frame-ancestors on your site: allow https://urna-mine-verine.wilianzilv.workers.dev (see headers). |
| player name never arrives              | SDK script missing or wrong data-portal-id; the game must be the exact registered url origin; check the console for postMessage errors. |
| Godot/Emscripten: SharedArrayBuffer error | Threads need cross-origin isolation, unavailable in the iframe: export without threads. |
| Unity: "unable to decompress"          | Compression Disabled or Decompression Fallback. |
| no sound                               | Browsers need a click/key first: resume your AudioContext on the first input. |
| mouse look does not lock               | Call requestPointerLock() inside a click handler (allowed in the iframe). |
| "Mixed content" errors                 | Every asset/API must be https. |
| CORS errors calling your own API       | Your API must allow your game origin; our /api/portals/verify already allows any origin. |
| game data/saves missing in the hub     | Storage inside iframes is partitioned per top site: saves made standalone are separate. |

## Reference

Runtime protocol (what the SDK does; implement it yourself only if you can't load the SDK). Always check
event.origin === "https://urna-mine-verine.wilianzilv.workers.dev" and post with exactly that targetOrigin.
  game -> urna  {type:"upp:hello", v:1, portalId}        every ~400 ms until upp:session
  urna -> game  {type:"upp:session", v:1, portalId, token, player:{name,color,character}, returnUrl, verifyUrl}
  game -> urna  {type:"upp:ready", v:1}                   counts the player on your arch
  game -> urna  {type:"upp:event", v:1, event:{type:"score", value:N}} | {type:"achievement"|"chat", text}
  game -> urna  {type:"upp:exit", v:1}                    back to the world, in front of the arch
Session token: JWT HS256 signed by the Urna server, ~10 min, claims {upp:1, iss, aud:<your origin>, sub:<player>,
pid, sid, player:{name,color,character}, ret:"<site>/?hub=<id>", iat, exp}.
Verify on your backend: GET /api/portals/verify?token=... -> {ok, portal, aud, sid, player, ret, exp} (401 if bad/expired).
Sandbox: iframe sandbox="allow-scripts allow-same-origin allow-pointer-lock", allow="fullscreen; gamepad; autoplay",
no popups/forms/top navigation. Scores pay 1-5 FICTIONAL coins (>= 30 s apart, <= 15/session, <= 60/player/day);
chat 1/3 s, achievements 1/10 s, <= 30 events/min; all text filtered. 8 arches are shown (oldest live first).

Endpoints:
  GET  /api/portals                         live portals (public)
  GET  /api/portals/:id                     detail (+ challenge, well_known, next when called by the owner)
  GET  /api/portals/verify?token=           verify a session token (CORS *)
  POST /api/portals/session                 {token, kind:"ready"|"exit"|"event", data}  (used by the Urna overlay)
  GET  /api/portals/mine                    (auth) your portals
  POST /api/portals                         (auth) create: manifest
  PUT  /api/portals/:id/versions            (auth, owner) new version: manifest
  POST /api/portals/:id/verify              (auth, owner) fetch + check /.well-known/urna-portal.json of the latest version's origin
  POST /api/portals/:id/activate            (auth, owner) {"version"} (default latest; origin must be verified)
  POST /api/portals/:id/rollback            (auth, owner) previous active version
  DELETE /api/portals/:id                   (auth, owner) unpublish (history kept; activate again any time)
  POST /api/mods/register                   {name} -> creator token (shown once; 3-24 chars [A-Za-z0-9_-]; 5/hour per IP)
Errors: {"ok":false,"error":"..."} with 400/401/403/404/409/413/422/429. Body <= 8 KB, <= 6 portals per creator.

## Finally

Tell the human in ONE sentence, in their language, e.g.:
"Pronto: teu jogo ta no ar no hub, no arco <Nome> - entra em https://urna-mine-verine.wilianzilv.workers.dev/?hub=<id>"
