// Servidor multiplayer: Cloudflare Worker (serve web/) + Durable Object "Room" (WebSocket relay).
// Primeiro jogador vira host (simula lutadores/urna); eventos de mundo ficam num log pra quem entra depois.
import { DurableObject } from "cloudflare:workers";
import { Economy } from "./economy.js";

export default {
    async fetch(req, env) {
        const url = new URL(req.url);
        if (url.pathname === "/ws") {
            if (req.headers.get("Upgrade") !== "websocket") return new Response("use websocket", { status: 426 });
            return env.ROOM.get(env.ROOM.idFromName("vila")).fetch(req);
        }
        return env.ASSETS.fetch(req);
    },
};

// Mensagem WebSocket do Cloudflare tem limite de 1 MiB (o log vai inteiro no "welcome").
const MAX_LOG = 6000;

// ---------------------------------------------------------------- Agente IA (/comando no chat)
// O modelo só devolve JSON com operações da whitelist abaixo; o servidor valida/limita e transmite
// em ordem pra todos (mesmo caminho dos eventos de mundo, então fica sincronizado e no log).
const BLOCKS = { ar: 0, air: 0, grama: 1, grass: 1, terra: 2, dirt: 2, pedra: 3, stone: 3, areia: 4, sand: 4, madeira: 5, tabua: 5, planks: 5, tronco: 6, log: 6, folha: 7, leaves: 7, pedregulho: 8, cobble: 8, vidro: 9, glass: 9, preto: 11, black: 11, tijolo: 13, brick: 13, cascalho: 14, gravel: 14, la: 15, wool: 15, neon: 16 };
const MAX_QUEUE = 12;
const BLOCK_BUDGET = 30000;

const SYSTEM = `Voce e a IA guardia do jogo URNA-MINE-VERINE (clone de Minecraft, satira politica brasileira, zoeira).
Jogadores mandam /comandos no chat e voce OBEDECE de forma criativa e exagerada, alterando o mundo com operacoes.
MUNDO: voxels 128x48x128. x e z de 0 a 127, y de 0 a 47. O chao da vila e y=20 (primeira camada de ar; blocos em y<20 sao terreno).
LUGARES: praca central da briga (64,20,64) com Lula, Flavio Bolsonaro, Renan Santos e Wolverine. Clube de house a oeste (x 8..36, z 48..80) protegido por escudo (centro 22,22,64 raio 22; explosoes nao afetam dentro). Laboratorio de robos a leste (x 94..112, z 52..76). Torre/spawn (64,30,105) ao sul. Placar das eleicoes ao norte da praca. Uma urna eletronica gigante anda pelo mapa atirando laser.
BLOCOS: grama, terra, pedra, areia, madeira, tronco, folha, pedregulho, vidro, preto, tijolo, cascalho, la, neon, ar (ar apaga).
OPERACOES (lista "ops", max 40):
{"op":"box","from":[x,y,z],"to":[x,y,z],"block":"neon","hollow":false}  caixa cheia ou oca (paredes). Combine varias pra formar piramides, letras, predios, estatuas.
{"op":"sphere","center":[x,y,z],"radius":1-12,"block":"vidro","hollow":true}
{"op":"explode","center":[x,y,z],"radius":1-8}
{"op":"banner","text":"TEXTO GRANDE NA TELA"}
{"op":"fireworks","seconds":1-20}
{"op":"sky","color":[r,g,b],"seconds":1-60}  cores 0..1
{"op":"urna_rage","seconds":1-30}  urna dispara sem parar
{"op":"teleport","player":"nome","to":[x,y,z]}
{"op":"wolverine"}  Wolverine cai do ceu
{"op":"tv","url":"https://www.youtube.com/watch?v=..."}  troca o video do telao do clube
Limite total ~30000 blocos por comando. Se o jogador nao disser onde, construa perto da posicao dele (na frente, sem prender ele dentro).
Nunca destrua o clube. Recuse discurso de odio/ofensa pesada com uma piada e sem ops. Sempre responda em portugues zoeiro.
RESPONDA SO JSON: {"say":"frase curta (ate 140 letras) que a IA fala no jogo","ops":[...]}`;

const ci = (v, lo, hi) => Math.max(lo, Math.min(hi, Math.round(Number(v) || 0)));
const pos3 = (a) => (Array.isArray(a) && a.length >= 3 ? [ci(a[0], 0, 127), ci(a[1], 1, 47), ci(a[2], 0, 127)] : null);
const block = (b) => BLOCKS[String(b ?? "").toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "")];

function sanitize(ops) {
    const out = [];
    let budget = BLOCK_BUDGET;
    for (const o of (Array.isArray(ops) ? ops : []).slice(0, 40)) {
        switch (o?.op) {
            case "box": {
                const a = pos3(o.from), b = pos3(o.to), k = block(o.block);
                if (!a || !b || k === undefined) break;
                const vol = (Math.abs(a[0] - b[0]) + 1) * (Math.abs(a[1] - b[1]) + 1) * (Math.abs(a[2] - b[2]) + 1);
                if (vol > budget) break;
                budget -= vol;
                out.push({ op: "box", a, b, k, h: !!o.hollow });
                break;
            }
            case "sphere": {
                const c = pos3(o.center), r = ci(o.radius, 1, 12), k = block(o.block);
                if (!c || k === undefined || 4.2 * r ** 3 > budget) break;
                budget -= 4.2 * r ** 3;
                out.push({ op: "ball", c, r, k, h: !!o.hollow });
                break;
            }
            case "explode": {
                const c = pos3(o.center);
                if (c) out.push({ op: "boom", c, r: ci(o.radius, 1, 8) });
                break;
            }
            case "banner": out.push({ op: "banner", text: String(o.text || "").slice(0, 60) }); break;
            case "fireworks": out.push({ op: "fireworks", s: ci(o.seconds, 1, 20) }); break;
            case "sky": {
                const c = Array.isArray(o.color) ? o.color.map((v) => Math.max(0, Math.min(1, Number(v) || 0))).slice(0, 3) : [1, 0, 0];
                out.push({ op: "sky", c, s: ci(o.seconds, 1, 60) });
                break;
            }
            case "urna_rage": out.push({ op: "rage", s: ci(o.seconds, 1, 30) }); break;
            case "teleport": {
                const to = pos3(o.to);
                if (to) out.push({ op: "tp", n: String(o.player || "").slice(0, 16), to });
                break;
            }
            case "wolverine": out.push({ op: "wolverine" }); break;
            case "tv": {
                const u = String(o.url || "");
                if (/^https:\/\/(www\.|m\.)?(youtube\.com|youtu\.be)\//.test(u)) out.push({ op: "tv", url: u.slice(0, 300) });
                break;
            }
        }
    }
    return out;
}

export class Room extends DurableObject {
    constructor(ctx, env) {
        super(ctx, env);
        this.clients = new Map();
        this.next = 1;
        this.host = 0;
        this.log = [];
        this.tv = null;
        this.queue = [];
        this.busy = false;
        this.eco = new Economy(this, sanitize);
    }

    alarm() {
        return this.eco.alarm();
    }

    sys(text) {
        this.broadcast({ t: "chat", id: 0, n: "IA", m: text });
    }

    enqueue(id, c, text) {
        if (!text) return this.sys("manda /alguma coisa. ex: /constroi uma piramide de neon na praca");
        if (!this.env.OPENAI_API_KEY) return this.sys("to sem cerebro: falta OPENAI_API_KEY no servidor");
        if (this.queue.some((q) => q.id === id)) return this.sys(`${c.name}, calma, teu comando anterior ainda ta na fila`);
        if (this.queue.length >= MAX_QUEUE) return this.sys("fila cheia, tenta daqui a pouco");
        this.queue.push({ id, name: c.name, text: text.slice(0, 200), pos: c.pos });
        this.sys(`fila #${this.queue.length}: ${c.name} pediu /${text.slice(0, 80)}`);
        this.pump();
    }

    async pump() {
        if (this.busy) return;
        this.busy = true;
        while (this.queue.length) {
            try {
                await this.run(this.queue[0]);
            } catch (e) {
                this.sys(`deu ruim: ${String(e.message || e).slice(0, 120)}`);
            }
            this.queue.shift();
        }
        this.busy = false;
    }

    async run(job) {
        const fmt = (p) => (Array.isArray(p) ? p.map((v) => Math.round(v)).join(",") : "?");
        const players = [...this.clients.values()].map((x) => `${x.name}@(${fmt(x.pos)})`).join(", ");
        const r = await fetch("https://api.openai.com/v1/chat/completions", {
            method: "POST",
            headers: { "content-type": "application/json", authorization: `Bearer ${this.env.OPENAI_API_KEY}` },
            body: JSON.stringify({
                model: this.env.OPENAI_MODEL || "gpt-4.1-mini",
                response_format: { type: "json_object" },
                messages: [
                    { role: "system", content: SYSTEM },
                    { role: "user", content: `Jogadores online: ${players}\nPedido de ${job.name} (posicao ${fmt(job.pos)}): ${job.text}` },
                ],
            }),
        });
        if (!r.ok) throw new Error(`OpenAI ${r.status} ${(await r.text()).slice(0, 80)}`);
        const out = JSON.parse((await r.json()).choices[0].message.content);
        const ops = sanitize(out.ops);
        const tv = ops.find((o) => o.op === "tv");
        if (tv) {
            this.tv = tv.url;
            this.broadcast({ t: "tv", id: 0, n: "IA", u: tv.url });
        }
        const w = { t: "w", k: "ai", id: 0, n: job.name, cmd: job.text, say: String(out.say || "").slice(0, 160), ops: ops.filter((o) => o.op !== "tv") };
        this.log.push(w);
        if (this.log.length > MAX_LOG) this.log.splice(0, this.log.length - MAX_LOG);
        this.broadcast(w);
    }

    async fetch() {
        const [client, server] = Object.values(new WebSocketPair());
        server.accept();
        const id = this.next++;
        const c = { ws: server, name: null };
        server.addEventListener("message", (e) => this.onMessage(id, c, e.data));
        server.addEventListener("close", () => this.leave(id));
        server.addEventListener("error", () => this.leave(id));
        return new Response(null, { status: 101, webSocket: client });
    }

    send(c, data) {
        try { c.ws.send(typeof data === "string" ? data : JSON.stringify(data)); } catch (e) { }
    }

    broadcast(obj, except) {
        const s = JSON.stringify(obj);
        for (const [id, c] of this.clients) if (id !== except) this.send(c, s);
    }

    onMessage(id, c, data) {
        let m;
        try { m = JSON.parse(data); } catch (e) { return; }
        if (!m || typeof m !== "object") return;
        if (!c.name) {
            if (m.t !== "hello") return;
            c.name = String(m.n || "anon").slice(0, 16);
            this.clients.set(id, c);
            if (!this.clients.has(this.host)) this.host = id;
            const players = [...this.clients].map(([i, x]) => [i, x.name]);
            this.send(c, { t: "welcome", id, host: this.host, log: this.log, tv: this.tv, players });
            this.broadcast({ t: "join", id, n: c.name }, id);
            this.eco.join(c);
            return;
        }
        m.id = id;
        switch (m.t) {
            case "p":
                c.pos = m.p;
                this.broadcast(m, id);
                this.eco.onPos(c);
                break;
            case "s":
                if (id === this.host) this.broadcast(m, id);
                break;
            case "chat":
                m.m = String(m.m || "").slice(0, 200);
                this.broadcast(m);
                if (m.m.startsWith("/") && !this.eco.command(id, c, m.m.slice(1).trim())) this.enqueue(id, c, m.m.slice(1).trim());
                break;
            case "tv":
                this.tv = String(m.u || "").slice(0, 500);
                this.broadcast(m);
                break;
            case "a": {
                const h = this.clients.get(this.host);
                if (h) this.send(h, m);
                this.eco.onHit(c, m);
                break;
            }
            case "w":
                this.eco.onWorld(c, m);
                if (m.k === "reset") this.log = [];
                else {
                    this.log.push(m);
                    if (this.log.length > MAX_LOG) this.log.splice(0, this.log.length - MAX_LOG);
                }
                this.broadcast(m);
                break;
        }
    }

    leave(id) {
        if (!this.clients.delete(id)) return;
        this.broadcast({ t: "leave", id });
        if (id === this.host) {
            this.host = this.clients.keys().next().value || 0;
            if (this.host) this.broadcast({ t: "host", id: this.host });
        }
    }
}
