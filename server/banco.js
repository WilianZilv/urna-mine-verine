// Banco Central da Vila: ranking, ledger, caixa eletronico e poupanca FICTICIA paga pelo cofre (src/places/banco.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("banco", this.s)).
// Poupanca: this.s.sv[nome normalizado] = {n, c, at}. Juros simples 2%/h, calculados na hora de qualquer operacao,
// pagos DO cofre da IA so acima da reserva (nunca cria moeda). Patrimonio = carteira + poupanca + acoes (Bolsa).
import { norm, amount } from "./places.js";

const RATE = 0.02; // por hora
const HOUR = 60 * 60 * 1000;
const CAP = 100; // juros maximo por operacao
const RESERVE = 300; // mesma reserva do economy.js
const ATM_MS = 3000;
const MAGNATA_MS = 60 * 1000;

export class Banco {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
        this.s.sv ??= {};
        this.last = "";
        this.atm = new Map();
        this.top = undefined; // ultimo #1 anunciado (normalizado); o primeiro visto nao e anunciado
        this.topAt = 0;
    }

    get eco() {
        return this.pl.room.eco;
    }

    /// Juros pendentes da conta `k` (sem aplicar): {due, pay}.
    pending(k) {
        const a = this.s.sv[k];
        if (!a) return { due: 0, pay: 0 };
        const due = Math.min(CAP, Math.max(0, Math.floor((a.c * RATE * (Date.now() - a.at)) / HOUR)));
        return { due, pay: Math.max(0, Math.min(due, this.eco.s.tr - RESERVE)) };
    }

    /// Aplica os juros pendentes (cofre -> poupanca). Retorna aviso pro jogador ("" se nada).
    accrue(k) {
        const a = this.s.sv[k];
        const { due, pay } = this.pending(k);
        if (!a || due <= 0) return "";
        a.at = Date.now();
        if (pay > 0) {
            this.eco.s.tr -= pay;
            a.c += pay;
            this.eco.entry(a.n, "juros da poupanca", pay, pay < due ? `2%/h pago pelo cofre (devia ${due}, cofre bateu na reserva)` : "2%/h pago pelo cofre da IA");
        }
        if (pay >= due) return `juros +${pay}`;
        return pay > 0 ? `juros +${pay} de ${due} (cofre na reserva, a IA ta liso)` : `juros ${due} NAO pagos: cofre na reserva (${RESERVE}). /doar ajuda`;
    }

    stocks(k) {
        return Math.max(0, Math.round(Number(this.pl.bolsa?.value?.(k)) || 0));
    }

    /// Todos com patrimonio, do mais rico pro mais liso: [{k, n, w, sv, ac, t}].
    rich() {
        return Object.entries(this.eco.s.w)
            .map(([k, w]) => {
                const sv = this.s.sv[k]?.c || 0, ac = this.stocks(k);
                return { k, n: w.n, w: w.c, sv, ac, t: w.c + sv + ac };
            })
            .sort((a, b) => b.t - a.t || a.n.localeCompare(b.n));
    }

    snap() {
        const sv = Object.values(this.s.sv);
        return {
            t: "pl",
            k: "banco",
            tr: this.eco.s.tr,
            rich: this.rich().slice(0, 8).map((r) => [r.n, r.t]),
            led: this.eco.s.led.slice(-6).map((e) => [e.who, e.what, e.amt]),
            sv: sv.reduce((s, a) => s + a.c, 0),
            nsv: sv.length,
            rate: RATE * 100,
        };
    }

    /// Manda pra todo mundo se mudou (ou forca).
    push(force = false) {
        const m = this.snap();
        this.magnata(m.rich[0]?.[0]);
        const h = JSON.stringify(m);
        if (!force && h === this.last) return;
        this.last = h;
        this.pl.room.broadcast(m);
    }

    /// Anuncia o novo #1 (no maximo 1 vez por minuto; troca no meio do cooldown sai depois).
    magnata(name) {
        if (!name) return;
        const k = norm(name);
        if (this.top === undefined) return void (this.top = k);
        const now = Date.now();
        if (k === this.top || now - this.topAt < MAGNATA_MS) return;
        this.top = k;
        this.topAt = now;
        this.pl.say("BANCO", `NOVO MAGNATA DA VILA: ${name}`);
    }

    join(c) {
        this.pl.room.send(c, this.snap());
    }

    tick(now, online) {
        if (online) this.push();
    }

    done(c, msg) {
        this.pl.priv(c, "BANCO", msg);
        this.eco.sendMe(c.name);
        this.pl.save("banco", this.s);
        this.push();
    }

    command(id, c, head, args) {
        if (!["poupar", "sacar", "ranking"].includes(head) || !c.name) return false;
        const k = norm(c.name);
        const w = this.eco.wallet(c.name);
        if (head === "ranking") {
            const all = this.rich();
            const me = all.findIndex((r) => r.k === k);
            this.pl.priv(c, "BANCO", "MAIS RICOS: " + all.slice(0, 5).map((r, i) => `${i + 1}. ${r.n} ${r.t}`).join(" | ") + (me >= 0 ? ` || tu: #${me + 1}` : ""));
            return true;
        }
        const tudo = norm(args[0]) === "tudo";
        const note = this.accrue(k);
        const a = this.s.sv[k];
        if (head === "poupar") {
            const n = tudo ? w.c : amount(args[0]);
            if (!(n >= 1) || n > w.c) return this.done(c, `uso: /poupar n ou /poupar tudo (carteira ${w.c}, poupanca ${a?.c || 0})${note ? " | " + note : ""}`), true;
            w.c -= n;
            if (a) a.c += n;
            else this.s.sv[k] = { n: c.name, c: n, at: Date.now() };
            this.eco.entry(c.name, "guardou na poupanca", n, "carteira -> poupanca, rende 2%/h pago pelo cofre");
            this.done(c, `guardou ${n}. poupanca ${this.s.sv[k].c}, carteira ${w.c}${note ? " | " + note : ""} | rende 2%/h ate a IA falir`);
            return true;
        }
        const have = a?.c || 0;
        const n = tudo ? have : amount(args[0]);
        if (!(n >= 1) || n > have) return this.done(c, `uso: /sacar n ou /sacar tudo (poupanca ${have})${note ? " | " + note : ""}`), true;
        a.c -= n;
        w.c += n;
        if (a.c <= 0) delete this.s.sv[k];
        this.eco.entry(c.name, "sacou da poupanca", n, "poupanca -> carteira");
        this.done(c, `sacou ${n}. carteira ${w.c}, poupanca ${a.c}${note ? " | " + note : ""}`);
        return true;
    }

    onMsg(id, c, m) {
        if (m.k !== "banco_atm" || !c.name) return;
        const k = norm(c.name);
        const now = Date.now();
        if (now - (this.atm.get(k) || 0) < ATM_MS) return;
        this.atm.set(k, now);
        const w = this.eco.wallet(c.name);
        const { due, pay } = this.pending(k);
        const sv = this.s.sv[k]?.c || 0;
        const ac = this.stocks(k);
        const all = this.rich();
        const pos = all.findIndex((r) => r.k === k) + 1;
        const juros = due ? ` (+${pay} de juros${pay < due ? `, cofre so paga ${pay} de ${due}` : ""} no proximo movimento)` : "";
        this.pl.priv(c, "BANCO", `CAIXA ELETRONICO | ${c.name}: carteira ${w.c} | poupanca ${sv}${juros} | acoes ${ac} | patrimonio ${w.c + sv + ac + pay} | ranking #${pos} de ${all.length} | cofre ${this.eco.s.tr}`);
    }
}
