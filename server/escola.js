// Escola de Agentes: ensina humano e agente de IA a criar mod/skin/item/jogo do Hub (src/places/escola.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("escola", this.s)).
//   /escola                                   4 passos + links (privado)
//   {t:"pl",k:"escola",site,mods,portals,creators,reads,last:[[nome,criador]],grad:[[criador,primeiro,"dd/mm"]]}
//   snapshot (join, tick, leitura nova); grad = MURAL DOS FORMADOS (ultimos 8, mais novo primeiro)
// Leituras do /skill.md: o worker manda POST /api/escola/hit pro DO (ctx.waitUntil) a cada GET da skill.
// Guarda SO a contagem por dia (horario de Brasilia), nunca IP/UA; dias velhos saem depois de KEEP_DAYS.
import { SITE } from "./mods.js";

const KEEP_DAYS = 14;
const PUSH_MS = 3000;
const GRAD_SHOW = 8;
const GRAD_MAX = 1000;
const LINKS = ["/skill.md", "/modding.txt", "/hub.txt", "/hub-templates/"];

export const day = (now = Date.now()) => new Date(now - 3 * 3600 * 1000).toISOString().slice(0, 10);

export class Escola {
    constructor(places, s) {
        this.pl = places;
        this.s = s;
        this.s.reads ??= {}; // dia -> leituras do /skill.md
        this.s.grad ??= {}; // criador normalizado -> {n, w: primeiro mod/portal, p: era portal, d: visto em}
        this.last = "";
        this.pt = null;
    }

    /// Portais no ar do Hub (mesma regra do hub.js: versao ativa com origem verificada).
    portals() {
        return Object.values(this.pl.room.hub?.s?.p || {}).filter((p) => {
            const a = p.versions?.find((x) => x.v === p.active);
            return a && p.verified?.origin === a.origin;
        });
    }

    mods() {
        return [...(this.pl.room.mods?.active?.values() || [])];
    }

    reads(now = Date.now()) {
        return this.s.reads[day(now)] || 0;
    }

    snap(now = Date.now()) {
        const mods = this.mods(), portals = this.portals();
        const creators = new Set([...mods.map((a) => a.creator), ...portals.map((p) => p.author)].filter(Boolean).map((n) => String(n).toLowerCase()));
        const last = [...mods].sort((a, b) => (b.at || 0) - (a.at || 0)).slice(0, 3).map((a) => [String(a.pkg?.manifest?.name || a.id), String(a.creator || "?")]);
        const grad = Object.values(this.s.grad).sort((a, b) => b.d - a.d).slice(0, GRAD_SHOW).map((g) => [g.n, g.w, day(g.d).slice(5).split("-").reverse().join("/")]);
        return { t: "pl", k: "escola", site: SITE, mods: mods.length, portals: portals.length, creators: creators.size, reads: this.reads(now), last, grad };
    }

    /// Formados: criador cujo primeiro mod/portal entrou no ar. A primeira varredura (estado sem `seeded`) so
    /// registra quem ja existia, sem anunciar; depois cada formado novo vira fala global.
    graduate(now = Date.now()) {
        const first = {};
        const see = (n, w, at, p) => {
            if (!n) return;
            const k = String(n).toLowerCase();
            if (this.s.grad[k] || (first[k] && first[k].at <= at)) return;
            first[k] = { n: String(n), w: String(w), at, p };
        };
        for (const a of this.mods()) see(a.creator, a.pkg?.manifest?.name || a.id, a.at || 0, false);
        for (const p of this.portals()) see(p.author, p.versions.find((x) => x.v === p.active)?.name || p.id, p.created || 0, true);
        const fresh = Object.entries(first);
        if (!fresh.length && this.s.seeded) return;
        for (const [k, g] of fresh) this.s.grad[k] = { n: g.n, w: g.w, p: g.p, d: now };
        if (this.s.seeded) for (const [, g] of fresh.slice(0, 3)) this.pl.say("ESCOLA", `${g.n} se formou! primeiro ${g.p ? "portal" : "mod"}: ${g.w}`);
        this.s.seeded = true;
        const ks = Object.keys(this.s.grad);
        if (ks.length > GRAD_MAX) for (const k of ks.sort((a, b) => this.s.grad[a].d - this.s.grad[b].d).slice(0, ks.length - GRAD_MAX)) delete this.s.grad[k];
        this.pl.save("escola", this.s);
    }

    push(force = false) {
        const m = this.snap();
        const h = JSON.stringify(m);
        if (!force && h === this.last) return;
        this.last = h;
        this.pl.room.broadcast(m);
    }

    /// Uma leitura do /skill.md (chamado pelo DO). Snapshot sai agrupado, no maximo 1 a cada PUSH_MS.
    hit(now = Date.now()) {
        const d = day(now);
        this.s.reads[d] = (this.s.reads[d] || 0) + 1;
        const cut = day(now - KEEP_DAYS * 86400 * 1000);
        for (const k of Object.keys(this.s.reads)) if (k <= cut) delete this.s.reads[k];
        this.pl.save("escola", this.s);
        if (!this.pt && this.pl.room.clients?.size) {
            this.pt = setTimeout(() => {
                this.pt = null;
                this.push();
            }, PUSH_MS);
        }
        return this.s.reads[d];
    }

    join(c) {
        this.pl.room.send(c, this.snap());
    }

    tick(now, online) {
        if (!online) return;
        this.graduate(now);
        this.push();
    }

    command(id, c, head) {
        if (head !== "escola") return false;
        const m = this.snap();
        this.pl.priv(c, "ESCOLA", `COMO CRIAR UM MOD EM 4 PASSOS: 1) cola ${SITE}/skill.md no teu agente (Cursor, Claude Code, Codex...) 2) o agente valida o pacote 3) sobe 4) ativa: aparece pra vila toda na hora. tu nao escreve codigo, so conversa`);
        this.pl.priv(c, "ESCOLA", `LINKS: ${LINKS.map((l) => SITE + l).join(" | ")}`);
        this.pl.priv(c, "ESCOLA", `AO VIVO: ${m.mods} mods, ${m.portals} portais, ${m.creators} criadores, ${m.reads} leituras do /skill.md hoje. aula presencial: Escola de Agentes, ao sul das casas`);
        return true;
    }
}
