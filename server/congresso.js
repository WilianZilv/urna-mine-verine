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

const DAY = 24 * 3600 * 1000;
const HALL = 8;
/// Semana ISO (UTC) de `t`: "2026-W40".
export const isoWeek = (t) => {
    const d = new Date(t);
    const th = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() - ((d.getUTCDay() + 6) % 7) + 3);
    const y = new Date(th).getUTCFullYear();
    return `${y}-W${String(1 + Math.floor((th - Date.UTC(y, 0, 1)) / (7 * DAY))).padStart(2, "0")}`;
};
/// Proxima segunda 00:00 UTC depois de `t`.
export const weekEnd = (t) => {
    const d = new Date(t);
    return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() - ((d.getUTCDay() + 6) % 7) + 7);
};
/// Mes UTC de `t`: "2026-10".
export const monthKey = (t) => new Date(t).toISOString().slice(0, 7);
/// Dia 1 do mes seguinte, 00:00 UTC.
export const monthEnd = (t) => {
    const d = new Date(t);
    return Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 1);
};

// O ROADMAP VIRA LEI: itens abertos de docs/ROADMAP.md que a vila prioriza (ids estaveis: o voto guarda o id).
export const ROADMAP = [
    { id: "perf", t: "PERF MOBILE 2", d: "vila lisinha ate no celular da tia" },
    { id: "eleicao", t: "ELEICAO SIMULADA", d: "a vila inteira vota na urna, apuracao ao vivo na TV" },
    { id: "shards", t: "SHARDS: VARIAS VILAS", d: "varias vilas com o mesmo mapa e voo entre elas" },
    { id: "indie", t: "PARCERIAS COM DEVS INDIE", d: "jogo indie vira portao fixo no Terminal" },
    { id: "partidos", t: "PARTIDOS DA VILA", d: "times de jogadores com bandeira e sede" },
    { id: "irma", t: "CIDADE-IRMA EM INGLES", d: "cada pais faz a sua urna" },
    { id: "agentes", t: "AGENTES RESIDENTES", d: "IAs de terceiros morando na vila" },
    { id: "mobile", t: "CONTROLES MOBILE 2", d: "joystick melhor e atalho pros lugares" },
];
const RM_HIST = 12;
const MOD_KINDS = ["npc", "avatar", "item"];

const asc = (s, n) => txt(String(s ?? "").normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/[^\x20-\x7e]/g, ""), n).trim();

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
        s.mc ??= { week: isoWeek(this.now()), votes: {} };
        s.mds ??= { week: "", winner: null, at: 0, hist: [] };
        s.rmv ??= {};
        s.rm ??= { items: [], month: monthKey(this.now()), winners: [] };
        // Contador de votos (leis + roadmap) desde o boot: o cliente anima o plenario pela diferenca entre snapshots
        this.ev = { law: Object.fromEntries(LAWS.map((l) => [l.id, 0])), rm: 0 };
        this.lastQ = 0;
        this.lastMc = "";
        this.tally();
        this.lastRm = this.rmKey();
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
            return { id: l.id, n: l.n, d: l.d, v: this.count(l.id, on), on: Math.round(a), cd: Math.round(cd), vc: this.ev.law[l.id] };
        });
        return { t: "pl", k: "lei", laws, q: this.quorum(on), last: this.s.last, passed: this.s.passed, mds: this.mdsSnap(now), rm: this.rmSnap(now) };
    }

    // ------------------------------------------------ O ROADMAP VIRA LEI
    /// Nomes (normalizados) de quem tem mod ativo: conselho de criadores, voto vale 2.
    creators() {
        const mods = this.pl.room.mods;
        return new Set(MOD_KINDS.flatMap((k) => mods?.list?.(k) || []).map((a) => norm(a.creator)).filter(Boolean));
    }

    /// Recalcula this.s.rm.items (estado publico: outros modulos leem daqui).
    tally(cr = this.creators()) {
        const v = {};
        for (const [who, id] of Object.entries(this.s.rmv)) v[id] = (v[id] || 0) + (cr.has(who) ? 2 : 1);
        this.s.rm.items = ROADMAP.map((r) => ({ id: r.id, t: r.t, v: v[r.id] || 0 }));
        return this.s.rm.items;
    }

    /// Do mais votado pro menos (empate: ordem da lista).
    rmRank() {
        return this.s.rm.items.map((x, i) => ({ ...x, i: i + 1 })).sort((a, b) => b.v - a.v || a.i - b.i);
    }

    rmKey() {
        return JSON.stringify([this.s.rm.month, this.s.rm.items.map((x) => x.v)]);
    }

    /// Virada de mes: o mais votado entra pro historico e os votos zeram. true = virou.
    rmRoll(now = this.now()) {
        const mk = monthKey(now);
        const rm = this.s.rm;
        if (rm.month === mk) return false;
        this.tally();
        const top = this.rmRank()[0];
        if (top?.v > 0) {
            rm.winners = [{ month: rm.month, id: top.id, t: top.t, v: top.v }, ...rm.winners].slice(0, RM_HIST);
            this.pl.say("CONGRESSO", `O ROADMAP VIRA LEI: ${top.t} foi o mais votado de ${rm.month} (${top.v} voto(s)) e vira prioridade dos devs`);
        }
        this.s.rmv = {};
        rm.month = mk;
        this.tally();
        this.save();
        return true;
    }

    rmSnap(now = this.now()) {
        const rm = this.s.rm;
        return { items: rm.items, month: rm.month, left: Math.max(0, Math.round((monthEnd(now) - now) / 1000)), winners: rm.winners.slice(0, 4), vc: this.ev.rm };
    }

    findItem(s) {
        const k = norm(s);
        if (!k) return undefined;
        if (/^\d{1,2}$/.test(k)) return ROADMAP[parseInt(k, 10) - 1];
        return ROADMAP.find((r) => r.id === k || norm(r.t) === k) || (k.length >= 3 ? ROADMAP.find((r) => r.id.startsWith(k) || norm(r.t).startsWith(k)) : undefined);
    }

    roadmap(c) {
        this.rmRoll();
        const cr = this.creators();
        const it = this.tally(cr);
        const who = norm(c.name);
        const w = this.s.rm.winners[0];
        if (w) this.pl.priv(c, "ROADMAP", `prioridade de ${w.month}: ${w.t} (${w.v} voto(s))`);
        this.pl.priv(c, "ROADMAP", `O ROADMAP VIRA LEI (${this.s.rm.month}): /proposta numero ou id. 1 voto, pode trocar, zera todo mes${cr.has(who) ? ". voce e do conselho de criadores: seu voto vale 2" : ". criador de mod ativo vale 2"}`);
        const mine = this.s.rmv[who];
        ROADMAP.forEach((r, i) => this.pl.priv(c, "ROADMAP", `${i + 1}: ${r.t} - ${r.d} - ${it[i].v} voto(s)${r.id === mine ? " [SEU VOTO]" : ""}`));
    }

    propose(c, s) {
        this.rmRoll();
        const r = this.findItem(s);
        if (!r) return this.pl.priv(c, "ROADMAP", "proposta nao existe. /roadmap pra ver a lista numerada"), null;
        const who = norm(c.name);
        const old = this.s.rmv[who];
        if (old === r.id) return this.pl.priv(c, "ROADMAP", `voce ja vota em ${r.t}. os devs ja anotaram (talvez)`), r;
        this.s.rmv[who] = r.id;
        const cr = this.creators();
        const v = this.tally(cr).find((x) => x.id === r.id).v;
        this.ev.rm++;
        this.lastRm = this.rmKey();
        this.pl.priv(c, "ROADMAP", `${old ? "voto trocado" : "voto registrado"}: ${r.t} (${v} voto(s))${cr.has(who) ? " - voto de criador vale 2" : ""}`);
        this.save();
        this.send();
        return r;
    }

    // ------------------------------------------------ CONCURSO SEMANAL DE MODS
    /// Mods npc ativos, em ordem de ativacao (o numero do /concurso e a posicao + 1).
    cands() {
        return (this.pl.room.mods?.list?.("npc") || []).map((a) => ({ id: a.id, n: asc(a.pkg?.manifest?.name || a.id, 40) || a.id, c: asc(a.creator, 24) || "anonimo", v: a.v }));
    }

    /// Candidatos com votos da semana, do mais votado (empate: ativado antes).
    standings(cs = this.cands()) {
        const vo = {};
        for (const id of Object.values(this.s.mc.votes)) vo[id] = (vo[id] || 0) + 1;
        return cs.map((m, i) => ({ ...m, i: i + 1, vo: vo[m.id] || 0 })).sort((a, b) => b.vo - a.vo || a.i - b.i);
    }

    findMod(s, cs = this.cands()) {
        const k = norm(s);
        if (!k) return undefined;
        if (/^\d{1,3}$/.test(k)) return cs[parseInt(k, 10) - 1];
        return cs.find((m) => norm(m.n) === k || m.id === k) || (k.length >= 3 ? cs.find((m) => norm(m.n).startsWith(k)) : undefined);
    }

    /// Virada de semana: fecha a anterior, anuncia o campeao e zera os votos. true = virou.
    rollover(now = this.now()) {
        const wk = isoWeek(now);
        const mc = this.s.mc;
        if (mc.week === wk) return false;
        const top = this.standings()[0];
        if (top?.vo > 0) {
            const w = { week: mc.week, id: top.id, n: top.n, c: top.c, v: top.v, vo: top.vo };
            const m = this.s.mds;
            m.winner = w;
            m.week = mc.week;
            m.at = now;
            m.hist = [w, ...m.hist].slice(0, HALL);
            this.pl.say("CONCURSO", `CONCURSO: ${top.n.toUpperCase()} de ${top.c} e o MOD DA SEMANA (${top.vo} voto(s))`);
        }
        this.s.mc = { week: wk, votes: {} };
        this.save();
        return true;
    }

    mdsSnap(now = this.now()) {
        const st = this.standings();
        const m = this.s.mds;
        return {
            week: this.s.mc.week,
            left: Math.max(0, Math.round((weekEnd(now) - now) / 1000)),
            n: st.length,
            tot: st.reduce((a, x) => a + x.vo, 0),
            top: st.slice(0, 3).map((x) => ({ id: x.id, n: x.n, c: x.c, vo: x.vo, i: x.i })),
            winner: m.winner,
            hist: m.hist.map((w) => ({ week: w.week, n: w.n, c: w.c, vo: w.vo })),
        };
    }

    contest(c) {
        this.rollover();
        const now = this.now();
        const st = this.standings();
        const left = weekEnd(now) - now;
        const days = `${Math.floor(left / DAY)}d ${Math.floor((left % DAY) / 3600000)}h`;
        const w = this.s.mds.winner;
        if (w) this.pl.priv(c, "CONCURSO", `MOD DA SEMANA (${w.week}): ${w.n} de ${w.c} com ${w.vo} voto(s)`);
        if (!st.length) return this.pl.priv(c, "CONCURSO", "nenhum mod ativo na vila. sem candidato, sem concurso. sobe um mod (/modding.txt)");
        this.pl.priv(c, "CONCURSO", `semana ${this.s.mc.week}, acaba em ${days}. vota com /votarmod numero ou nome (1 voto por semana, pode trocar)`);
        const mine = this.s.mc.votes[norm(c.name)];
        for (const m of [...st].sort((a, b) => a.i - b.i).slice(0, 12)) this.pl.priv(c, "CONCURSO", `${m.i}: ${m.n} de ${m.c} - ${m.vo} voto(s)${m.id === mine ? " [SEU VOTO]" : ""}`);
    }

    voteMod(c, s) {
        this.rollover();
        const cs = this.cands();
        if (!cs.length) return this.pl.priv(c, "CONCURSO", "nenhum mod ativo pra votar. a urna ta vazia de candidato"), null;
        const m = this.findMod(s, cs);
        if (!m) return this.pl.priv(c, "CONCURSO", "mod nao encontrado. /concurso pra ver a lista numerada"), null;
        const who = norm(c.name);
        const old = this.s.mc.votes[who];
        if (old === m.id) return this.pl.priv(c, "CONCURSO", `voce ja vota em ${m.n}. fidelidade de torcedor`), m;
        this.s.mc.votes[who] = m.id;
        const vo = this.standings(cs).find((x) => x.id === m.id).vo;
        this.pl.priv(c, "CONCURSO", `${old ? "voto trocado" : "voto registrado"}: ${m.n} de ${m.c} (${vo} voto(s))`);
        this.save();
        this.send();
        return m;
    }

    /// Manchete pra TV (plantao quando muda): campeao das ultimas 24 h.
    news() {
        const m = this.s.mds;
        return m.winner && this.now() - m.at < DAY ? [`MOD DA SEMANA: ${m.winner.n} de ${m.winner.c} vence o concurso do Congresso`] : [];
    }

    send(c) {
        if (c) this.pl.room.send(c, this.snap());
        else this.pl.room.broadcast(this.snap());
    }

    join(c) {
        this.send(this.rollover() ? null : c);
    }

    vote(c, s) {
        const l = findLaw(s);
        if (!l) return this.pl.priv(c, "CONGRESSO", "lei nao existe. /leis pra ver a pauta"), null;
        const who = norm(c.name);
        if (this.s.votes[who] === l.id) {
            this.pl.priv(c, "CONGRESSO", `voce ja votou em ${l.n}. voto de cabresto nao conta duas vezes`);
        } else {
            this.s.votes[who] = l.id;
            this.ev.law[l.id]++;
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
        if (head === "roadmap" || (head === "proposta" && !args.length)) return this.roadmap(c), true;
        if (head === "proposta") return this.propose(c, args.join(" ")), true;
        if (head === "concurso" || (head === "votarmod" && !args.length)) return this.contest(c), true;
        if (head === "votarmod") return this.voteMod(c, args.join(" ")), true;
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
        const rolledMc = this.rollover(now);
        const rolled = this.rmRoll(now) || rolledMc;
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
        // Mod ativado/desativado muda os candidatos sem passar por voto
        const mc = JSON.stringify(this.standings().slice(0, 3).map((x) => [x.id, x.vo]));
        // Mod ativado/desativado tambem muda quem e do conselho (peso 2)
        this.tally();
        const rk = this.rmKey();
        if (changed || busy || rolled || q !== this.lastQ || mc !== this.lastMc || rk !== this.lastRm) this.send();
        this.lastQ = q;
        this.lastMc = mc;
        this.lastRm = rk;
    }

    priceFactor() {
        return this.law("promo").until > this.now() ? 0.5 : 1;
    }

    save() {
        this.pl.save("congresso", this.s);
    }
}
