// Teste do server/tour.js com places/room/eco falsos: node tools/test_tour.mjs
import assert from "node:assert/strict";
import { Tour, IDS, PRIZE, TOUR_MS } from "../server/tour.js";
import { SPOTS } from "../server/layout.js";

const sent = [];
const bc = [];
const ledger = [];
const norm = (s) => String(s).toLowerCase();
const room = {
    clients: new Map(),
    send: (c, m) => sent.push({ to: c.name, m }),
    broadcast: (m) => bc.push(m),
    eco: {
        s: { tr: 1000, w: {} },
        wallet(n) {
            return (this.s.w[n] ??= { n, c: 100 });
        },
        entry: (...a) => ledger.push(a),
        sendMe: () => { },
    },
};
const saves = [];
const places = {
    room,
    save: (k) => saves.push(k),
    priv: (c, from, m) => room.send(c, { t: "chat", n: from, m }),
    say: (from, m) => room.broadcast({ t: "chat", n: from, m }),
    online: () => [...room.clients.values()].map((c) => norm(c.name)),
};
const t = new Tour(places, {});
clearInterval(t.iv);
const ana = { name: "ana" }, bob = { name: "bob" };
room.clients.set(1, ana).set(2, bob);
const chats = (who) => sent.filter((x) => x.to === who && x.m.t === "chat").map((x) => x.m.m);
const last = (who) => sent.filter((x) => x.to === who && x.m.k === "tour").pop()?.m;
const visit = (c, id) => (c.pos = [SPOTS[id][0] + 3, 20, SPOTS[id][1] - 3]);

// join: novato ganha dica + progresso vazio
t.join(ana);
assert.deepEqual(last("ana"), { t: "pl", k: "tour", on: false, left: [], done: [], sec: 0 });
assert.ok(/\/tour/.test(chats("ana")[0]));

// /tour comeca; comando alheio passa
assert.equal(t.command(1, ana, "saldo", []), false);
assert.equal(t.command(1, ana, "tour", []), true);
clearInterval(t.iv);
t.iv = null;
let p = last("ana");
assert.equal(p.on, true);
assert.deepEqual(p.left, IDS);
assert.ok(p.sec > 890 && p.sec <= 900);

// carimbos: longe nao conta, perto conta 1x, qualquer ordem
ana.pos = [160, 20, 160];
t.scan(Date.now());
assert.deepEqual(last("ana").done, []);
visit(ana, "bolsa");
t.scan(Date.now());
t.scan(Date.now());
assert.deepEqual(last("ana").done, ["bolsa"]);
assert.ok(chats("ana").includes("carimbo BOLSA 1/5"));
ana.pos = [SPOTS.tv[0] + 9, 20, SPOTS.tv[1]];
t.scan(Date.now());
assert.deepEqual(last("ana").done, ["bolsa"]);

// /tour com tour ativo mostra progresso, nao reinicia
sent.length = 0;
t.command(1, ana, "tour", []);
assert.ok(/1\/5 carimbos/.test(chats("ana")[0]));
assert.deepEqual(last("ana").done, ["bolsa"]);

// completa: paga 40 do cofre, ledger, chat global
for (const id of ["terminal", "tv", "banco", "congresso"]) {
    visit(ana, id);
    t.scan(Date.now());
}
assert.equal(room.eco.s.w.ana.c, 100 + PRIZE);
assert.equal(room.eco.s.tr, 1000 - PRIZE);
assert.deepEqual(ledger.pop(), ["ana", "completou o TOUR DOS PODERES", PRIZE, "premio do tour pago pelo cofre (1x por dia)"]);
assert.ok(bc.some((m) => m.t === "chat" && m.m === "ana completou o TOUR DOS PODERES"));
assert.equal(last("ana").on, false);
assert.equal(t.s.fin.ana, 1);

// join de quem ja fez: sem dica
sent.length = 0;
t.join(ana);
assert.equal(chats("ana").length, 0);

// limite diario: segundo tour no mesmo dia nao paga
t.command(1, ana, "tour", []);
clearInterval(t.iv);
t.iv = null;
for (const id of IDS) {
    visit(ana, id);
    t.scan(Date.now());
}
assert.equal(room.eco.s.w.ana.c, 100 + PRIZE);
assert.equal(t.s.fin.ana, 2);
assert.ok(chats("ana").some((m) => /ja foi pago/.test(m)));

// reserva: cofre 339 nao paga (ficaria 299)
room.eco.s.tr = 339;
t.command(2, bob, "tour", []);
clearInterval(t.iv);
t.iv = null;
for (const id of IDS) {
    visit(bob, id);
    t.scan(Date.now());
}
assert.equal(room.eco.s.w.bob, undefined);
assert.equal(room.eco.s.tr, 339);
assert.ok(chats("bob").some((m) => /sem premio/.test(m)));
// cofre 340: paga e fica em 300 (dia novo pro bob? nao: bob nunca recebeu)
room.eco.s.tr = 340;
t.command(2, bob, "tour", []);
clearInterval(t.iv);
t.iv = null;
for (const id of IDS) {
    visit(bob, id);
    t.scan(Date.now());
}
assert.equal(room.eco.s.w.bob.c, 100 + PRIZE);
assert.equal(room.eco.s.tr, 300);

// tempo esgota: tour some e avisa
t.command(2, bob, "tour", []);
clearInterval(t.iv);
t.iv = null;
t.s.act.bob.until = Date.now() - 1;
t.tick(Date.now());
assert.equal(t.s.act.bob, undefined);
assert.ok(/tempo esgotado/.test(chats("bob").pop()));

// virada do dia libera premio de novo
t.s.paid.ana = "2000-01-01";
t.tick(Date.now());
assert.equal(t.s.paid.ana, undefined);

// estado persistido volta (tour ativo sobrevive)
const t2 = new Tour(places, { act: { ana: { until: Date.now() + TOUR_MS, got: ["tv"] } } });
sent.length = 0;
t2.join(ana);
clearInterval(t2.iv);
assert.deepEqual(last("ana").done, ["tv"]);
assert.ok(saves.includes("tour"));
console.log("test_tour: ok");
