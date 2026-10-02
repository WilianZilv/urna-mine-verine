// Teste da maquina de estados do server/ring.js com places/room falsos: node tools/test_ring.mjs
import assert from "node:assert/strict";
import { Ring, CORNERS, OUT, HP, ROUND_MS, COUNT_MS, BREAK_MS, END_MS, OUT_MS, JAB, PRIZE, inLona } from "../server/ring.js";
import { RINGUE, LANDMARKS, shielded, blocked } from "../server/layout.js";

const sent = [], bc = [], said = [], priv = [];
const eco = { s: { tr: 10000, w: {} }, wallet(n) { return (this.s.w[n] ??= { c: 0 }); }, entry() { }, sendMe() { } };
const room = { clients: new Map(), eco, send: (c, m) => sent.push([c.id, m]), broadcast: (m) => bc.push(m) };
const pl = { room, save() { }, say: (f, m) => said.push(m), priv: (c, f, m) => priv.push([c.id, m]) };
const st = {};
const r = new Ring(pl, st);
const IN = (dx = 0, dz = 0) => [74 + dx, 22, 54 + dz];
const add = (id, name, pos, ch = 0) => room.clients.set(id, { id, name, pos, ch });
const mine = (id) => sent.filter(([i]) => i === id).map(([, m]) => m);
const unarm = () => {
    clearInterval(r.iv);
    r.iv = null;
};
let now = 1_000_000;
const run = (ms) => {
    for (const end = now + ms; now < end;) {
        now += 250;
        r.step(now);
    }
};

// planta: lona dentro do lote; lote protegido (IA e jogadores)
assert.ok(inLona(IN()) && !inLona([74, 20, 54]) && !inLona(OUT));
assert.ok(inLona(CORNERS[0]) && inLona(CORNERS[1]));
assert.ok(LANDMARKS.some(([x0, x1, z0, z1]) => x0 === RINGUE.x0 - 2 && x1 === RINGUE.x1 + 2 && z0 === RINGUE.z0 - 2 && z1 === RINGUE.z1 + 2));
assert.ok(shielded([74, 21, 54]) && shielded([56, 25, 70]) && !shielded([55, 21, 54]));
assert.ok(blocked([91, 20, 80], [91, 22, 80])); // trilha

// 1 pessoa na lona: fila, sem luta
add(1, "Ana", IN());
add(2, "Bia", [100, 20, 100]);
run(250);
assert.equal(r.ph, "idle");
assert.deepEqual(r.q.map((x) => x.name), ["Ana"]);
// /ringue de longe entra na fila e comeca a contagem com teleporte pros corners + cura
assert.equal(r.command(2, room.clients.get(2), "ringue", []), true);
unarm();
assert.match(priv.at(-1)[1], /fila #2/);
run(250);
assert.equal(r.ph, "count");
assert.deepEqual(r.f.map((x) => x.name), ["Ana", "Bia"]);
assert.deepEqual(mine(2).find((m) => m.a === "tp"), { t: "pl", k: "ringue", a: "tp", p: CORNERS[1], heal: 1 });
room.clients.get(1).pos = CORNERS[0];
room.clients.get(2).pos = CORNERS[1];
assert.ok(mine(1).some((m) => m.a === "msg" && /ROUND 1 {2}- {2}3/.test(m.m)));
// dano antes do LUTA nao conta
r.onPos(1, room.clients.get(1), { c: 0, pv: [[2, 7.2, [0, 0, 1]]] }, now);
assert.equal(r.f[1].hp, HP);
run(COUNT_MS);
assert.equal(r.ph, "fight");
assert.ok(mine(2).some((m) => m.m === "ROUND 1: LUTA!"));

// round 1: nocaute pelo "pv" do Steve (x6, teto por mensagem); pv pra outro id nao conta
r.onPos(1, room.clients.get(1), { c: 0, pv: [[2, 7.2, [0, 0, 1]], [9, 5, [0, 0, 1]]] }, now);
assert.equal(Math.round(r.f[1].hp), HP - 43);
r.onPos(1, room.clients.get(1), { c: 0, pv: [[2, 99, [0, 0, 1]]] }, now);
assert.equal(r.f[1].hp, 0);
assert.equal(r.f[0].rw, 1);
assert.equal(r.ph, "break");
run(BREAK_MS);
assert.equal(r.ph, "count");
assert.equal(r.round, 2);
assert.deepEqual(r.f.map((x) => x.hp), [HP, HP]);

// round 2: jab (personagem sem PvP proprio), com distancia e intervalo validados; Steve nao usa jab
run(COUNT_MS);
room.clients.get(2).ch = 6;
room.clients.get(1).pos = IN(0, 0);
room.clients.get(2).pos = IN(5, 0);
r.onMsg(2, room.clients.get(2), { k: "ringue", a: "soco" }, now);
assert.equal(r.f[0].hp, HP); // longe
room.clients.get(2).pos = IN(2, 0);
r.onMsg(2, room.clients.get(2), { k: "ringue", a: "soco" }, now);
r.onMsg(2, room.clients.get(2), { k: "ringue", a: "soco" }, now + 100); // cooldown
assert.equal(r.f[0].hp, HP - JAB);
assert.deepEqual(mine(1).at(-1), { t: "pl", k: "ringue", a: "hit", d: [-1, 0] });
r.onMsg(1, room.clients.get(1), { k: "ringue", a: "soco" }, now + 1000);
assert.equal(r.f[1].hp, HP); // Steve (ch 0) bate pelo pv
// tempo acaba: quem tem mais vida (= deu mais dano) leva
run(ROUND_MS);
assert.equal(r.f[1].rw, 1);
assert.equal(r.ph, "break");

// round 3: sair das cordas por OUT_MS = perde o round -> Bia vence 2-1 e vira rei
run(BREAK_MS + COUNT_MS);
assert.equal(r.ph, "fight");
room.clients.get(1).pos = [74, 20, 64];
run(OUT_MS - 500);
assert.equal(r.ph, "fight");
room.clients.get(1).pos = IN();
run(500);
room.clients.get(1).pos = [74, 20, 64];
run(OUT_MS + 250);
assert.equal(r.ph, "end");
assert.deepEqual(r.king, { id: 2, name: "Bia", streak: 1 });
assert.match(said.at(-1), /Bia venceu Ana por 2-1 \(SAIU DO RINGUE\)\. sequencia: 1/);
assert.equal(eco.s.w.Bia.c, PRIZE);
assert.deepEqual(st.last[0], ["Bia", "Ana", "2-1"]);
assert.deepEqual(st.top.bia, { n: "Bia", w: 1, b: 1 });

// outro jogador pisa na lona durante a luta: fila + teleporte pra fora
add(3, "Caio", IN(1, 1), 7);
run(250);
assert.deepEqual(mine(3).at(-1), { t: "pl", k: "ringue", a: "tp", p: OUT, heal: 0 });
assert.deepEqual(r.q.map((x) => x.name), ["Caio"]);
room.clients.get(3).pos = OUT;
// fim: perdedor sai, rei fica, proximo da fila entra
run(END_MS);
assert.ok(mine(1).some((m) => m.a === "tp" && m.p === OUT));
assert.equal(r.ph, "count");
assert.deepEqual(r.f.map((x) => x.name), ["Bia", "Caio"]);
const snap = bc.at(-1);
assert.equal(snap.k, "ringue");
assert.deepEqual(snap.f.map((x) => x.slice(1)), [["Bia", HP, 0], ["Caio", HP, 0]]);
assert.deepEqual(snap.king, ["Bia", 1]);
assert.ok(snap.left > 0 && snap.left <= COUNT_MS);

// melhor de 3 por nocaute 2-0; sequencia do rei sobe
for (let k = 0; k < 2; k++) {
    run(COUNT_MS + 250);
    assert.equal(r.ph, "fight");
    room.clients.get(1).pos = [100, 20, 100];
    room.clients.get(2).pos = IN();
    room.clients.get(3).pos = IN(1, 0);
    for (let i = 0; i < 9; i++) r.onPos(2, room.clients.get(2), { c: 0, pv: [[3, 2.7, [0, 0, 1]]] }, now);
    if (k === 0) run(BREAK_MS);
}
assert.equal(r.ph, "end");
assert.deepEqual(r.king, { id: 2, name: "Bia", streak: 2 });
assert.deepEqual(st.top.bia, { n: "Bia", w: 2, b: 2 });
run(END_MS);
assert.equal(r.ph, "idle");

// W.O. por desconexao no meio da luta; rei que sai perde a coroa
r.command(3, room.clients.get(3), "ring", []);
unarm();
run(250);
assert.equal(r.ph, "count");
room.clients.delete(2);
r.leave(2, now);
assert.equal(r.ph, "end");
assert.equal(r.king.name, "Caio");
assert.match(said.at(-1), /Caio venceu Bia.*W\.O\./);
// /ringue sair: sai da fila
r.command(1, room.clients.get(1), "ringue", []);
unarm();
assert.deepEqual(r.q.map((x) => x.name), ["Ana"]);
r.command(1, room.clients.get(1), "ringue", ["sair"]);
assert.equal(r.q.length, 0);
assert.equal(r.command(1, room.clients.get(1), "banco", []), false);
// premio limitado por dia
for (let i = 0; i < 20; i++) r.prize("Caio", now);
assert.equal(eco.s.w.Caio.c, PRIZE * 10);
// sala vazia: zera luta e para o relogio
room.clients.clear();
r.step(now);
assert.equal(r.ph, "idle");
assert.equal(r.king, null);
for (const m of priv) assert.ok(m[1].length <= 300 && /^[\x20-\x7e]+$/.test(m[1]), "chat ASCII ate 300");
console.log("test_ring OK");
