// Teste offline da Bolsa (server/bolsa.js) com places/room/eco falsos: node tools/test_bolsa.mjs
import assert from "node:assert/strict";
import { Bolsa } from "../server/bolsa.js";

const chat = [], sent = [];
const eco = {
    s: { tr: 1000, w: {} },
    wallet(name) {
        const k = name.toLowerCase();
        return (this.s.w[k] ??= { n: name, c: 100, at: 0 });
    },
    entry(who, what, amt) {
        chat.push(`[ledger] ${who}: ${what} ${amt}`);
    },
    sendMe() { },
};
const room = { eco, clients: new Map(), broadcast: (m) => sent.push(m), send: (c, m) => sent.push(m) };
const pl = { room, save() { }, priv: (c, from, m) => chat.push(`${from}: ${m}`) };
const b = new Bolsa(pl, {});
const ana = { name: "Ana", pos: [60, 20, 160] };
room.clients.set(1, ana);
const total = () => eco.s.tr + Object.values(eco.s.w).reduce((s, w) => s + w.c, 0);
const say = (t) => {
    const [head, ...args] = t.split(/\s+/);
    return b.command(1, ana, head.toLowerCase(), args);
};

eco.wallet("Ana");
const t0 = total();

// compra: carteira paga, cofre recebe (100 * 3 + taxa 3 = 303 > 100 moedas -> recusa)
assert.equal(say("investir LULA 3"), true);
assert.match(chat.at(-1), /custam 303/);
eco.s.w.ana.c = 1000;
const t1 = total();
say("investir LULA 3");
assert.equal(eco.s.w.ana.c, 1000 - 303);
assert.equal(eco.s.tr, 1303);
assert.equal(b.s.pos.ana.LULA, 3);
assert.equal(b.value("Ana"), 300);
assert.equal(total(), t1);
say("investir flavio 2");
assert.equal(b.s.pos.ana.FLAV, 2);
assert.equal(say("comprar LULA 1"), false);
say("investir LULA 600");
assert.match(chat.at(-1), /uso/);

// venda paga do cofre menos taxa
say("vender LULA 1");
assert.equal(b.s.pos.ana.LULA, 2);
assert.equal(eco.s.w.ana.c, 1000 - 303 - 202 + 99);
assert.equal(total(), t1);

// cofre sem liquidez
const tr = eco.s.tr;
eco.s.tr = 350;
const t2 = total();
say("vender LULA 1");
assert.match(chat.at(-1), /cofre sem liquidez/);
assert.equal(b.s.pos.ana.LULA, 2);
assert.equal(total(), t2);
eco.s.tr = tr;

// golpes derrubam so o ticker certo (sigma desligado via Math.random fixo)
const rnd = Math.random;
Math.random = () => 0.25; // cos(pi/2) = 0 -> ruido zero
b.s.p.LULA = b.s.p.RENA = 100;
b.imp.LULA.trade = b.imp.FLAV.trade = 0;
for (let i = 0; i < 10; i++) b.onHit(ana, { k: "pf", g: 0, i: 2, dmg: 10 });
for (let i = 0; i < 4; i++) b.onHit(ana, { k: "hit", g: 9, i: 0, dmg: 10 });
b.tick(Date.now(), true);
Math.random = rnd;
assert.equal(b.s.p.RENA, 97.5);
assert.equal(b.s.p.LULA, 100);
assert.equal(b.s.p.GODZ, 99);
assert.equal(b.s.why.RENA, "apanhou na arena");
assert.ok(b.s.p.HOUSE > 100, "club com gente sobe");
assert.match(b.s.why.HOUSE, /casa cheia/);
assert.equal(b.value("ana"), Math.round(2 * b.s.p.LULA + 2 * b.s.p.FLAV));

// snapshot
const snap = sent.at(-1);
assert.equal(snap.k, "bolsa");
assert.equal(snap.tk.length, 8);
assert.deepEqual(snap.top[0], ["Ana", b.value("ana")]);
assert.ok(snap.hot.startsWith("ANALISTA"));
const before = sent.length;
b.push();
assert.equal(sent.length, before, "sem mudanca, sem broadcast");

// nunca cria moeda: tudo junto so muda por compra/venda (taxa fica no cofre)
for (let i = 0; i < 50; i++) {
    say(`investir ${["LULA", "URNA", "COFRE"][i % 3]} ${1 + (i % 4)}`);
    b.tick(Date.now(), true);
    say(`vender ${["LULA", "URNA", "COFRE"][(i + 1) % 3]} ${1 + (i % 3)}`);
}
assert.equal(total(), t1);
assert.ok(Object.values(b.s.p).every((p) => p >= 5 && p <= 1000));
assert.ok(Object.values(b.s.h).every((h) => h.length <= 48));
say("carteira");
console.log(chat.slice(-4).join("\n"));
console.log(snap.hot);
console.log("OK test_bolsa");
