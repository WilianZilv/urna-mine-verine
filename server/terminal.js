// Terminal Interdimensional: painel de partidas (vila + jogos do Hub) e viagens rapidas (src/places/terminal.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("terminal", this.s)).
//   /viajar destino   cobra FARE moedas ficticias (vai pro cofre da IA) e responde {t:"pl",k:"go",d} so pra quem pediu
//   /destinos         lista os ids
//   {t:"pl",k:"term_trip",d}   cliente andou num portao (ja teleportou local): so conta, throttle por conexao
//   snapshot {t:"pl",k:"term",trips:{id:n} (hoje),total (desde sempre)}: no join e no tick se mudou
import { norm } from "./places.js";

const FARE = 5;
const TRIP_MS = 1500;
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
        this.s = { day: today(), trips: {}, total: 0, ...s };
        this.last = new Map();
        this.dirty = false;
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

    count(d) {
        this.roll();
        this.s.trips[d] = (this.s.trips[d] || 0) + 1;
        this.s.total++;
        this.dirty = true;
        this.pl.save("terminal", this.s);
    }

    join(c) {
        this.roll();
        this.pl.room.send(c, this.snap());
    }

    command(id, c, head, args) {
        if (head === "destinos") {
            this.pl.priv(c, "TERMINAL", `destinos: ${Object.keys(DESTS).join(", ")} | /viajar destino (${FARE} moedas ficticias) ou anda no portao do terminal (sul da avenida GTA)`);
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
        this.count(d);
        this.pl.room.send(c, { t: "pl", k: "go", d });
        this.pl.priv(c, "TERMINAL", `voo pra ${d} decolando. apertem os cintos, a turbulencia e interdimensional`);
        return true;
    }

    onMsg(id, c, m) {
        if (m.k !== "term_trip" || typeof m.d !== "string" || !Object.hasOwn(DESTS, m.d)) return;
        const now = Date.now();
        if (now - (this.last.get(id) || 0) < TRIP_MS) return;
        this.last.set(id, now);
        this.count(m.d);
    }

    tick(now, online) {
        this.roll();
        for (const id of this.last.keys()) if (!this.pl.room.clients.has(id)) this.last.delete(id);
        if (!online || !this.dirty) return;
        this.dirty = false;
        this.pl.room.broadcast(this.snap());
    }
}
