// RINGUE DA VILA: boxe PvP entre jogadores (src/places/ringue.rs). Interface: ver server/places.js.
// O servidor manda em tudo: fila, contagem, rounds, vida do ringue, nocaute, W.O. e premio.
//   pisar na lona (posicao do "p") ou /ringue  -> fila (2 lutam, o resto espera; quem vence fica)
//   /ringue sair                               -> sai da fila (lutando = desiste da luta)
//   {t:"pl",k:"ringue",a:"soco"}               jab de quem joga com personagem sem PvP proprio (validado por distancia)
//   {t:"pl",k:"ringue",ph,round,left,f:[[id,nome,hp,rounds],..],q,king:[nome,seq],top:[[nome,vitorias,melhor seq]],last:[[venc,perd,placar]]}
//     snapshot (broadcast quando muda; left = ms ate o fim da fase)
//   privado: {a:"tp",p,heal} corner/fora do ringue; {a:"msg",m} faixa so dos 2 lutadores; {a:"hit",d} empurrao do jab
// Vida do ringue: 100 por round; dano = "pv" do "p" (Steve/Wolverine) x6 ou jab 10. Melhor de 3, round 60 s.
import { G, RINGUE } from "./layout.js";
import { day } from "./escola.js";
import { norm } from "./places.js";

export const TICK_MS = 250;
export const HP = 100;
export const ROUND_MS = 60 * 1000;
export const COUNT_MS = 3000;
export const BREAK_MS = 4000;
export const END_MS = 4000;
/// Fora das cordas por mais que isso = perde o round.
export const OUT_MS = 3000;
const MAX_ROUNDS = 5;
const PV_K = 6;
const PV_MAX = 12;
export const JAB = 10;
const JAB_MS = 400;
const JAB_R = 3.2;
/// Personagens com PvP proprio no "pv" (Steve, Wolverine): esses nao usam o jab.
const NATIVE = [0, 7];
const QMAX = 12;
/// Rei longe disso do centro (ou offline) perde a coroa.
const KING_R = 40;
export const PRIZE = 15;
const PRIZE_DAY = 10;
const RESERVE = 300;

const [LX0, LZ0, LX1, LZ1] = RINGUE.lona;
const CX = (LX0 + LX1 + 1) / 2, CZ = (LZ0 + LZ1 + 1) / 2;
const FLOOR = G + 2;
export const CORNERS = [[LX0 + 1.5, FLOOR, LZ0 + 1.5], [LX1 - 0.5, FLOOR, LZ1 - 0.5]];
export const OUT = [CX, G, LZ1 + 5.5];

export const inLona = (p) => Array.isArray(p) && p[0] >= LX0 && p[0] < LX1 + 1 && p[2] >= LZ0 && p[2] < LZ1 + 1 && p[1] >= FLOOR - 0.5;

export class Ring {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
        this.s.top ??= {}; // nome normalizado -> {n, w: lutas ganhas, b: melhor sequencia}
        this.s.last ??= []; // [vencedor, perdedor, placar], mais novo primeiro
        this.s.paid ??= {}; // nome normalizado -> [dia, premios pagos]
        this.ph = "idle";
        this.f = [];
        this.q = [];
        this.round = 0;
        this.until = 0;
        this.king = null;
        this.cd = 0;
        this.last = "";
        this.iv = null;
    }

    get room() {
        return this.pl.room;
    }

    pos(id) {
        return this.room.clients.get(id)?.pos;
    }

    online(id) {
        return !!this.room.clients.get(id)?.name;
    }

    send(id, m) {
        const c = this.room.clients.get(id);
        if (c) this.room.send(c, { t: "pl", k: "ringue", ...m });
    }

    msg(m) {
        for (const x of this.f) this.send(x.id, { a: "msg", m });
    }

    tp(id, p, heal = false) {
        this.send(id, { a: "tp", p, heal: heal ? 1 : 0 });
    }

    idx(id) {
        return this.f.findIndex((x) => x.id === id);
    }

    /// Posicao na fila (1..), 0 = lutando/rei, -1 = fila cheia.
    enqueue(id, name) {
        if (this.idx(id) >= 0 || this.king?.id === id) return 0;
        const i = this.q.findIndex((x) => x.id === id);
        if (i >= 0) return i + 1;
        if (this.q.length >= QMAX) return -1;
        this.q.push({ id, name: String(name).slice(0, 16) });
        return this.q.length;
    }

    start(now) {
        this.q = this.q.filter((x) => this.online(x.id));
        if (this.king && (!this.online(this.king.id) || !this.near(this.king.id))) this.king = null;
        const a = this.king ? { id: this.king.id, name: this.king.name } : this.q.shift();
        const b = this.q.shift();
        if (!a || !b) {
            if (a && !this.king) this.q.unshift(a);
            return false;
        }
        const mk = (x) => ({ id: x.id, name: x.name, hp: HP, rw: 0, dmg: 0, out: 0, jab: 0 });
        this.f = [mk(a), mk(b)];
        this.round = 1;
        this.begin(now);
        return true;
    }

    near(id) {
        const p = this.pos(id);
        return !!p && Math.hypot(p[0] - CX, p[2] - CZ) < KING_R;
    }

    begin(now) {
        this.f.forEach((x, i) => {
            Object.assign(x, { hp: HP, dmg: 0, out: 0 });
            this.tp(x.id, CORNERS[i], true);
        });
        this.ph = "count";
        this.until = now + COUNT_MS;
        this.cd = Math.ceil(COUNT_MS / 1000);
        this.msg(`ROUND ${this.round}  -  ${this.cd}`);
    }

    hit(ai, d, now) {
        const a = this.f[ai], b = this.f[1 - ai];
        d = Math.min(d, b.hp);
        if (d <= 0) return;
        b.hp -= d;
        a.dmg += d;
        if (b.hp <= 0) this.endRound(now, ai, "NOCAUTE");
    }

    endRound(now, wi, why) {
        if (wi !== null) this.f[wi].rw++;
        this.msg(wi === null ? `ROUND ${this.round} EMPATADO` : `ROUND ${this.round}: ${this.f[wi].name.toUpperCase()} (${why})`);
        const [a, b] = this.f;
        if (a.rw >= 2 || b.rw >= 2 || this.round >= MAX_ROUNDS) return this.matchOver(now, b.rw > a.rw ? 1 : 0, why);
        this.ph = "break";
        this.until = now + BREAK_MS;
    }

    matchOver(now, wi, why) {
        const w = this.f[wi], l = this.f[1 - wi];
        const streak = this.king?.id === w.id ? this.king.streak + 1 : 1;
        this.king = { id: w.id, name: w.name, streak };
        const k = norm(w.name);
        const t = (this.s.top[k] ??= { n: w.name, w: 0, b: 0 });
        t.n = w.name;
        t.w++;
        t.b = Math.max(t.b, streak);
        const score = `${w.rw}-${l.rw}`;
        this.s.last.unshift([w.name, l.name, score]);
        this.s.last.length = Math.min(this.s.last.length, 5);
        const keys = Object.keys(this.s.top);
        if (keys.length > 500) for (const x of keys.sort((p, q) => this.s.top[p].w - this.s.top[q].w).slice(0, keys.length - 500)) delete this.s.top[x];
        this.pl.say("RINGUE", `${w.name} venceu ${l.name} por ${score} (${why}). sequencia: ${streak}`);
        this.prize(w.name, now);
        this.pl.save("ringue", this.s);
        this.msg(`${w.name.toUpperCase()} VENCEU! ${score}`);
        this.ph = "end";
        this.until = now + END_MS;
    }

    /// Premio pequeno em moeda FICTICIA do cofre, ate PRIZE_DAY por jogador por dia.
    prize(name, now) {
        const eco = this.room.eco;
        if (!eco?.s || eco.s.tr - PRIZE < RESERVE) return;
        const k = norm(name), d = day(now);
        const p = this.s.paid[k]?.[0] === d ? this.s.paid[k] : [d, 0];
        if (p[1] >= PRIZE_DAY) return;
        this.s.paid[k] = [d, p[1] + 1];
        for (const x of Object.keys(this.s.paid)) if (this.s.paid[x][0] !== d) delete this.s.paid[x];
        eco.s.tr -= PRIZE;
        eco.wallet(name).c += PRIZE;
        eco.entry(name, "venceu uma luta no RINGUE", PRIZE, `premio do ringue pago pelo cofre (ate ${PRIZE_DAY} por dia)`);
        eco.sendMe(name);
    }

    /// Desistencia/saida no meio da luta: o outro leva a luta.
    forfeit(id, now) {
        const i = this.idx(id);
        if (i < 0 || this.ph === "idle" || this.ph === "end") return;
        const o = this.f[1 - i];
        o.rw = Math.max(o.rw, 2);
        this.matchOver(now, 1 - i, "W.O.");
    }

    step(now) {
        if (!this.room.clients.size) {
            if (this.iv) clearInterval(this.iv);
            this.iv = null;
            Object.assign(this, { ph: "idle", f: [], q: [], king: null });
            return;
        }
        for (const [id, c] of this.room.clients) {
            if (!c.name || !inLona(c.pos) || this.idx(id) >= 0 || this.king?.id === id) continue;
            const n = this.enqueue(id, c.name);
            if (this.ph !== "idle") {
                this.tp(id, OUT);
                this.pl.priv(c, "RINGUE", n > 0 ? `luta rolando. tu ta na fila: #${n}` : "luta rolando e fila cheia. espera");
            }
        }
        this.q = this.q.filter((x) => this.online(x.id));
        if (this.ph === "idle") {
            if (this.king && !this.online(this.king.id)) this.king = null;
            if (this.q.length >= (this.king ? 1 : 2)) this.start(now);
        } else if (this.ph === "count") {
            const n = Math.ceil((this.until - now) / 1000);
            if (now >= this.until) {
                this.ph = "fight";
                this.until = now + ROUND_MS;
                this.msg(`ROUND ${this.round}: LUTA!`);
            } else if (n !== this.cd) {
                this.cd = n;
                this.msg(`ROUND ${this.round}  -  ${n}`);
            }
        } else if (this.ph === "fight") {
            for (let i = 0; i < 2 && this.ph === "fight"; i++) {
                const x = this.f[i];
                if (inLona(this.pos(x.id))) x.out = 0;
                else if (!x.out) x.out = now;
                else if (now - x.out >= OUT_MS) this.endRound(now, 1 - i, "SAIU DO RINGUE");
            }
            if (this.ph === "fight" && now >= this.until) {
                const [a, b] = this.f;
                this.endRound(now, a.hp === b.hp ? null : a.hp > b.hp ? 0 : 1, "PONTOS");
            }
        } else if (this.ph === "break") {
            if (now >= this.until) {
                this.round++;
                this.begin(now);
            }
        } else if (this.ph === "end" && now >= this.until) {
            const loser = this.f.find((x) => x.id !== this.king?.id);
            if (loser && this.online(loser.id)) this.tp(loser.id, OUT);
            this.f = [];
            this.ph = "idle";
            if (this.q.length) this.start(now);
        }
        this.push(now);
    }

    snap(now = Date.now()) {
        const top = Object.values(this.s.top).sort((a, b) => b.b - a.b || b.w - a.w).slice(0, 5).map((t) => [t.n, t.w, t.b]);
        return {
            t: "pl",
            k: "ringue",
            ph: this.ph,
            round: this.round,
            left: this.ph === "idle" ? 0 : Math.max(0, this.until - now),
            f: this.f.map((x) => [x.id, x.name, Math.round(x.hp), x.rw]),
            q: this.q.map((x) => x.name),
            king: this.king ? [this.king.name, this.king.streak] : null,
            top,
            last: this.s.last,
        };
    }

    push(now = Date.now()) {
        const m = this.snap(now);
        const h = JSON.stringify({ ...m, left: this.until });
        if (h === this.last) return;
        this.last = h;
        this.room.broadcast(m);
    }

    arm() {
        if (!this.iv) this.iv = setInterval(() => this.step(Date.now()), TICK_MS);
    }

    join(c) {
        this.room.send(c, this.snap());
        this.arm();
    }

    onPos(id, c, m, now = Date.now()) {
        c.ch = Number(m.c) || 0;
        const ai = this.idx(id);
        if (this.ph !== "fight" || ai < 0 || !Array.isArray(m.pv) || !inLona(c.pos)) return;
        const bid = this.f[1 - ai].id;
        let d = 0;
        for (const e of m.pv) if (Array.isArray(e) && e[0] === bid) d += Math.max(0, Number(e[1]) || 0);
        if (d > 0) this.hit(ai, Math.min(d, PV_MAX) * PV_K, now);
    }

    onMsg(id, c, m, now = Date.now()) {
        if (m.k !== "ringue" || m.a !== "soco") return;
        const ai = this.idx(id);
        if (this.ph !== "fight" || ai < 0 || NATIVE.includes(c.ch ?? 0)) return;
        const a = this.f[ai], b = this.f[1 - ai];
        const pa = c.pos, pb = this.pos(b.id);
        if (now - a.jab < JAB_MS || !inLona(pa) || !inLona(pb)) return;
        const dx = pb[0] - pa[0], dz = pb[2] - pa[2], r = Math.hypot(dx, dz);
        if (r > JAB_R) return;
        a.jab = now;
        this.send(b.id, { a: "hit", d: [dx / (r || 1), dz / (r || 1)] });
        this.hit(ai, JAB, now);
    }

    leave(id, now = Date.now()) {
        this.q = this.q.filter((x) => x.id !== id);
        this.forfeit(id, now);
        if (this.king?.id === id) this.king = null;
    }

    command(id, c, head, args, now = Date.now()) {
        if (!["ringue", "ring", "boxe"].includes(head)) return false;
        if (norm(args[0]) === "sair") {
            const fighting = this.idx(id) >= 0 && this.ph !== "idle" && this.ph !== "end";
            this.leave(id, now);
            this.pl.priv(c, "RINGUE", fighting ? "tu desistiu da luta (W.O.)" : "saiu da fila do ringue");
            this.push(now);
            return true;
        }
        const n = this.enqueue(id, c.name);
        const where = `RINGUE DA VILA: noroeste, trilha da Avenida dos Poderes (x ${RINGUE.path[0]}) pro norte`;
        this.pl.priv(c, "RINGUE", n === 0 ? "tu ja ta no ringue" : n < 0 ? "fila cheia, tenta ja ja" : `fila #${n}. ${where}. melhor de 3, round 60s, nocaute ou pontos; sair das cordas 3s = perde o round. /ringue sair`);
        this.arm();
        this.push(now);
        return true;
    }
}
