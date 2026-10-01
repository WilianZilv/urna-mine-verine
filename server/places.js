// Lugares funcionais da vila (Congresso, Bolsa, TV, Banco, Terminal). Cada lugar e uma classe no seu
// arquivo com a mesma interface (todos os metodos opcionais):
//   join(c)                      jogador entrou: manda snapshot {t:"pl", k:<lugar>, ...}
//   command(id, c, head, args)   /comando no chat; true = era desse lugar
//   onMsg(id, c, m)              mensagem {t:"pl", k:<acao>} do cliente (pisar em pulpito, portao...)
//   onHit(c, m)                  golpe/tiro ("a") num NPC: g, i, dmg
//   tick(now, online)            a cada TICK_MS com gente online (e 1x quando esvazia)
//   priceFactor()                multiplicador de preco da /loja (lei de promocao)
// Estado de cada lugar mora em storage "pl_<lugar>" (load/save daqui). Moeda e sempre a FICTICIA do economy.js.
import { Congresso } from "./congresso.js";
import { Bolsa } from "./bolsa.js";
import { Tv } from "./tv.js";
import { Banco } from "./banco.js";
import { Terminal } from "./terminal.js";

const TICK_MS = 20 * 1000;

export const norm = (s) => String(s ?? "").toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "").trim();
export const amount = (v) => (/^\d{1,7}$/.test(String(v ?? "")) ? parseInt(v, 10) : NaN);
export const txt = (s, n) => String(s ?? "").replace(/[\u0000-\u001f<>]/g, "").slice(0, n);

export class Places {
    constructor(room) {
        this.room = room;
        this.iv = null;
        this.timers = {};
        this.list = [];
        room.ctx.blockConcurrencyWhile(async () => {
            const st = room.ctx.storage;
            const load = async (k) => (await st.get(`pl_${k}`)) || {};
            this.congresso = new Congresso(this, await load("congresso"));
            this.bolsa = new Bolsa(this, await load("bolsa"));
            this.tv = new Tv(this, await load("tv"));
            this.banco = new Banco(this, await load("banco"));
            this.terminal = new Terminal(this, await load("terminal"));
            this.list = [this.congresso, this.bolsa, this.tv, this.banco, this.terminal];
        });
    }

    /// Salva o estado `s` do lugar `key` (debounce 1 s).
    save(key, s) {
        if (this.timers[key]) return;
        this.timers[key] = setTimeout(() => {
            delete this.timers[key];
            this.room.ctx.storage.put(`pl_${key}`, s);
        }, 1000);
    }

    /// Fala privada no chat com remetente `from`.
    priv(c, from, m) {
        this.room.send(c, { t: "chat", id: 0, n: from, m: String(m).slice(0, 300) });
    }

    say(from, m) {
        this.room.broadcast({ t: "chat", id: 0, n: from, m: String(m).slice(0, 300) });
    }

    /// Nomes (normalizados) de quem esta online.
    online() {
        return [...this.room.clients.values()].filter((c) => c.name).map((c) => norm(c.name));
    }

    join(c) {
        for (const p of this.list) p.join?.(c);
        if (!this.iv) this.iv = setInterval(() => this.tick(), TICK_MS);
    }

    tick() {
        const online = this.room.clients.size > 0;
        const now = Date.now();
        for (const p of this.list) {
            try {
                p.tick?.(now, online);
            } catch (e) { }
        }
        if (!online && this.iv) {
            clearInterval(this.iv);
            this.iv = null;
        }
    }

    command(id, c, text) {
        const [head, ...args] = text.split(/\s+/);
        const h = norm(head);
        return this.list.some((p) => p.command?.(id, c, h, args) === true);
    }

    onMsg(id, c, m) {
        for (const p of this.list) p.onMsg?.(id, c, m);
    }

    onHit(c, m) {
        for (const p of this.list) p.onHit?.(c, m);
    }

    priceFactor() {
        return this.list.reduce((k, p) => k * (p.priceFactor?.() ?? 1), 1);
    }
}
