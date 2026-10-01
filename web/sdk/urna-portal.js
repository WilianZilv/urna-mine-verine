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
                    if (!m || m.type !== "upp:session" || m.v !== 1 || (portalId && m.portalId !== portalId)) return;
                    session = { portalId: m.portalId, token: m.token, player: m.player || {}, returnUrl: m.returnUrl, verifyUrl: m.verifyUrl };
                    send({ type: "upp:ready", v: 1 });
                    done(session);
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
    };

    root.UrnaPortal = UrnaPortal;
    if (typeof module === "object" && module.exports) module.exports = UrnaPortal;
    const me = typeof document !== "undefined" && document.currentScript;
    if (me && me.dataset && me.dataset.portalId) UrnaPortal.connect({ portalId: me.dataset.portalId, hostOrigin: me.dataset.host || undefined });
})(typeof window !== "undefined" ? window : this);
