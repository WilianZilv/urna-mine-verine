// Teste do server/banco.js com places/room/eco falsos: node tools/test_banco.mjs
import assert from "node:assert/strict";
import { Banco } from "../server/banco.js";

const HOUR = 3600 * 1000;
const chat = [], sent = [];
const norm = (s) => String(s).toLowerCase();
const eco = {
    s: { tr: 1000, w: {}, led: [] },
    wallet(n) {
        return (this.s.w[norm(n)] ??= { n, c: 100, at: 0 });
    },
    entry(who, what, amt, why) {
        this.s.led.push({ who, what, amt, why, tr: this.s.tr });
    },
    sendMe() { },
};
const stocks = { ana: 500 };
const room = { eco, send: (c, m) => sent.push(m), broadcast: (m) => sent.push(m) };
const pl = { room, bolsa: { value: (k) => stocks[k] || 0 }, save() { }, say() { }, priv: (c, from, m) => chat.push(m) };
const b = new Banco(pl, {});
const bob = { name: "Bob" }, ana = { name: "Ana" };
const total = () => eco.s.tr + Object.values(eco.s.w).reduce((s, w) => s + w.c, 0) + Object.values(b.s.sv).reduce((s, a) => s + a.c, 0);
const last = () => chat[chat.length - 1];

eco.wallet("Bob");
eco.wallet("Ana");
const t0 = total();

// poupar / sacar conservam moeda
assert.equal(b.command(1, bob, "poupar", ["50"]), true);
assert.equal(eco.s.w.bob.c, 50);
assert.equal(b.s.sv.bob.c, 50);
assert.equal(total(), t0);
b.command(1, bob, "poupar", ["999"]);
assert.match(last(), /uso/);
assert.equal(total(), t0);
b.command(1, bob, "sacar", ["20"]);
assert.equal(b.s.sv.bob.c, 30);
assert.equal(eco.s.w.bob.c, 70);
b.command(1, bob, "sacar", ["tudo"]);
assert.equal(b.s.sv.bob, undefined);
assert.equal(eco.s.w.bob.c, 100);
assert.equal(total(), t0);
assert.equal(b.command(1, bob, "banco", []), false);

// juros: 2%/h simples, do cofre, limitado a 100
b.command(1, bob, "poupar", ["100"]);
b.s.sv.bob.at -= 10 * HOUR; // 100 * 0.02 * 10 = 20
b.command(1, bob, "sacar", ["1"]);
assert.equal(b.s.sv.bob.c, 119);
assert.equal(eco.s.tr, 980);
assert.equal(total(), t0);
b.s.sv.bob.c = 10000;
const before = total();
b.s.sv.bob.at -= 100 * HOUR; // devia 20000, cap 100
b.command(1, bob, "sacar", ["1"]);
assert.equal(b.s.sv.bob.c, 10000 + 100 - 1);
assert.equal(total(), before);

// cofre na reserva: paga parcial, depois zero
eco.s.tr = 340;
b.s.sv.bob.at -= 100 * HOUR;
b.command(1, bob, "sacar", ["1"]);
assert.equal(eco.s.tr, 300);
assert.match(last(), /reserva/);
b.s.sv.bob.at -= 100 * HOUR;
const sv = b.s.sv.bob.c;
b.command(1, bob, "sacar", ["1"]);
assert.equal(eco.s.tr, 300);
assert.equal(b.s.sv.bob.c, sv - 1);
assert.match(last(), /NAO pagos/);

// ranking: carteira + poupanca + acoes (bolsa)
b.command(1, bob, "sacar", ["tudo"]);
eco.s.w.bob.c = 300;
b.command(1, bob, "poupar", ["250"]); // bob 300
b.command(1, ana, "ranking", []); // ana 100 + 500 acoes = 600
assert.match(last(), /1\. Ana 600 \| 2\. Bob 300/);
assert.match(last(), /tu: #1/);
const rich = b.snap().rich;
assert.deepEqual(rich[0], ["Ana", 600]);
pl.bolsa = undefined; // bolsa ausente: acoes 0
assert.deepEqual(b.snap().rich[0], ["Bob", 300]);

// caixa eletronico + cooldown
b.onMsg(1, bob, { k: "banco_atm" });
assert.match(last(), /carteira 50 \| poupanca 250 .*acoes 0 .*patrimonio 300 .*#1 de 2/);
const n = chat.length;
b.onMsg(1, bob, { k: "banco_atm" });
assert.equal(chat.length, n);
b.onMsg(1, ana, { k: "outra" });
assert.equal(chat.length, n);

// snapshot so muda com hash
sent.length = 0;
b.tick(0, true);
b.tick(0, true);
assert.equal(sent.length, 1);
const s = sent[0];
assert.equal(s.k, "banco");
assert.equal(s.nsv, 1);
assert.equal(s.sv, 250);
assert.equal(s.rate, 2);
assert.equal(s.led.length, 6);

// magnata: primeiro #1 nao anuncia; troca anuncia; cooldown de 60 s
const said = [];
pl.say = (from, m) => said.push(`${from}: ${m}`);
const b2 = new Banco(pl, {});
eco.s.w = {};
eco.wallet("Bob").c = 500;
eco.wallet("Ana").c = 100;
b2.push(true);
assert.equal(said.length, 0);
eco.s.w.ana.c = 900;
b2.push();
assert.deepEqual(said, ["BANCO: NOVO MAGNATA DA VILA: Ana"]);
b2.push();
assert.equal(said.length, 1);
eco.s.w.bob.c = 2000; // troca dentro do cooldown: segura
b2.push();
assert.equal(said.length, 1);
b2.topAt -= 61 * 1000; // passou 1 min: anuncia quem ta no topo agora
b2.push();
assert.equal(said[1], "BANCO: NOVO MAGNATA DA VILA: Bob");
eco.s.w.bob.c = 0;
eco.s.w.ana.c = 0;
b2.topAt -= 61 * 1000;
b2.push(); // empate 0 x 0: Ana por nome
assert.equal(said[2], "BANCO: NOVO MAGNATA DA VILA: Ana");
console.log("test_banco OK");
