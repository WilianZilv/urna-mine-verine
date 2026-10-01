// Teste do server/terminal.js com places/room/eco falsos: node tools/test_terminal.mjs
import assert from "node:assert/strict";
import { Terminal, resolve, VILLAGE, PRIZE } from "../server/terminal.js";
import { Universe } from "../server/universe.js";

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
        me(n) {
            return this.wallet(n);
        },
        entry: (...a) => ledger.push(a),
        sendMe: () => { },
    },
};
const saves = [];
const says = [];
const places = { room, save: (k, s) => saves.push(k), priv: (c, from, m) => room.send(c, { t: "chat", n: from, m }), say: (from, m) => says.push(m) };
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
clearInterval(t.iv);
assert.deepEqual(sent.splice(0), [{ to: "ana", m: { t: "pl", k: "term", trips: {}, total: 0 } }, { to: "ana", m: { t: "pl", k: "term", stamps: [] } }]);

// /viajar cobra 5, cofre recebe, go so pra quem pediu
assert.equal(t.command(1, ana, "viajar", ["arena"]), true);
assert.equal(room.eco.s.w.ana.c, 7);
assert.equal(room.eco.s.tr, 105);
assert.deepEqual(ledger.pop(), ["ana", "viajou pra arena", 5, "passagem do terminal vai pro cofre da IA"]);
const go = sent.filter((x) => x.m.k === "go");
assert.deepEqual(go, [{ to: "ana", m: { t: "pl", k: "go", d: "arena" } }]);
assert.equal(t.s.trips.arena, 1);
// chegada vai pra todo mundo (feixe no destino)
assert.deepEqual(bc.splice(0), [{ t: "pl", k: "term", arr: { d: "arena", n: "ana" } }]);

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
// sem chegada pra destino desconhecido/saldo curto/throttle: lab (1) + bolsa (3)
assert.deepEqual(bc.splice(0).map((m) => m.arr), [{ d: "lab", n: "ana" }, { d: "bolsa", n: "bob" }, { d: "bolsa", n: "ana" }, { d: "bolsa", n: "bob" }]);

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

// ---------------------------------------------------------------- passaporte
// chegada pelo terminal (/viajar e portao) carimba, por nome normalizado
assert.deepEqual(t.stampsOf("ANA"), ["arena", "lab", "bolsa"]);
assert.deepEqual(t.stampsOf("bob"), ["bolsa"]);

const p = new Terminal(places, {});
sent.length = 0;
room.eco.s.tr = 1000;
room.hub = { snapshot: () => ({ portals: [{ id: "demo", name: "Chuva de Votos" }] }) };
const wallet0 = room.eco.wallet("ana").c;
// andar perto do SPOT carimba 1x; longe nao
ana.pos = [163, 20, 162];
bob.pos = [5, 20, 5];
p.scan();
p.scan();
assert.deepEqual(p.stampsOf("ana"), ["praca"]);
assert.deepEqual(p.stampsOf("bob"), []);
assert.deepEqual(sent.filter((x) => x.m.stamps).map((x) => [x.to, x.m.stamps]), [["ana", ["praca"]]]);
assert.ok(sent.some((x) => x.to === "ana" && /carimbo PRACA! vila 1\/10/.test(x.m.m)));
ana.pos = [163, 20, 170];
p.scan();
assert.deepEqual(p.stampsOf("ana"), ["praca"]);

// vila inteira: anuncio + premio 1x
for (const [sx, sz] of Object.values(VILLAGE)) {
    ana.pos = [sx + 3, 20, sz - 2];
    p.scan();
}
assert.equal(p.stampsOf("ana").length, 10);
assert.deepEqual(says, ["ana completou o PASSAPORTE DA VILA"]);
assert.equal(room.eco.wallet("ana").c, wallet0 + PRIZE);
assert.equal(room.eco.s.tr, 1000 - PRIZE);
assert.deepEqual(ledger.pop(), ["ana", "completou o PASSAPORTE DA VILA", PRIZE, "premio do passaporte pago pelo cofre (1x por jogador)"]);
p.prize("ana");
p.scan();
sent.length = 0;
assert.equal(p.command(1, ana, "passaporte", []), true);
assert.equal(room.eco.wallet("ana").c, wallet0 + PRIZE);
assert.match(sent[0].m.m, /^10\/11 carimbos \| vila 10\/10 COMPLETA \| portais do hub 0\/1 \(faltam Chuva de Votos\)$/);
assert.equal(sent[1].m.stamps.length, 10);

// portal do Hub: carimbo hub:<id>, id torto ignorado
p.portal(ana, "demo");
p.portal(ana, "../x");
p.portal({ name: "" }, "demo");
assert.ok(p.stampsOf("ana").includes("hub:demo") && p.stampsOf("ana").length === 11);
sent.length = 0;
p.command(1, ana, "passaporte", []);
assert.match(sent[0].m.m, /^11\/11 carimbos/);

// cofre na reserva: anuncia mas premio fica pendente; paga depois no /passaporte, 1x
room.eco.s.tr = 310;
const bob0 = room.eco.wallet("bob").c;
for (const id of Object.keys(VILLAGE)) p.stamp("Bob", id);
assert.equal(says.at(-1), "Bob completou o PASSAPORTE DA VILA");
assert.equal(room.eco.wallet("bob").c, bob0);
assert.equal(room.eco.s.tr, 310);
sent.length = 0;
p.command(2, bob, "passaporte", []);
assert.match(sent[0].m.m, /^10\/11 carimbos \| vila 10\/10 COMPLETA/);
assert.ok(sent.some((x) => /pendente/.test(x.m.m || "")));
room.eco.s.tr = 1000;
p.command(2, bob, "passaporte", []);
p.command(2, bob, "passaporte", []);
assert.equal(room.eco.wallet("bob").c, bob0 + PRIZE);
assert.equal(room.eco.s.tr, 1000 - PRIZE);

// faltando: lista nomes
const q = new Terminal(places, {});
q.stamp("ana", "tv");
sent.length = 0;
q.passport(ana);
assert.match(sent[0].m.m, /^1\/11 carimbos \| vila 1\/10 \(faltam PRACA, ARENA, CLUB, LAB, GAME HUB, CONGRESSO, BOLSA, BANCO CENTRAL, TERMINAL\)/);

// persistencia: estado salvo volta igual (carimbos + premio pago)
const p2 = new Terminal(places, JSON.parse(JSON.stringify(p.s)));
assert.deepEqual(p2.stampsOf("ana"), p.stampsOf("ana"));
const tr = room.eco.s.tr;
p2.prize("ana");
p2.prize("bob");
assert.equal(room.eco.s.tr, tr);

// /api/passport ganha "stamps"; entrar num portal (onSession) carimba
room.places = { terminal: p };
room.ctx = { storage: { get: async () => undefined, put: async () => { } }, blockConcurrencyWhile: (f) => f() };
room.mods = { loadPkg: async () => ({}), list: () => [], active: new Map() };
const exp = Math.floor(Date.now() / 1000) + 600;
room.hub.check = async (tok) => (tok === "ok" ? { sub: "Ana", pid: "demo", sid: "s1", exp, player: { character: "steve", color: "#ffffff" } } : null);
room.hub.s = { p: {} };
const uni = new Universe(room);
const r = await uni.passport("ok");
const j = await r.json();
assert.equal(r.status, 200);
assert.deepEqual(j.stamps, p.stampsOf("ana"));
assert.equal((await uni.passport("bad")).status, 401);
const carl = { name: "carl" };
await uni.onSession(carl, { sub: "carl", pid: "jogo-x", sid: "s2" });
assert.deepEqual(p.stampsOf("carl"), ["hub:jogo-x"]);
console.log("test_terminal: ok");
