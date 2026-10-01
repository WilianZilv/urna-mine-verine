/*! Urna Portal Protocol (UPP) v1 - SDK do lado do jogo externo. Doc: https://urna-mine-verine.wilianzilv.workers.dev/hub.txt
 *
 *   <script src="https://urna-mine-verine.wilianzilv.workers.dev/sdk/urna-portal.js"></script>
 *   const s = await UrnaPortal.connect({ portalId: "meu-jogo" });   // { player: {name,color,character}, token, returnUrl, ... }
 *   UrnaPortal.event("score", 1200);            // pode render moedas ficticias no Urna (limitado)
 *   UrnaPortal.event("achievement", "zerou a fase 1");
 *   UrnaPortal.event("chat", "gg");
 *   UrnaPortal.exit();                          // volta pra vila, na frente do arco
 *
 * Fora do Urna (aberto direto), connect() rejeita: rode o jogo normal como convidado.
 */
(function (root) {
    "use strict";
    const HOST = "https://urna-mine-verine.wilianzilv.workers.dev";
    let host = HOST, session = null;

    function send(msg) {
        if (window.parent !== window) window.parent.postMessage(msg, host);
    }

    const UrnaPortal = {
        version: 1,
        get session() { return session; },
        get embedded() { return window.parent !== window; },

        /** Handshake: manda upp:hello até o Urna responder upp:session (só aceita da origem `hostOrigin`). */
        connect(opts = {}) {
            host = opts.hostOrigin || HOST;
            const portalId = String(opts.portalId || "");
            const timeout = opts.timeout || 15000;
            return new Promise((resolve, reject) => {
                if (window.parent === window) return reject(new Error("fora do Urna (sem iframe pai)"));
                let timer = null, tries = 0;
                function onMsg(e) {
                    if (e.source !== window.parent || e.origin !== host) return;
                    const m = e.data;
                    if (!m || m.type !== "upp:session" || m.v !== 1 || (portalId && m.portalId !== portalId)) return;
                    clearInterval(timer);
                    window.removeEventListener("message", onMsg);
                    session = { portalId: m.portalId, token: m.token, player: m.player || {}, returnUrl: m.returnUrl, verifyUrl: m.verifyUrl };
                    send({ type: "upp:ready", v: 1 });
                    resolve(session);
                }
                window.addEventListener("message", onMsg);
                const hello = () => {
                    if (++tries * 400 > timeout) {
                        clearInterval(timer);
                        window.removeEventListener("message", onMsg);
                        return reject(new Error("Urna nao respondeu"));
                    }
                    send({ type: "upp:hello", v: 1, portalId });
                };
                hello();
                timer = setInterval(hello, 400);
                if (opts.escToExit !== false) {
                    window.addEventListener("keydown", (e) => { if (e.key === "Escape" && session && !document.pointerLockElement) UrnaPortal.exit("esc"); });
                }
            });
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
            send({ type: "upp:exit", v: 1, reason: String(reason || "") });
        },

        /** Confere o token no servidor (útil no backend do jogo): {ok, portal, aud, player, exp}. */
        async verify(token) {
            const r = await fetch(`${host}/api/portals/verify?token=${encodeURIComponent(token || (session && session.token) || "")}`);
            return r.json();
        },
    };

    root.UrnaPortal = UrnaPortal;
    if (typeof module === "object" && module.exports) module.exports = UrnaPortal;
})(typeof window !== "undefined" ? window : this);
