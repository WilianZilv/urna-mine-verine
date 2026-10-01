"use strict";
// Game Hub (Urna Portal Protocol v1), lado do Urna: abre o jogo externo num iframe sandbox em tela cheia,
// entrega o token de sessão por postMessage (origem conferida nos dois lados) e repassa eventos ao servidor.
// Esc, o botão VOLTAR e o voltar do navegador sempre devolvem o jogador pra vila. Funções usadas em src/hub.rs.
(function () {
    const enc = new TextEncoder(), dec = new TextDecoder();
    const str = (p, n) => dec.decode(new Uint8Array(wasm_memory.buffer, p, n));
    function put(bytes, ptr, cap) {
        if (bytes.length > cap) return -bytes.length;
        new Uint8Array(wasm_memory.buffer, ptr, bytes.length).set(bytes);
        return bytes.length;
    }

    const css = document.createElement("style");
    css.textContent = `
#upp-overlay { position: fixed; inset: 0; z-index: 50; background: #07030f; display: flex; flex-direction: column; }
#upp-overlay .upp-bar { height: 38px; flex: 0 0 38px; display: flex; align-items: center; gap: 14px; padding: 0 10px; background: #1a0d33; color: #cdb8ff; font: 14px monospace; }
#upp-overlay button { font: bold 14px monospace; color: #fff; background: #6b2cff; border: 0; border-radius: 4px; padding: 7px 14px; cursor: pointer; }
#upp-overlay iframe { flex: 1; width: 100%; border: 0; background: #000; }`;
    document.head.appendChild(css);

    let cur = null;
    let sent = [];

    function claims(tok) {
        try {
            const s = tok.split(".")[1].replace(/-/g, "+").replace(/_/g, "/");
            return JSON.parse(dec.decode(Uint8Array.from(atob(s), (c) => c.charCodeAt(0))));
        } catch (e) {
            return {};
        }
    }

    function post(tok, kind, data) {
        fetch("/api/portals/session", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ token: tok, kind, data }), keepalive: true }).catch(() => { });
    }

    function open(info) {
        if (cur) close();
        let origin;
        try { origin = new URL(info.url).origin; } catch (e) { return; }
        // Nunca na nossa origem: allow-scripts + allow-same-origin aqui furaria o sandbox.
        if (!/^https:\/\//.test(info.url) || origin !== info.origin || origin === location.origin) return;
        const el = document.createElement("div");
        el.id = "upp-overlay";
        el.innerHTML = '<div class="upp-bar"><button type="button">&larr; VOLTAR PRA VILA (Esc)</button><span></span></div>';
        el.querySelector("span").textContent = `${info.name} — ${origin}`;
        const f = document.createElement("iframe");
        f.setAttribute("sandbox", "allow-scripts allow-same-origin allow-pointer-lock");
        f.setAttribute("allow", "fullscreen; gamepad; autoplay");
        f.setAttribute("referrerpolicy", "no-referrer");
        f.src = info.url;
        el.appendChild(f);
        document.body.appendChild(el);
        el.querySelector("button").onclick = () => close();
        if (document.exitPointerLock) document.exitPointerLock();
        history.pushState({ upp: 1 }, "");
        cur = { el, frame: f, origin, tok: info.tok, pid: info.p, ret: info.ret, player: claims(info.tok).player || {}, ready: false };
        f.focus();
    }

    function close(fromHistory) {
        if (!cur) return;
        const c = cur;
        cur = null;
        post(c.tok, "exit");
        c.el.remove();
        if (!fromHistory) history.back();
        const cv = document.getElementById("glcanvas");
        if (cv) cv.focus();
    }

    window.addEventListener("message", (e) => {
        if (!cur || e.source !== cur.frame.contentWindow || e.origin !== cur.origin) return;
        const m = e.data;
        if (!m || typeof m !== "object" || typeof m.type !== "string") return;
        switch (m.type) {
            case "upp:hello":
                cur.frame.contentWindow.postMessage({ type: "upp:session", v: 1, portalId: cur.pid, token: cur.tok, player: cur.player, returnUrl: cur.ret, verifyUrl: `${location.origin}/api/portals/verify` }, cur.origin);
                break;
            case "upp:ready":
                if (!cur.ready) {
                    cur.ready = true;
                    post(cur.tok, "ready");
                }
                break;
            case "upp:event": {
                const now = Date.now();
                sent = sent.filter((t) => now - t < 10000);
                const ev = m.event && typeof m.event === "object" ? m.event : {};
                if (sent.length >= 10 || JSON.stringify(ev).length > 600) break;
                sent.push(now);
                post(cur.tok, "event", { type: String(ev.type || ""), value: Number(ev.value) || 0, text: typeof ev.text === "string" ? ev.text.slice(0, 160) : undefined });
                break;
            }
            case "upp:exit":
                close();
                break;
        }
    });
    window.addEventListener("keydown", (e) => {
        if (!cur || e.key !== "Escape") return;
        e.preventDefault();
        e.stopImmediatePropagation();
        close();
    }, true);
    window.addEventListener("popstate", () => close(true));

    miniquad_add_plugin({
        register_plugin: function (io) {
            Object.assign(io.env, {
                urna_hub_open: (p, n) => { try { open(JSON.parse(str(p, n))); } catch (e) { } },
                urna_hub_state: () => (cur ? 1 : 0),
                urna_hub_query: (p, cap) => put(enc.encode((new URLSearchParams(location.search).get("hub") || "").trim().slice(0, 40)), p, cap),
            });
        },
    });
})();
