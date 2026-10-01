"use strict";
// Presenca sem codigo no jogo: a pagina do Urna e dona do overlay em volta do iframe sandbox (web/hub.js), entao
// ela mesma entra na presenca do portal (?via=overlay, mesmo token de sessao) e desenha, POR CIMA do iframe, quem do
// Urna esta no mesmo jogo: cards com busto do avatar (pacote do passaporte via UrnaAvatar2D), nome, status ao vivo,
// reacoes que voam pela tela, baloes de chat rapido e "X jogadores aqui". Funciona em qualquer portal, com ou sem SDK.
// So os nossos botoes/cards pegam clique (pointer-events), o resto passa direto pro jogo.
(function () {
    const EMO = [["wave", "\u{1F44B}", "acenar"], ["laugh", "\u{1F602}", "rir"], ["love", "\u2764\uFE0F", "amei"], ["fire", "\u{1F525}", "fogo"], ["clap", "\u{1F44F}", "palmas"], ["wow", "\u{1F62E}", "uau"], ["dance", "\u{1F483}", "dancar"], ["gg", "GG", "gg"]];
    const EMO_OF = Object.fromEntries(EMO.map(([k, g]) => [k, g]));
    const QUICK = ["oi!", "bora!", "kkkk", "gg", "me espera", "vem ca", "valeu!", "tchau"];
    const MAX_CARDS = 8;

    const css = document.createElement("style");
    css.textContent = `
#upp-pres { position: absolute; left: 0; right: 0; top: 38px; bottom: 0; pointer-events: none; z-index: 3; font: 12px system-ui, sans-serif; color: #fff; overflow: hidden; }
#upp-pres .pp-ui { pointer-events: auto; }
#upp-pres .pp-cards { position: absolute; right: 8px; top: 8px; display: flex; flex-direction: column; gap: 6px; align-items: flex-end; max-height: calc(100% - 70px); }
#upp-pres .pp-card { position: relative; display: flex; align-items: center; gap: 7px; background: rgba(16,8,34,.72); border: 1px solid rgba(180,140,255,.35); border-radius: 22px 8px 8px 22px; padding: 3px 10px 3px 3px; min-width: 120px; max-width: 190px; box-shadow: 0 2px 8px #0006; backdrop-filter: blur(3px); transition: opacity .3s; }
#upp-pres .pp-card.me { border-color: rgba(255,210,61,.6); }
#upp-pres .pp-bust { width: 40px; height: 40px; border-radius: 50%; flex: 0 0 40px; background: #2a1650; display: flex; align-items: center; justify-content: center; font: bold 17px system-ui; overflow: hidden; border: 2px solid #fff3; }
#upp-pres .pp-bust canvas { width: 40px; height: 40px; }
#upp-pres .pp-txt { min-width: 0; line-height: 1.25; }
#upp-pres .pp-name { font-weight: bold; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 128px; }
#upp-pres .pp-st { opacity: .8; font-size: 11px; white-space: nowrap; }
#upp-pres .pp-dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; margin-right: 4px; background: #7cff6b; vertical-align: 0; }
#upp-pres .pp-dot.idle { background: #ffd23d; } #upp-pres .pp-dot.away { background: #9a8fb0; }
#upp-pres .pp-more { background: rgba(16,8,34,.72); border-radius: 10px; padding: 3px 9px; }
#upp-pres .pp-bubble { position: absolute; right: calc(100% + 8px); top: 50%; transform: translateY(-50%); background: #fff; color: #1a0d33; border-radius: 12px; padding: 5px 10px; font-weight: 600; max-width: 220px; width: max-content; box-shadow: 0 2px 8px #0007; animation: pp-pop .2s ease-out; }
#upp-pres .pp-bubble::after { content: ""; position: absolute; left: 100%; top: 50%; margin-top: -6px; border: 6px solid transparent; border-left-color: #fff; }
#upp-pres .pp-bar { position: absolute; left: 8px; bottom: 8px; display: flex; align-items: center; gap: 4px; background: rgba(16,8,34,.78); border: 1px solid rgba(180,140,255,.35); border-radius: 20px; padding: 3px 5px 3px 10px; box-shadow: 0 2px 8px #0006; flex-wrap: wrap; max-width: calc(100% - 16px); }
#upp-pres .pp-count { font-weight: bold; margin-right: 4px; white-space: nowrap; }
#upp-pres .pp-bar button, #upp-pres .pp-chat button { font: 16px system-ui; background: transparent; border: 0; color: #fff; cursor: pointer; border-radius: 50%; width: 30px; height: 30px; padding: 0; transition: transform .1s, background .1s; }
#upp-pres .pp-bar button:hover { background: #ffffff22; transform: scale(1.15); }
#upp-pres .pp-bar button.pp-gg { font: bold 12px system-ui; }
#upp-pres .pp-bar button.pp-min { font: bold 14px system-ui; opacity: .7; }
#upp-pres.pp-hide .pp-cards, #upp-pres.pp-hide .pp-emo { display: none; }
#upp-pres .pp-chat { position: absolute; left: 8px; bottom: 50px; background: rgba(16,8,34,.92); border: 1px solid rgba(180,140,255,.5); border-radius: 12px; padding: 8px; display: none; width: 250px; box-shadow: 0 4px 16px #000a; }
#upp-pres .pp-chat.open { display: block; }
#upp-pres .pp-chat .pp-q { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 6px; }
#upp-pres .pp-chat .pp-q button { width: auto; height: auto; border-radius: 12px; background: #6b2cff; font: 600 12px system-ui; padding: 4px 9px; }
#upp-pres .pp-chat input { width: 100%; box-sizing: border-box; font: 13px system-ui; border-radius: 8px; border: 1px solid #b48cff; background: #0d0519; color: #fff; padding: 6px 8px; outline: none; }
#upp-pres .pp-toast { position: absolute; left: 10px; bottom: 50px; background: rgba(16,8,34,.8); border-radius: 10px; padding: 4px 10px; animation: pp-fade 3.2s forwards; }
#upp-pres .pp-fly { position: absolute; font-size: 44px; line-height: 1; animation: pp-fly 2.6s ease-out forwards; text-shadow: 0 3px 10px #0008; white-space: nowrap; }
#upp-pres .pp-fly small { display: block; font: bold 12px system-ui; text-align: center; color: #fff; text-shadow: 0 1px 3px #000; }
@keyframes pp-fly { 0% { transform: translate(0, 0) scale(.4); opacity: 0; } 12% { transform: translate(0, -30px) scale(1.15); opacity: 1; } 100% { transform: translate(var(--dx), -260px) scale(1); opacity: 0; } }
@keyframes pp-pop { from { transform: translateY(-50%) scale(.6); opacity: 0; } }
@keyframes pp-fade { 0%, 75% { opacity: 1; } 100% { opacity: 0; } }
@media (max-width: 640px) { #upp-pres .pp-card { min-width: 0; padding-right: 3px; } #upp-pres .pp-txt { display: none; } #upp-pres .pp-bar button { width: 26px; height: 26px; font-size: 14px; } }`;
    document.head.appendChild(css);

    let cur = null;
    const pkgs = new Map();
    const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

    function bust(el, p) {
        el.textContent = (p.name || "?").slice(0, 1).toUpperCase();
        el.style.background = p.color || "#2a1650";
        if (!p.avatar || !window.UrnaAvatar2D) return;
        const draw = (pkg) => {
            if (!pkg) return;
            const av = window.UrnaAvatar2D.create(pkg, { view: "3q", size: 120 });
            if (!av) return;
            const img = av.still(), cv = document.createElement("canvas");
            cv.width = cv.height = 80;
            // busto: metade de cima do retrato
            cv.getContext("2d").drawImage(img, img.width * 0.18, img.height * 0.02, img.width * 0.64, img.height * 0.64, 0, 0, 80, 80);
            el.textContent = "";
            el.style.background = "#2a1650";
            el.appendChild(cv);
        };
        if (pkgs.has(p.avatar)) {
            const v = pkgs.get(p.avatar);
            return v instanceof Promise ? v.then(draw) : draw(v);
        }
        const [id, v] = String(p.avatar).split("@");
        const pr = fetch(`/api/mods/${encodeURIComponent(id)}/versions/${encodeURIComponent(v)}`).then((r) => (r.ok ? r.json() : null)).catch(() => null);
        pkgs.set(p.avatar, pr);
        pr.then((pkg) => { pkgs.set(p.avatar, pkg); draw(pkg); });
    }

    function status(p) {
        const age = Date.now() - (p.seen || 0);
        if (p.me) return ["", "voce"];
        if (p.sdk && age < 4000) return ["", "jogando agora"];
        if (p.sdk) return ["idle", "parado"];
        return [age < 15000 ? "" : "idle", "neste jogo"];
    }

    function render() {
        if (!cur) return;
        const all = [cur.you, ...cur.players.values()].filter(Boolean);
        const n = all.length;
        cur.ui.count.textContent = `${n} ${n === 1 ? "jogador" : "jogadores"} aqui`;
        const box = cur.ui.cards;
        const keep = new Set();
        all.slice(0, MAX_CARDS).forEach((p, i) => {
            keep.add(p.sid);
            let c = cur.cards.get(p.sid);
            if (!c) {
                const el = document.createElement("div");
                el.className = `pp-card pp-ui${p.me ? " me" : ""}`;
                el.innerHTML = '<div class="pp-bust"></div><div class="pp-txt"><div class="pp-name"></div><div class="pp-st"><span class="pp-dot"></span><span></span></div></div>';
                el.querySelector(".pp-name").textContent = p.name + (p.me ? " (voce)" : "");
                el.title = p.name;
                c = { el, st: el.querySelector(".pp-st span:last-child"), dot: el.querySelector(".pp-dot") };
                bust(el.querySelector(".pp-bust"), p);
                cur.cards.set(p.sid, c);
            }
            const [cls, txt] = status(p);
            c.dot.className = `pp-dot ${cls}`;
            c.st.textContent = txt;
            if (box.children[i] !== c.el) box.insertBefore(c.el, box.children[i] || null);
        });
        for (const [sid, c] of cur.cards) if (!keep.has(sid)) { c.el.remove(); cur.cards.delete(sid); }
        let more = box.querySelector(".pp-more");
        if (n > MAX_CARDS) {
            if (!more) { more = document.createElement("div"); more.className = "pp-more pp-ui"; }
            more.textContent = `+${n - MAX_CARDS}`;
            box.appendChild(more);
        } else if (more) more.remove();
    }

    function toast(text) {
        if (!cur) return;
        const t = document.createElement("div");
        t.className = "pp-toast";
        t.textContent = text;
        cur.ui.root.appendChild(t);
        setTimeout(() => t.remove(), 3300);
    }

    /// Reacao voando pela tela, saindo do card de quem mandou (ou da barra, se for a gente).
    function fly(sid, e) {
        if (!cur) return;
        const p = sid === cur.you?.sid ? cur.you : cur.players.get(sid);
        const R = cur.ui.root.getBoundingClientRect();
        const card = cur.cards.get(sid)?.el;
        // sobe do rodape pela tela toda, na coluna do card de quem mandou (ou num ponto aleatorio)
        const x = card && !cur.ui.root.classList.contains("pp-hide") ? card.getBoundingClientRect().left - R.left - 50 : 40 + Math.random() * R.width * 0.6;
        const f = document.createElement("div");
        f.className = "pp-fly";
        f.style.left = `${Math.max(0, x)}px`;
        f.style.top = `${Math.max(60, R.height - 110)}px`;
        f.style.setProperty("--dx", `${Math.round((Math.random() - 0.6) * 160)}px`);
        f.innerHTML = `${esc(EMO_OF[e] || "?")}<small></small>`;
        f.querySelector("small").textContent = p ? p.name : "";
        cur.ui.root.appendChild(f);
        setTimeout(() => f.remove(), 2700);
    }

    function bubble(sid, text) {
        const c = cur && cur.cards.get(sid);
        if (!c) return toast(`${(cur.players.get(sid) || {}).name || "?"}: ${text}`);
        c.el.querySelector(".pp-bubble")?.remove();
        const b = document.createElement("div");
        b.className = "pp-bubble";
        b.textContent = text;
        c.el.appendChild(b);
        clearTimeout(c.bt);
        c.bt = setTimeout(() => b.remove(), 5500);
    }

    function send(o) {
        if (cur && cur.ws && cur.ws.readyState === 1) cur.ws.send(JSON.stringify(o));
    }
    function emote(e) {
        if (!cur || Date.now() - cur.lastFx < 750) return;
        cur.lastFx = Date.now();
        send({ t: "fx", e });
        if (cur.you) fly(cur.you.sid, e);
    }
    function say(text) {
        text = String(text || "").replace(/\s+/g, " ").trim().slice(0, 80);
        if (!cur || !text || Date.now() - cur.lastSay < 1600) return;
        cur.lastSay = Date.now();
        send({ t: "say", m: text });
        if (cur.you) bubble(cur.you.sid, text);
    }

    function connect(c) {
        if (cur !== c) return;
        const url = `${location.origin.replace(/^http/, "ws")}/api/portals/${encodeURIComponent(c.pid)}/presence?via=overlay&token=${encodeURIComponent(c.tok)}`;
        let ws;
        try { ws = new WebSocket(url); } catch (e) { return; }
        c.ws = ws;
        ws.onmessage = (e) => {
            let m;
            try { m = JSON.parse(e.data); } catch (x) { return; }
            const now = Date.now();
            if (m.t === "welcome") {
                c.tries = 0;
                c.you = { ...m.you, me: true };
                c.players.clear();
                for (const p of m.players || []) c.players.set(p.sid, { ...p, seen: Object.keys(p.s || {}).length ? now : 0 });
            } else if (m.t === "join") {
                const old = c.players.get(m.player.sid);
                c.players.set(m.player.sid, { ...(old || {}), ...m.player, seen: old ? old.seen : now });
                if (!old) toast(`${m.player.name} entrou no jogo`);
            } else if (m.t === "leave") {
                const p = c.players.get(m.sid);
                c.players.delete(m.sid);
                if (p) toast(`${p.name} saiu`);
            } else if (m.t === "u") {
                const p = c.players.get(m.sid);
                if (p) { p.s = m.s; p.seen = now; }
                return;
            } else if (m.t === "fx") {
                const p = c.players.get(m.sid);
                if (p) p.seen = now;
                return fly(m.sid, m.e);
            } else if (m.t === "say") return bubble(m.sid, m.m);
            else if (m.t === "err") return toast(m.message || "recusado");
            render();
        };
        ws.onclose = () => {
            if (cur !== c) return;
            c.players.clear();
            render();
            if (++c.tries <= 6) c.retry = setTimeout(() => connect(c), 1500 * c.tries);
        };
    }

    function build(c) {
        const root = document.createElement("div");
        root.id = "upp-pres";
        root.innerHTML = `<div class="pp-cards"></div><div class="pp-chat pp-ui"><div class="pp-q"></div><input maxlength="80" placeholder="mensagem rapida (Enter)"></div>
<div class="pp-bar pp-ui"><span class="pp-count">...</span><span class="pp-emo"></span><button type="button" class="pp-say" title="chat rapido">\u{1F4AC}</button><button type="button" class="pp-min" title="esconder/mostrar jogadores">\u2013</button></div>`;
        const emo = root.querySelector(".pp-emo");
        for (const [k, g, label] of EMO) {
            const b = document.createElement("button");
            b.type = "button";
            b.title = label;
            b.textContent = g;
            if (k === "gg") b.className = "pp-gg";
            b.onclick = () => { emote(k); back(); };
            emo.appendChild(b);
        }
        const chat = root.querySelector(".pp-chat"), input = chat.querySelector("input");
        const back = () => { try { c.frame.focus(); } catch (e) { } };
        const shut = () => { chat.classList.remove("open"); input.value = ""; back(); };
        for (const q of QUICK) {
            const b = document.createElement("button");
            b.type = "button";
            b.textContent = q;
            b.onclick = () => { say(q); shut(); };
            chat.querySelector(".pp-q").appendChild(b);
        }
        input.addEventListener("keydown", (e) => {
            e.stopPropagation();
            if (e.key === "Enter") { say(input.value); shut(); }
            else if (e.key === "Escape") { e.preventDefault(); shut(); }
        });
        root.querySelector(".pp-say").onclick = () => { chat.classList.toggle("open"); if (chat.classList.contains("open")) input.focus(); else back(); };
        root.querySelector(".pp-min").onclick = (e) => { root.classList.toggle("pp-hide"); e.currentTarget.textContent = root.classList.contains("pp-hide") ? "+" : "\u2013"; back(); };
        c.el.appendChild(root);
        return { root, cards: root.querySelector(".pp-cards"), count: root.querySelector(".pp-count") };
    }

    window.addEventListener("upp:open", (e) => {
        const d = e.detail || {};
        if (cur) stop();
        const c = { ...d, players: new Map(), cards: new Map(), you: null, ws: null, tries: 0, lastFx: 0, lastSay: 0 };
        cur = c;
        c.ui = build(c);
        c.tick = setInterval(render, 1000);
        connect(c);
    });
    function stop() {
        if (!cur) return;
        const c = cur;
        cur = null;
        clearInterval(c.tick);
        clearTimeout(c.retry);
        try { c.ws && c.ws.close(); } catch (e) { }
        c.ui.root.remove();
    }
    window.addEventListener("upp:close", stop);
    window.UrnaPresenceOverlay = { emote, say, get state() { return cur ? { players: cur.players.size + (cur.you ? 1 : 0), you: cur.you && cur.you.name, open: !!cur.ws && cur.ws.readyState === 1 } : null; } };
})();
