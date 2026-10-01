// Universo: identidade do jogador que atravessa jogos (docs em /universe.txt e docs/UNIVERSE.md).
// - Guarda-roupa: skin = mod kind "avatar", escolhida por nome de jogador, carimbada no "p" pelo servidor.
// - Passaporte: GET /api/passport?token= devolve jogador + pacote do avatar + carteira + itens.
// - Presenca: WebSocket por portal (/api/portals/:id/presence?token=); o servidor so repassa, com limites.
// - Itens/moedas entre jogos: grant (pedido do jogo, orcamento/dia, allowlist, idempotencia) e spend (so com
//   o token de confirmacao que vai pra NOSSA pagina, nunca pro iframe).
import { kindOf, creatorFromRequest } from "./mods.js";
import { safe } from "./economy.js";
import { builtinRef } from "./avatars.js";

const PRESENCE_MAX = 32;
const STATE_BYTES = 512;
const RATE = 12; // msgs/s por socket de presenca (excesso e descartado)
const GRANT_COINS_DAY = 60; // por jogador, por portal
const SPEND_COINS_MAX = 500;
const ALLOW_MAX = 8;
const HOLD_MAX = 999;
const IDEM_MAX = 1500;
const CTOK_S = 3600;
// Reacoes da presenca (overlay do Urna em volta de qualquer jogo + SDK): ids fixos, o cliente desenha.
const EMOTES = ["wave", "laugh", "love", "fire", "clap", "wow", "dance", "gg"];

const enc = new TextEncoder();
const norm = (s) => String(s ?? "").toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "").trim();
const clean = (s, n) => String(s ?? "").replace(/[\u0000-\u001f\u007f<>]/g, "").replace(/\s+/g, " ").trim().slice(0, n);
const b64u = (bytes) => btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const hex = (n) => [...crypto.getRandomValues(new Uint8Array(n))].map((b) => b.toString(16).padStart(2, "0")).join("");
const today = () => new Date().toISOString().slice(0, 10);

const CORS = { "access-control-allow-origin": "*", "access-control-allow-headers": "authorization, content-type", "access-control-allow-methods": "GET, POST, PUT, OPTIONS" };
const json = (obj, status = 200) => new Response(JSON.stringify(obj), { status, headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...CORS } });
const fail = (status, error, message, extra = {}) => json({ ok: false, error, message, ...extra }, status);

/// Estado de presenca: <= 16 chaves [a-z0-9_], valores numero/bool/texto curto/[<=4 numeros]; JSON <= STATE_BYTES.
function cleanState(s) {
    if (!s || typeof s !== "object" || Array.isArray(s)) return null;
    const out = {};
    let n = 0;
    for (const [k, v] of Object.entries(s)) {
        if (++n > 16 || !/^[A-Za-z0-9_]{1,16}$/.test(k)) continue;
        if (typeof v === "number" && Number.isFinite(v)) out[k] = Math.round(v * 1000) / 1000;
        else if (typeof v === "boolean") out[k] = v;
        else if (typeof v === "string") out[k] = clean(v, 32);
        else if (Array.isArray(v) && v.length <= 4 && v.every((x) => typeof x === "number" && Number.isFinite(x))) out[k] = v.map((x) => Math.round(x * 1000) / 1000);
    }
    return JSON.stringify(out).length <= STATE_BYTES ? out : null;
}

export class Universe {
    constructor(room) {
        this.room = room;
        this.st = room.ctx.storage;
        this.s = { av: {}, bag: {}, allow: {}, day: "", used: {}, idem: {} };
        this.rooms = new Map(); // portal -> Map(sid -> membro)
        this.rl = new Map();
        this.key = null;
        this.timer = null;
        room.ctx.blockConcurrencyWhile(async () => {
            for (const k of Object.keys(this.s)) {
                const v = await this.st.get(`uni:${k}`);
                if (v !== undefined) this.s[k] = v;
            }
        });
    }

    save() {
        if (this.timer) return;
        this.timer = setTimeout(() => {
            this.timer = null;
            for (const [k, v] of Object.entries(this.s)) this.st.put(`uni:${k}`, v);
        }, 500);
    }

    limit(k, n, ms) {
        const now = Date.now();
        let b = this.rl.get(k);
        if (!b || b.reset < now) {
            if (this.rl.size > 5000) for (const [x, y] of this.rl) if (y.reset < now) this.rl.delete(x);
            b = { c: 0, reset: now + ms };
            this.rl.set(k, b);
        }
        return ++b.c <= n;
    }

    get mods() {
        return this.room.mods;
    }
    get hub() {
        return this.room.hub;
    }

    // ------------------------------------------------ guarda-roupa
    avatars() {
        return this.mods.list("avatar").map((a) => ({ id: a.id, v: a.v, name: a.pkg.manifest.name, creator: a.creator, h: a.pkg.avatar?.height || 1.8 }));
    }
    /// "id@versao" da skin do jogador (sempre a versao ativa do mod) ou null.
    ref(name) {
        const id = this.s.av[norm(name)];
        const a = id && this.mods.active.get(id);
        return a && kindOf(a.pkg) === "avatar" ? `${a.id}@${a.v}` : null;
    }
    join(c) {
        c.av = this.ref(c.name);
        this.room.send(c, { t: "uv_list", av: this.avatars(), me: c.av });
        this.room.send(c, { t: "uv_bag", items: this.bagView(c.name) });
    }
    modsChanged() {
        const list = this.avatars();
        for (const c of this.room.clients.values()) {
            c.av = this.ref(c.name);
            this.room.send(c, { t: "uv_list", av: list, me: c.av });
        }
    }
    /// Servidor e quem diz a skin no "p" (cliente nao inventa).
    stamp(c, m) {
        if (c.av) m.av = c.av;
        else delete m.av;
    }
    /// Token de confirmacao de gasto: vai so pra pagina do Urna (WS), o iframe nunca ve.
    async onSession(c, t) {
        const ctok = await this.sign({ k: "cfm", sub: t.sub, pid: t.pid, sid: t.sid, exp: Math.floor(Date.now() / 1000) + CTOK_S });
        this.room.send(c, { t: "uv_ctok", p: t.pid, sid: t.sid, ctok });
    }

    onMsg(id, c, m) {
        if (m.t === "uv_set") {
            if (!this.limit(`set:${id}`, 3, 3000)) return;
            const want = String(m.mod ?? "");
            const k = norm(c.name);
            if (!want) delete this.s.av[k];
            else {
                const a = this.mods.active.get(want);
                if (!a || kindOf(a.pkg) !== "avatar") return this.room.send(c, { t: "chat", id: 0, n: "SKIN", m: "essa skin nao ta ativa" });
                this.s.av[k] = want;
            }
            this.save();
            for (const x of this.room.clients.values()) if (norm(x.name) === k) {
                x.av = this.ref(c.name);
                this.room.send(x, { t: "uv_me", me: x.av });
            }
            return;
        }
        if (m.t === "uv_get") {
            if (!this.limit(`get:${id}`, 30, 10000)) return;
            const a = this.mods.active.get(String(m.mod ?? ""));
            if (a && kindOf(a.pkg) !== "npc") this.room.send(c, { t: "uv_pkg", id: a.id, v: a.v, pkg: a.pkg });
        }
    }

    // ------------------------------------------------ inventario de itens de mod
    bagView(name) {
        return Object.entries(this.s.bag[norm(name)] || {}).filter(([, e]) => e.n > 0).map(([id, e]) => ({ id, name: e.name, n: e.n, color: e.c1, color2: e.c2, pattern: e.p }));
    }
    sendBag(name) {
        const k = norm(name);
        const msg = { t: "uv_bag", items: this.bagView(name) };
        for (const c of this.room.clients.values()) if (norm(c.name) === k) this.room.send(c, msg);
    }

    // ------------------------------------------------ assinatura propria (confirmacao de gasto)
    async hmac() {
        if (!this.key) {
            let sec = await this.st.get("uv_secret");
            if (!sec) {
                sec = hex(32);
                await this.st.put("uv_secret", sec);
            }
            this.key = await crypto.subtle.importKey("raw", enc.encode(sec), { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
        }
        return this.key;
    }
    async sign(p) {
        const body = b64u(enc.encode(JSON.stringify(p)));
        return `${body}.${b64u(await crypto.subtle.sign("HMAC", await this.hmac(), enc.encode(body)))}`;
    }
    async verifyCtok(tok) {
        const [body, sig] = String(tok ?? "").split(".");
        if (!body || !sig || tok.length > 1024) return null;
        try {
            const raw = Uint8Array.from(atob(sig.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((sig.length + 3) % 4)), (ch) => ch.charCodeAt(0));
            if (!(await crypto.subtle.verify("HMAC", await this.hmac(), raw, enc.encode(body)))) return null;
            const p = JSON.parse(atob(body.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((body.length + 3) % 4)));
            return p.k === "cfm" && p.exp * 1000 > Date.now() ? p : null;
        } catch (e) {
            return null;
        }
    }

    // ------------------------------------------------ orcamento diario / idempotencia
    used(key) {
        if (this.s.day !== today()) this.s = { ...this.s, day: today(), used: {} };
        return this.s.used[key] || 0;
    }
    idem(key, res) {
        if (res === undefined) return this.s.idem[key];
        this.s.idem[key] = { at: Date.now(), ok: res.ok, granted: res.granted, spent: res.spent };
        const ks = Object.keys(this.s.idem);
        if (ks.length > IDEM_MAX) for (const k of ks.sort((a, b) => this.s.idem[a].at - this.s.idem[b].at).slice(0, ks.length - IDEM_MAX)) delete this.s.idem[k];
    }
    portal(pid) {
        const p = this.hub.s.p[pid];
        const a = p && p.versions.find((x) => x.v === p.active);
        return a && p.verified?.origin === a.origin ? { p, name: a.name } : null;
    }
    allowView(pid) {
        return (this.s.allow[pid] || []).map((id) => {
            const a = this.mods.active.get(id);
            return a && kindOf(a.pkg) === "item" ? { id, name: a.pkg.manifest.name, daily_cap: a.pkg.item.daily_cap, color: a.pkg.item.color, pattern: a.pkg.item.pattern } : null;
        }).filter(Boolean);
    }
    limitsFor(pid, name) {
        const k = norm(name);
        return {
            grant_coins_per_day: GRANT_COINS_DAY,
            grant_coins_left_today: Math.max(0, GRANT_COINS_DAY - this.used(`${pid}|${k}|$`)),
            grant_items: this.allowView(pid).map((x) => ({ ...x, left_today: Math.max(0, x.daily_cap - this.used(`${pid}|${k}|${x.id}`)) })),
            spend_coins_max: SPEND_COINS_MAX,
        };
    }

    // ------------------------------------------------ presenca por portal
    presenceOnly(pid, ses) {
        const r = this.rooms.get(pid);
        if (!r) return 0;
        let n = 0;
        for (const sid of r.keys()) if (!ses.get(sid)?.ready) n++;
        return n;
    }
    /// sdk = o jogo tem o SDK conectado (senao o jogador so existe pelo overlay do Urna em volta do iframe).
    pub(m) {
        return { sid: m.sid, name: m.name, color: m.color, character: m.character, avatar: m.avatar, s: m.s, sdk: [...m.socks.values()].includes("game") };
    }
    /// Manda pra todos os sockets da sala menos `skip` (socket) e, se `others`, menos os do proprio membro.
    relay(room, from, obj, skip = null, others = true) {
        const s = JSON.stringify(obj);
        for (const m of room.values()) if (!others || m !== from) for (const w of m.socks.keys()) if (w !== skip) try { w.send(s); } catch (e) { }
    }
    // Um membro por sessao (sid) com ate 1 socket "game" (SDK no iframe) + 1 "overlay" (?via=overlay: a pagina do Urna).
    async presence(req, pid) {
        if (req.headers.get("Upgrade") !== "websocket") return fail(426, "upgrade", "use WebSocket: wss://<site>/api/portals/:id/presence?token=<session token>");
        const q = new URL(req.url).searchParams;
        const t = await this.hub.check(q.get("token"));
        if (!t || t.pid !== pid) return fail(401, "unauthorized", "token invalido/expirado ou de outro portal");
        if (!this.portal(pid)) return fail(410, "gone", "portal fora do ar");
        const via = q.get("via") === "overlay" ? "overlay" : "game";
        let room = this.rooms.get(pid);
        if (!room) this.rooms.set(pid, (room = new Map()));
        if (room.size >= PRESENCE_MAX && !room.has(t.sid)) return fail(429, "full", `max ${PRESENCE_MAX} jogadores na presenca desse portal`);
        let me = room.get(t.sid);
        const fresh = !me;
        if (me) for (const [w, v] of me.socks) if (v === via) {
            me.socks.delete(w);
            try { w.close(4000, "replaced"); } catch (e) { }
        }
        const [client, ws] = Object.values(new WebSocketPair());
        ws.accept();
        if (!me) {
            me = { socks: new Map(), sid: t.sid, name: t.sub, color: t.player?.color || "#ffffff", character: t.player?.character || "steve", avatar: t.player?.avatar || builtinRef(t.player?.character), s: {}, tokens: RATE, at: Date.now(), dropped: 0, fx: 0, say: 0 };
            room.set(t.sid, me);
        }
        const sdk0 = this.pub(me).sdk;
        me.socks.set(ws, via);
        ws.send(JSON.stringify({ t: "welcome", you: this.pub(me), players: [...room.values()].filter((m) => m !== me).map((m) => this.pub(m)), emotes: EMOTES, limits: { rate_hz: RATE, state_bytes: STATE_BYTES, max_players: PRESENCE_MAX } }));
        if (fresh || this.pub(me).sdk !== sdk0) this.relay(room, me, { t: "join", player: this.pub(me) });
        if (fresh) this.hub.changed();
        const bye = () => {
            if (!me.socks.delete(ws)) return;
            if (me.socks.size) return void (via === "game" && this.relay(room, me, { t: "join", player: this.pub(me) }));
            if (room.get(me.sid) !== me) return;
            room.delete(me.sid);
            if (!room.size) this.rooms.delete(pid);
            this.relay(room, me, { t: "leave", sid: me.sid });
            this.hub.changed();
        };
        ws.addEventListener("close", bye);
        ws.addEventListener("error", bye);
        ws.addEventListener("message", (e) => {
            const now = Date.now();
            me.tokens = Math.min(RATE, me.tokens + ((now - me.at) / 1000) * RATE);
            me.at = now;
            if (typeof e.data !== "string" || e.data.length > STATE_BYTES * 2 || me.tokens < 1) return void me.dropped++;
            me.tokens -= 1;
            let m;
            try { m = JSON.parse(e.data); } catch (x) { return; }
            if (m?.t === "u") {
                const s = cleanState(m.s);
                if (!s) return void me.dropped++;
                me.s = s;
                this.relay(room, me, { t: "u", sid: me.sid, s });
            } else if (m?.t === "fx") {
                if (!EMOTES.includes(m.e) || now - me.fx < 700) return void me.dropped++;
                me.fx = now;
                this.relay(room, me, { t: "fx", sid: me.sid, e: m.e }, ws, false);
            } else if (m?.t === "say") {
                const text = clean(m.m, 80);
                if (!text || now - me.say < 1500) return void me.dropped++;
                if (!safe(text)) return void ws.send(JSON.stringify({ t: "err", error: "filtered", message: "mensagem recusada pelo filtro (sem link/dinheiro real/chave)" }));
                me.say = now;
                this.relay(room, me, { t: "say", sid: me.sid, m: text }, ws, false);
            } else if (m?.t === "ping") ws.send(JSON.stringify({ t: "pong", dropped: me.dropped }));
        });
        return new Response(null, { status: 101, webSocket: client });
    }

    // ------------------------------------------------ HTTP (null = nao e rota daqui)
    async route(req) {
        const url = new URL(req.url);
        const path = url.pathname.replace(/\/+$/, "");
        const pm = /^\/api\/portals\/([a-z0-9-]{2,32})\/(presence|items)$/.exec(path);
        if (!pm && path !== "/api/passport" && !path.startsWith("/api/universe")) return null;
        if (req.method === "OPTIONS") return new Response(null, { status: 204, headers: CORS });
        const ip = req.headers.get("cf-connecting-ip") || "?";
        if (!this.limit(`ip:${ip}`, 240, 60000)) return fail(429, "rate_limited", "240 req/min por IP");
        try {
            if (pm?.[2] === "presence") return await this.presence(req, pm[1]);
            if (pm?.[2] === "items") return await this.items(req, pm[1]);
            if (path === "/api/passport") return await this.passport(url.searchParams.get("token"));
            const op = path.slice("/api/universe/".length);
            if (op === "grant" && req.method === "POST") return await this.grant(await this.body(req));
            if (op === "spend" && req.method === "POST") return await this.spend(await this.body(req));
            if (op === "receipt" && req.method === "GET") return await this.receipt(url.searchParams.get("token"), url.searchParams.get("key"));
            if (op === "" || op === "avatars") return json({ ok: true, avatars: this.avatars(), docs: `${url.origin}/universe.txt` });
            return fail(404, "not_found", "rotas: GET /api/passport, POST /api/universe/grant, POST /api/universe/spend, GET /api/universe/receipt, GET|PUT /api/portals/:id/items, WS /api/portals/:id/presence");
        } catch (e) {
            return fail(e.status || 500, e.code || "internal", String(e.message || e).slice(0, 160));
        }
    }

    async body(req) {
        const t = await req.text();
        if (t.length > 4096) throw Object.assign(new Error("corpo > 4096 bytes"), { status: 413, code: "too_big" });
        try {
            const b = JSON.parse(t || "{}");
            if (b && typeof b === "object" && !Array.isArray(b)) return b;
        } catch (e) { }
        throw Object.assign(new Error("json invalido"), { status: 400, code: "bad_json" });
    }

    async passport(tok) {
        const t = await this.hub.check(tok);
        if (!t) return fail(401, "unauthorized", "token de sessao invalido ou expirado");
        const ref = t.player?.avatar || builtinRef(t.player?.character);
        const [id, v] = ref.split("@");
        const avatar = await this.mods.loadPkg(id, v);
        const w = this.room.eco.me(t.sub);
        return json({
            ok: true,
            protocol: "UPP/1",
            portal: t.pid,
            player: { name: t.sub, color: t.player?.color, character: t.player?.character, avatar: ref },
            color: t.player?.color,
            character: t.player?.character,
            avatar_ref: ref,
            avatar,
            wallet: { coins: w.c, currency: "moeda ficticia (sem valor real)" },
            inventory: this.bagView(t.sub),
            limits: this.limitsFor(t.pid, t.sub),
            exp: t.exp,
        });
    }

    async items(req, pid) {
        const p = this.hub.s.p[pid];
        if (!p) return fail(404, "not_found", "portal nao existe");
        if (req.method === "GET") return json({ ok: true, portal: pid, items: this.allowView(pid), pending: [] });
        if (req.method !== "PUT" && req.method !== "POST") return fail(405, "method", "GET ou PUT");
        const who = await creatorFromRequest(this.room, req);
        if (!who) return fail(401, "unauthorized", "Authorization: Bearer <token de criador>");
        if (who.id !== p.owner) return fail(403, "forbidden", "so o dono do portal define a allowlist");
        if (!this.limit(`allow:${who.id}`, 10, 60000)) return fail(429, "rate_limited", "10/min");
        const list = (await this.body(req)).items;
        if (!Array.isArray(list) || list.length > ALLOW_MAX) return fail(422, "invalid", `items: array de ate ${ALLOW_MAX} ids de mods kind "item"`);
        const approved = [], rejected = [];
        for (const raw of [...new Set(list.map(String))]) {
            const meta = await this.st.get(`mod:${raw}`);
            const a = this.mods.active.get(raw);
            if (!meta) rejected.push({ id: raw, reason: "mod nao existe" });
            else if (meta.owner !== who.id) rejected.push({ id: raw, reason: "v1: so itens do mesmo criador do portal (moderacao entre criadores ainda nao existe)" });
            else if (!a || kindOf(a.pkg) !== "item") rejected.push({ id: raw, reason: 'precisa ser um mod kind "item" ATIVO (ja passou pela moderacao no upload)' });
            else approved.push(raw);
        }
        this.s.allow[pid] = approved;
        this.save();
        return json({ ok: true, portal: pid, approved, rejected, items: this.allowView(pid) });
    }

    async grant(d) {
        const t = await this.hub.check(d.token);
        if (!t) return fail(401, "unauthorized", "token de sessao invalido ou expirado");
        const po = this.portal(t.pid);
        if (!po) return fail(410, "gone", "portal fora do ar");
        if (!this.limit(`g:${t.sid}`, 20, 60000)) return fail(429, "rate_limited", "20 grants/min por sessao");
        const key = String(d.key ?? "");
        if (!/^[A-Za-z0-9_.:-]{6,64}$/.test(key)) return fail(422, "invalid", "key: 6-64 chars [A-Za-z0-9_.:-] (idempotencia: mesma key = mesmo pedido)");
        const k = norm(t.sub);
        const ik = `g|${t.pid}|${k}|${key}`;
        const first = this.idem(ik);
        if (first) return fail(409, "duplicate", "key ja usada: pedido repetido ignorado", { first });
        const reason = clean(d.reason, 80) || "premio";
        if (!safe(reason)) return fail(422, "invalid", "reason recusado pelo filtro (sem link/dinheiro real/chave)");
        const eco = this.room.eco;
        let res;
        if (d.coins !== undefined) {
            const n = Number(d.coins);
            if (!Number.isInteger(n) || n < 1 || n > GRANT_COINS_DAY) return fail(422, "invalid", `coins: inteiro 1..${GRANT_COINS_DAY}`);
            const uk = `${t.pid}|${k}|$`;
            const left = GRANT_COINS_DAY - this.used(uk);
            if (n > left) return fail(429, "budget", `orcamento do dia desse portal pra ${t.sub}: restam ${Math.max(0, left)} moedas`, { left: Math.max(0, left) });
            if (eco.s.tr < n) return fail(503, "treasury", "cofre da vila sem moedas agora");
            this.s.used[uk] = this.used(uk) + n;
            const w = eco.wallet(t.sub);
            eco.s.tr -= n;
            w.c += n;
            eco.entry(t.sub, `veio do jogo ${po.name}: ${reason}`, n, `grant do portal ${t.pid}, pago do cofre (max ${GRANT_COINS_DAY}/dia por jogo, moeda ficticia)`);
            eco.sendMe(t.sub);
            res = { ok: true, granted: { coins: n }, wallet: { coins: w.c }, left_today: left - n };
        } else if (d.item !== undefined) {
            const id = String(d.item);
            const n = d.n === undefined ? 1 : Number(d.n);
            const a = this.mods.active.get(id);
            if (!(this.s.allow[t.pid] || []).includes(id) || !a || kindOf(a.pkg) !== "item") return fail(403, "not_allowed", `item "${clean(id, 40)}" nao esta na allowlist aprovada desse portal (PUT /api/portals/${t.pid}/items)`);
            const cap = a.pkg.item.daily_cap;
            if (!Number.isInteger(n) || n < 1 || n > cap) return fail(422, "invalid", `n: inteiro 1..${cap}`);
            const uk = `${t.pid}|${k}|${id}`;
            const left = cap - this.used(uk);
            if (n > left) return fail(429, "budget", `limite diario desse item nesse portal: restam ${Math.max(0, left)}`, { left: Math.max(0, left) });
            const bag = (this.s.bag[k] ||= {});
            const e = (bag[id] ||= { n: 0 });
            if (e.n + n > HOLD_MAX) return fail(409, "full", `max ${HOLD_MAX} por item no inventario`);
            Object.assign(e, { n: e.n + n, name: a.pkg.manifest.name, c1: a.pkg.item.color, c2: a.pkg.item.color2, p: a.pkg.item.pattern });
            this.s.used[uk] = this.used(uk) + n;
            eco.entry(t.sub, `ganhou ${n}x ${a.pkg.manifest.name}, veio do jogo ${po.name}: ${reason}`, 0, `item de mod via grant do portal ${t.pid} (limite ${cap}/dia)`);
            this.sendBag(t.sub);
            res = { ok: true, granted: { item: id, n }, inventory: this.bagView(t.sub), left_today: left - n };
        } else return fail(422, "invalid", "mande coins (numero) OU item (id) + n");
        this.idem(ik, res);
        this.save();
        return json(res);
    }

    async spend(d) {
        const c = await this.verifyCtok(d.ctok);
        if (!c) return fail(403, "confirm_required", "gasto so com confirmacao do jogador na tela do Urna (UrnaPortal.spend abre o dialogo)");
        const po = this.portal(c.pid);
        if (!po) return fail(410, "gone", "portal fora do ar");
        if (!this.limit(`s:${c.sid}`, 20, 60000)) return fail(429, "rate_limited", "20 spends/min");
        const key = String(d.key ?? "");
        if (!/^[A-Za-z0-9_.:-]{6,64}$/.test(key)) return fail(422, "invalid", "key: 6-64 chars [A-Za-z0-9_.:-]");
        const k = norm(c.sub);
        const ik = `s|${c.pid}|${k}|${key}`;
        const first = this.idem(ik);
        if (first) return fail(409, "duplicate", "key ja usada", { first });
        const reason = clean(d.reason, 80) || "compra no jogo";
        if (!safe(reason)) return fail(422, "invalid", "reason recusado pelo filtro");
        const eco = this.room.eco;
        let res;
        if (d.coins !== undefined) {
            const n = Number(d.coins);
            if (!Number.isInteger(n) || n < 1 || n > SPEND_COINS_MAX) return fail(422, "invalid", `coins: inteiro 1..${SPEND_COINS_MAX}`);
            const w = eco.wallet(c.sub);
            if (w.c < n) return fail(402, "insufficient", `saldo ${w.c}`);
            w.c -= n;
            eco.s.tr += n;
            eco.entry(c.sub, `gastou no jogo ${po.name}: ${reason}`, n, `spend confirmado pelo jogador no portal ${c.pid} (vai pro cofre)`);
            eco.sendMe(c.sub);
            res = { ok: true, spent: { coins: n }, wallet: { coins: w.c } };
        } else if (d.item !== undefined) {
            const id = String(d.item);
            const n = d.n === undefined ? 1 : Number(d.n);
            const e = this.s.bag[k]?.[id];
            if (!Number.isInteger(n) || n < 1 || n > HOLD_MAX) return fail(422, "invalid", "n invalido");
            if (!e || e.n < n) return fail(402, "insufficient", `tem ${e?.n || 0}`);
            e.n -= n;
            eco.entry(c.sub, `usou ${n}x ${e.name} no jogo ${po.name}: ${reason}`, 0, `spend de item confirmado pelo jogador no portal ${c.pid}`);
            this.sendBag(c.sub);
            res = { ok: true, spent: { item: id, n }, inventory: this.bagView(c.sub) };
        } else return fail(422, "invalid", "mande coins OU item + n");
        this.idem(ik, res);
        this.save();
        return json(res);
    }

    async receipt(tok, key) {
        const t = await this.hub.check(tok);
        if (!t) return fail(401, "unauthorized", "token invalido ou expirado");
        const k = norm(t.sub);
        const g = this.idem(`g|${t.pid}|${k}|${key}`), s = this.idem(`s|${t.pid}|${k}|${key}`);
        return g || s ? json({ ok: true, kind: g ? "grant" : "spend", result: g || s }) : fail(404, "not_found", "nenhum grant/spend com essa key");
    }
}
