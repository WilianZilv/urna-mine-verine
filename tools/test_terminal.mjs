// Teste do server/terminal.js com places/room/eco falsos: node tools/test_terminal.mjs
import assert from "node:assert/strict";
import { Terminal, resolve } from "../server/terminal.js";

const sent = [];
const bc = [];
const ledger = [];
const room = {
    clients: new Map(),
    send: (c, m) => sent.push({ to: c.name, m }),
    broadcast: (m) => bc.push(m),
    eco: {
        s: { tr: 100, w: {} },
        wallet(n) {
            return (this.s.w[n] ??= { n, c: 12 });
        },
        entry: (...a) => ledger.push(a),
        sendMe: () => { },
    },
};
const saves = [];
const places = { room, save: (k, s) => saves.push(k), priv: (c, from, m) => room.send(c, { t: "chat", n: from, m }) };
const t = new Terminal(places, {});
const ana = { name: "ana" }, bob = { name: "bob" };
room.clients.set(1, ana).set(2, bob);

// resolve aceita nome solto
assert.equal(resolve("Praça"), "praca");
assert.equal(resolve("game hub"), "hub");
assert.equal(resolve("TV URNA NEWS"), "tv");
assert.equal(resolve("congr"), "congresso");
assert.equal(resolve("xyz"), null);
assert.equal(resolve("__proto__"), null);

t.join(ana);
assert.deepEqual(sent.pop(), { to: "ana", m: { t: "pl", k: "term", trips: {}, total: 0 } });

// /viajar cobra 5, cofre recebe, go so pra quem pediu
assert.equal(t.command(1, ana, "viajar", ["arena"]), true);
assert.equal(room.eco.s.w.ana.c, 7);
assert.equal(room.eco.s.tr, 105);
assert.deepEqual(ledger.pop(), ["ana", "viajou pra arena", 5, "passagem do terminal vai pro cofre da IA"]);
const go = sent.filter((x) => x.m.k === "go");
assert.deepEqual(go, [{ to: "ana", m: { t: "pl", k: "go", d: "arena" } }]);
assert.equal(t.s.trips.arena, 1);

// destino desconhecido: nada cobrado
sent.length = 0;
assert.equal(t.command(1, ana, "viajar", ["marte"]), true);
assert.equal(room.eco.s.w.ana.c, 7);
assert.ok(sent.every((x) => x.m.t === "chat") && /desconhecido/.test(sent[0].m.m));

// saldo insuficiente
t.command(1, ana, "viajar", ["lab"]);
sent.length = 0;
t.command(1, ana, "viajar", ["lab"]);
assert.equal(room.eco.s.w.ana.c, 2);
assert.ok(/passagem custa 5, tens 2/.test(sent[0].m.m) && !sent.some((x) => x.m.k === "go"));

// /destinos e comando alheio
assert.equal(t.command(2, bob, "destinos", []), true);
assert.equal(t.command(2, bob, "saldo", []), false);

// portao: conta, throttle 1,5 s por conexao, ignora lixo
t.onMsg(2, bob, { t: "pl", k: "term_trip", d: "bolsa" });
t.onMsg(2, bob, { t: "pl", k: "term_trip", d: "bolsa" });
t.onMsg(1, ana, { t: "pl", k: "term_trip", d: "bolsa" });
t.onMsg(1, ana, { t: "pl", k: "term_trip", d: "__proto__" });
t.onMsg(1, ana, { t: "pl", k: "lei", d: "bolsa" });
assert.equal(t.s.trips.bolsa, 2);
t.last.set(2, Date.now() - 2000);
t.onMsg(2, bob, { t: "pl", k: "term_trip", d: "bolsa" });
assert.equal(t.s.trips.bolsa, 3);
assert.equal(t.s.total, 5);

// tick: broadcast so se mudou
t.tick(Date.now(), true);
assert.deepEqual(bc.pop(), { t: "pl", k: "term", trips: { arena: 1, lab: 1, bolsa: 3 }, total: 5 });
t.tick(Date.now(), true);
assert.equal(bc.length, 0);

// virada do dia zera contagem do dia, total fica
t.s.day = "2000-01-01";
t.tick(Date.now(), true);
assert.deepEqual(bc.pop(), { t: "pl", k: "term", trips: {}, total: 5 });
assert.ok(saves.includes("terminal"));
console.log("test_terminal: ok");
