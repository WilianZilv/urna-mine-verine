// Servidor multiplayer: Cloudflare Worker (serve web/) + Durable Object "Room" (WebSocket relay).
// Primeiro jogador vira host (simula lutadores/urna); eventos de mundo ficam num log pra quem entra depois.
import { DurableObject } from "cloudflare:workers";

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

export class Room extends DurableObject {
    constructor(ctx, env) {
        super(ctx, env);
        this.clients = new Map();
        this.next = 1;
        this.host = 0;
        this.log = [];
        this.tv = null;
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
            return;
        }
        m.id = id;
        switch (m.t) {
            case "p":
                this.broadcast(m, id);
                break;
            case "s":
                if (id === this.host) this.broadcast(m, id);
                break;
            case "chat":
                m.m = String(m.m || "").slice(0, 200);
                this.broadcast(m);
                break;
            case "tv":
                this.tv = String(m.u || "").slice(0, 500);
                this.broadcast(m);
                break;
            case "a": {
                const h = this.clients.get(this.host);
                if (h) this.send(h, m);
                break;
            }
            case "w":
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
