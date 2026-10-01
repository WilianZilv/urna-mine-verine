// Teste do Congresso sem worker: node tools/test_congresso.mjs
import assert from "node:assert/strict";
import { Congresso, ON_MS, REC_MS } from "../server/congresso.js";

let T = 1_000_000;
const names = ["ana"];
const chat = [], sent = [], led = [];
const room = {
    send: (c, m) => sent.push(m),
    broadcast: (m) => sent.push(m),
    eco: { entry: (...a) => led.push(a) },
    brain: { ask: async () => null },
};
const pl = { room, online: () => names, say: (f, m) => chat.push(m), priv: (c, f, m) => chat.push(m), save: () => { } };
const cg = new Congresso(pl, {});
cg.now = () => T;
const ana = { name: "Ana" }, bia = { name: "Bia" }, caio = { name: "Caio" };
const last = () => sent.filter((m) => m.k === "lei").at(-1);
const law = (id) => last().laws.find((l) => l.id === id);

// 1 online: um voto aprova
assert.equal(cg.priceFactor(), 1);
assert.equal(cg.command(0, ana, "lei", ["loja"]), true);
assert.ok(chat.some((m) => m.startsWith("LEI APROVADA: LOJA EM PROMOCAO")));
assert.equal(led.length, 1);
assert.equal(cg.priceFactor(), 0.5);
assert.equal(law("promo").on, 300);
assert.equal(last().passed, 1);

// recesso: votar de novo nao reaprova depois de expirar
T += ON_MS + 1;
cg.tick(T, true);
assert.ok(chat.some((m) => m.startsWith("LEI EXPIROU: LOJA EM PROMOCAO")));
assert.equal(cg.priceFactor(), 1);
assert.ok(law("promo").cd > 0 && law("promo").on === 0);
cg.command(0, ana, "lei", ["promo"]);
assert.equal(cg.priceFactor(), 1);
assert.equal(last().passed, 1);
T += REC_MS;
cg.tick(T, true);
assert.equal(cg.priceFactor(), 0.5, "voto pendente aprova quando acaba o recesso");
assert.equal(last().passed, 2);

// max 2 ativas
cg.command(0, ana, "lei", ["gravidade"]);
assert.equal(cg.active(T).length, 2);
cg.onMsg(0, ana, { t: "pl", k: "lei_voto", lei: "turbo", id: 7 });
assert.equal(cg.active(T).length, 2);
assert.equal(law("turbo").on, 0);
assert.equal(law("turbo").v, 1);

// quorum com 3 online = 2; votos de offline nao contam
names.push("bia", "caio");
T += ON_MS + 1;
cg.tick(T, true);
assert.equal(last().q, 2);
assert.equal(cg.active(T).length, 0);
assert.equal(law("turbo").on, 0, "1/2 nao aprova");
cg.command(0, bia, "lei", ["turbo"]);
assert.ok(law("turbo").on > 0);
names.splice(1, 2);
cg.command(0, bia, "lei", ["festa"]);
assert.equal(law("festa").v, 0, "bia offline nao conta");
cg.command(0, caio, "lei", ["xyz"]);
assert.ok(chat.at(-1).includes("nao existe"));

console.log("ok: congresso", chat.length, "falas,", sent.length, "snapshots");
