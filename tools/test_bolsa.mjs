// Teste offline da Bolsa (server/bolsa.js) com places/room/eco falsos: node tools/test_bolsa.mjs
import assert from "node:assert/strict";
import { Bolsa, modSymbol } from "../server/bolsa.js";
const r2 = (v) => Math.round(v * 100) / 100;

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
const pl = { room, save() { }, priv: (c, from, m) => chat.push(`${from}: ${m}`), say: (from, m) => chat.push(`ALL ${from}: ${m}`) };
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
// humor do mercado: indice = media das variacoes; tombo >= 8% num tick dispara o circuit breaker
Math.random = () => 0.25;
assert.equal(snap.mood, 0);
assert.equal(snap.alert, "");
assert.equal(typeof snap.ix, "number");
const yell = () => chat.filter((l) => l.startsWith("ALL BOLSA")).length;
b.imp.LULA.hit = -0.06;
b.imp.LULA.trade = -0.05;
b.tick(Date.now(), true);
let s2 = sent.at(-1);
assert.equal(s2.mood, -1);
assert.match(s2.alert, /CIRCUIT BREAKER: LULA -1\d\.\d%/);
assert.equal(yell(), 1);
assert.match(chat.filter((l) => l.startsWith("ALL")).at(-1), /CIRCUIT BREAKER/);
assert.equal(s2.ix, b.index());
const fresh = new Bolsa({ ...pl, room: { eco: { s: { tr: 1000 } }, clients: new Map(), broadcast() { } }, say() { } }, {});
assert.equal(fresh.index(), 0);
fresh.imp.LULA.hit = -0.06;
fresh.tick(Date.now(), true);
assert.ok(fresh.index() < 0, "tombo puxa o indice pra baixo");
// cofre gordo puxa COFRE devagar (drift limitado), sem alarme falso
fresh.pl.room.eco.s.tr = 50000;
fresh.alert.until = 0;
fresh.tick(Date.now(), true);
assert.ok(fresh.s.p.COFRE > 100 && fresh.s.p.COFRE < 104);
assert.equal(fresh.snapshot().mood, 0);
// rali logo depois: painel troca pra +1, mas o chat respeita o rate limit
b.imp.HOUSE.trade = 0.05; // club cheio (+5%) + compra (+5%)
for (let i = 2; i <= 5; i++) room.clients.set(i, { name: `dj${i}`, pos: ana.pos });
b.tick(Date.now(), true);
for (let i = 2; i <= 5; i++) room.clients.delete(i);
s2 = sent.at(-1);
assert.equal(s2.mood, 1);
assert.match(s2.alert, /DISPAROU: HOUSE \+\d+\.\d%/);
assert.equal(yell(), 1, "rate limit no chat");
// alerta expira
b.alert.until = Date.now() - 1;
assert.equal(b.snapshot().mood, 0);
b.said = 0;
b.imp.RENA.hit = -0.06;
b.imp.RENA.trade = -0.05;
b.tick(Date.now(), true);
assert.equal(yell(), 2);
// tick calmo nao dispara
b.alert.until = 0;
b.s.p.COFRE = eco.s.tr / 10;
b.tick(Date.now(), true);
assert.equal(sent.at(-1).mood, 0);
Math.random = rnd;
console.log(chat.filter((l) => l.startsWith("ALL")).join("\n"));

say("carteira");
console.log(chat.slice(-4).join("\n"));
console.log(snap.hot);

// ------------------------------------------------ acoes de mods
assert.equal(modSymbol("King Kong", new Set()), "KING");
assert.equal(modSymbol("King Kong", new Set(["KING"])), "KINGK");
assert.equal(modSymbol("King", new Set(["KING"])), "KIN2");
assert.equal(modSymbol("Lula Robo", new Set(["LULA", "LULAR"])), "LUL2");
assert.equal(modSymbol("Ox!", new Set()), "OXMO");
assert.equal(modSymbol("Ção Ébrio", new Set()), "CAOE");

const npc = (id, name, at, extra = {}) => ({ id, v: "1.0.0", creator: "bot", at, pkg: { manifest: { name }, behavior: { stats: { hp: 100 }, spawn: { max_instances: extra.inst ?? 1 } } } });
const active = [];
room.mods = { list: (k) => (k === "npc" ? [...active].sort((a, b) => a.at - b.at) : []) };
const T0 = Date.now();
Math.random = () => 0.25;
active.push(npc("king-kong", "King Kong", T0 - 1000, { inst: 2 }), npc("kingzao", "Kingzao", T0 - 500), npc("lula-robo", "Lula Robo", T0));
b.tick(T0, true);
assert.deepEqual(Object.keys(b.s.m).sort(), ["KING", "KINGZ", "LULAR"]);
assert.equal(b.s.m.KING.n, "KING KONG");
assert.ok(chat.some((l) => /ALL BOLSA: KING KONG \(KING\) estreou na bolsa a 50\.00/.test(l)));
let ms = sent.at(-1);
assert.equal(ms.tk.length, 8, "tickers fixos intactos");
assert.equal(ms.mk.length, 3);
assert.equal(ms.mk[0].c, "bot");
const syms = [...TK_SYMS(), ...ms.mk.map((t) => t.s)];
assert.equal(new Set(syms).size, syms.length, "simbolos unicos");
function TK_SYMS() { return ms.tk.map((t) => t.s); }

// impulsos: porrada (g 8, indice achatado) sobe so o mod certo; dano >= hp conta abate
assert.equal(b.s.p.KING, 50);
for (let i = 0; i < 10; i++) b.onHit(ana, { k: "hit", g: 8, i: 1, dmg: 50 }); // 2a instancia do King Kong (hp 100): 5 abates
b.onHit(ana, { k: "hit", g: 8, i: 2, dmg: 5 }); // Kingzao
b.onHit(ana, { k: "hit", g: 8, i: 99, dmg: 5 }); // fora da lista: ignora
b.tick(T0 + 20000, true);
assert.equal(b.s.p.KING, r2(50 * 1.02 * 1.08 * Math.exp(0.003)));
assert.equal(b.s.why.KING, "abatido na arena (viralizou)");
assert.equal(b.s.p.KINGZ, r2(50 * 1.002 * Math.exp(0.003)));
assert.equal(b.s.p.LULAR, r2(50 * Math.exp(0.003)));
assert.equal(b.s.why.LULAR, "hype de lancamento");
assert.ok(b.s.pop["king-kong"] > b.s.pop.kingzao);
// esquecido depois do hype: cai devagar
b.s.m.LULAR.at = T0 - 2 * 3600000;
b.s.m.LULAR.idle = 20;
const pl2 = b.s.p.LULAR;
b.tick(T0 + 40000, true);
assert.ok(b.s.p.LULAR < pl2);
assert.equal(b.s.why.LULAR, "esquecido no canto da zona");
// versao nova da um empurrao
active[0].v = "1.1.0";
b.tick(T0 + 60000, true);
assert.equal(b.s.m.KING.v, "1.1.0");
assert.equal(b.s.why.KING, "versao nova (v1.1.0)");

// compra de acao de mod e /bolsa mods
eco.s.w.ana.c = 5000;
const t3 = total();
say("investir KING 10");
assert.equal(b.s.pos.ana.KING, 10);
say("investir kingzao 4");
assert.equal(b.s.pos.ana.KINGZ, 4);
assert.equal(total(), t3);
say("bolsa mods");
assert.match(chat.at(-1), /^BOLSA: MODS: KING \d+\.\d\d .*KING KONG por bot/);
say("bolsa");
assert.match(chat.at(-1), /3 mods: \/bolsa mods/);

// deslistagem: mod desativado paga o ultimo preco do cofre
const pK = b.s.p.KING;
const w0 = eco.s.w.ana.c, tr0 = eco.s.tr;
active.splice(0, 1);
b.tick(T0 + 80000, true);
assert.equal(b.s.m.KING, undefined);
assert.equal(b.s.p.KING, undefined);
assert.equal(b.s.pos.ana.KING, undefined);
assert.equal(eco.s.w.ana.c, w0 + Math.round(pK * 10));
assert.equal(eco.s.tr, tr0 - Math.round(pK * 10));
assert.equal(total(), t3);
assert.match(chat.filter((l) => l.startsWith("ALL")).at(-1), /KING KONG \(KING\) saiu da bolsa \(mod desativado\)/);
assert.equal(sent.at(-1).mk.length, 2);
assert.equal(b.value("ana"), Math.round(Object.entries(b.s.pos.ana).reduce((v, [S, n]) => v + b.s.p[S] * n, 0)));

// cofre curto: paga o que da acima da reserva e registra
const pZ = b.s.p.KINGZ;
eco.s.tr = 300 + 50;
const t4 = total();
active.splice(0, 1);
b.tick(T0 + 100000, true);
assert.equal(b.s.pos.ana?.KINGZ, undefined);
assert.equal(eco.s.tr, 300 + 50 - Math.floor(Math.round(pZ * 4) * Math.min(1, 50 / Math.round(pZ * 4))));
assert.ok(eco.s.tr >= 300, "reserva respeitada");
assert.equal(total(), t4);
assert.ok(chat.some((l) => /^\[ledger\] Ana: KINGZ saiu da bolsa: 4 x/.test(l)));
assert.match(chat.filter((l) => l.startsWith("ALL")).at(-1), /cofre curto/);
eco.s.tr = 2000;

// limite: no maximo 6 mods listados (os mais populares); quem cai do top e deslistado
for (let i = 0; i < 9; i++) active.push(npc(`m${i}`, `Mod Numero ${i}`, T0 + 1000 + i));
b.tick(T0 + 120000, true);
assert.equal(Object.keys(b.s.m).length, 6);
assert.ok(Object.values(b.s.m).some((m) => m.id === "lula-robo"), "listado tem vantagem");
const out = active.find((a) => !Object.values(b.s.m).some((m) => m.id === a.id));
for (let i = 0; i < 60; i++) b.onHit(ana, { k: "hit", g: 8, i: active.indexOf(out), dmg: 1 });
b.tick(T0 + 140000, true);
assert.ok(Object.values(b.s.m).some((m) => m.id === out.id), "mod popular entra");
assert.equal(Object.keys(b.s.m).length, 6);
assert.match(chat.filter((l) => l.startsWith("ALL")).join("\n"), /caiu do top 6/);
const all = sent.at(-1);
const s3 = [...all.tk, ...all.mk].map((t) => t.s);
assert.equal(new Set(s3).size, s3.length);
assert.equal(all.tk.length, 8);
// estado sobrevive a reload
const b2 = new Bolsa(pl, JSON.parse(JSON.stringify(b.s)));
assert.equal(b2.modTk().length, 6);
assert.ok(b2.find(all.mk[0].s));
Math.random = rnd;
console.log(chat.filter((l) => l.startsWith("ALL BOLSA")).slice(-4).join("\n"));
console.log("OK test_bolsa");
