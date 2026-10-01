/*! Urna Portal Protocol (UPP) v1 - SDK do lado do jogo externo. Doc: https://urna-mine-verine.wilianzilv.workers.dev/hub.txt
 *
 *   <script src="https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-portal.js" data-portal-id="meu-jogo"></script>
 *   (data-portal-id = conecta sozinho; ou chame UrnaPortal.connect({ portalId: "meu-jogo" }))
 *
 *   const s = await UrnaPortal.connect({ portalId: "meu-jogo" });  // null fora do Urna (jogo segue normal como convidado)
 *   if (s) console.log(s.player.name, s.player.color, s.player.character);
 *   UrnaPortal.event("score", 1200);            // pode render moedas ficticias no Urna (limitado)
 *   UrnaPortal.event("achievement", "zerou a fase 1");
 *   UrnaPortal.event("chat", "gg");
 *   UrnaPortal.exit();                          // volta pra vila, na frente do arco
 *
 * Nunca lança erro e nunca trava o jogo: fora do iframe do Urna resolve null na hora; dentro de outro
 * iframe (itch.io etc.) resolve null depois do timeout. event()/exit() sem sessão não fazem nada.
 * Engines (Godot/Unity): leia UrnaPortal.state ("connecting"|"connected"|"standalone") e UrnaPortal.playerJson().
 *
 * Universo (doc: /universe.txt): s.avatar = pacote do avatar (skin) do jogador ou null; UrnaPortal.avatar();
 *   await UrnaPortal.passport()                 // {player, avatar, wallet, inventory, limits}
 *   const pr = UrnaPortal.presence; await pr.join(); pr.on("player", p => ...); pr.on("leave", p => ...); pr.update({x, y, anim});
 *   await UrnaPortal.grant({ coins: 5, reason: "fase 1", key: "fase1-" + runId })   // pedido; o servidor limita
 *   await UrnaPortal.spend({ coins: 10, reason: "vida extra", key: "vida-" + n })   // jogador confirma na tela do Urna
 *   pr.emote("wave"); pr.say("bora!"); pr.on("fx", m => ...); pr.on("say", m => ...)
 *
 * Modo auto (zero codigo): <script src=".../sdk/urna-portal.js" data-auto></script> ANTES do jogo. Conecta, entra na
 * presenca e, se o jogo usa three.js, poe os outros jogadores do Urna na cena (onde a camera deles esta) com nome.
 * Opcional: UrnaPortal.auto.attach({ THREE, scene, camera }) e UrnaPortal.auto.pose({ x, y, z, yaw, speed, air }).
 * Opt-out: data-auto="off", window.URNA_AUTO = false, <meta name="urna-auto" content="off"> ou UrnaPortal.auto.disable().
 */
(function (root) {
    "use strict";
    const HOST = "https://urna-mine-verine.wilianzilv.workers.dev";
    const embedded = (() => { try { return window.parent !== window; } catch (e) { return true; } })();
    let host = HOST, session = null, state = embedded ? "connecting" : "standalone", pending = null, escBound = false;

    function send(msg) {
        if (embedded) window.parent.postMessage(msg, host);
    }

    const UrnaPortal = {
        version: 1,
        get session() { return session; },
        get state() { return state; },
        get embedded() { return embedded; },
        /** JSON do jogador ("null" sem sessão): prático pra Godot JavaScriptBridge.eval / Unity jslib. */
        playerJson() { return JSON.stringify(session ? session.player : null); },

        /** Handshake: upp:hello até o Urna responder upp:session (só aceita da origem `hostOrigin`). Resolve sessão ou null. */
        connect(opts = {}) {
            if (pending) return pending;
            host = opts.hostOrigin || HOST;
            const portalId = String(opts.portalId || "");
            const timeout = opts.timeout || 8000;
            pending = new Promise((resolve) => {
                if (!embedded) {
                    state = "standalone";
                    return resolve(null);
                }
                let timer = null, tries = 0;
                const done = (s) => {
                    clearInterval(timer);
                    window.removeEventListener("message", onMsg);
                    state = s ? "connected" : "standalone";
                    resolve(s);
                };
                function onMsg(e) {
                    if (e.source !== window.parent || e.origin !== host) return;
                    const m = e.data;
                    if (!m || m.type !== "upp:session" || m.v !== 1 || (portalId && m.portalId !== portalId) || session) return;
                    session = { portalId: m.portalId, token: m.token, player: m.player || {}, returnUrl: m.returnUrl, verifyUrl: m.verifyUrl, avatar: null };
                    send({ type: "upp:ready", v: 1 });
                    clearInterval(timer);
                    const s = session;
                    if (!s.player.avatar || opts.passport === false) return done(s);
                    let fin = false;
                    const end = () => { if (!fin) { fin = true; done(s); } };
                    setTimeout(end, 4000);
                    UrnaPortal.passport().then((p) => { if (p && p.ok) { s.avatar = p.avatar || null; s.passport = p; } end(); });
                }
                window.addEventListener("message", onMsg);
                const hello = () => {
                    if (++tries * 400 > timeout) return done(null);
                    try { send({ type: "upp:hello", v: 1, portalId }); } catch (e) { done(null); }
                };
                hello();
                timer = setInterval(hello, 400);
                if (opts.escToExit !== false && !escBound) {
                    escBound = true;
                    window.addEventListener("keydown", (e) => { if (e.key === "Escape" && session && !document.pointerLockElement) UrnaPortal.exit("esc"); });
                }
            });
            return pending;
        },

        /** type: "score" (value número) | "achievement" (text) | "chat" (text). O servidor filtra e limita. */
        event(type, value) {
            if (!session) return;
            const ev = { type: String(type) };
            if (typeof value === "number") ev.value = value;
            else ev.text = String(value ?? "").slice(0, 160);
            send({ type: "upp:event", v: 1, event: ev });
        },

        exit(reason) {
            if (session) send({ type: "upp:exit", v: 1, reason: String(reason || "") });
        },

        /** Confere o token no servidor (útil no backend do jogo): {ok, portal, aud, player, exp}. */
        async verify(token) {
            const r = await fetch(`${host}/api/portals/verify?token=${encodeURIComponent(token || (session && session.token) || "")}`);
            return r.json();
        },

        /** Pacote JSON do avatar (mod kind "avatar") do jogador, ou null. Render: /sdk/urna-avatar-three.js | urna-avatar-canvas2d.js */
        avatar() { return (session && session.avatar) || null; },

        /** Passaporte fresco: {ok, player, avatar, avatar_ref, wallet:{coins}, inventory:[...], limits}. Sem sessao: null. */
        async passport() {
            if (!session) return null;
            try { return await (await fetch(`${host}/api/passport?token=${encodeURIComponent(session.token)}`)).json(); } catch (e) { return null; }
        },

        /** PEDIDO de premio: {coins:N} ou {item:"id", n}, + reason e key (mesma key = mesmo pedido, nunca paga 2x).
         *  O servidor decide (orcamento/dia por jogo, allowlist de itens). Resolve {ok, granted, ...} ou {ok:false, error}. */
        async grant(req = {}) {
            if (!session) return { ok: false, error: "no_session" };
            const body = { token: session.token, reason: req.reason, key: req.key || `g-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}` };
            if (req.coins !== undefined) body.coins = req.coins; else { body.item = req.item; body.n = req.n; }
            try {
                const r = await fetch(`${host}/api/universe/grant`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
                return await r.json();
            } catch (e) { return { ok: false, error: "network" }; }
        },

        /** Gasta moedas/itens do jogador: o Urna mostra um dialogo de confirmacao NA PAGINA DELE (o jogo nao consegue pular).
         *  {coins:N} ou {item:"id", n}, + reason e key. Resolve {ok, spent, ...} ou {ok:false, error:"declined"|"timeout"|...}. */
        spend(req = {}) {
            if (!session) return Promise.resolve({ ok: false, error: "no_session" });
            const rid = Math.random().toString(36).slice(2, 12);
            const r = { reason: String(req.reason || "").slice(0, 80), key: req.key || `s-${Date.now().toString(36)}-${rid}` };
            if (req.coins !== undefined) r.coins = Number(req.coins); else { r.item = String(req.item || ""); r.n = Number(req.n || 1); }
            return new Promise((resolve) => {
                const t = setTimeout(() => { spends.delete(rid); resolve({ ok: false, error: "timeout" }); }, 120000);
                spends.set(rid, (res) => { clearTimeout(t); resolve(res); });
                send({ type: "upp:spend", v: 1, rid, req: r });
            });
        },

        presence: null,
    };

    // ---------------------------------------------------------------- spend: resposta do overlay do Urna
    const spends = new Map();
    if (embedded) window.addEventListener("message", (e) => {
        const m = e.data;
        if (e.source !== window.parent || e.origin !== host || !m || m.type !== "upp:spend_result") return;
        const cb = spends.get(m.rid);
        if (cb) { spends.delete(m.rid); cb(m.result || { ok: false, error: "bad_result" }); }
    });

    // ---------------------------------------------------------------- presenca: quem do Urna ta no mesmo jogo
    // Servidor so repassa: <= 12 msgs/s, estado <= 512 bytes, <= 32 jogadores. Aqui: update() manda no maximo ~10 Hz.
    // O servidor guarda o ULTIMO estado inteiro de cada um: mandamos sempre o estado completo (mine) e de novo a
    // cada welcome, entao quem entra depois (ou reconecta) ve quem esta parado.
    UrnaPortal.presence = (() => {
        const subs = { player: [], join: [], leave: [], welcome: [], close: [], fx: [], say: [] };
        const players = new Map();
        let ws = null, mine = null, dirty = false, last = 0, flushT = null, joined = null, wanted = false, you = null;
        const emit = (k, v) => (subs[k] || []).forEach((f) => { try { f(v); } catch (e) { } });
        const raw = (o) => { if (ws && ws.readyState === 1) try { ws.send(JSON.stringify(o)); } catch (e) { } };
        function flush() {
            flushT = null;
            if (!dirty || !mine || !ws || ws.readyState !== 1) return;
            const s = JSON.stringify({ t: "u", s: mine });
            dirty = false;
            last = Date.now();
            if (s.length <= 560) ws.send(s);
        }
        function open(resolve) {
            const url = `${host.replace(/^http/, "ws")}/api/portals/${encodeURIComponent(session.portalId)}/presence?token=${encodeURIComponent(session.token)}`;
            try { ws = new WebSocket(url); } catch (e) { return resolve(null); }
            ws.onmessage = (e) => {
                let m;
                try { m = JSON.parse(e.data); } catch (x) { return; }
                if (m.t === "welcome") {
                    players.clear();
                    you = m.you || null;
                    for (const p of m.players || []) { players.set(p.sid, p); emit("player", p); }
                    emit("welcome", m);
                    resolve(m);
                    dirty = true;
                    flush();
                } else if (m.t === "join") {
                    const old = players.get(m.player.sid);
                    if (old) { Object.assign(old, m.player); emit("player", old); }
                    else { players.set(m.player.sid, m.player); emit("join", m.player); emit("player", m.player); }
                }
                else if (m.t === "u") { const p = players.get(m.sid); if (p) { p.s = m.s; emit("player", p); } }
                else if (m.t === "leave") { const p = players.get(m.sid); players.delete(m.sid); if (p) emit("leave", p); }
                else if (m.t === "fx" || m.t === "say") emit(m.t, { sid: m.sid, self: !!you && m.sid === you.sid, player: players.get(m.sid) || you, e: m.e, text: m.m });
            };
            ws.onclose = () => {
                for (const p of players.values()) emit("leave", p);
                players.clear();
                emit("close", null);
                resolve(null);
                if (wanted) setTimeout(() => { if (wanted && session) joined = new Promise(open); }, 2000);
            };
        }
        return {
            players,
            /** Entra na sala de presenca desse portal. Resolve {you, players} ou null (sem sessao/erro). */
            join() {
                if (!session) return Promise.resolve(null);
                wanted = true;
                return joined || (joined = new Promise(open));
            },
            /** Estado livre ate 16 chaves (numero/bool/texto<=32/[<=4 numeros]), ex {x, y, z, anim:"run", dir:1}. */
            update(s) {
                mine = { ...(mine || {}), ...s };
                dirty = true;
                if (!flushT) flushT = setTimeout(flush, Math.max(0, 100 - (Date.now() - last)));
            },
            /** Reacao (aparece no overlay do Urna e pra todo SDK): "wave"|"laugh"|"love"|"fire"|"clap"|"wow"|"dance"|"gg". */
            emote(e) { raw({ t: "fx", e: String(e) }); },
            /** Balao de chat rapido (<= 80 letras, filtrado no servidor). */
            say(text) { raw({ t: "say", m: String(text || "").slice(0, 80) }); },
            get you() { return you; },
            /** "player" (entrou/atualizou), "join", "leave", "welcome", "close", "fx" {sid, e, self}, "say" {sid, text, self}. */
            on(k, f) { (subs[k] ||= []).push(f); return () => (subs[k] = subs[k].filter((x) => x !== f)); },
            leave() { wanted = false; joined = null; if (ws) ws.close(); },
        };
    })();

    // ---------------------------------------------------------------- auto: outros jogadores do Urna em 3D, sem codigo
    // <script src=".../urna-portal.js" data-auto></script> ANTES do jogo. Com three.js: pega renderer/cena/camera pelo
    // gancho __THREE_DEVTOOLS__ ("observe") + render() embrulhado, publica a camera local (_p = [x,y,z,yaw], _k = "cam")
    // e poe os avatares dos outros na cena onde a camera deles esta, com nome. Sem three.js nao faz nada aqui: o
    // overlay do Urna em volta do iframe ja mostra todo mundo. Tudo em try/catch: nunca quebra o jogo.
    UrnaPortal.auto = (() => {
        const AV_URL = "/sdk/urna-avatar-three.js";
        const EMO = { wave: "\u{1F44B}", laugh: "\u{1F602}", love: "\u2764\uFE0F", fire: "\u{1F525}", clap: "\u{1F44F}", wow: "\u{1F62E}", dance: "\u{1F483}", gg: "GG" };
        let on = false, off = false, T = null, tLoad = null, scene = null, camera = null, group = null, opts = {}, body = null, bodyAt = 0;
        let lastTick = 0, lastSend = 0, sent = null, avLoad = null, patched = false;
        const avs = new Map(), pkgs = new Map();
        const safe = (f) => { try { return f(); } catch (e) { return undefined; } };
        const P = () => UrnaPortal.presence;

        function loadThree() {
            if (T || tLoad) return;
            T = opts.THREE || root.THREE || null;
            if (T) return;
            const rev = parseInt(root.__THREE__, 10);
            if (!rev) return;
            tLoad = import(`https://cdn.jsdelivr.net/npm/three@0.${rev}.0/build/three.module.js`).then((m) => { T = m; }, () => { });
        }
        function loadAvatarLib() {
            if (root.UrnaAvatarThree || avLoad || typeof document === "undefined") return;
            avLoad = new Promise((res) => {
                const s = document.createElement("script");
                s.src = host + AV_URL;
                s.onload = s.onerror = res;
                (document.head || document.documentElement).appendChild(s);
            });
        }
        function hook() {
            safe(() => {
                let dt = root.__THREE_DEVTOOLS__;
                if (!dt) root.__THREE_DEVTOOLS__ = dt = new EventTarget();
                dt.addEventListener("observe", (e) => safe(() => { if (e.detail && e.detail.isWebGLRenderer) patch(e.detail); }));
            });
        }
        /** Embrulha renderer.render: a cena/camera perspectiva desenhada e a "ativa". */
        function patch(r) {
            if (!r || r.__urnaAuto || typeof r.render !== "function") return;
            r.__urnaAuto = patched = true;
            const orig = r.render;
            r.render = function (sc, cam) {
                if (!off) safe(() => { if (sc && sc.isScene && cam && cam.isPerspectiveCamera) tick(sc, cam); });
                return orig.apply(this, arguments);
            };
        }

        function tagSprite(text, color) {
            const cv = document.createElement("canvas"), g = cv.getContext("2d");
            cv.width = 256; cv.height = 64;
            g.font = "bold 30px system-ui, sans-serif"; g.textAlign = "center"; g.textBaseline = "middle";
            const w = Math.min(250, g.measureText(text).width + 28);
            g.fillStyle = "rgba(0,0,0,.55)"; g.beginPath(); (g.roundRect ? g.roundRect(128 - w / 2, 8, w, 48, 22) : g.rect(128 - w / 2, 8, w, 48)); g.fill();
            g.fillStyle = color || "#fff"; g.fillRect(128 - w / 2 + 14, 50, w - 28, 3);
            g.fillStyle = "#fff"; g.fillText(text, 128, 31);
            const tex = new T.CanvasTexture(cv);
            if ("colorSpace" in tex && T.SRGBColorSpace) tex.colorSpace = T.SRGBColorSpace;
            const s = new T.Sprite(new T.SpriteMaterial({ map: tex, depthWrite: false, fog: false, transparent: true }));
            s.scale.set(1.6, 0.4, 1);
            s.renderOrder = 10;
            return s;
        }
        function blocky(color) {
            const g = new T.Group(), M = (c) => new T.MeshLambertMaterial({ color: c });
            const add = (w, h, d, m, y) => { const b = new T.Mesh(new T.BoxGeometry(w, h, d), m); b.position.y = y; g.add(b); };
            const c = new T.Color(color || "#00e5ff");
            add(0.5, 0.78, 0.26, M(c.clone().multiplyScalar(0.45)), 0.39);
            add(0.56, 0.72, 0.3, M(c), 1.14);
            add(0.42, 0.42, 0.42, M(0xe9b98f), 1.72);
            return { group: g, height: 1.95, update() { } };
        }
        function makeAvatar(p, pkg) {
            const custom = opts.avatar && safe(() => opts.avatar(p, pkg, T));
            if (custom && custom.group) return custom;
            const built = pkg && root.UrnaAvatarThree && safe(() => root.UrnaAvatarThree.build(pkg, T));
            return built || blocky(p.color);
        }
        function pkgOf(ref, cb) {
            if (!ref) return;
            if (pkgs.has(ref)) return void (pkgs.get(ref) && cb(pkgs.get(ref)));
            pkgs.set(ref, null);
            loadAvatarLib();
            const [id, v] = String(ref).split("@");
            fetch(`${host}/api/mods/${encodeURIComponent(id)}/versions/${encodeURIComponent(v)}`).then((r) => r.json()).then(async (pkg) => {
                await avLoad;
                pkgs.set(ref, pkg);
                cb(pkg);
            }).catch(() => { });
        }
        function actor(p) {
            const a = { root: new T.Group(), av: makeAvatar(p, null), tag: tagSprite(p.name || "?", p.color), tgt: null, s: {}, fx: null, fxT: 0, say: null, sayT: 0 };
            a.tag.position.y = (a.av.height || 1.9) + 0.35;
            a.root.add(a.av.group, a.tag);
            a.setAvatar = (pkg) => {
                const n = makeAvatar(p, pkg);
                a.root.remove(a.av.group); safe(() => a.av.dispose && a.av.dispose());
                a.av = n; a.root.add(n.group); a.tag.position.y = (n.height || 1.9) + 0.35;
            };
            pkgOf(p.avatar, (pkg) => a.setAvatar(pkg));
            return a;
        }
        function sync(p) {
            const st = p.s || {}, q = st._p;
            if (!T || !group || !Array.isArray(q) || q.length < 3) return;
            let a = avs.get(p.sid);
            const eye = st._k === "cam" ? (opts.eye != null ? opts.eye : 1.6) : 0;
            const t = [q[0], q[1] - eye, q[2], q[3] || 0];
            if (!a) {
                a = actor(p);
                a.root.position.set(t[0], t[1], t[2]);
                avs.set(p.sid, a);
                group.add(a.root);
            }
            a.tgt = t;
            a.s = st;
        }
        function drop(p) {
            const a = avs.get(p.sid);
            if (!a) return;
            group && group.remove(a.root);
            avs.delete(p.sid);
        }
        function bubble(a, text, key, secs) {
            if (a[key]) a.root.remove(a[key]);
            const s = tagSprite(text, null);
            s.scale.set(key === "fx" ? 0.9 : 2.2, key === "fx" ? 0.9 : 0.55, 1);
            s.position.y = (a.av.height || 1.9) + (key === "fx" ? 1.2 : 0.85);
            a.root.add(s);
            a[key] = s; a[key + "T"] = secs;
        }

        function tick(sc, cam) {
            const now = performance.now(), dt = Math.min(0.1, (now - lastTick) / 1000);
            if (!session || now - lastTick < 2) return;
            lastTick = now;
            scene = opts.scene || sc;
            camera = opts.camera || cam;
            loadThree();
            if (!T) return;
            if (!group) {
                group = new T.Group();
                group.name = "urna-auto-players";
                for (const p of P().players.values()) sync(p);
            }
            if (group.parent !== scene) scene.add(group);
            // pose local: corpo explicito (auto.pose) recente vale mais que a camera
            const nowMs = Date.now();
            let pose;
            if (body && nowMs - bodyAt < 1500) pose = { p: [body.x, body.y, body.z, body.yaw || 0], k: "body", v: body.speed || 0, j: !!body.air };
            else {
                camera.updateMatrixWorld();
                const e = camera.matrixWorld.elements, d = [-e[8], -e[10]];
                pose = { p: [e[12], e[13], e[14], Math.atan2(d[0], d[1])], k: "cam", v: 0, j: false };
            }
            const r2 = (x) => Math.round(x * 100) / 100;
            const st = { _p: pose.p.map(r2), _k: pose.k, _v: r2(pose.v), _j: pose.j };
            const key = JSON.stringify(st);
            if (key !== sent && nowMs - lastSend > 100) { sent = key; lastSend = nowMs; P().update(st); }
            const k = 1 - Math.exp(-10 * dt);
            for (const a of avs.values()) {
                if (!a.tgt) continue;
                const R = a.root.position;
                R.x += (a.tgt[0] - R.x) * k; R.y += (a.tgt[1] - R.y) * k; R.z += (a.tgt[2] - R.z) * k;
                const g = a.av.group, dy = Math.atan2(Math.sin(a.tgt[3] - g.rotation.y), Math.cos(a.tgt[3] - g.rotation.y));
                g.rotation.y += dy * k;
                safe(() => a.av.update && a.av.update(dt, { speed: a.s._v || 0, moving: (a.s._v || 0) > 0.2, air: !!a.s._j }));
                for (const b of ["fx", "say"]) if (a[b] && (a[b + "T"] -= dt) <= 0) { a.root.remove(a[b]); a[b] = null; }
                if (a.fx) a.fx.position.y += dt * 0.4;
            }
        }

        function start(o = {}) {
            opts = { ...opts, ...o };
            if (opts.THREE) T = opts.THREE;
            safe(() => { if (opts.renderer) patch(opts.renderer); });
            if (on || off) return UrnaPortal.auto;
            on = true;
            hook();
            UrnaPortal.connect({ portalId: opts.portalId || "", hostOrigin: opts.hostOrigin }).then((s) => {
                if (!s || off) return;
                const pr = P();
                pr.on("player", (p) => safe(() => sync(p)));
                pr.on("leave", (p) => safe(() => drop(p)));
                pr.on("close", () => safe(() => { for (const p of [...avs.keys()]) drop({ sid: p }); }));
                pr.on("fx", (m) => safe(() => { const a = avs.get(m.sid); if (a) bubble(a, EMO[m.e] || m.e, "fx", 2.5); }));
                pr.on("say", (m) => safe(() => { const a = avs.get(m.sid); if (a) bubble(a, m.text, "say", 5); }));
                pr.join();
            });
            return UrnaPortal.auto;
        }
        return {
            /** Liga o modo auto (o data-auto chama sozinho). opts: {portalId, THREE, renderer, scene, camera, eye, avatar(player, pkg, THREE)}. */
            start,
            /** API explicita: entrega THREE/cena/camera (e opcional renderer, avatar factory) em vez de detectar. */
            attach(o) { return start(o || {}); },
            /** Pose do CORPO do jogador local (mundo); sem isso o auto publica a camera. {x, y, z, yaw, speed, air}. */
            pose(b) { body = b; bodyAt = Date.now(); if (opts.scene && opts.camera && !patched) safe(() => tick(opts.scene, opts.camera)); },
            /** Opt-out: some com os avatares e para de publicar a camera (o overlay do Urna continua). */
            disable() { off = true; safe(() => { if (group && group.parent) group.parent.remove(group); }); },
            get enabled() { return on && !off; },
            avatars: avs,
        };
    })();

    root.UrnaPortal = UrnaPortal;
    if (typeof module === "object" && module.exports) module.exports = UrnaPortal;
    const me = typeof document !== "undefined" && document.currentScript;
    const autoOff = () => root.URNA_AUTO === false || safeMeta();
    function safeMeta() { try { return /^(off|false|0)$/i.test(document.querySelector('meta[name="urna-auto"]')?.content || ""); } catch (e) { return false; } }
    if (me && me.dataset && "auto" in me.dataset && !/^(off|false|0)$/i.test(me.dataset.auto) && !autoOff()) UrnaPortal.auto.start({ portalId: me.dataset.portalId || "", hostOrigin: me.dataset.host || undefined });
    else if (me && me.dataset && me.dataset.portalId) UrnaPortal.connect({ portalId: me.dataset.portalId, hostOrigin: me.dataset.host || undefined });
})(typeof window !== "undefined" ? window : this);
