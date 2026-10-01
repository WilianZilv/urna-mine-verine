// Terminal Interdimensional: painel de partidas (vila + jogos do Hub) e viagens rapidas (src/places/terminal.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("terminal", this.s)).
//   /viajar destino   cobra FARE moedas ficticias (vai pro cofre da IA) e responde {t:"pl",k:"go",d} so pra quem pediu
//   /destinos         lista os ids
//   {t:"pl",k:"term_trip",d}   cliente andou num portao (ja teleportou local): so conta, throttle por conexao
//   snapshot {t:"pl",k:"term",trips:{id:n} (hoje),total (desde sempre)}: no join e no tick se mudou
//   {t:"pl",k:"term",arr:{d,n}}   broadcast a cada viagem contada (feixe de chegada em todo cliente)
// Passaporte (persistente, por nome normalizado): carimbo da vila = chegar pelo terminal ou andar a STAMP_R
// blocos do lugar (c.pos varrido a cada SCAN_MS); carimbo "hub:<portal>" = entrar num portal do Hub
// (universe.onSession chama portal()). Lista vai no /api/passport ("stamps").
//   /passaporte                   N/M + o que falta
//   {t:"pl",k:"term",stamps:[ids]}  privado: no join e a cada carimbo novo
// Vila completa: chat pra todos + PRIZE do cofre 1x por jogador (so se o cofre fica >= RESERVE; senao tenta de novo depois).
import { norm } from "./places.js";
import { SPOTS, HUB } from "./layout.js";

const FARE = 5;
const TRIP_MS = 1500;
// id -> [x, z]; o hub nao tem SPOT de missao, usa o centro do domo
export const VILLAGE = {
    praca: SPOTS.praca,
    arena: SPOTS.arena,
    club: SPOTS.clube,
    lab: SPOTS.lab,
    hub: [HUB.dome[0], HUB.dome[2]],
    congresso: SPOTS.congresso,
    bolsa: SPOTS.bolsa,
    tv: SPOTS.tv,
    banco: SPOTS.banco,
    terminal: SPOTS.terminal,
};
const NAMES = { praca: "PRACA", arena: "ARENA", club: "CLUB", lab: "LAB", hub: "GAME HUB", congresso: "CONGRESSO", bolsa: "BOLSA", tv: "TV URNA NEWS", banco: "BANCO CENTRAL", terminal: "TERMINAL" };
const STAMP_R = 8;
const SCAN_MS = 2000;
const STAMP_MAX = 64;
export const PRIZE = 25;
const RESERVE = 300;
// id -> apelidos aceitos no /viajar (ja normalizados, sem espaco)
const DESTS = {
    praca: ["praca", "centro", "spawn", "inicio", "fonte"],
    arena: ["arena", "gigantes", "briga", "godzilha"],
    club: ["club", "clube", "balada", "house", "boate"],
    lab: ["lab", "laboratorio", "ciencia", "cerebro"],
    hub: ["hub", "gamehub", "jogos", "portais", "games"],
    congresso: ["congresso", "camara", "leis", "senado"],
    bolsa: ["bolsa", "acoes", "mercado", "bolsadevalores"],
    tv: ["tv", "news", "urnanews", "tvurnanews", "jornal"],
};

export function resolve(arg) {
    const a = norm(arg).replace(/[^a-z0-9]/g, "");
    if (!a) return null;
    for (const [id, al] of Object.entries(DESTS)) if (al.includes(a)) return id;
    if (a.length < 3) return null;
    for (const [id, al] of Object.entries(DESTS)) if (al.some((x) => x.startsWith(a) || a.startsWith(x))) return id;
    return null;
}

const today = () => new Date().toISOString().slice(0, 10);

export class Terminal {
    constructor(places, s) {
        this.pl = places;
        // pp: nome -> [carimbos]; ppd: nome -> 1 quando o premio da vila ja foi pago
        this.s = { day: today(), trips: {}, total: 0, pp: {}, ppd: {}, ...s };
        this.last = new Map();
        this.dirty = false;
        this.iv = null;
    }

    stampsOf(name) {
        return [...(this.s.pp[norm(name)] || [])];
    }

    /// Manda carimbos (e um aviso opcional) pra toda conexao com esse nome.
    sendStamps(k, note) {
        const msg = { t: "pl", k: "term", stamps: [...(this.s.pp[k] || [])] };
        for (const c of this.pl.room.clients.values()) {
            if (norm(c.name) !== k) continue;
            this.pl.room.send(c, msg);
            if (note) this.pl.priv(c, "PASSAPORTE", note);
        }
    }

    villageCount(k) {
        const got = this.s.pp[k] || [];
        return Object.keys(VILLAGE).filter((i) => got.includes(i)).length;
    }

    stamp(name, id) {
        const k = norm(name);
        if (!k) return false;
        const l = (this.s.pp[k] ||= []);
        if (l.includes(id) || l.length >= STAMP_MAX) return false;
        l.push(id);
        this.pl.save("terminal", this.s);
        const vil = Object.hasOwn(VILLAGE, id);
        const n = this.villageCount(k), all = Object.keys(VILLAGE).length;
        this.sendStamps(k, vil ? `carimbo ${NAMES[id]}! vila ${n}/${all}` : `carimbo do portal ${id.slice(4)} no passaporte`);
        if (vil && n === all) {
            this.pl.say("TERMINAL", `${String(name).slice(0, 16)} completou o PASSAPORTE DA VILA`);
            this.prize(name);
        }
        return true;
    }

    /// Premio da vila completa, 1x por jogador.
    prize(name) {
        const k = norm(name);
        if (this.s.ppd[k] || this.villageCount(k) < Object.keys(VILLAGE).length) return;
        const eco = this.pl.room.eco;
        if (eco.s.tr - PRIZE < RESERVE) return this.sendStamps(k, `cofre da IA abaixo de ${RESERVE + PRIZE}, premio do passaporte fica pendente (doa com /doar)`);
        this.s.ppd[k] = 1;
        this.pl.save("terminal", this.s);
        const w = eco.wallet(name);
        eco.s.tr -= PRIZE;
        w.c += PRIZE;
        eco.entry(name, "completou o PASSAPORTE DA VILA", PRIZE, "premio do passaporte pago pelo cofre (1x por jogador)");
        eco.sendMe(name);
        this.sendStamps(k, `PASSAPORTE DA VILA completo: +${PRIZE} moedas ficticias do cofre`);
    }

    portal(c, pid) {
        if (c?.name && /^[a-z0-9-]{2,32}$/.test(String(pid))) this.stamp(c.name, `hub:${pid}`);
    }

    arm() {
        if (!this.iv) this.iv = setInterval(() => this.scan(), SCAN_MS);
    }

    scan() {
        const clients = this.pl.room.clients;
        if (!clients.size && this.iv) {
            clearInterval(this.iv);
            this.iv = null;
        }
        for (const c of clients.values()) {
            if (!c.name || !Array.isArray(c.pos)) continue;
            const [x, , z] = c.pos.map(Number);
            for (const [id, [sx, sz]] of Object.entries(VILLAGE)) if (Math.hypot(x - sx, z - sz) < STAMP_R) this.stamp(c.name, id);
        }
    }

    passport(c) {
        const k = norm(c.name);
        const got = this.s.pp[k] || [];
        const ids = Object.keys(VILLAGE);
        const n = this.villageCount(k);
        const miss = ids.filter((i) => !got.includes(i)).map((i) => NAMES[i]);
        const live = this.pl.room.hub?.snapshot?.().portals || [];
        const pn = live.filter((p) => got.includes(`hub:${p.id}`)).length;
        const pmiss = live.filter((p) => !got.includes(`hub:${p.id}`)).map((p) => String(p.name).slice(0, 24));
        const vila = miss.length ? `vila ${n}/${ids.length} (faltam ${miss.join(", ")})` : `vila ${n}/${ids.length} COMPLETA`;
        const hub = live.length ? ` | portais do hub ${pn}/${live.length}${pmiss.length ? ` (faltam ${pmiss.slice(0, 4).join(", ")}${pmiss.length > 4 ? "..." : ""})` : ""}` : "";
        this.pl.priv(c, "PASSAPORTE", `${n + pn}/${ids.length + live.length} carimbos | ${vila}${hub}`);
        this.pl.room.send(c, { t: "pl", k: "term", stamps: [...got] });
        this.prize(c.name);
    }

    roll() {
        if (this.s.day === today()) return;
        this.s.day = today();
        this.s.trips = {};
        this.dirty = true;
        this.pl.save("terminal", this.s);
    }

    snap() {
        return { t: "pl", k: "term", trips: { ...this.s.trips }, total: this.s.total };
    }

    count(d, name) {
        this.roll();
        this.s.trips[d] = (this.s.trips[d] || 0) + 1;
        this.s.total++;
        this.dirty = true;
        this.pl.save("terminal", this.s);
        this.pl.room.broadcast({ t: "pl", k: "term", arr: { d, n: String(name || "").slice(0, 20) } });
        this.stamp(name, d);
    }

    join(c) {
        this.roll();
        this.pl.room.send(c, this.snap());
        this.pl.room.send(c, { t: "pl", k: "term", stamps: this.stampsOf(c.name) });
        this.arm();
    }

    command(id, c, head, args) {
        if (head === "passaporte") return this.passport(c), true;
        if (head === "destinos") {
            this.pl.priv(c, "TERMINAL", `destinos: ${Object.keys(DESTS).join(", ")} | /viajar destino (${FARE} moedas ficticias) ou anda no portao do terminal (sul da avenida GTA) | /passaporte mostra teus carimbos`);
            return true;
        }
        if (head !== "viajar") return false;
        const d = resolve(args.join(" "));
        if (!d) return this.pl.priv(c, "TERMINAL", `destino desconhecido. tenta: ${Object.keys(DESTS).join(", ")}`), true;
        const eco = this.pl.room.eco;
        const w = eco.wallet(c.name);
        if (w.c < FARE) return this.pl.priv(c, "TERMINAL", `passagem custa ${FARE}, tens ${w.c}. vai a pe, faz bem pra saude (ou anda no portao, que e de graca)`), true;
        w.c -= FARE;
        eco.s.tr += FARE;
        eco.entry(c.name, `viajou pra ${d}`, FARE, "passagem do terminal vai pro cofre da IA");
        eco.sendMe(c.name);
        this.pl.room.send(c, { t: "pl", k: "go", d });
        this.count(d, c.name);
        this.pl.priv(c, "TERMINAL", `voo pra ${d} decolando. apertem os cintos, a turbulencia e interdimensional`);
        return true;
    }

    onMsg(id, c, m) {
        if (m.k !== "term_trip" || typeof m.d !== "string" || !Object.hasOwn(DESTS, m.d)) return;
        const now = Date.now();
        if (now - (this.last.get(id) || 0) < TRIP_MS) return;
        this.last.set(id, now);
        this.count(m.d, c.name);
    }

    tick(now, online) {
        this.roll();
        for (const id of this.last.keys()) if (!this.pl.room.clients.has(id)) this.last.delete(id);
        if (!online || !this.dirty) return;
        this.dirty = false;
        this.pl.room.broadcast(this.snap());
    }
}
