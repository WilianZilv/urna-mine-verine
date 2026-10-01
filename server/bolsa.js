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
const TIPS = [
    "analista: compra na alta e vende na baixa, confia",
    "analista: e oportunidade ou cilada? nem ele sabe",
    "analista: diversifica (compra tudo que ta caindo)",
    "analista: isso aqui nao e conselho financeiro, e zoeira",
    "analista: o grafico parece um pato, entao e alta",
];

const r2 = (v) => Math.round(v * 100) / 100;
const clampP = (v) => Math.min(1000, Math.max(5, v));
const gauss = () => Math.sqrt(-2 * Math.log(1 - Math.random())) * Math.cos(2 * Math.PI * Math.random());
const find = (q) => {
    const u = norm(q).toUpperCase();
    return u ? TK.find((t) => t.s === u || (u.length >= 3 && t.n.startsWith(u))) : null;
};

export class Bolsa {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
        for (const k of ["p", "h", "why", "pos", "nm"]) s[k] ??= {};
        for (const t of TK) {
            s.p[t.s] ??= BASE;
            s.h[t.s] ??= [s.p[t.s]];
            s.why[t.s] ??= "abertura do pregao";
        }
        this.imp = Object.fromEntries(TK.map((t) => [t.s, { hit: 0, trade: 0 }]));
        this.last = "";
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

    hot() {
        const best = TK.map((t) => [t, this.delta(t.s)]).sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))[0];
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
        return { t: "pl", k: "bolsa", tk, top, hot: this.hot() };
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
        const t = TK.find((t) => t.g === g && (g !== 0 || t.i === i));
        if (t) this.imp[t.s].hit = Math.max(-0.06, this.imp[t.s].hit - 0.0025);
    }

    tick(now, online) {
        if (!online) return;
        const club = [...this.pl.room.clients.values()].filter((c) => {
            const p = Array.isArray(c.pos) ? c.pos.map(Number) : null;
            return p && p[0] >= CLUB.x0 && p[0] <= CLUB.x1 && p[2] >= CLUB.z0 && p[2] <= CLUB.z1;
        }).length;
        const tr = this.eco?.s?.tr ?? BASE * 10;
        for (const t of TK) {
            const S = t.s;
            const p = this.s.p[S];
            const cofre = S === "COFRE";
            const mean = cofre ? clampP(tr / 10) : BASE;
            const imp = this.imp[S];
            // contribuicoes em log-preco (cada uma com o seu "porque")
            const c = {
                drift: -(cofre ? 0.1 : 0.01) * Math.log(p / mean),
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
        }
        this.pl.save("bolsa", this.s);
        this.push();
    }

    command(id, c, head, args) {
        switch (head) {
            case "bolsa":
                this.pl.priv(c, "BOLSA", TK.map((t) => `${t.s} ${this.s.p[t.s].toFixed(2)} (${this.delta(t.s) >= 0 ? "+" : ""}${this.delta(t.s)}%)`).join(" | ") + " | /investir TICKER n /vender TICKER n /carteira");
                return true;
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
        const t = find(args[0]);
        const n = amount(args[1]);
        if (!t || !(n >= 1 && n <= MAX)) return this.pl.priv(c, "BOLSA", `uso: /${verb} TICKER n (1..${MAX}). tickers: ${TK.map((t) => t.s).join(" ")}`);
        const eco = this.eco;
        const w = eco.wallet(c.name);
        const k = norm(c.name);
        const S = t.s;
        const p = this.s.p[S];
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
            eco.entry(c.name, `investiu ${n} ${S} a ${p.toFixed(2)}`, cost, WHY);
            this.pl.priv(c, "BOLSA", `comprou ${n} ${S} por ${cost} (taxa ${fee}). agora tens ${pos[S]}`);
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
        if (Object.keys(pos).length) this.s.pos[k] = pos;
        else delete this.s.pos[k];
        this.s.nm[k] = c.name;
        eco.sendMe(c.name);
        this.pl.save("bolsa", this.s);
        this.push();
    }
}
