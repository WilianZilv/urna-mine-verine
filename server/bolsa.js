// Bolsa de Valores da Vila: acoes FICTICIAS que reagem a arena e a cidade; contraparte e o cofre da IA (src/places/bolsa.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("bolsa", this.s)).
// Moeda nunca e criada: compra paga o cofre (eco.s.tr), venda sai do cofre (so acima da reserva), taxa fica no cofre.
import { norm, amount } from "./places.js";
import { CLUB } from "./layout.js";

// g/i = grupo e indice do NPC no golpe ("a"): 0 lutadores (ordem de actors.rs), 4 urna, 9 kaiju
const TK = [
    { s: "LULA", n: "LULA", g: 0, i: 0 },
    { s: "FLAV", n: "FLAVIO", g: 0, i: 1 },
    { s: "RENA", n: "RENAN", g: 0, i: 2 },
    { s: "WOLV", n: "WOLVERINE", g: 0, i: 3 },
    { s: "URNA", n: "URNA GIGANTE", g: 4 },
    { s: "GODZ", n: "GODZILHA", g: 9 },
    { s: "HOUSE", n: "HOUSE CLUB" },
    { s: "COFRE", n: "COFRE DA IA" },
];
const BASE = 100;
const HIST = 48;
const AGO = 30; // ticks de 20 s = 10 min
const MAX = 500;
const RESERVE = 300;
const WHY = "bolsa ficticia: paga o cofre da IA";
const BREAK = 0.08; // variacao num tick que dispara o alerta
const ALERT_MS = 60 * 1000;
const SAY_MS = 2 * 60 * 1000;
const TIPS = [
    "analista: compra na alta e vende na baixa, confia",
    "analista: e oportunidade ou cilada? nem ele sabe",
    "analista: diversifica (compra tudo que ta caindo)",
    "analista: isso aqui nao e conselho financeiro, e zoeira",
    "analista: o grafico parece um pato, entao e alta",
];

// Acoes de mods: todo mod npc ativo concorre; os MOD_CAP mais populares ficam listados (s.m[SYM]).
const MOD_BASE = 50;
const MOD_CAP = 6;
const MOD_GROUP = 8; // grupo das entidades de mod no golpe (npc.rs MODS), indice achatado por max_instances
const HYST = 20; // vantagem de quem ja esta listado no ranking (evita entra-e-sai)
const HYPE_MS = 60 * 60 * 1000;
const IDLE = 15; // ticks sem porrada/compra = esquecido
const IPO_MS = 20 * 1000; // takeover do painel
const IPO_DISC_MS = 2 * 60 * 1000;
const IPO_SLOTS = 3;
const IPO_OFF = 0.1;

const r2 = (v) => Math.round(v * 100) / 100;
const clampP = (v) => Math.min(1000, Math.max(5, v));
const gauss = () => Math.sqrt(-2 * Math.log(1 - Math.random())) * Math.cos(2 * Math.PI * Math.random());
const inst = (a) => Math.min(4, Math.max(1, Math.round(Number(a.pkg?.behavior?.spawn?.max_instances) || 1)));
const modName = (a) => norm(a.pkg?.manifest?.name || a.id).toUpperCase().replace(/[^A-Z0-9 ]/g, "").replace(/\s+/g, " ").trim().slice(0, 24);

/// Simbolo unico: 4 primeiras letras do nome normalizado, depois 5, depois prefixo + numero.
export function modSymbol(name, taken) {
    const u = norm(name).toUpperCase().replace(/[^A-Z]/g, "");
    const s4 = (u + "MODX").slice(0, 4);
    const cands = [s4, u.length >= 5 ? u.slice(0, 5) : null];
    for (let d = 2; d < 100; d++) cands.push(s4.slice(0, 4 - String(d).length) + d);
    return cands.find((c) => c && !taken.has(c));
}

export class Bolsa {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
        for (const k of ["p", "h", "why", "pos", "nm", "m", "pop"]) s[k] ??= {};
        for (const t of TK) {
            s.p[t.s] ??= BASE;
            s.h[t.s] ??= [s.p[t.s]];
            s.why[t.s] ??= "abertura do pregao";
        }
        this.imp = Object.fromEntries(TK.map((t) => [t.s, { hit: 0, trade: 0 }]));
        for (const S of Object.keys(s.m)) {
            s.p[S] ??= MOD_BASE;
            s.h[S] ??= [s.p[S]];
            s.why[S] ??= "IPO na bolsa";
            this.imp[S] = { hit: 0, trade: 0, kill: 0, news: 0 };
        }
        this.mh = new Map(); // id do mod -> {hit, kill} do tick
        this.dmg = new Map(); // "id#k" -> dano acumulado na instancia (estimativa de abate)
        this.last = "";
        this.alert = { mood: 0, text: "", until: 0 };
        this.said = 0;
    }

    /// Todos os tickers negociaveis: fixos + mods listados ({s, n, id, c}).
    tickers() {
        return [...TK, ...Object.entries(this.s.m).map(([S, m]) => ({ s: S, n: m.n, id: m.id, c: m.c }))];
    }

    find(q) {
        const u = norm(q).toUpperCase();
        const all = this.tickers();
        return u ? all.find((t) => t.s === u) || (u.length >= 3 ? all.find((t) => t.n.startsWith(u)) : null) : null;
    }

    modTk() {
        return Object.entries(this.s.m).map(([S, m]) => ({ s: S, n: m.n, c: m.c, p: this.s.p[S], d: this.delta(S), h: this.s.h[S], w: this.s.why[S] }));
    }

    /// Mod npc ativo dono da instancia achatada `i` (mesma ordem do cliente: ativacao, max_instances cada).
    modAt(i) {
        let k = i;
        for (const a of this.pl.room.mods?.list?.("npc") || []) {
            const n = inst(a);
            if (k < n) return [a, k];
            k -= n;
        }
        return [null, 0];
    }

    /// Reconcilia a lista de mods ativos com os listados: desliste quem saiu, liste o top MOD_CAP.
    sync(now) {
        const mods = this.pl.room.mods?.list?.("npc") || [];
        const act = new Map(mods.map((a) => [a.id, a]));
        for (const id of Object.keys(this.s.pop)) if (!act.has(id)) delete this.s.pop[id];
        const listed = new Map(Object.entries(this.s.m).map(([S, m]) => [m.id, S]));
        for (const [id, S] of listed) if (!act.has(id)) this.delist(S, "mod desativado"), listed.delete(id);
        const score = (a) => (this.s.pop[a.id] || 0) + inst(a) + Math.min(5, (now - (a.at || now)) / 3600000) + (listed.has(a.id) ? HYST : 0);
        const top = mods.map((a) => [a, score(a)]).sort((x, y) => y[1] - x[1] || x[0].at - y[0].at).slice(0, MOD_CAP).map((e) => e[0]);
        const want = new Set(top.map((a) => a.id));
        for (const [id, S] of listed) if (!want.has(id)) this.delist(S, "caiu do top 6 de popularidade");
        for (const a of top) {
            const S = listed.get(a.id);
            if (!S) this.list(a, now);
            else if (this.s.m[S].v !== a.v) {
                this.s.m[S].v = a.v;
                this.imp[S].news = 0.03;
            }
        }
    }

    list(a, now) {
        const S = modSymbol(modName(a), new Set(this.tickers().map((t) => t.s)));
        if (!S) return;
        const n = modName(a) || S;
        this.s.m[S] = { id: a.id, n, c: String(a.creator || "").slice(0, 16), v: a.v, at: now, idle: 0, ipo: { until: now + IPO_DISC_MS, b: [] } };
        this.s.p[S] = MOD_BASE;
        this.s.h[S] = [MOD_BASE];
        this.s.why[S] = "IPO na bolsa";
        this.imp[S] = { hit: 0, trade: 0, kill: 0, news: 0 };
        this.pl.say("BOLSA", `IPO de ${n} (${S}) a ${MOD_BASE.toFixed(2)} - /investir ${S} n`);
    }

    /// IPO mais recente ainda no takeover do painel (20 s), com as vagas de desconto.
    ipo(now) {
        const e = Object.entries(this.s.m).filter(([, m]) => now - m.at < IPO_MS).sort((x, y) => y[1].at - x[1].at)[0];
        if (!e) return null;
        const [S, m] = e;
        const slots = m.ipo && now < m.ipo.until ? IPO_SLOTS - m.ipo.b.length : 0;
        return { s: S, n: m.n, c: m.c, p: this.s.p[S], left: Math.ceil((m.at + IPO_MS - now) / 1000), slots, dl: slots ? Math.ceil((m.ipo.until - now) / 1000) : 0 };
    }

    /// Manchete pra TV (plantao quando muda).
    news() {
        const e = Object.entries(this.s.m).filter(([, m]) => Date.now() - m.at < 10 * 60 * 1000).sort((x, y) => y[1].at - x[1].at)[0];
        return e ? [`IPO na bolsa: ${e[1].n} (${e[0]}) estreia a ${MOD_BASE.toFixed(2)}`] : [];
    }

    /// Tira o ticker do pregao pagando o ultimo preco aos donos (do cofre, so acima da reserva).
    delist(S, reason) {
        const m = this.s.m[S];
        const p = this.s.p[S] ?? 0;
        const eco = this.eco;
        const hold = Object.entries(this.s.pos).filter(([, pos]) => pos[S] > 0).map(([k, pos]) => [k, pos[S], Math.round(p * pos[S])]);
        const owed = hold.reduce((a, h) => a + h[2], 0);
        const avail = Math.max(0, (eco?.s?.tr ?? 0) - RESERVE);
        const ratio = owed ? Math.min(1, avail / owed) : 1;
        let paid = 0;
        for (const [k, n, due] of hold) {
            const pay = Math.floor(due * ratio);
            const name = this.s.nm[k] || k;
            eco.wallet(name).c += pay;
            eco.s.tr -= pay;
            paid += pay;
            eco.entry(name, `${S} saiu da bolsa: ${n} x ${p.toFixed(2)}`, pay, ratio < 1 ? `cofre curto: pagou ${pay} de ${due}` : WHY);
            delete this.s.pos[k][S];
            if (!Object.keys(this.s.pos[k]).length) delete this.s.pos[k];
            eco.sendMe(name);
        }
        for (const o of [this.s.p, this.s.h, this.s.why, this.s.m, this.imp]) delete o[S];
        const short = paid < owed ? ` de ${owed} (cofre curto, o resto virou prejuizo)` : "";
        this.pl.say("BOLSA", `${m?.n || S} (${S}) saiu da bolsa (${reason}). donos receberam ${paid}${short}`);
    }

    get eco() {
        return this.pl.room.eco;
    }

    /// Valor da carteira (nome normalizado) a preco atual. O Banco usa isso.
    value(k) {
        const pos = this.s.pos[norm(k)] || {};
        return Math.round(Object.entries(pos).reduce((v, [S, n]) => v + (this.s.p[S] ?? 0) * n, 0));
    }

    delta(S) {
        const h = this.s.h[S];
        const ref = h[Math.max(0, h.length - 1 - AGO)] || BASE;
        return Math.round((this.s.p[S] / ref - 1) * 1000) / 10;
    }

    /// Indice da vila: media das variacoes de 10 min (%).
    index() {
        return Math.round((TK.reduce((a, t) => a + this.delta(t.s), 0) / TK.length) * 10) / 10;
    }

    hot() {
        const best = this.tickers().map((t) => [t, this.delta(t.s)]).sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))[0];
        const [t, d] = best;
        if (Math.abs(d) < 0.1) return "ANALISTA: mercado de lado. ninguem sabe de nada, como sempre";
        const tip = TIPS[(t.s.length + Math.floor(Math.abs(d))) % TIPS.length];
        return `ANALISTA: ${t.n} ${d > 0 ? "dispara" : "despenca"} ${Math.abs(d)}% em 10 min (${this.s.why[t.s]}). ${tip}`;
    }

    snapshot() {
        const top = Object.keys(this.s.pos)
            .map((k) => [this.s.nm[k] || k, this.value(k)])
            .filter((e) => e[1] > 0)
            .sort((a, b) => b[1] - a[1])
            .slice(0, 5);
        const tk = TK.map((t) => ({ s: t.s, n: t.n, p: this.s.p[t.s], d: this.delta(t.s), h: this.s.h[t.s], w: this.s.why[t.s] }));
        const a = Date.now() < this.alert.until ? this.alert : { mood: 0, text: "" };
        return { t: "pl", k: "bolsa", tk, mk: this.modTk(), ipo: this.ipo(Date.now()), top, hot: this.hot(), ix: this.index(), mood: a.mood, alert: a.text };
    }

    push() {
        const snap = this.snapshot();
        const j = JSON.stringify(snap);
        if (j === this.last) return;
        this.last = j;
        this.pl.room.broadcast(snap);
    }

    join(c) {
        this.pl.room.send(c, this.snapshot());
    }

    onHit(c, m) {
        if (!["pf", "hit", "npc"].includes(m.k)) return;
        const g = Number(m.g), i = Number(m.i);
        if (g === MOD_GROUP) return this.modHit(i, m.dmg);
        const t = TK.find((t) => t.g === g && (g !== 0 || t.i === i));
        if (t) this.imp[t.s].hit = Math.max(-0.06, this.imp[t.s].hit - 0.0025);
    }

    /// Porrada em entidade de mod = engajamento (sobe). Abate estimado quando o dano somado passa do hp.
    modHit(i, dmg) {
        if (!(i >= 0)) return;
        const [a, k] = this.modAt(i);
        if (!a) return;
        const e = this.mh.get(a.id) || { hit: 0, kill: 0 };
        e.hit++;
        const key = `${a.id}#${k}`;
        const sum = (this.dmg.get(key) || 0) + Math.min(500, Math.max(0, Number(dmg) || 0));
        const hp = Math.max(1, Number(a.pkg?.behavior?.stats?.hp) || 200);
        if (sum >= hp) {
            e.kill++;
            this.dmg.delete(key);
        } else this.dmg.set(key, sum);
        if (this.dmg.size > 200) this.dmg.clear();
        this.mh.set(a.id, e);
    }

    /// Popularidade (decai) e impulsos de preco dos mods a partir dos golpes do tick.
    modSignals() {
        for (const id of Object.keys(this.s.pop)) this.s.pop[id] *= 0.97;
        const sym = new Map(Object.entries(this.s.m).map(([S, m]) => [m.id, S]));
        for (const [id, e] of this.mh) {
            this.s.pop[id] = (this.s.pop[id] || 0) + e.hit + 5 * e.kill;
            const S = sym.get(id);
            if (!S) continue;
            this.imp[S].hit = Math.min(0.06, this.imp[S].hit + 0.002 * e.hit);
            this.imp[S].kill = Math.min(0.08, this.imp[S].kill + 0.02 * e.kill);
        }
        this.mh.clear();
        for (const id of Object.keys(this.s.pop)) this.s.pop[id] = Math.round(this.s.pop[id] * 100) / 100;
    }

    tick(now, online) {
        if (!online) return;
        this.modSignals();
        this.sync(now);
        const club = [...this.pl.room.clients.values()].filter((c) => {
            const p = Array.isArray(c.pos) ? c.pos.map(Number) : null;
            return p && p[0] >= CLUB.x0 && p[0] <= CLUB.x1 && p[2] >= CLUB.z0 && p[2] <= CLUB.z1;
        }).length;
        const tr = this.eco?.s?.tr ?? BASE * 10;
        let big = null;
        for (const t of TK) {
            const S = t.s;
            const p = this.s.p[S];
            const cofre = S === "COFRE";
            const mean = cofre ? clampP(tr / 10) : BASE;
            const imp = this.imp[S];
            // contribuicoes em log-preco (cada uma com o seu "porque")
            const c = {
                drift: Math.max(-0.03, Math.min(0.03, -(cofre ? 0.1 : 0.01) * Math.log(p / mean))),
                noise: 0.006 * gauss(),
                hit: Math.log(1 + imp.hit),
                trade: Math.log(1 + imp.trade),
                club: S === "HOUSE" ? (club ? Math.log(1 + Math.min(0.05, 0.01 * club)) : -0.004) : 0,
            };
            const np = r2(clampP(Math.exp(Math.log(p) + Object.values(c).reduce((a, b) => a + b, 0))));
            const [k, v] = Object.entries(c).sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))[0];
            const up = v > 0;
            this.s.why[S] = {
                drift: cofre ? (up ? "cofre engordou" : "cofre emagreceu") : up ? "volta pra media (barganha)" : "volta pra media (bolha murchou)",
                noise: up ? "otimismo sem motivo" : "boato no zap da familia",
                hit: t.g === 0 ? "apanhou na arena" : "levou porrada dos jogadores",
                trade: up ? "compra de jogadores" : "venda de jogadores (realizacao)",
                club: up ? `casa cheia no club (${club})` : "club vazio, DJ dormindo",
            }[k];
            this.s.p[S] = np;
            const h = this.s.h[S];
            h.push(np);
            if (h.length > HIST) h.splice(0, h.length - HIST);
            imp.hit = 0;
            imp.trade = 0;
            const mv = np / p - 1;
            if (Math.abs(mv) >= BREAK && (!big || Math.abs(mv) > Math.abs(big.mv))) big = { t, mv };
        }
        for (const [S, m] of Object.entries(this.s.m)) {
            if (m.at === now) continue;
            const p = this.s.p[S];
            const imp = this.imp[S];
            const busy = imp.hit || imp.kill || imp.trade;
            m.idle = busy ? 0 : (m.idle || 0) + 1;
            const c = {
                drift: Math.max(-0.03, Math.min(0.03, -0.01 * Math.log(p / MOD_BASE))),
                noise: 0.008 * gauss(),
                hit: Math.log(1 + imp.hit),
                kill: Math.log(1 + imp.kill),
                trade: Math.log(1 + imp.trade),
                hype: now - m.at < HYPE_MS ? 0.003 : m.idle >= IDLE ? -0.003 : 0,
                news: Math.log(1 + imp.news),
            };
            const np = r2(clampP(Math.exp(Math.log(p) + Object.values(c).reduce((a, b) => a + b, 0))));
            const [k, v] = Object.entries(c).sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))[0];
            const up = v > 0;
            this.s.why[S] = {
                drift: up ? "volta pra media (barganha)" : "volta pra media (bolha murchou)",
                noise: up ? "otimismo sem motivo" : "boato no zap da familia",
                hit: "apanhando muito = engajamento",
                kill: "abatido na arena (viralizou)",
                trade: up ? "compra de jogadores" : "venda de jogadores (realizacao)",
                hype: up ? "hype de lancamento" : "esquecido no canto da zona",
                news: `versao nova (v${m.v})`,
            }[k];
            this.s.p[S] = np;
            const h = this.s.h[S];
            h.push(np);
            if (h.length > HIST) h.splice(0, h.length - HIST);
            imp.hit = imp.kill = imp.trade = imp.news = 0;
            const mv = np / p - 1;
            if (Math.abs(mv) >= BREAK && (!big || Math.abs(mv) > Math.abs(big.mv))) big = { t: { s: S }, mv };
        }
        if (big) this.breaker(now, big.t, big.mv);
        this.pl.save("bolsa", this.s);
        this.push();
    }

    /// Movimento >= 8% num tick: alerta no painel por 1 min e grito no chat (no maximo 1 a cada 2 min).
    breaker(now, t, mv) {
        const d = Math.round(mv * 1000) / 10;
        const text = mv < 0
            ? `CIRCUIT BREAKER: ${t.s} ${d}% (${this.s.why[t.s]}). pregao em panico, alguem chama o Banco`
            : `DISPAROU: ${t.s} +${d}% (${this.s.why[t.s]}). euforia total, tio do zap ja ta vendendo curso`;
        this.alert = { mood: mv < 0 ? -1 : 1, text, until: now + ALERT_MS };
        if (now - this.said < SAY_MS) return;
        this.said = now;
        this.pl.say("BOLSA", text);
    }

    command(id, c, head, args) {
        switch (head) {
            case "bolsa": {
                const q = (t) => `${t.s} ${this.s.p[t.s].toFixed(2)} (${this.delta(t.s) >= 0 ? "+" : ""}${this.delta(t.s)}%)`;
                const mk = Object.keys(this.s.m).length;
                if (norm(args[0]) === "mods") {
                    if (!mk) return this.pl.priv(c, "BOLSA", `nenhum mod listado. todo mod npc ativo vira acao (top ${MOD_CAP} por popularidade)`), true;
                    this.pl.priv(c, "BOLSA", "MODS: " + this.tickers().filter((t) => t.id).map((t) => `${q(t)} ${t.n} por ${t.c}`).join(" | "));
                    return true;
                }
                this.pl.priv(c, "BOLSA", TK.map(q).join(" | ") + (mk ? ` | ${mk} mods: /bolsa mods` : "") + " | /investir TICKER n /vender TICKER n /carteira");
                return true;
            }
            case "investir":
                this.trade(c, args, 1);
                return true;
            case "vender":
                this.trade(c, args, -1);
                return true;
            case "carteira": {
                const k = norm(c.name);
                const pos = Object.entries(this.s.pos[k] || {});
                const w = this.eco.wallet(c.name);
                if (!pos.length) return this.pl.priv(c, "BOLSA", `carteira vazia (${w.c} moedas). tenta /investir LULA 5`), true;
                const list = pos.map(([S, n]) => `${S} ${n} x ${this.s.p[S].toFixed(2)} = ${Math.round(this.s.p[S] * n)}`).join(" | ");
                this.pl.priv(c, "BOLSA", `${list} | total ${this.value(k)} | moedas ${w.c}`);
                return true;
            }
        }
        return false;
    }

    trade(c, args, dir) {
        const verb = dir > 0 ? "investir" : "vender";
        const t = this.find(args[0]);
        const n = amount(args[1]);
        if (!t || !(n >= 1 && n <= MAX)) return this.pl.priv(c, "BOLSA", `uso: /${verb} TICKER n (1..${MAX}). tickers: ${this.tickers().map((t) => t.s).join(" ")}`);
        const eco = this.eco;
        const w = eco.wallet(c.name);
        const k = norm(c.name);
        const S = t.s;
        const ipo = t.id ? this.s.m[S].ipo : null;
        const off = dir > 0 && ipo && Date.now() < ipo.until && ipo.b.length < IPO_SLOTS && !ipo.b.includes(k);
        const p = off ? r2(this.s.p[S] * (1 - IPO_OFF)) : this.s.p[S];
        const pos = this.s.pos[k] || {};
        const have = pos[S] || 0;
        const gross = Math.round(p * n);
        const fee = Math.max(1, Math.round(gross * 0.01));
        const imp = this.imp[S];
        if (dir > 0) {
            if (have + n > MAX) return this.pl.priv(c, "BOLSA", `limite ${MAX} acoes de ${S} por jogador (tens ${have})`);
            const cost = gross + fee;
            if (w.c < cost) return this.pl.priv(c, "BOLSA", `${n} ${S} custam ${cost} com taxa, tens ${w.c}`);
            w.c -= cost;
            eco.s.tr += cost;
            pos[S] = have + n;
            imp.trade = Math.min(0.05, imp.trade + 0.003 * n);
            if (off) ipo.b.push(k);
            eco.entry(c.name, `investiu ${n} ${S} a ${p.toFixed(2)}${off ? " (IPO -10%)" : ""}`, cost, WHY);
            this.pl.priv(c, "BOLSA", `comprou ${n} ${S} por ${cost} (taxa ${fee})${off ? ` com desconto de IPO 10% (${ipo.b.length}/${IPO_SLOTS})` : ""}. agora tens ${pos[S]}`);
        } else {
            if (have < n) return this.pl.priv(c, "BOLSA", `tens so ${have} ${S}`);
            if (eco.s.tr - gross < RESERVE) return this.pl.priv(c, "BOLSA", "cofre sem liquidez, tenta depois");
            const pay = gross - fee;
            w.c += pay;
            eco.s.tr -= pay;
            pos[S] = have - n;
            if (!pos[S]) delete pos[S];
            imp.trade = Math.max(-0.05, imp.trade - 0.003 * n);
            eco.entry(c.name, `vendeu ${n} ${S} a ${p.toFixed(2)}`, pay, WHY);
            this.pl.priv(c, "BOLSA", `vendeu ${n} ${S} por ${pay} (taxa ${fee}). restam ${pos[S] || 0}`);
        }
        if (t.id) this.s.pop[t.id] = (this.s.pop[t.id] || 0) + 0.2 * n;
        if (Object.keys(pos).length) this.s.pos[k] = pos;
        else delete this.s.pos[k];
        this.s.nm[k] = c.name;
        eco.sendMe(c.name);
        this.pl.save("bolsa", this.s);
        this.push();
    }
}
