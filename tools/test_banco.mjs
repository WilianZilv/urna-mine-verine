// Teste do server/banco.js com places/room/eco falsos: node tools/test_banco.mjs
import assert from "node:assert/strict";
import { Banco, today } from "../server/banco.js";

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
b.last = ""; // o caixa acima fez check-in da ofensiva e ja empurrou o snapshot
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

// ranking de criadores: impacto (mods, portais, visitas, presenca, golpes), nunca moeda
{
    const npc = (id, creator, max, at) => ({ id, creator, at, pkg: { behavior: { spawn: { max_instances: max } } } });
    const mods = {
        active: new Map([
            ["kong", npc("kong", "Zeca", 2, 1)],
            ["sapo", npc("sapo", "Lia", 1, 2)],
            ["skin", { id: "skin", creator: "Zeca", at: 3, pkg: { manifest: { kind: "avatar" } } }],
        ]),
        list(kind) {
            return [...this.active.values()].filter((a) => (a.pkg.manifest?.kind || "npc") === kind).sort((a, b) => a.at - b.at);
        },
    };
    const portal = (id, author, ok = true) => ({ id, author, active: "1.0.0", versions: [{ v: "1.0.0", origin: "https://g.example" }], verified: ok ? { origin: "https://g.example" } : null });
    const counts = { corrida: 0, off: 5 };
    const hub = {
        s: { p: { corrida: portal("corrida", "Lia"), off: portal("off", "Zeca", false) } },
        ses: new Map(),
        count: (pid) => counts[pid] || 0,
    };
    const saved = [];
    const croom = { eco, send: () => { }, broadcast: () => { }, mods, hub };
    const cpl = { room: croom, save: (k, s) => saved.push(k), say() { }, priv: (c, from, m) => chat.push(m) };
    const bc = new Banco(cpl, {});
    const now = Date.now();

    // ao vivo: Zeca 2 mods (npc + skin) = 80; Lia 1 mod + 1 portal no ar = 65; portal nao verificado nao conta
    let cr = bc.creators();
    assert.deepEqual(cr.map((r) => [r.n, r.score, r.mods, r.portals]), [["Zeca", 80, 2, 0], ["Lia", 65, 1, 1]]);

    // visitas: 1 por jogador+portal por hora; o proprio criador nao conta
    hub.ses.set("a", { pid: "corrida", name: "Bob", ready: true, exp: now + 10 * HOUR });
    hub.ses.set("b", { pid: "corrida", name: "lia", ready: true, exp: now + 10 * HOUR });
    hub.ses.set("c", { pid: "corrida", name: "Ana", ready: false, exp: now + 10 * HOUR });
    hub.ses.set("d", { pid: "corrida", name: "Velho", ready: true, exp: now - 1 });
    counts.corrida = 3; // Bob + Lia + 1 so com presenca
    bc.tick(now, true);
    assert.equal(bc.s.cr.lia.e, 2);
    assert.equal(bc.s.cr.lia.p, 2); // 3 presentes - a propria Lia
    bc.tick(now + 20000, true); // mesmas sessoes: sem visita nova, presenca acumula
    assert.equal(bc.s.cr.lia.e, 2);
    assert.equal(bc.s.cr.lia.p, 4);
    hub.ses.set("e", { pid: "corrida", name: "Bob", ready: true, exp: now + 10 * HOUR }); // Bob entrou de novo
    bc.tick(now + 40000, true);
    assert.equal(bc.s.cr.lia.e, 2);
    bc.tick(now + 2 * HOUR, true); // passou 1h: Bob e Ana contam de novo
    assert.equal(bc.s.cr.lia.e, 4);
    assert.ok(saved.includes("banco"));

    // golpes em mods: g=8, i achatado (kong tem 2 instancias, sapo 1); 1 a cada 2 s por jogador; dono nao conta
    bc.onHit(bob, { k: "hit", g: 8, i: 2, dmg: 5 }); // i=2 -> sapo (Lia)
    bc.onHit(bob, { k: "hit", g: 8, i: 2, dmg: 5 }); // cooldown
    bc.onHit(ana, { k: "hit", g: 8, i: 1, dmg: 5 }); // i=1 -> kong (Zeca)
    bc.onHit({ name: "Zeca" }, { k: "hit", g: 8, i: 0, dmg: 5 }); // dono
    bc.onHit({ name: "Rui" }, { k: "hit", g: 8, i: 9, dmg: 5 }); // fora da lista
    bc.onHit({ name: "Rui" }, { k: "hit", g: 1, i: 0, dmg: 5 }); // aldeao
    assert.equal(bc.s.cr.lia.h, 1);
    assert.equal(bc.s.cr.zeca.h, 1);
    assert.equal(bc.modAt(0).id, "kong");
    assert.equal(bc.modAt(3), null);

    // Lia: 40 + 25 + 4*10 + presenca + 0 (1 golpe < 5)
    const lia = bc.creators().find((r) => r.k === "lia");
    assert.equal(lia.score, 40 + 25 + 40 + lia.p);
    assert.equal(bc.s.cri[0][0], "Lia");
    assert.deepEqual(bc.snap().cri[0], ["Lia", lia.score, 1, 1, 4]);

    // impacto nunca mexe em moeda
    const trBefore = eco.s.tr, wBefore = JSON.stringify(eco.s.w);
    bc.tick(now + 3 * HOUR, true);
    assert.equal(eco.s.tr, trBefore);
    assert.equal(JSON.stringify(eco.s.w), wBefore);

    // /criadores: top 5 + posicao (tick de +3h contou Bob e Ana de novo)
    assert.equal(bc.command(1, { name: "Zeca" }, "criadores", []), true);
    assert.match(last(), /^CRIADORES \(impacto, nao dinheiro.*1\. Lia \d+ \[1m 1p 6v\] \| 2\. Zeca 80 \[2m 0p 0v\] \|\| tu: #2$/);
    assert.equal(bc.command(1, { name: "Rui" }, "criadores", []), true);
    assert.doesNotMatch(last(), /tu:/);

    // mod desativado: impacto ao vivo some, acumulado fica; criador sem nada sai do ranking
    mods.active.clear();
    hub.s.p = {};
    cr = bc.creators();
    assert.equal(cr.find((r) => r.k === "lia").score, 10 * bc.s.cr.lia.e + bc.s.cr.lia.p);
    assert.equal(cr.find((r) => r.k === "zeca"), undefined);

    // sem mods/hub (testes antigos, DO subindo): ranking vazio, sem erro
    const empty = new Banco({ room: { eco, broadcast() { } }, save() { }, say() { }, priv: (c, f, m) => chat.push(m) }, {});
    empty.tick(now, true);
    assert.deepEqual(empty.creators(), []);
    empty.command(1, bob, "criadores", []);
    assert.match(last(), /ninguem trouxe nada/);

    // no maximo 100 criadores guardados, sai quem tem menos impacto
    for (let i = 0; i < 120; i++) bc.s.cr[`c${i}`] = { n: `C${i}`, e: Math.floor(i / 20), p: 0, h: 0 };
    bc.prune();
    assert.equal(Object.keys(bc.s.cr).length, 100);
    assert.ok(bc.s.cr.lia && bc.s.cr.c119 && !bc.s.cr.c0);
}

// HALL DA FAMA: quem entra no top 3 de criadores e anunciado (1x por minuto, primeiro top sem ranking salvo nao)
{
    const said = [];
    const st = {};
    const hroom = { eco, send: () => { }, broadcast: () => { } };
    const hpl = { room: hroom, save() { }, say: (from, m) => said.push(`${from}: ${m}`), priv() { } };
    const h = new Banco(hpl, st);
    const set = (o) => (h.s.cr = Object.fromEntries(Object.entries(o).map(([n, e]) => [norm(n), { n, e, p: 0, h: 0 }])));
    set({ A: 5, B: 4, C: 3 });
    h.push();
    assert.equal(said.length, 0);
    set({ A: 5, B: 4, C: 3, D: 1 }); // D fora do top 3
    h.push();
    assert.equal(said.length, 0);
    set({ A: 5, B: 4, C: 3, D: 9 });
    h.push();
    assert.deepEqual(said, ["BANCO: D entrou pro HALL DA FAMA dos criadores"]);
    set({ A: 5, B: 4, C: 30, D: 9, E: 20 }); // E entra no cooldown: segura
    h.push();
    assert.equal(said.length, 1);
    h.hofAt -= 61 * 1000;
    h.push();
    assert.equal(said[1], "BANCO: C e E entraram pro HALL DA FAMA dos criadores"); // C tinha caido pro #4 quando D entrou
    set({ A: 50, B: 40, C: 30 });
    h.hofAt -= 61 * 1000;
    h.push();
    assert.equal(said[2], "BANCO: A e B entraram pro HALL DA FAMA dos criadores");
    h.tick(Date.now(), true);
    assert.deepEqual(st.cri.slice(0, 3), [["A", 500], ["B", 400], ["C", 300]]);

    // DO reinicia com ranking salvo: top 3 conhecido nao e reanunciado
    said.length = 0;
    const h2 = new Banco(hpl, st);
    h2.push();
    assert.equal(said.length, 0);
    h2.s.cr.z = { n: "Z", e: 99, p: 0, h: 0 };
    h2.push();
    assert.deepEqual(said, ["BANCO: Z entrou pro HALL DA FAMA dos criadores"]);
}
// OFENSIVA DIARIA: 1 check-in por dia local, pula um dia e zera, ciclo de 7, reserva protege o cofre
{
    const msgs = [], out = [];
    const oeco = {
        s: { tr: 1000, w: {}, led: [] },
        wallet: eco.wallet,
        entry: eco.entry,
        sendMe() { },
    };
    const oroom = { eco: oeco, send: (c, m) => (m.t === "chat" ? msgs.push(m.m) : out.push(m)), broadcast() { } };
    const opl = { room: oroom, save() { }, say() { }, priv: (c, from, m) => msgs.push(m) };
    const o = new Banco(opl, {});
    const zeca = { name: "Zeca" };
    const day = today();
    const lastMsg = () => msgs[msgs.length - 1];
    const sum = () => oeco.s.tr + Object.values(oeco.s.w).reduce((s, w) => s + w.c, 0);
    oeco.wallet("Zeca");
    const t0 = sum();

    // dia local UTC-3: 02:59 UTC ainda e o dia anterior
    assert.equal(today(Date.UTC(2026, 9, 2, 2, 59)), today(Date.UTC(2026, 9, 1, 12, 0)));
    assert.equal(today(Date.UTC(2026, 9, 2, 3, 0)), today(Date.UTC(2026, 9, 1, 12, 0)) + 1);

    // join com check-in disponivel: dica privada
    o.join(zeca);
    assert.match(lastMsg(), /check-in do dia disponivel/);

    // dia 1 paga 3; segunda vez no mesmo dia nao paga
    assert.equal(o.command(1, zeca, "diario", []), true);
    assert.deepEqual(o.s.of.zeca, { n: "Zeca", d: 1, last: day });
    assert.equal(oeco.s.w.zeca.c, 103);
    assert.equal(oeco.s.tr, 997);
    assert.match(lastMsg(), /OFENSIVA 1 DIA! \+3/);
    assert.deepEqual(out.at(-1), { t: "pl", k: "banco", ofx: 1, pay: 3 });
    assert.deepEqual(oeco.s.led.at(-1).what, "ofensiva diaria: 1 dia");
    const nled = oeco.s.led.length;
    o.command(1, zeca, "diario", []);
    assert.match(lastMsg(), /ja pegou o de hoje/);
    assert.equal(oeco.s.w.zeca.c, 103);
    assert.equal(oeco.s.led.length, nled);
    const nmsg = msgs.length;
    o.join(zeca); // ja pegou hoje: sem dica
    assert.equal(msgs.length, nmsg);

    // tabela de pagamento: dias 1..7 = 3,4,5,6,8,10,15, dia 8 recomeca o ciclo
    const pays = [3];
    for (let i = 2; i <= 9; i++) {
        o.s.of.zeca.last = day - 1; // virou o dia
        const before = oeco.s.w.zeca.c;
        o.command(1, zeca, "diario", []);
        assert.equal(o.s.of.zeca.d, i);
        pays.push(oeco.s.w.zeca.c - before);
    }
    assert.deepEqual(pays, [3, 4, 5, 6, 8, 10, 15, 3, 4]);
    assert.equal(sum(), t0);
    assert.equal(oeco.s.led.filter((e) => e.what.startsWith("ofensiva")).length, 9);

    // /ofensiva: mostra a tua
    o.command(1, zeca, "ofensiva", []);
    assert.match(lastMsg(), /tua OFENSIVA: 9 dias seguidos \(#1 da vila\) \| hoje ja foi\. amanha paga 5/);
    o.command(1, { name: "Rui" }, "ofensiva", []);
    assert.match(lastMsg(), /nao tem OFENSIVA.*\/diario paga 3/);

    // pulou um dia: zera e paga o dia 1 de novo
    o.s.of.zeca.last = day - 2;
    assert.equal(o.streak("zeca"), 0);
    o.join(zeca);
    assert.match(lastMsg(), /check-in do dia disponivel/);
    o.command(1, zeca, "diario", []);
    assert.equal(o.s.of.zeca.d, 1);
    assert.match(lastMsg(), /\+3/);

    // ofensiva viva de ontem ainda conta no ranking e na dica
    o.s.of.zeca.last = day - 1;
    assert.equal(o.streak("zeca"), 1);
    o.join(zeca);
    assert.match(lastMsg(), /OFENSIVA de 1 dia ta esperando/);

    // reserva: cofre nao paga abaixo de 300, mas a ofensiva conta
    oeco.s.tr = 302; // dia 2 pagaria 4 -> 298
    const w0 = oeco.s.w.zeca.c, l0 = oeco.s.led.length;
    o.command(1, zeca, "diario", []);
    assert.equal(o.s.of.zeca.d, 2);
    assert.equal(oeco.s.tr, 302);
    assert.equal(oeco.s.w.zeca.c, w0);
    assert.equal(oeco.s.led.length, l0);
    assert.match(lastMsg(), /OFENSIVA 2 DIAS registrada.*reserva/);
    assert.deepEqual(out.at(-1), { t: "pl", k: "banco", ofx: 2, pay: 0 });
    oeco.s.tr = 303; // dia 3 paga 5 -> 298: nao; exatamente 300 paga
    o.s.of.zeca.last = day - 1;
    o.command(1, zeca, "diario", []);
    assert.equal(oeco.s.tr, 303);
    oeco.s.tr = 306; // dia 4 paga 6 -> 300
    o.s.of.zeca.last = day - 1;
    o.command(1, zeca, "diario", []);
    assert.equal(oeco.s.tr, 300);
    assert.equal(o.s.of.zeca.d, 4);

    // caixa eletronico faz o check-in (1x por dia), extrato continua saindo
    oeco.s.tr = 1000;
    const ana2 = { name: "Ana" };
    o.onMsg(1, ana2, { k: "banco_atm" });
    assert.equal(o.s.of.ana.d, 1);
    assert.ok(msgs.at(-2).startsWith("OFENSIVA 1 DIA!"));
    assert.match(lastMsg(), /CAIXA ELETRONICO/);
    o.atm.clear();
    const c0 = oeco.s.w.ana.c;
    o.onMsg(1, ana2, { k: "banco_atm" });
    assert.equal(oeco.s.w.ana.c, c0);
    assert.match(lastMsg(), /CAIXA ELETRONICO/);

    // fachada: top 5 ofensivas vivas + soma (fogueira); mortas nao contam
    for (let i = 0; i < 6; i++) o.s.of[`p${i}`] = { n: `P${i}`, d: 10 + i, last: day };
    o.s.of.velho = { n: "Velho", d: 99, last: day - 2 };
    const sn = o.snap();
    assert.deepEqual(sn.of, [["P5", 15], ["P4", 14], ["P3", 13], ["P2", 12], ["P1", 11]]);
    assert.equal(sn.ofs, 15 + 14 + 13 + 12 + 11 + 10 + 4 + 1);
}
console.log("test_banco OK");
