// Ringue com 3 clientes WebSocket falsos (wrangler dev local ou BASE=wss://...): fila pela lona e por /ringue,
// contagem, round por nocaute via "pv", jab validado, W.O. por desconexao, terceiro entra contra o rei, escudo do lote.
//   node tools/verify_ring_ws.mjs            (BASE=ws://127.0.0.1:8791)
import assert from "node:assert/strict";

const BASE = process.env.BASE || "ws://127.0.0.1:8791";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const tag = Date.now().toString(36).slice(-4);

async function client(n) {
    const ws = new WebSocket(`${BASE}/ws`);
    const c = { ws, got: [], id: 0, n: `${n}${tag}` };
    ws.onmessage = (e) => c.got.push(JSON.parse(e.data));
    await new Promise((r, j) => ((ws.onopen = r), (ws.onerror = j)));
    ws.send(JSON.stringify({ t: "hello", n: c.n }));
    for (let i = 0; i < 50 && !c.id; i++) {
        await sleep(100);
        c.id = c.got.find((m) => m.t === "welcome")?.id || 0;
    }
    c.send = (m) => ws.send(JSON.stringify(m));
    c.pos = (p, ch = 0, pv) => c.send({ t: "p", p, y: 0, c: ch, ...(pv ? { pv } : {}) });
    c.ring = () => c.got.filter((m) => m.t === "pl" && m.k === "ringue");
    c.snap = () => c.ring().filter((m) => !m.a).at(-1);
    return c;
}

async function until(f, ms = 8000, what = "condicao") {
    for (let t = 0; t < ms; t += 100) {
        if (f()) return;
        await sleep(100);
    }
    throw new Error(`timeout: ${what}`);
}

const a = await client("ana"), b = await client("bia");
assert.ok(a.id && b.id, "welcome");
assert.ok(a.snap(), "snapshot do ringue no join");
const IN = [74, 22, 54];
// Ana pisa na lona; Bia manda /ringue de longe
a.pos(IN);
b.pos([100, 20, 100]);
await sleep(600);
b.send({ t: "chat", m: "/ringue" });
await until(() => a.snap()?.ph === "count", 5000, "contagem");
const s0 = a.snap();
assert.deepEqual(s0.f.map((f) => f[1]).sort(), [a.n, b.n].sort());
const tpB = b.ring().find((m) => m.a === "tp");
assert.ok(tpB && tpB.heal === 1, "bia teleportada pro corner com cura");
const [ca, cb] = [s0.f.find((f) => f[0] === a.id), s0.f.find((f) => f[0] === b.id)];
assert.ok(ca && cb);
a.pos(a.ring().find((m) => m.a === "tp").p);
b.pos(tpB.p);
assert.ok(b.ring().some((m) => m.a === "msg" && /ROUND 1/.test(m.m)), "faixa do round so pros lutadores");
await until(() => a.snap()?.ph === "fight", 5000, "LUTA");
// Bia joga de ENCANADOR (sem PvP proprio): jab de longe nao conta, de perto conta
b.pos([78.5, 22, 54], 6);
a.pos([74, 22, 54], 0);
await sleep(300);
b.send({ t: "pl", k: "ringue", a: "soco" });
await sleep(400);
assert.equal(a.snap().f.find((f) => f[0] === a.id)[2], 100, "jab longe");
b.pos([75.5, 22, 54], 6);
await sleep(300);
b.send({ t: "pl", k: "ringue", a: "soco" });
await until(() => a.snap().f.find((f) => f[0] === a.id)[2] === 90, 3000, "jab de perto");
assert.ok(a.ring().some((m) => m.a === "hit"), "empurrao do jab");
// Ana (Steve) nocauteia pelo "pv"
for (let i = 0; i < 3; i++) {
    a.pos([74, 22, 54], 0, [[b.id, 7.2, [1, 0, 0]]]);
    await sleep(160);
}
await until(() => a.snap().ph === "break", 3000, "nocaute -> intervalo");
assert.equal(a.snap().f.find((f) => f[0] === a.id)[3], 1, "round pra Ana");
// Terceiro pisa na lona no meio da luta: fila + teleporte pra fora
const c = await client("caio");
c.pos([75, 22, 55], 7);
await until(() => c.ring().some((m) => m.a === "tp"), 3000, "caio expulso da lona");
await until(() => a.snap().q.includes(c.n), 3000, "caio na fila");
// Bia cai fora (desconecta): W.O., Ana vira rei, Caio entra contra ela
b.ws.close();
await until(() => a.snap()?.king?.[0] === a.n, 5000, "W.O. -> rei");
assert.ok(a.got.some((m) => m.t === "chat" && m.n === "RINGUE" && m.m.includes("W.O.")), "anuncio no chat");
c.pos([100, 20, 100], 7);
await until(() => a.snap().ph === "count" && a.snap().f.some((f) => f[0] === c.id), 8000, "proximo da fila contra o rei");
assert.equal(a.snap().f[0][0], a.id, "rei fica no corner vermelho");
// Escudo: bloco no lote nao entra no log/eco
a.got.length = 0;
a.send({ t: "w", k: "set", p: [60, 20, 66], b: 3 });
a.send({ t: "w", k: "set", p: [40, 30, 30], b: 3 });
await sleep(800);
const echo = a.got.filter((m) => m.t === "w").map((m) => m.p.join(","));
assert.deepEqual(echo, ["40,30,30"], "set dentro do ringue bloqueado");
a.send({ t: "w", k: "set", p: [40, 30, 30], b: 0 });
c.send({ t: "chat", m: "/ringue sair" });
await sleep(500);
for (const x of [a, c]) x.ws.close();
console.log("verify_ring_ws OK");
process.exit(0);
