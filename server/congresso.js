// Congresso da Vila: jogadores votam leis que mudam o jogo por alguns minutos (src/places/congresso.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("congresso", this.s)).
import { norm, txt } from "./places.js";
import { safe } from "./economy.js";

export const LAWS = [
    { id: "lua", n: "GRAVIDADE LUNAR", d: "pulo de astronauta", alias: ["lua", "gravidade", "lunar", "grav"] },
    { id: "turbo", n: "TURBO NACIONAL", d: "todo mundo 60% mais rapido", alias: ["turbo", "rapido", "velocidade"] },
    { id: "paz", n: "URNA PACIFISTA", d: "a urna gigante nao atira", alias: ["paz", "urna", "pacifista"] },
    { id: "festa", n: "FESTA OBRIGATORIA", d: "fogos no mapa todo", alias: ["festa", "fogos", "balada"] },
    { id: "promo", n: "LOJA EM PROMOCAO", d: "loja pela metade do preco", alias: ["promo", "promocao", "loja", "desconto"] },
];
export const ON_MS = 5 * 60 * 1000;
export const REC_MS = 10 * 60 * 1000;
const MAX_ON = 2;

const SYS = "Voce e o relator satirico do Congresso de uma vila de blocos num jogo. Escreva UMA frase curta (max 120 caracteres), " +
    "portugues sem acentos, tom de zoeira, justificando a lei aprovada. Sem politicos reais, sem partidos, sem ofensas. " +
    'Responda so JSON: {"frase":"..."}';
const FALLBACK = {
    lua: "aprovada porque a vila queria pular mais alto que a inflacao",
    turbo: "aprovada em regime de urgencia, igual todo mundo correndo pro almoco",
    paz: "a urna gigante vai fazer terapia e voltar mais calma",
    festa: "relator alegou que sem fogos ninguem aparece na sessao",
    promo: "a loja chorou, mas o povo votou e o desconto passou",
};

export const findLaw = (s) => {
    const k = norm(s);
    return k ? LAWS.find((l) => l.alias.some((a) => a === k || (k.length >= 3 && a.startsWith(k)))) : undefined;
};

export class Congresso {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
        s.votes ??= {};
        s.laws ??= {};
        s.last ??= "";
        s.passed ??= 0;
        this.lastQ = 0;
    }

    now() {
        return Date.now();
    }

    law(id) {
        return (this.s.laws[id] ??= { until: 0, rec: 0 });
    }

    active(now = this.now()) {
        return LAWS.filter((l) => this.law(l.id).until > now);
    }

    /// Quorum: metade (pra cima) de quem ta online, minimo 1.
    quorum(on = this.pl.online()) {
        return Math.max(1, Math.ceil(on.length / 2));
    }

    count(id, on = this.pl.online()) {
        const set = new Set(on);
        return Object.entries(this.s.votes).filter(([n, v]) => v === id && set.has(n)).length;
    }

    snap(now = this.now()) {
        const on = this.pl.online();
        const laws = LAWS.map((l) => {
            const st = this.law(l.id);
            const a = Math.max(0, (st.until - now) / 1000);
            const cd = a > 0 ? 0 : Math.max(0, (st.rec - now) / 1000);
            return { id: l.id, n: l.n, d: l.d, v: this.count(l.id, on), on: Math.round(a), cd: Math.round(cd) };
        });
        return { t: "pl", k: "lei", laws, q: this.quorum(on), last: this.s.last, passed: this.s.passed };
    }

    send(c) {
        if (c) this.pl.room.send(c, this.snap());
        else this.pl.room.broadcast(this.snap());
    }

    join(c) {
        this.send(c);
    }

    vote(c, s) {
        const l = findLaw(s);
        if (!l) return this.pl.priv(c, "CONGRESSO", "lei nao existe. /leis pra ver a pauta"), null;
        const who = norm(c.name);
        if (this.s.votes[who] === l.id) {
            this.pl.priv(c, "CONGRESSO", `voce ja votou em ${l.n}. voto de cabresto nao conta duas vezes`);
        } else {
            this.s.votes[who] = l.id;
            this.pl.priv(c, "CONGRESSO", `voto registrado: ${l.n} (${this.count(l.id)}/${this.quorum()})`);
        }
        this.check(l);
        this.save();
        this.send();
        return l;
    }

    /// Aprova se bateu quorum e as regras deixam.
    check(l, now = this.now()) {
        const st = this.law(l.id);
        const n = this.count(l.id);
        const q = this.quorum();
        if (n < q || st.until > now || st.rec > now || this.active(now).length >= MAX_ON) return false;
        st.until = now + ON_MS;
        st.rec = st.until + REC_MS;
        for (const k of Object.keys(this.s.votes)) if (this.s.votes[k] === l.id) delete this.s.votes[k];
        this.s.passed++;
        this.s.last = `${l.n}: ${FALLBACK[l.id]}`;
        this.pl.say("CONGRESSO", `LEI APROVADA: ${l.n} (${l.d}) por 5 min!`);
        this.pl.room.eco?.entry("CONGRESSO", `aprovou lei: ${l.n}`, 0, `${n} voto(s), quorum ${q}`);
        this.relator(l, this.s.passed);
        return true;
    }

    async relator(l, nr) {
        let f = "";
        try {
            const out = await this.pl.room.brain?.ask(SYS, `Lei aprovada: ${l.n} (${l.d}).`, 2, "small");
            f = txt(out?.frase, 200).trim();
        } catch (e) { }
        if (!f || f.length > 140 || !safe(f) || nr !== this.s.passed) return;
        this.s.last = `${l.n}: ${f}`.slice(0, 180);
        this.save();
        this.send();
    }

    list(c) {
        const now = this.now();
        const q = this.quorum();
        this.pl.priv(c, "CONGRESSO", `pauta (quorum ${q}, max ${MAX_ON} juntas, 5 min cada): /lei id ou pisa no pulpito`);
        for (const l of LAWS) {
            const st = this.law(l.id);
            const tag = st.until > now ? `EM VIGOR ${Math.ceil((st.until - now) / 60000)} min` : st.rec > now ? `RECESSO ${Math.ceil((st.rec - now) / 60000)} min` : `${this.count(l.id)}/${q} votos`;
            this.pl.priv(c, "CONGRESSO", `${l.id}: ${l.n} - ${l.d} [${tag}]`);
        }
    }

    command(id, c, head, args) {
        if (head === "leis" || (head === "lei" && !args.length)) return this.list(c), true;
        if (head !== "lei") return false;
        this.vote(c, args.join(" "));
        return true;
    }

    onMsg(id, c, m) {
        // worker sobrescreve m.id com o id da conexao: a lei vem em m.lei
        const lei = typeof m.lei === "string" ? m.lei : m.id;
        if (m.k !== "lei_voto" || typeof lei !== "string") return;
        const t = this.now();
        if (t - (c.leiAt || 0) < 1000) return;
        c.leiAt = t;
        this.vote(c, lei);
    }

    tick(now, online) {
        let changed = false;
        for (const l of LAWS) {
            const st = this.law(l.id);
            if (st.until && st.until <= now) {
                st.until = 0;
                changed = true;
                if (online) this.pl.say("CONGRESSO", `LEI EXPIROU: ${l.n}. recesso de 10 min`);
            }
            if (st.rec && st.rec <= now) {
                st.rec = 0;
                changed = true;
            }
        }
        if (!online) return changed && this.save();
        // Alguem saiu/entrou: quorum muda e pode aprovar lei parada
        for (const l of LAWS) changed = this.check(l, now) || changed;
        const q = this.quorum();
        const busy = LAWS.some((l) => this.law(l.id).until > now || this.law(l.id).rec > now);
        if (changed) this.save();
        if (changed || busy || q !== this.lastQ) this.send();
        this.lastQ = q;
    }

    priceFactor() {
        return this.law("promo").until > this.now() ? 0.5 : 1;
    }

    save() {
        this.pl.save("congresso", this.s);
    }
}
