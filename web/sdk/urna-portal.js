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
    UrnaPortal.presence = (() => {
        const subs = { player: [], join: [], leave: [], welcome: [], close: [] };
        const players = new Map();
        let ws = null, want = null, last = 0, flushT = null, joined = null, wanted = false;
        const emit = (k, v) => (subs[k] || []).forEach((f) => { try { f(v); } catch (e) { } });
        function flush() {
            flushT = null;
            if (!want || !ws || ws.readyState !== 1) return;
            const s = JSON.stringify({ t: "u", s: want });
            want = null;
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
                    for (const p of m.players || []) { players.set(p.sid, p); emit("player", p); }
                    emit("welcome", m);
                    resolve(m);
                } else if (m.t === "join") { players.set(m.player.sid, m.player); emit("join", m.player); emit("player", m.player); }
                else if (m.t === "u") { const p = players.get(m.sid); if (p) { p.s = m.s; emit("player", p); } }
                else if (m.t === "leave") { const p = players.get(m.sid); players.delete(m.sid); if (p) emit("leave", p); }
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
                want = { ...(want || {}), ...s };
                if (!flushT) flushT = setTimeout(flush, Math.max(0, 100 - (Date.now() - last)));
            },
            /** "player" (entrou/atualizou), "join", "leave", "welcome", "close". */
            on(k, f) { (subs[k] ||= []).push(f); return () => (subs[k] = subs[k].filter((x) => x !== f)); },
            leave() { wanted = false; joined = null; if (ws) ws.close(); },
        };
    })();

    root.UrnaPortal = UrnaPortal;
    if (typeof module === "object" && module.exports) module.exports = UrnaPortal;
    const me = typeof document !== "undefined" && document.currentScript;
    if (me && me.dataset && me.dataset.portalId) UrnaPortal.connect({ portalId: me.dataset.portalId, hostOrigin: me.dataset.host || undefined });
})(typeof window !== "undefined" ? window : this);
