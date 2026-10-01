// Game Hub: corredor de portais pra jogos web externos (Urna Portal Protocol v1, docs em web/hub.txt).
// Criador (mesmo token Bearer do modding) registra portal versionado; o servidor so poe no ar depois de
// buscar /.well-known/urna-portal.json na origem do jogo com o challenge (prova de dono). Na hora de
// entrar, o DO assina um token de sessao HMAC (JWT HS256, ~10 min) com nome/cor/personagem do jogador;
// o cliente abre o jogo num iframe sandbox e repassa o token por postMessage (origem conferida).
import { creatorFromRequest } from "./mods.js";

const SESSION_S = 600;
const MAX_PORTALS = 60;
const PER_OWNER = 6;
const MAX_VERSIONS = 20;
const SHOWN = 8; // arcos no corredor
const MAX_BODY = 8192;
const DAILY_COINS = 60;
const SESSION_COINS = 15;
const DEFAULT_COLORS = ["#7a3cff", "#00e5ff"];

const enc = new TextEncoder();
const dec = new TextDecoder();
const b64u = (bytes) => btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const unb64u = (s) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((s.length + 3) % 4)), (c) => c.charCodeAt(0));
const hex = (n) => [...crypto.getRandomValues(new Uint8Array(n))].map((b) => b.toString(16).padStart(2, "0")).join("");

const norm = (s) => String(s ?? "").toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "");
const clean = (s, n) => String(s ?? "").replace(/[\u0000-\u001f\u007f<>]/g, "").replace(/\s+/g, " ").trim().slice(0, n);
// Texto publico (nome, descricao, eventos no chat): sem link, dinheiro real, chave/senha nem odio explicito.
const BANNED = /(r\$|reais|\bpix\b|cripto|crypto|bitcoin|\bnft|api.?key|senha|password|token|cartao|paypal|https?:|www\.|\.(com|net|org|br|io|gg|xyz|ru)\b|@|nazi|hitler)/;
const okText = (s) => !BANNED.test(norm(s));

const SEMVER = /^(\d{1,4})\.(\d{1,4})\.(\d{1,4})$/;
const semv = (v) => {
    const m = SEMVER.exec(String(v ?? ""));
    return m ? m.slice(1).map(Number) : null;
};
const newer = (a, b) => {
    for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] > b[i];
    return false;
};
const rgb = (h) => [1, 3, 5].map((i) => Math.round((parseInt(h.slice(i, i + 2), 16) / 255) * 100) / 100);

const CORS = {
    "access-control-allow-origin": "*",
    "access-control-allow-headers": "authorization, content-type",
    "access-control-allow-methods": "GET, POST, PUT, DELETE, OPTIONS",
};
const json = (obj, status = 200) =>
    new Response(JSON.stringify(obj, null, 1), { status, headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...CORS } });
const fail = (status, error, extra = {}) => json({ ok: false, error, ...extra }, status);

class HttpError extends Error {
    constructor(status, msg) {
        super(msg);
        this.status = status;
    }
}

async function body(req) {
    const t = await req.text();
    if (t.length > MAX_BODY) throw new HttpError(413, `corpo maior que ${MAX_BODY} bytes`);
    try {
        const b = t ? JSON.parse(t) : {};
        if (!b || typeof b !== "object" || Array.isArray(b)) throw 0;
        return b;
    } catch (e) {
        throw new HttpError(400, "json invalido");
    }
}

/// https only, sem credencial/porta/IP/localhost e nunca a nossa origem.
function gameUrl(raw, site) {
    let u;
    try {
        u = new URL(String(raw ?? ""));
    } catch (e) {
        throw new HttpError(400, "url invalida");
    }
    const h = u.hostname;
    if (u.protocol !== "https:") throw new HttpError(400, "url precisa ser https://");
    if (u.username || u.password || u.port) throw new HttpError(400, "url sem usuario/senha/porta");
    if (!h.includes(".") || /^[\d.]+$/.test(h) || h.startsWith("[") || /(^|\.)(localhost|local|internal|lan|home)$/.test(h)) throw new HttpError(400, "host invalido");
    if (site && h === new URL(site).hostname) throw new HttpError(400, "o jogo nao pode estar na mesma origem do urna (sandbox)");
    u.hash = "";
    if (u.href.length > 300) throw new HttpError(400, "url longa demais (300)");
    return u;
}

function manifest(b, site) {
    const ver = semv(b.version);
    if (!ver) throw new HttpError(400, "version precisa ser semver X.Y.Z");
    const name = clean(b.name, 32);
    if (name.length < 2 || !okText(name)) throw new HttpError(400, "name 2-32 letras, sem link/dinheiro/chave");
    const desc = clean(b.description, 200);
    if (!okText(desc)) throw new HttpError(400, "description recusada pelo filtro (sem link/dinheiro/chave)");
    const u = gameUrl(b.url, site);
    if (b.origin != null && String(b.origin).replace(/\/+$/, "") !== u.origin) throw new HttpError(400, `origin (${clean(b.origin, 80)}) nao bate com a url (${u.origin})`);
    const raw = Array.isArray(b.thumbnail?.colors) ? b.thumbnail.colors : Array.isArray(b.colors) ? b.colors : [];
    const colors = raw.filter((c) => /^#[0-9a-f]{6}$/i.test(c)).slice(0, 2).map((c) => c.toLowerCase());
    while (colors.length < 2) colors.push(DEFAULT_COLORS[colors.length]);
    return { v: ver.join("."), name, desc, url: u.href, origin: u.origin, colors, at: Date.now() };
}

const cur = (p) => p.versions.find((x) => x.v === p.active) || null;
const live = (p) => {
    const a = cur(p);
    return !!(a && p.verified && p.verified.origin === a.origin);
};

export class Hub {
    constructor(room) {
        this.room = room;
        this.s = { p: {}, d: { day: "", m: {} } };
        this.ses = new Map(); // sid -> sessao (memoria; token continua valido pela assinatura)
        this.rl = new Map();
        this.site = "";
        this.timer = null;
        this.push = null;
        this.key = null;
        room.ctx.blockConcurrencyWhile(async () => {
            const s = await room.ctx.storage.get("hub");
            if (s) this.s = { ...this.s, ...s };
        });
    }

    save() {
        if (this.timer) return;
        this.timer = setTimeout(() => {
            this.timer = null;
            this.room.ctx.storage.put("hub", this.s);
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

    // ------------------------------------------------ token de sessao (JWT HS256)
    async hmac() {
        if (!this.key) {
            let sec = this.room.env.HUB_SECRET;
            if (!sec) {
                sec = await this.room.ctx.storage.get("hub_secret");
                if (!sec) {
                    sec = hex(32);
                    await this.room.ctx.storage.put("hub_secret", sec);
                }
            }
            this.key = await crypto.subtle.importKey("raw", enc.encode(sec), { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
        }
        return this.key;
    }

    async sign(payload) {
        const head = b64u(enc.encode(JSON.stringify({ alg: "HS256", typ: "JWT" })));
        const pay = b64u(enc.encode(JSON.stringify(payload)));
        const sig = await crypto.subtle.sign("HMAC", await this.hmac(), enc.encode(`${head}.${pay}`));
        return `${head}.${pay}.${b64u(sig)}`;
    }

    async check(tok) {
        const t = String(tok ?? "");
        const parts = t.split(".");
        if (parts.length !== 3 || t.length > 2048) return null;
        try {
            if (!(await crypto.subtle.verify("HMAC", await this.hmac(), unb64u(parts[2]), enc.encode(`${parts[0]}.${parts[1]}`)))) return null;
            const p = JSON.parse(dec.decode(unb64u(parts[1])));
            return p.upp === 1 && p.exp * 1000 > Date.now() ? p : null;
        } catch (e) {
            return null;
        }
    }

    // ------------------------------------------------ estado pro cliente
    count(pid) {
        const now = Date.now();
        let n = 0;
        for (const [sid, s] of this.ses) {
            if (s.exp < now) this.ses.delete(sid);
            else if (s.pid === pid && s.ready) n++;
        }
        return n + (this.room.uni?.presenceOnly(pid, this.ses) || 0);
    }

    snapshot() {
        const portals = Object.values(this.s.p)
            .filter(live)
            .sort((a, b) => a.created - b.created)
            .slice(0, SHOWN)
            .map((p) => {
                const a = cur(p);
                return { id: p.id, name: a.name, by: p.author, v: a.v, c: a.colors.map(rgb), n: this.count(p.id) };
            });
        return { t: "hub", site: this.site, portals };
    }

    changed() {
        if (this.push) return;
        this.push = setTimeout(() => {
            this.push = null;
            this.room.broadcast(this.snapshot());
        }, 600);
    }

    say(m, from) {
        this.room.broadcast({ t: "chat", id: 0, n: from ? `HUB/${from.slice(0, 12)}` : "HUB", m: m.slice(0, 200) });
    }

    join(c) {
        this.room.send(c, this.snapshot());
    }

    leave(cid) {
        let hit = false;
        for (const [sid, s] of this.ses) {
            if (s.cid !== cid) continue;
            hit ||= s.ready;
            this.ses.delete(sid);
        }
        if (hit) this.changed();
    }

    /// WebSocket {t:"hub", k:"enter", p:id, col:"#rrggbb", ch:"steve"} -> {t:"hub_s", tok, url, origin, ...}
    async onMsg(id, c, m) {
        if (m.k !== "enter" || !c.name) return;
        const now = Date.now();
        if (now - (c.hubT || 0) < 2500) return;
        c.hubT = now;
        const p = this.s.p[String(m.p ?? "")];
        if (!p || !live(p)) return this.room.send(c, { t: "chat", id: 0, n: "HUB", m: "esse portal saiu do ar" });
        const a = cur(p);
        const iat = Math.floor(now / 1000);
        const sid = hex(9);
        const player = { name: c.name, color: /^#[0-9a-f]{6}$/i.test(m.col) ? m.col : "#ffffff", character: clean(m.ch, 16) || "steve", avatar: this.room.uni?.ref(c.name) || null };
        const tok = await this.sign({ upp: 1, iss: this.site, aud: a.origin, sub: c.name, pid: p.id, sid, player, ret: `${this.site}/?hub=${encodeURIComponent(p.id)}`, iat, exp: iat + SESSION_S });
        this.ses.set(sid, { pid: p.id, cid: id, name: c.name, ready: false, exp: (iat + SESSION_S) * 1000, coins: 0, n: 0, t: {} });
        this.room.send(c, { t: "hub_s", p: p.id, tok, url: a.url, origin: a.origin, name: a.name, ret: `${this.site}/?hub=${encodeURIComponent(p.id)}` });
        await this.room.uni?.onSession(c, { sub: c.name, pid: p.id, sid });
    }

    // ------------------------------------------------ eventos do jogo (upp:event repassado pelo overlay)
    event(t, s, p, d) {
        if (++s.n > 300 || !this.limit(`sid:${t.sid}`, 30, 60000)) return { ok: false, code: 429, error: "rate limit" };
        const now = Date.now();
        const pname = cur(p)?.name || p.id;
        const type = String(d?.type ?? "");
        if (type === "score") {
            const v = Math.max(0, Math.min(1e9, Math.floor(Number(d.value) || 0)));
            if (!v || now - (s.t.score || 0) < 30000 || s.coins >= SESSION_COINS) return { ok: true, coins: 0 };
            const day = new Date(now).toISOString().slice(0, 10);
            if (this.s.d.day !== day) this.s.d = { day, m: {} };
            const k = norm(t.sub);
            const used = this.s.d.m[k] || 0;
            const eco = this.room.eco;
            const coins = Math.min(5, 1 + Math.floor(Math.log10(v + 1) / 2), SESSION_COINS - s.coins, DAILY_COINS - used, eco.s.tr);
            if (coins <= 0) return { ok: true, coins: 0 };
            s.t.score = now;
            s.coins += coins;
            this.s.d.m[k] = used + coins;
            const w = eco.wallet(t.sub);
            eco.s.tr -= coins;
            w.c += coins;
            eco.entry(t.sub, `fez ${v} pontos no portal ${pname}`, coins, "premio do HUB pago do cofre (moeda ficticia, limitado por sessao/dia)");
            eco.sendMe(t.sub);
            this.save();
            this.say(`${t.sub} fez ${v} pontos em ${pname} (+${coins} moedas ficticias)`);
            return { ok: true, coins };
        }
        if (type === "achievement" || type === "chat") {
            const gap = type === "chat" ? 3000 : 10000;
            if (now - (s.t[type] || 0) < gap) return { ok: false, code: 429, error: "devagar" };
            const text = clean(d.text ?? d.value, type === "chat" ? 120 : 60);
            if (!text || !okText(text)) return { ok: false, code: 422, error: "texto recusado pelo filtro" };
            s.t[type] = now;
            this.say(type === "chat" ? `${t.sub}: ${text}` : `${t.sub} conquistou "${text}"`, pname);
            return { ok: true };
        }
        return { ok: false, code: 400, error: "type: score | achievement | chat" };
    }

    // ------------------------------------------------ verificacao da origem
    async prove(p, origin) {
        let r;
        try {
            r = await fetch(`${origin}/.well-known/urna-portal.json?c=${hex(4)}`, {
                redirect: "manual",
                headers: { accept: "application/json", "user-agent": "UrnaPortalVerifier/1" },
                signal: AbortSignal.timeout(8000),
            });
        } catch (e) {
            return `nao consegui buscar ${origin}/.well-known/urna-portal.json (${String(e.message || e).slice(0, 80)})`;
        }
        if (r.status !== 200) return `HTTP ${r.status} em ${origin}/.well-known/urna-portal.json (redirect nao vale)`;
        const text = await r.text();
        if (text.length > 16384) return "urna-portal.json maior que 16 KB";
        let j;
        try {
            j = JSON.parse(text);
        } catch (e) {
            return "urna-portal.json nao e JSON";
        }
        const list = Array.isArray(j?.portals) ? j.portals : [j];
        return list.some((x) => x && x.id === p.id && x.challenge === p.challenge) ? null : "urna-portal.json sem {id, challenge} desse portal";
    }

    view(p, owner) {
        const a = cur(p);
        const last = p.versions[p.versions.length - 1];
        const v = {
            id: p.id,
            author: p.author,
            active: p.active,
            live: live(p),
            verified_origin: p.verified?.origin || null,
            players: this.count(p.id),
            current: a ? { version: a.v, name: a.name, description: a.desc, url: a.url, origin: a.origin, colors: a.colors } : null,
            versions: p.versions.map((x) => ({ version: x.v, name: x.name, url: x.url, origin: x.origin, at: x.at })),
        };
        if (owner) {
            const origin = last.origin;
            v.challenge = p.challenge;
            v.well_known = { url: `${origin}/.well-known/urna-portal.json`, body: { urna_portal: 1, portals: [{ id: p.id, challenge: p.challenge }] } };
            v.next = !p.verified || p.verified.origin !== origin ? `sirva well_known.body em well_known.url e chame POST /api/portals/${p.id}/verify` : !live(p) ? `POST /api/portals/${p.id}/activate {"version":"${last.v}"}` : "no ar";
        }
        return v;
    }

    // ------------------------------------------------ HTTP /api/portals/*  (null = nao e rota do hub)
    async route(req) {
        const url = new URL(req.url);
        if (!this.site) this.site = url.origin;
        if (!url.pathname.startsWith("/api/portals")) return null;
        try {
            return await this.http(req, url);
        } catch (e) {
            if (e instanceof HttpError) return fail(e.status, e.message);
            return fail(500, String(e.message || e).slice(0, 120));
        }
    }

    async http(req, url) {
        if (req.method === "OPTIONS") return new Response(null, { status: 204, headers: CORS });
        const ip = req.headers.get("cf-connecting-ip") || "?";
        if (!this.limit(`ip:${ip}`, 120, 60000)) return fail(429, "rate limit (120/min por IP)");
        const [a, b] = url.pathname.replace(/\/+$/, "").split("/").slice(3).map(decodeURIComponent);
        const m = req.method;

        if (!a && m === "GET") return json({ ok: true, protocol: "UPP/1", docs: `${this.site}/hub.txt`, portals: Object.values(this.s.p).filter(live).map((p) => this.view(p, false)) });
        if (a === "verify" && m === "GET") {
            const t = await this.check(url.searchParams.get("token"));
            if (!t) return fail(401, "token invalido ou expirado");
            const p = this.s.p[t.pid];
            if (!p || !live(p)) return fail(410, "portal fora do ar");
            return json({ ok: true, portal: t.pid, aud: t.aud, sid: t.sid, player: t.player, ret: t.ret, exp: t.exp });
        }
        if (a === "session" && m === "POST") {
            const d = await body(req);
            const t = await this.check(d.token);
            if (!t) return fail(401, "token invalido ou expirado");
            const p = this.s.p[t.pid];
            if (!p) return fail(410, "portal fora do ar");
            let s = this.ses.get(t.sid);
            if (!s) {
                s = { pid: t.pid, cid: 0, name: t.sub, ready: false, exp: t.exp * 1000, coins: 0, n: 0, t: {} };
                this.ses.set(t.sid, s);
            }
            if (d.kind === "ready") {
                if (!s.ready) {
                    s.ready = true;
                    this.changed();
                    this.say(`${t.sub} entrou no portal ${cur(p)?.name || p.id}`);
                }
                return json({ ok: true });
            }
            if (d.kind === "exit") {
                if (this.ses.delete(t.sid) && s.ready) this.changed();
                return json({ ok: true });
            }
            if (d.kind === "event") {
                const { code, ...r } = this.event(t, s, p, d.data);
                return json(r, code || 200);
            }
            return fail(400, "kind: ready | exit | event");
        }

        const who = await creatorFromRequest(this.room, req);
        if (a === "mine" && m === "GET") {
            if (!who) return fail(401, "Authorization: Bearer <token de criador> (POST /api/mods/register)");
            return json({ ok: true, portals: Object.values(this.s.p).filter((p) => p.owner === who.id).map((p) => this.view(p, true)) });
        }
        if (!a && m === "POST") {
            if (!who) return fail(401, "Authorization: Bearer <token de criador> (POST /api/mods/register)");
            if (!this.limit(`w:${who.id}`, 20, 60000)) return fail(429, "rate limit de escrita (20/min)");
            const d = await body(req);
            const man = manifest(d, this.site);
            const id = clean(d.id || norm(man.name).replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""), 32);
            if (!/^[a-z0-9][a-z0-9-]{1,31}$/.test(id)) throw new HttpError(400, "id: slug a-z0-9- (2-32)");
            if (this.s.p[id]) throw new HttpError(409, `id ${id} ja existe (use PUT /api/portals/${id}/versions pra nova versao)`);
            const all = Object.values(this.s.p);
            if (all.length >= MAX_PORTALS) throw new HttpError(507, "hub cheio");
            if (all.filter((p) => p.owner === who.id).length >= PER_OWNER) throw new HttpError(403, `max ${PER_OWNER} portais por criador`);
            const p = { id, owner: who.id, author: clean(who.name, 24), created: Date.now(), challenge: `upp-${hex(16)}`, verified: null, active: null, prev: [], versions: [man] };
            this.s.p[id] = p;
            this.save();
            return json({ ok: true, portal: this.view(p, true) }, 201);
        }

        const p = this.s.p[a];
        if (!p) return fail(404, "portal nao existe");
        const owner = !!who && who.id === p.owner;
        if (!b && m === "GET") return json({ ok: true, portal: this.view(p, owner) });
        if (!who) return fail(401, "Authorization: Bearer <token de criador>");
        if (!owner) return fail(403, "so o dono mexe nesse portal");
        if (!this.limit(`w:${who.id}`, 20, 60000)) return fail(429, "rate limit de escrita (20/min)");

        if (b === "versions" && (m === "PUT" || m === "POST")) {
            const man = manifest(await body(req), this.site);
            const last = p.versions[p.versions.length - 1];
            if (!newer(semv(man.v), semv(last.v))) throw new HttpError(409, `version precisa ser maior que ${last.v}`);
            p.versions.push(man);
            if (p.versions.length > MAX_VERSIONS) p.versions = p.versions.filter((x, i) => x.v === p.active || i >= p.versions.length - MAX_VERSIONS + 1);
            this.save();
            return json({ ok: true, portal: this.view(p, true) }, 201);
        }
        if (b === "verify" && m === "POST") {
            if (!this.limit(`v:${p.id}`, 6, 60000)) return fail(429, "max 6 verificacoes/min");
            const origin = p.versions[p.versions.length - 1].origin;
            const e = await this.prove(p, origin);
            if (e) return fail(422, e, { well_known: this.view(p, true).well_known });
            p.verified = { origin, at: Date.now() };
            this.save();
            this.changed();
            return json({ ok: true, portal: this.view(p, true) });
        }
        if (b === "activate" && m === "POST") {
            const d = await body(req);
            const v = String(d.version ?? p.versions[p.versions.length - 1].v);
            const x = p.versions.find((y) => y.v === v);
            if (!x) throw new HttpError(404, `versao ${v} nao existe`);
            if (!p.verified || p.verified.origin !== x.origin) throw new HttpError(409, `origem ${x.origin} nao verificada: POST /api/portals/${p.id}/verify antes`);
            if (p.active && p.active !== v) p.prev = [...p.prev, p.active].slice(-MAX_VERSIONS);
            p.active = v;
            this.save();
            this.changed();
            this.say(`portal novo no corredor: ${x.name} v${x.v} (por ${p.author})`);
            return json({ ok: true, portal: this.view(p, true) });
        }
        if (b === "rollback" && m === "POST") {
            let v;
            while ((v = p.prev.pop()) && !p.versions.some((y) => y.v === v));
            if (!v) throw new HttpError(409, "sem versao anterior pra voltar");
            const x = p.versions.find((y) => y.v === v);
            if (!p.verified || p.verified.origin !== x.origin) throw new HttpError(409, `origem ${x.origin} nao verificada`);
            p.active = v;
            this.save();
            this.changed();
            return json({ ok: true, portal: this.view(p, true) });
        }
        if (!b && m === "DELETE") {
            if (p.active) p.prev = [...p.prev, p.active].slice(-MAX_VERSIONS);
            p.active = null;
            this.save();
            this.changed();
            return json({ ok: true, portal: this.view(p, true) });
        }
        return fail(404, "rota nao existe (veja /hub.txt)");
    }
}
