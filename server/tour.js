// Tour dos Poderes (onboarding): /tour da 15 min pra visitar os 5 lugares (src/places/tour.rs).
// Interface: ver server/places.js. Estado persistido: this.s (salvar com this.pl.save("tour", this.s)).
//   /tour                 comeca (ou mostra o progresso se ja ta rodando)
//   {t:"pl",k:"tour",on,left:[ids],done:[ids],sec}   progresso privado (sec = segundos restantes)
// Carimbo = chegar a RADIUS blocos do SPOT do lugar (c.pos, varrido a cada SCAN_MS enquanto ha tour ativo).
// Completar paga PRIZE do cofre (1x por dia por jogador, so se o cofre fica >= RESERVE).
import { norm } from "./places.js";
import { SPOTS } from "./layout.js";

export const IDS = ["congresso", "bolsa", "tv", "banco", "terminal"];
const NAMES = { congresso: "CONGRESSO", bolsa: "BOLSA", tv: "TV URNA NEWS", banco: "BANCO CENTRAL", terminal: "TERMINAL" };
export const TOUR_MS = 15 * 60 * 1000;
const RADIUS = 8;
export const PRIZE = 40;
const RESERVE = 300;
const SCAN_MS = 1000;

const today = () => new Date().toISOString().slice(0, 10);

export class Tour {
    constructor(places, s) {
        this.pl = places;
        // act: nome -> {until, got:[ids]}; paid: nome -> dia do ultimo premio; fin: nome -> tours completos
        this.s = { act: {}, paid: {}, fin: {}, ...s };
        this.iv = null;
    }

    save() {
        this.pl.save("tour", this.s);
    }

    progress(k, now = Date.now()) {
        const a = this.s.act[k];
        if (!a) return { t: "pl", k: "tour", on: false, left: [], done: [], sec: 0 };
        return { t: "pl", k: "tour", on: true, left: IDS.filter((i) => !a.got.includes(i)), done: [...a.got], sec: Math.max(0, Math.round((a.until - now) / 1000)) };
    }

    /// Manda o progresso pra toda conexao com esse nome.
    sendTo(k, extra) {
        const msg = this.progress(k);
        for (const c of this.pl.room.clients.values()) {
            if (norm(c.name) !== k) continue;
            this.pl.room.send(c, msg);
            if (extra) this.pl.priv(c, "TOUR", extra);
        }
    }

    join(c) {
        const k = norm(c.name);
        this.pl.room.send(c, this.progress(k));
        if (this.s.act[k]) this.arm();
        else if (!this.s.fin[k]) this.pl.priv(c, "TOUR", `novo na vila? digita /tour: visita os 5 poderes em 15 min e ganha ${PRIZE} moedas ficticias do cofre`);
    }

    command(id, c, head) {
        if (head !== "tour") return false;
        const k = norm(c.name);
        const now = Date.now();
        const a = this.s.act[k];
        if (a && a.until > now) {
            const p = this.progress(k, now);
            const left = p.left.map((i) => NAMES[i]).join(", ");
            this.pl.priv(c, "TOUR", `${p.done.length}/5 carimbos, faltam ${left} | ${Math.ceil(p.sec / 60)} min no relogio`);
            this.pl.room.send(c, p);
            return true;
        }
        this.s.act[k] = { until: now + TOUR_MS, got: [] };
        this.save();
        this.sendTo(k, `TOUR DOS PODERES comecou! 15 min pra carimbar CONGRESSO, BOLSA, TV, BANCO e TERMINAL (qualquer ordem). segue os feixes de luz`);
        this.arm();
        return true;
    }

    /// Liga a varredura de posicao (desliga sozinha quando nao sobra tour ativo).
    arm() {
        if (this.iv) return;
        this.iv = setInterval(() => this.scan(Date.now()), SCAN_MS);
    }

    scan(now) {
        for (const c of this.pl.room.clients.values()) {
            const k = norm(c.name);
            const a = this.s.act[k];
            if (!a || a.until <= now || !Array.isArray(c.pos)) continue;
            const [x, , z] = c.pos.map(Number);
            for (const i of IDS) {
                if (a.got.includes(i) || !(Math.hypot(x - SPOTS[i][0], z - SPOTS[i][1]) < RADIUS)) continue;
                a.got.push(i);
                this.save();
                if (a.got.length < IDS.length) this.sendTo(k, `carimbo ${NAMES[i]} ${a.got.length}/5`);
                else this.finish(c, k);
            }
        }
        this.expire(now);
        const online = new Set(this.pl.online());
        if (this.iv && !Object.keys(this.s.act).some((k) => online.has(k))) {
            clearInterval(this.iv);
            this.iv = null;
        }
    }

    finish(c, k) {
        delete this.s.act[k];
        this.s.fin[k] = (this.s.fin[k] || 0) + 1;
        const day = today();
        const eco = this.pl.room.eco;
        let note;
        if (this.s.paid[k] === day) note = "premio de hoje ja foi pago, amanha tem mais";
        else if (eco.s.tr - PRIZE < RESERVE) note = `cofre da IA abaixo de ${RESERVE + PRIZE}, sem premio (doa com /doar)`;
        else {
            this.s.paid[k] = day;
            const w = eco.wallet(c.name);
            eco.s.tr -= PRIZE;
            w.c += PRIZE;
            eco.entry(c.name, "completou o TOUR DOS PODERES", PRIZE, "premio do tour pago pelo cofre (1x por dia)");
            eco.sendMe(c.name);
            note = `+${PRIZE} moedas ficticias do cofre`;
        }
        this.save();
        this.sendTo(k, `carimbo final! 5/5, ${note}`);
        this.pl.say("TOUR", `${c.name} completou o TOUR DOS PODERES`);
    }

    expire(now) {
        for (const [k, a] of Object.entries(this.s.act)) {
            if (a.until > now) continue;
            delete this.s.act[k];
            this.save();
            this.sendTo(k, `tempo esgotado com ${a.got.length}/5. /tour pra tentar de novo`);
        }
    }

    tick(now) {
        this.expire(now);
        const day = today();
        for (const [k, d] of Object.entries(this.s.paid)) {
            if (d === day) continue;
            delete this.s.paid[k];
            this.save();
        }
    }
}
