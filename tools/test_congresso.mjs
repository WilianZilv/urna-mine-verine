// Teste do Congresso sem worker: node tools/test_congresso.mjs
import assert from "node:assert/strict";
import { Congresso, ON_MS, REC_MS, isoWeek, weekEnd } from "../server/congresso.js";

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

// ------------------------------------------------ CONCURSO SEMANAL DE MODS
assert.equal(isoWeek(Date.UTC(2026, 9, 1)), "2026-W40");
assert.equal(isoWeek(Date.UTC(2021, 0, 3)), "2020-W53");
assert.equal(isoWeek(Date.UTC(2024, 11, 30)), "2025-W01");
assert.equal(weekEnd(Date.UTC(2026, 9, 1, 15)), Date.UTC(2026, 9, 5));

{
    let W = Date.UTC(2026, 9, 1, 12); // quinta, 2026-W40
    const mods = [];
    const msgs = [], snaps = [];
    const mroom = { send: (c, m) => snaps.push(m), broadcast: (m) => snaps.push(m), mods: { list: (k) => (k === "npc" ? mods : []) } };
    const mpl = { room: mroom, online: () => ["ana"], say: (f, m) => msgs.push(m), priv: (c, f, m) => msgs.push(m), save: () => { } };
    const st = {};
    const mc = new Congresso(mpl, st);
    mc.now = () => W;
    const mds = () => snaps.filter((m) => m.k === "lei").at(-1).mds;

    // sem mods ativos
    mc.command(0, ana, "concurso", []);
    assert.ok(msgs.at(-1).includes("nenhum mod ativo"));
    assert.equal(mc.command(0, ana, "votarmod", ["1"]), true);
    assert.ok(msgs.at(-1).includes("nenhum mod ativo"));
    assert.deepEqual(mc.news(), []);

    mods.push({ id: "kk", v: 2, creator: "Zeca", at: 1, pkg: { manifest: { name: "King Kong" } } });
    mods.push({ id: "pt", v: 1, creator: "Lia", at: 2, pkg: { manifest: { name: "Pato Ninja" } } });
    mc.command(0, ana, "concurso", []);
    assert.ok(msgs.some((m) => m.startsWith("1: King Kong de Zeca - 0 voto(s)")));
    assert.ok(msgs.some((m) => m.startsWith("2: Pato Ninja de Lia")));

    // votar, repetir, trocar
    mc.command(0, ana, "votarmod", ["1"]);
    assert.ok(msgs.at(-1).startsWith("voto registrado: King Kong"));
    assert.equal(mds().top[0].n, "King Kong");
    assert.equal(mds().top[0].vo, 1);
    mc.command(0, { name: "ANA" }, "votarmod", ["king", "kong"]);
    assert.ok(msgs.at(-1).includes("ja vota"));
    mc.command(0, ana, "votarmod", ["pato"]);
    assert.ok(msgs.at(-1).startsWith("voto trocado: Pato Ninja"));
    assert.equal(mds().tot, 1, "trocar nao duplica");
    assert.equal(mds().top[0].id, "pt");
    mc.command(0, bia, "votarmod", ["2"]);
    mc.command(0, caio, "votarmod", ["1"]);
    mc.command(0, caio, "votarmod", ["99"]);
    assert.ok(msgs.at(-1).includes("nao encontrado"));
    assert.deepEqual(mds().top.map((x) => [x.id, x.vo]), [["pt", 2], ["kk", 1]]);
    assert.equal(mds().week, "2026-W40");
    assert.ok(mds().left > 3 * 86400 && mds().left < 4 * 86400);
    assert.equal(mds().winner, null);

    // virada de semana
    W = Date.UTC(2026, 9, 5, 0, 0, 30);
    mc.tick(W, true);
    assert.ok(msgs.includes("CONCURSO: PATO NINJA de Lia e o MOD DA SEMANA (2 voto(s))"));
    assert.deepEqual(mds().winner, { week: "2026-W40", id: "pt", n: "Pato Ninja", c: "Lia", v: 1, vo: 2 });
    assert.equal(mds().week, "2026-W41");
    assert.equal(mds().tot, 0, "votos zeram");
    assert.equal(mds().hist.length, 1);
    assert.ok(mc.news()[0].includes("Pato Ninja"));
    assert.equal(st.mds.winner.id, "pt", "persistido no estado");

    // semana sem votos nao gera campeao; hall guarda no maximo 8
    W += 7 * 86400000;
    const n0 = msgs.length;
    mc.tick(W, true);
    assert.equal(msgs.slice(n0).filter((m) => m.startsWith("CONCURSO:")).length, 0);
    assert.equal(mds().winner.id, "pt");
    for (let k = 0; k < 10; k++) {
        mc.command(0, ana, "votarmod", ["1"]);
        W += 7 * 86400000;
        mc.tick(W, false);
    }
    assert.equal(st.mds.hist.length, 8);
    assert.equal(st.mds.winner.id, "kk");
    W += 2 * 86400000;
    assert.deepEqual(mc.news(), [], "manchete so nas primeiras 24 h");

    // mod campeao desativado: concurso continua sem ele
    mods.length = 0;
    mc.command(0, ana, "concurso", []);
    assert.ok(msgs.at(-1).includes("nenhum mod ativo"));
    console.log("ok: concurso", msgs.length, "falas");
}

console.log("ok: congresso", chat.length, "falas,", sent.length, "snapshots");
