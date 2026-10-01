"use strict";
// Universo, lado da pagina do Urna: confirma gastos pedidos pelo jogo do portal (UrnaPortal.spend).
// O dialogo e DOM desta pagina (fora do iframe) e so ela tem o token de confirmacao (veio pelo WebSocket do jogo,
// via src/universe.rs -> urna_uv_ctok). O jogo so recebe o resultado por postMessage.
(function () {
    const dec = new TextDecoder();
    let ctok = null, busy = false;

    const css = document.createElement("style");
    css.textContent = `
#uv-confirm { position: fixed; inset: 0; z-index: 80; display: flex; align-items: center; justify-content: center; background: rgba(5,2,12,.72); font: 15px monospace; color: #fff; }
#uv-confirm .box { background: #1a0d33; border: 2px solid #b48cff; border-radius: 8px; padding: 18px 22px; max-width: 420px; box-shadow: 0 0 40px #6b2cff88; }
#uv-confirm h3 { margin: 0 0 8px; color: #ffd23d; font-size: 16px; }
#uv-confirm p { margin: 6px 0; line-height: 1.4; }
#uv-confirm .row { display: flex; gap: 10px; margin-top: 14px; justify-content: flex-end; }
#uv-confirm button { font: bold 14px monospace; border: 0; border-radius: 4px; padding: 8px 16px; cursor: pointer; color: #fff; }
#uv-confirm button.yes { background: #2fa84f; } #uv-confirm button.no { background: #a83a3a; }
#uv-confirm button:disabled { opacity: .45; cursor: wait; }`;
    document.head.appendChild(css);

    const frame = () => document.querySelector("#upp-overlay iframe");
    const reply = (f, origin, rid, result) => { try { f.contentWindow.postMessage({ type: "upp:spend_result", v: 1, rid, result }, origin); } catch (e) { } };

    function ask(text) {
        return new Promise((resolve) => {
            const el = document.createElement("div");
            el.id = "uv-confirm";
            el.innerHTML = '<div class="box"><h3>CONFIRMAR GASTO (moeda ficticia)</h3><p class="t"></p><p style="color:#a99">So voce pode confirmar: o jogo nao ve nem clica nesta janela.</p><div class="row"><button class="no" type="button">NAO</button><button class="yes" type="button" disabled>SIM, GASTAR</button></div></div>';
            el.querySelector(".t").textContent = text;
            const yes = el.querySelector(".yes"), no = el.querySelector(".no");
            const done = (v) => { el.remove(); window.removeEventListener("keydown", key, true); resolve(v); };
            const key = (e) => { if (e.key === "Escape") { e.stopImmediatePropagation(); e.preventDefault(); done(false); } };
            yes.onclick = () => done(true);
            no.onclick = () => done(false);
            window.addEventListener("keydown", key, true);
            document.body.appendChild(el);
            setTimeout(() => (yes.disabled = false), 700);
        });
    }

    window.addEventListener("message", async (e) => {
        const f = frame();
        const m = e.data;
        if (!f || e.source !== f.contentWindow || !m || m.type !== "upp:spend" || m.v !== 1) return;
        let origin;
        try { origin = new URL(f.src).origin; } catch (x) { return; }
        if (e.origin !== origin) return;
        const rid = String(m.rid || "").slice(0, 24), r = m.req && typeof m.req === "object" ? m.req : {};
        if (busy) return reply(f, origin, rid, { ok: false, error: "busy" });
        if (!ctok) return reply(f, origin, rid, { ok: false, error: "no_confirm_token" });
        const reason = String(r.reason || "").replace(/[\u0000-\u001f<>]/g, "").slice(0, 80);
        const what = r.coins !== undefined ? `${Math.floor(Number(r.coins)) || 0} moedas` : `${Math.floor(Number(r.n)) || 1}x item "${String(r.item || "").slice(0, 32)}"`;
        busy = true;
        const ok = await ask(`O jogo ${origin} quer gastar ${what}${reason ? ` — motivo: ${reason}` : ""}.`);
        busy = false;
        if (!ok) return reply(f, origin, rid, { ok: false, error: "declined" });
        const body = { ctok, reason, key: String(r.key || "").slice(0, 64) };
        if (r.coins !== undefined) body.coins = Number(r.coins); else { body.item = String(r.item || ""); body.n = Number(r.n || 1); }
        let result;
        try {
            result = await (await fetch("/api/universe/spend", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) })).json();
        } catch (x) { result = { ok: false, error: "network" }; }
        reply(f, origin, rid, result);
    });

    miniquad_add_plugin({
        register_plugin: function (io) {
            io.env.urna_uv_ctok = (p, n) => {
                try { ctok = JSON.parse(dec.decode(new Uint8Array(wasm_memory.buffer, p, n))).ctok || null; } catch (e) { }
            };
        },
    });
})();
