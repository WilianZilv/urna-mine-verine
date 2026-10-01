// Teste do server/escola.js com places/room falsos: node tools/test_escola.mjs
import assert from "node:assert/strict";
import { Escola, day } from "../server/escola.js";
import { SITE } from "../server/mods.js";
import { ESCOLA, SPOTS, LANDMARKS, blocked } from "../server/layout.js";

const sent = [], bc = [], chat = [], saves = [];
const mod = (id, creator, at, name) => ({ id, creator, at, pkg: { manifest: { name: name || id } } });
const portal = (id, author, ok = true) => ({ id, author, active: "1.0.0", versions: [{ v: "1.0.0", name: id, origin: "https://g.example" }], verified: ok ? { origin: "https://g.example" } : null });
const room = {
    clients: new Map(),
    send: (c, m) => sent.push(m),
    broadcast: (m) => bc.push(m),
    mods: { active: new Map() },
    hub: { s: { p: {} } },
};
const pl = { room, save: (k, s) => saves.push([k, JSON.parse(JSON.stringify(s))]), priv: (c, from, m) => chat.push(`${from}: ${m}`), say() { } };
const st = {};
const e = new Escola(pl, st);

// sem mods/portais: snapshot zerado
e.join({ name: "ana" });
assert.deepEqual(sent.pop(), { t: "pl", k: "escola", site: SITE, mods: 0, portals: 0, creators: 0, reads: 0, last: [] });

// contadores ao vivo: mods ativos, portais verificados, criadores distintos (case-insensitive), ultimos 3 mods
room.mods.active.set("a", mod("a", "Zeca", 1, "King Kong"));
room.mods.active.set("b", mod("b", "Lia", 5, "Sapo"));
room.mods.active.set("c", mod("c", "zeca", 3));
room.mods.active.set("d", mod("d", "Rui", 9, "Capivara"));
room.hub.s.p = { corrida: portal("corrida", "Lia"), off: portal("off", "Bia", false), novo: portal("novo", "Bia") };
let s = e.snap();
assert.equal(s.mods, 4);
assert.equal(s.portals, 2);
assert.equal(s.creators, 4); // zeca, lia, rui, bia
assert.deepEqual(s.last, [["Capivara", "Rui"], ["Sapo", "Lia"], ["c", "zeca"]]);

// tick so transmite se mudou
e.tick(Date.now(), true);
e.tick(Date.now(), true);
assert.equal(bc.length, 1);
e.tick(Date.now(), false);
assert.equal(bc.length, 1);

// leituras do /skill.md: contagem por dia (Brasilia), sem nada alem de numeros
const t0 = Date.UTC(2026, 9, 2, 2, 0); // 23:00 de 01/10 em Brasilia
assert.equal(day(t0), "2026-10-01");
assert.equal(day(t0 + 2 * 3600 * 1000), "2026-10-02");
assert.equal(e.hit(t0), 1);
assert.equal(e.hit(t0 + 1000), 2);
assert.equal(e.hit(t0 + 2 * 3600 * 1000), 1);
assert.deepEqual(st.reads, { "2026-10-01": 2, "2026-10-02": 1 });
assert.equal(e.reads(t0), 2);
assert.equal(saves.at(-1)[0], "escola");
assert.deepEqual(Object.keys(saves.at(-1)[1]), ["reads"]);
for (const v of Object.values(st.reads)) assert.equal(typeof v, "number");
assert.equal(e.pt, null); // ninguem online: sem push agendado

// so guarda os ultimos 14 dias
for (let i = 0; i < 20; i++) e.hit(t0 + i * 86400 * 1000);
assert.equal(Object.keys(st.reads).length, 14);
assert.ok(!st.reads["2026-10-01"]);

// com gente online, leitura agenda 1 push agrupado
room.clients.set(1, { name: "ana" });
const n = bc.length;
e.hit();
e.hit();
assert.ok(e.pt);
await new Promise((r) => setTimeout(r, 3100));
assert.equal(e.pt, null);
assert.equal(bc.length, n + 1);
assert.equal(bc.at(-1).reads, e.reads());

// /escola: 4 passos + links + ao vivo, privado
assert.equal(e.command(1, { name: "ana" }, "escola", []), true);
assert.equal(chat.length, 3);
assert.match(chat[0], /^ESCOLA: COMO CRIAR UM MOD EM 4 PASSOS: 1\) cola https:\/\/.+\/skill\.md .*2\).*valida.*3\) sobe 4\) ativa/);
for (const l of ["/skill.md", "/modding.txt", "/hub.txt", "/hub-templates/"]) assert.ok(chat[1].includes(SITE + l), l);
assert.match(chat[2], /4 mods, 2 portais, 4 criadores, 2 leituras do \/skill\.md hoje/);
for (const m of chat) assert.ok(m.length <= 300 && /^[\x20-\x7e]+$/.test(m), "chat ASCII ate 300");
assert.equal(e.command(1, { name: "ana" }, "banco", []), false);

// planta: lote protegido das obras da IA (+-2) e spot de visita dentro da sala
assert.deepEqual(ESCOLA, { x0: 100, x1: 130, z0: 266, z1: 292 });
assert.ok(LANDMARKS.some(([x0, x1, z0, z1]) => x0 === 98 && x1 === 132 && z0 === 264 && z1 === 294));
assert.deepEqual(SPOTS.escola, [115, 279]);
assert.ok(blocked([110, 20, 270], [112, 22, 272]));
assert.ok(blocked([115, 20, 250], [115, 22, 250])); // trilha
console.log("test_escola OK");
