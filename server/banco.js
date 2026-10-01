// Banco Central da Vila: ranking, ledger, caixa eletronico e poupanca FICTICIA paga pelo cofre (src/places/banco.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("banco", this.s)).
// Poupanca: this.s.sv[nome normalizado] = {n, c, at}. Juros simples 2%/h, calculados na hora de qualquer operacao,
// pagos DO cofre da IA so acima da reserva (nunca cria moeda). Patrimonio = carteira + poupanca + acoes (Bolsa).
// Ranking de criadores (/criadores): IMPACTO = o que cada criador trouxe pra vila (mods, portais, visitas), nunca moeda.
import { norm, amount } from "./places.js";

const RATE = 0.02; // por hora
const HOUR = 60 * 60 * 1000;
const CAP = 100; // juros maximo por operacao
const RESERVE = 300; // mesma reserva do economy.js
const ATM_MS = 3000;
const MAGNATA_MS = 60 * 1000;
const HOF_MS = 60 * 1000;
// IMPACTO de criador (nunca dinheiro): mods ativos e portais no ar contam ao vivo; visitas, presenca e golpes acumulam.
const W = { mod: 40, portal: 25, visit: 10, pres: 1, hits: 5 }; // hits: 1 ponto a cada 5 golpes
const MAX_CR = 100;
const VISIT_MS = HOUR; // mesma pessoa no mesmo portal conta 1 visita por hora
const HIT_MS = 2000;
const MODS = 8; // npc::MODS no cliente: i = indice achatado das instancias dos mods npc em ordem de ativacao

export class Banco {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
        this.s.sv ??= {};
        this.s.cr ??= {}; // criador normalizado -> {n, e: visitas, p: amostras de presenca, h: golpes em mods}
        this.s.cri ??= []; // top 10 [nome, impacto]
        this.last = "";
        this.atm = new Map();
        this.seen = new Map(); // "jogador|portal" -> ultima visita contada
        this.hitAt = new Map();
        this.dirty = false;
        // Top 3 ja anunciado no HALL DA FAMA; sem ranking salvo, o primeiro top visto nao e anunciado
        this.hof = this.s.cri.length ? this.s.cri.slice(0, 3).map(([n]) => norm(n)) : undefined;
        this.hofAt = 0;
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

    /// Portais no ar do Hub (mesma regra do hub.js: versao ativa com origem verificada).
    portals() {
        return Object.values(this.pl.room.hub?.s?.p || {}).filter((p) => {
            const a = p.versions?.find((x) => x.v === p.active);
            return a && p.verified?.origin === a.origin;
        });
    }

    stat(name) {
        const c = (this.s.cr[norm(name)] ??= { n: name, e: 0, p: 0, h: 0 });
        c.n = name;
        this.dirty = true;
        return c;
    }

    /// Criadores do mais impactante pro menos (so quem tem impacto > 0): [{k, n, mods, portals, e, p, h, score}].
    creators() {
        const live = {};
        const get = (name) => (live[norm(name)] ??= { n: name, mods: 0, portals: 0 });
        for (const a of this.pl.room.mods?.active?.values() || []) if (a.creator) get(a.creator).mods++;
        for (const p of this.portals()) if (p.author) get(p.author).portals++;
        return [...new Set([...Object.keys(this.s.cr), ...Object.keys(live)])]
            .map((k) => {
                const c = this.s.cr[k] || { e: 0, p: 0, h: 0 }, l = live[k] || { mods: 0, portals: 0 };
                const score = l.mods * W.mod + l.portals * W.portal + c.e * W.visit + c.p * W.pres + Math.floor(c.h / W.hits);
                return { k, n: l.n || c.n, mods: l.mods, portals: l.portals, e: c.e, p: c.p, h: c.h, score };
            })
            .filter((r) => r.score > 0)
            .sort((a, b) => b.score - a.score || a.n.localeCompare(b.n));
    }

    /// Visitas novas (sessoes do Hub, 1/h por jogador+portal) e presenca ('n' do Hub) em cada portal no ar.
    /// O proprio criador entrando/ficando no portal dele nao conta.
    sample(now) {
        const hub = this.pl.room.hub;
        if (!hub?.ses) return;
        const by = Object.fromEntries(this.portals().filter((p) => p.author).map((p) => [p.id, p]));
        const self = new Set();
        for (const s of hub.ses.values()) {
            const p = by[s.pid];
            if (!p || s.exp < now) continue;
            if (norm(s.name) === norm(p.author)) {
                if (s.ready) self.add(p.id);
                continue;
            }
            const key = `${norm(s.name)}|${p.id}`;
            if (now - (this.seen.get(key) || 0) < VISIT_MS) continue;
            this.seen.set(key, now);
            this.stat(p.author).e++;
        }
        if (this.seen.size > 5000) for (const [k, t] of this.seen) if (now - t >= VISIT_MS) this.seen.delete(k);
        for (const p of Object.values(by)) {
            const n = Math.max(0, (Number(hub.count?.(p.id)) || 0) - (self.has(p.id) ? 1 : 0));
            if (n > 0) this.stat(p.author).p += n;
        }
    }

    /// Mantem no maximo MAX_CR criadores guardados (sai quem tem menos impacto).
    prune() {
        const ks = Object.keys(this.s.cr);
        if (ks.length <= MAX_CR) return;
        const rank = new Map(this.creators().map((r, i) => [r.k, i]));
        ks.sort((a, b) => (rank.get(a) ?? 1e9) - (rank.get(b) ?? 1e9));
        for (const k of ks.slice(MAX_CR)) delete this.s.cr[k];
    }

    /// Mod npc dono da instancia `i` do grupo MODS (cada mod tem max_instances instancias, 1..4).
    modAt(i) {
        if (!(i >= 0)) return null;
        for (const a of this.pl.room.mods?.list?.("npc") || []) {
            const n = a.pkg?.behavior?.spawn?.max_instances;
            i -= Number.isInteger(n) ? Math.min(4, Math.max(1, n)) : 1;
            if (i < 0) return a;
        }
        return null;
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
            cri: this.creators().slice(0, 8).map((r) => [r.n, r.score, r.mods, r.portals, r.e]),
        };
    }

    /// Manda pra todo mundo se mudou (ou forca).
    push(force = false) {
        const m = this.snap();
        this.magnata(m.rich[0]?.[0]);
        this.hall(m.cri.slice(0, 3).map((r) => r[0]));
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

    /// Anuncia quem entrou no top 3 de criadores (1 fala por minuto; quem entrar no cooldown sai depois).
    hall(names) {
        const ks = names.map(norm);
        if (this.hof === undefined) return void (this.hof = ks);
        const fresh = names.filter((n, i) => !this.hof.includes(ks[i]));
        if (!fresh.length) return void (this.hof = ks);
        const now = Date.now();
        if (now - this.hofAt < HOF_MS) return;
        this.hof = ks;
        this.hofAt = now;
        this.pl.say("BANCO", `${fresh.join(" e ")} ${fresh.length > 1 ? "entraram" : "entrou"} pro HALL DA FAMA dos criadores`);
    }

    join(c) {
        this.pl.room.send(c, this.snap());
    }

    tick(now, online) {
        if (!online) return;
        this.sample(now);
        this.prune();
        const top = this.creators().slice(0, 10).map((r) => [r.n, r.score]);
        if (JSON.stringify(top) !== JSON.stringify(this.s.cri)) {
            this.s.cri = top;
            this.dirty = true;
        }
        if (this.dirty) {
            this.dirty = false;
            this.pl.save("banco", this.s);
        }
        this.push();
    }

    onHit(c, m) {
        if (!c.name || Number(m.g) !== MODS || !["pf", "hit", "npc"].includes(m.k)) return;
        const a = this.modAt(Number(m.i));
        const k = norm(c.name);
        if (!a?.creator || norm(a.creator) === k) return;
        const now = Date.now();
        if (now - (this.hitAt.get(k) || 0) < HIT_MS) return;
        this.hitAt.set(k, now);
        if (this.hitAt.size > 2000) this.hitAt.clear();
        this.stat(a.creator).h++;
    }

    done(c, msg) {
        this.pl.priv(c, "BANCO", msg);
        this.eco.sendMe(c.name);
        this.pl.save("banco", this.s);
        this.push();
    }

    command(id, c, head, args) {
        if (!["poupar", "sacar", "ranking", "criadores"].includes(head) || !c.name) return false;
        const k = norm(c.name);
        if (head === "criadores") {
            const all = this.creators();
            const me = all.findIndex((r) => r.k === k);
            this.pl.priv(c, "BANCO", all.length ? "CRIADORES (impacto, nao dinheiro; m=mods p=portais v=visitas): " + all.slice(0, 5).map((r, i) => `${i + 1}. ${r.n} ${r.score} [${r.mods}m ${r.portals}p ${r.e}v]`).join(" | ") + (me >= 0 ? ` || tu: #${me + 1}` : "") : "CRIADORES: ninguem trouxe nada pra vila ainda. sobe um mod ou portal e vira lenda (/modding.txt)");
            return true;
        }
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
