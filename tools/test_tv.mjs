// Teste offline do server/tv.js com sala/economia falsas e brain null. Uso: node tools/test_tv.mjs
import { Tv, urgent, today, week } from "../server/tv.js";
import { norm } from "../server/places.js";

let fails = 0;
const ok = (c, m) => {
    console.log(`${c ? "ok  " : "FAIL"} ${m}`);
    if (!c) fails++;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const sent = [];
const bc = [];
const c = { name: "bot" };
const eco = {
    s: { tr: 100, led: [{ t: 1, who: "ana", what: "comprou TNT", amt: 10, why: "loja" }], builds: ["torre torta"] },
    w: {},
    wallet(n) {
        return (this.w[norm(n)] ??= { n, c: 50 });
    },
    entry(who, what, amt, why) {
        this.s.led.push({ t: Date.now(), who, what, amt, why });
    },
    sendMe() { },
    ask: async () => null,
};
const room = {
    clients: new Map([[1, c]]),
    eco,
    brain: { ask: async () => null },
    lab: { s: { f: [{ pt: "gatos dormem muito" }] } },
    hub: { snapshot: () => ({ portals: [{ name: "mario", n: 2 }, { name: "gta", n: 0 }] }) },
    send: (cl, m) => sent.push(m),
    broadcast: (m) => bc.push(m),
};
const pl = {
    room,
    save() { },
    priv: (cl, from, m) => sent.push({ t: "chat", n: from, m }),
    say: (from, m) => bc.push({ t: "chat", n: from, m }),
    online: () => ["bot"],
};
const tv = new Tv(pl, {});
const chats = () => sent.filter((m) => m.t === "chat").map((m) => m.m);
const last = () => [...bc].reverse().find((m) => m.k === "tv");

// Boletim fallback no join
tv.join(c);
await sleep(20);
let s = last();
ok(s && s.h.length === 3 && s.h.every((h) => h.length > 0 && h.length <= 90), `fallback 3 manchetes: ${JSON.stringify(s?.h)}`);
ok(s.tk.length > 0 && s.tk.length <= 200 && s.a.length > 0, "ticker + ancora");
ok(s.n === 1, "boletim #1");

// /noticia cobra 5 e entra em cooldown
tv.command(1, c, "noticia", []);
await sleep(20);
ok(eco.w.bot.c === 45 && eco.s.tr === 105, `noticia cobrou 5 (carteira ${eco.w.bot.c}, cofre ${eco.s.tr})`);
ok(last().n === 2, "boletim #2 saiu");
ok(eco.s.led.some((e) => /boletim extra/.test(e.what)), "ledger registrou");
tv.command(1, c, "noticia", []);
await sleep(20);
ok(eco.w.bot.c === 45 && /redacao ocupada/.test(chats().at(-1)), "noticia em cooldown, nao cobrou");

// /tv lista e /tv com url passa adiante
sent.length = 0;
ok(tv.command(1, c, "tv", []) === true && chats().length >= 4, "/tv mostra manchetes");
ok(tv.command(1, c, "tv", ["https://youtu.be/x"]) === false, "/tv url nao e da TV URNA");

// AO VIVO + cooldown
tv.onMsg(1, c, { t: "pl", k: "tv_aovivo" });
s = last();
ok(s.air && s.air.name === "bot" && s.air.q && s.air.left > 30, `no ar: ${JSON.stringify(s.air)}`);
const other = { name: "zeca" };
sent.length = 0;
tv.onMsg(2, other, { t: "pl", k: "tv_aovivo" });
ok(/ja tem gente no ar/.test(chats().at(-1)), "um por vez");
tv.air.until = 0;
sent.length = 0;
tv.onMsg(1, c, { t: "pl", k: "tv_aovivo" });
ok(/volta em/.test(chats().at(-1)), "cooldown por jogador");
clearTimeout(tv.airT);

// /manchete
sent.length = 0;
tv.sponsor(c, ["compre", "bitcoin", "agora", "20"]);
ok(/recusada/.test(chats().at(-1)) && eco.w.bot.c === 45, "manchete unsafe recusada, nada cobrado");
tv.sponsor(c, ["oi", "20"]);
ok(/uso:/.test(chats().at(-1)), "texto curto recusado");
tv.sponsor(c, ["vila", "linda", "10"]);
ok(/minimo/.test(chats().at(-1)), "minimo 20");
tv.sponsor(c, ["vila", "linda", "demais", "20"]);
await sleep(20);
s = last();
ok(eco.w.bot.c === 25 && s.sp.length === 1 && s.sp[0].left > 500 && s.sp[0].left <= 600, `manchete aceita sem IA: ${JSON.stringify(s.sp)}`);

// PLANTAO URGENTE: deteccao + limite de 1 por 60 s
ok(urgent({ who: "IA", what: "construiu: torre torta (120 blocos)", amt: 80 })?.[1] === 'IA ERGUE "torre torta" SEM LICITACAO; COFRE PAGA 80', "detecta obra da IA");
ok(urgent({ who: "ana", what: "comprou Wolverine cai do ceu", amt: 120 })?.[0] === 4, "detecta wolverine");
ok(urgent({ who: "IA", what: "bancou evento publico: fogos", amt: 60 }) !== null, "detecta fogos da IA");
ok(urgent({ who: "ana", what: "doou pro cofre da IA", amt: 100 }) !== null && urgent({ who: "ana", what: "doou pro cofre da IA", amt: 99 }) === null, "doacao >= 100");
ok(urgent({ who: "CONGRESSO", what: "aprovou: lei do pulo duplo", amt: 0 })?.[1] === "CONGRESSO APROVA: lei do pulo duplo", "detecta lei");
ok(urgent({ who: "LAB", what: "pesquisou: gatos (pedido de ana)", amt: 5 }) !== null, "detecta lab");
ok(urgent({ who: "ana", what: "comprou TNT", amt: 10 }) === null && urgent({ who: "bot", what: "pagou boletim extra da TV URNA", amt: 5 }) === null, "ignora miudeza");
clearInterval(tv.iv);
const says = () => bc.filter((m) => m.t === "chat" && /^PLANTAO:/.test(m.m)).map((m) => m.m);
let T = Date.now() + 1000;
const ev = (who, what, amt) => eco.s.led.push({ t: T++, who, what, amt });
ev("ana", "comprou TNT", 10);
tv.watch(T);
ok(!says().length && !last().urg, "sem fato grande, sem plantao");
ev("ana", "doou pro cofre da IA", 150);
ev("LAB", "pesquisou: gatos", 5);
tv.watch(T);
s = last();
ok(s.urg && /DOACAO HISTORICA: ANA DOA 150/.test(s.urg.text) && s.urg.left >= 24, `plantao escolhe o maior: ${JSON.stringify(s.urg)}`);
ok(says().length === 1 && /^PLANTAO: DOACAO/.test(says()[0]), "chat PLANTAO");
ev("IA", "construiu: ponte (50 blocos)", 40);
tv.watch(T + 10000);
ok(says().length === 1, "segundo fato em 10 s espera (limite 60 s)");
tv.watch(T + 30000);
ok(last().urg === null, "plantao some depois de 25 s");
tv.watch(T + 61000);
ok(says().length === 2 && /PONTE/.test(last().urg?.text), "fato pendente sai depois do cooldown");
pl.congresso = { news: () => ["BOLSA? NAO, CONGRESSO: SESSAO NORMAL"] };
tv.watch(T + 62000);
pl.congresso.news = () => ["CONGRESSO DERRUBA LEI DA GRAVIDADE"];
tv.watch(T + 63000);
tv.watch(T + 130000);
ok(says().length === 3 && /GRAVIDADE/.test(says()[2]), "mudanca no news() do congresso vira plantao");
ev("ana", "doou pro cofre da IA", 500);
tv.watch(T + 300000);
ok(says().length === 3, "fato velho (> 90 s) descartado");

// MOMENTO DO DIA: recordes do dia
const N = Date.now();
ok(tv.s.rec.d === today(N) && today(Date.UTC(2026, 0, 2, 2, 0)) === "2026-01-01", "dia local (UTC-3)");
ok(tv.s.rec.don?.v === 500 && tv.s.rec.don.n === "ana", `maior doacao do ledger: ${JSON.stringify(tv.s.rec.don)}`);
tv.record({ who: "zeca", what: "doou pro fundo do lab", amt: 80 }, N);
ok(tv.s.rec.don.v === 500, "doacao menor nao bate recorde");
tv.record({ who: "CONGRESSO", what: "aprovou lei: GRAVIDADE LUNAR", amt: 0, why: "3 voto(s), quorum 2" }, N);
tv.record({ who: "CONGRESSO", what: "aprovou lei: TURBO NACIONAL", amt: 0, why: "2 voto(s), quorum 2" }, N);
ok(tv.s.rec.law?.n === "GRAVIDADE LUNAR" && tv.s.rec.law.v === 3, "lei mais votada");
tv.record({ who: "ana", what: "viajou pra arena", amt: 5 }, N);
const t1 = { name: "ana" };
for (let i = 0; i < 3; i++) {
    t1.tvTripAt = 0;
    tv.onMsg(9, t1, { t: "pl", k: "term_trip", d: "lab" });
}
tv.onMsg(9, t1, { t: "pl", k: "term_trip", d: "lab" });
tv.onMsg(9, t1, { t: "pl", k: "term_trip", d: "marte" });
ok(tv.s.rec.trips.ana?.c === 4, `viagens: ledger + portao, com throttle e destino valido (${tv.s.rec.trips.ana?.c})`);
const h1 = { name: "bot" };
for (let i = 0; i < 5; i++) {
    h1.tvHitAt = 0;
    tv.onHit(h1, { k: "pf", g: 0, i: 3 });
}
tv.onHit(h1, { k: "pf", g: 0, i: 3 });
h1.tvHitAt = 0;
tv.onHit(h1, { k: "npc", g: 9 });
ok(tv.s.rec.hits.WOLVERINE?.c === 5 && tv.s.rec.hits.GODZILHA?.c === 1, "porradas por lutador (throttle)");
pl.bolsa = { s: { p: { LULA: 1, WOLV: 1, COFRE: 1 } }, delta: (S) => ({ LULA: 4.2, WOLV: -12.5, COFRE: "x" })[S] };
pl.banco = { rich: () => [{ k: "offline", n: "Fantasma", t: 9999 }, { k: "bot", n: "bot", t: 321 }] };
tv.sample(N);
pl.bolsa.delta = (S) => ({ LULA: 2, WOLV: -3, COFRE: 1 })[S];
tv.sample(N);
ok(tv.s.rec.drop?.n === "WOLV" && tv.s.rec.drop.v === 12.5 && tv.s.rec.rally?.n === "LULA" && tv.s.rec.rally.v === 4.2, "tombo e alta da bolsa (guarda o pior/melhor)");
ok(tv.s.rec.rich?.n === "bot" && tv.s.rec.rich.v === 321, "magnata do dia so entre quem ta online");
pl.bolsa.delta = () => {
    throw new Error("x");
};
tv.sample(N);
ok(true, "bolsa quebrada nao derruba a TV");

// segmento: /momento, snapshot, chat, cooldown
tv.urg = null;
tv.momAt = 0;
sent.length = 0;
tv.command(1, c, "momento", []);
s = last();
ok(s.mom && s.mom.left >= 29 && s.mom.items.length >= 3 && s.mom.items.length <= 5, `snapshot mom: ${JSON.stringify(s.mom)}`);
ok(s.mom.items.every((x) => x.cat && x.v && x.n && x.n.length <= 28), "itens com categoria, valor e nome");
const momSay = bc.filter((m) => m.t === "chat" && /^MOMENTO DO DIA:/.test(m.m));
ok(momSay.length === 1 && /TOMBO/.test(momSay[0].m), `chat resumo: ${momSay[0]?.m}`);
tv.command(1, c, "momento", []);
ok(/ja ta no telao/.test(chats().at(-1)), "nao repete com segmento no ar");
tv.mom.until = 0;
tv.watch(Date.now());
ok(last().mom === null, "segmento sai do ar");
tv.command(1, c, "momento", []);
ok(/descansando/.test(chats().at(-1)), "cooldown do /momento");
const rot = tv.items(N).map((x) => x.cat).join();
tv.s.mn++;
ok(tv.items(N).length === 5 && tv.items(N).map((x) => x.cat).join() !== rot, "mais de 5 recordes: gira a selecao");

// automatico a cada 8 min com gente online
tv.momAt = Date.now() - 8 * 60 * 1000 - 1;
tv.s.at = Date.now();
tv.tick(Date.now(), true);
ok(tv.mom && last().mom, "segmento automatico depois de 8 min");
tv.mom = null;

// virada do dia zera
tv.s.rec.d = "2000-01-01";
const empty = tv.items(N);
ok(tv.s.rec.d === today(N) && !tv.s.rec.don && !Object.keys(tv.s.rec.trips).length, "virada do dia zera recordes");
ok(empty.length === 3 && empty[0].cat === "COFRE DA IA", "sem recordes: tapa-buraco com fatos fixos");
ok(new Tv(pl, { rec: { d: "2000-01-01", don: { v: 9, n: "x" } } }).s.rec.don === null, "estado salvo de outro dia zera no load");
ok(new Tv(pl, { rec: { ...freshKeep(), d: today() } }).s.rec.don?.v === 7, "estado salvo de hoje persiste");

// AUDIENCIA: plateia em frente ao telao + recorde persistido
const recSay = () => bc.filter((m) => m.t === "chat" && /recorde de audiencia/.test(m.m)).map((m) => m.m);
tv.s.aud = { v: 0, d: "" };
tv.audSaid = 0;
c.pos = [64, 20, 250];
room.clients.set(2, { name: "zeca", pos: [40, 20, 231] });
room.clients.set(3, { name: "longe", pos: [64, 20, 280] });
room.clients.set(4, { pos: [64, 20, 250] });
tv.watch(Date.now());
s = last();
ok(s.aud === 2 && s.rec === 2 && s.recd === today(), `audiencia conta so quem ta na frente do telao: ${s.aud}`);
ok(recSay().length === 1 && /: 2$/.test(recSay()[0]), `chat recorde: ${recSay()[0]}`);
const nb = bc.length;
tv.watch(Date.now());
ok(bc.length === nb, "sem mudanca, sem push");
room.clients.set(5, { name: "nova", pos: [99, 20, 265] });
tv.watch(Date.now());
ok(last().aud === 3 && tv.s.aud.v === 3 && recSay().length === 1, "recorde novo em menos de 1 min: atualiza calado");
tv.audSaid -= 61000;
tv.watch(Date.now());
ok(recSay().length === 2 && /: 3$/.test(recSay()[1]), "anuncio pendente sai depois de 1 min");
for (const k of [2, 3, 4, 5]) room.clients.delete(k);
tv.watch(Date.now());
ok(last().aud === 1 && last().rec === 3 && recSay().length === 2, "plateia cai, recorde fica");
ok(new Tv(pl, { aud: { v: 9, d: "2026-01-01" } }).snap().rec === 9, "recorde persiste no load");
delete c.pos;

// AO VIVO DA ARENA: janela, combo, nocaute, pico, campeonato
ok(week(Date.UTC(2026, 0, 1, 12)) === "2026-W01" && week(Date.UTC(2027, 0, 1, 12)) === "2026-W53" && week(Date.UTC(2024, 11, 30, 12)) === "2025-W01", "semana ISO");
ok(week(Date.UTC(2026, 8, 28, 2, 30)) === "2026-W39" && week(Date.UTC(2026, 8, 28, 3, 30)) === "2026-W40", "semana vira segunda 00:00 de Brasilia");
const arSay = () => bc.filter((m) => m.t === "chat" && /^AO VIVO DA ARENA/.test(m.m)).map((m) => m.m);
const tv2 = new Tv(pl, {});
tv2.seenT = Infinity;
delete pl.congresso;
const pa = { name: "Ana" }, pb = { name: "beto" }, pc = { name: "caio" };
const T0 = Date.now() - 10000;
for (let i = 0; i < 10; i++) tv2.arenaHit(pa, "WOLVERINE", { k: "pf", g: 0, i: 3, dmg: 10, w: "soco" }, T0 + i * 200);
tv2.arenaHit(pa, "WOLVERINE", { k: "pf", dmg: 10 }, T0 + 1850);
for (let i = 0; i < 3; i++) tv2.arenaHit(pb, "LULA", { k: "pf", dmg: 5 }, T0 + 3000 + i * 2000);
tv2.arenaHit(pb, "LULA", { k: "pf", dmg: 9999 }, T0 + 9000);
tv2.arenaHit(pb, "LULA", { k: "pf", dmg: -50 }, T0 + 9500);
ok(tv2.hpm(T0 + 9500) === 14 && !tv2.ar, `janela conta golpes (throttle 100 ms, dano negativo fora): ${tv2.hpm(T0 + 9500)}`);
ok(tv2.hits.filter((x) => x.ko).length === 2 && tv2.hits[7].ko && tv2.hits[13].ko && tv2.hits[9].cb === 10, "nocaute (80 em 3 s, 1 por gigante) e combo do mesmo atacante");
ok(tv2.hits.at(-1).dmg === 100, "dano por golpe limitado a 100");
for (let i = 0; i < 15; i++) tv2.arenaHit(pc, i % 2 ? "GODZILHA" : "LULA", { k: "hit", dmg: 1 }, T0 + 9600 + i * 150);
ok(!tv2.ar && tv2.hpm(T0 + 11700) === 29, "29 golpes/min: ainda nao");
tv2.arenaHit(pc, "LULA", { k: "hit", dmg: 1 }, T0 + 12000);
s = last();
ok(tv2.ar && s.ar && s.ar.left >= 34 && arSay().length === 1 && /golpes por minuto/.test(arSay()[0]), `pico de 30 golpes/min entra no ar: ${arSay()[0]}`);
ok(s.ar.f[0].n === "LULA" && s.ar.f[0].d === 124 && s.ar.f[0].hp === 75 && s.ar.f[1].n === "WOLVERINE" && s.ar.f[1].d === 100 && s.ar.f[1].hp === 80, `placar top 2: ${JSON.stringify(s.ar.f)}`);
ok(s.ar.top[0].n === "beto" && s.ar.top[1].n === "Ana" && s.ar.top.length === 3 && s.ar.hits === 30, `top porradeiros: ${JSON.stringify(s.ar.top)}`);
ok(s.ar.combo?.c === 10 && s.ar.combo.n === "Ana" && s.ar.ko === 2 && /^NOCAUTE TECNICO! LULA .*2 NOCAUTES/.test(s.ar.nar), `combo + narrador template: ${s.ar.nar}`);
ok(s.ar.lead?.n === "beto" && s.ar.lead.v === 115, `lider do campeonato no telao: ${JSON.stringify(s.ar.lead)}`);
tv2.watch(Date.now());
tv2.urg = null;
const nb2 = bc.length;
tv2.watch(Date.now());
ok(bc.length === nb2, "sem golpe novo, sem push");
tv2.arenaHit(pa, "WOLVERINE", { k: "pf", dmg: 7 }, Date.now());
tv2.watch(Date.now());
ok(last().ar?.f.find((x) => x.n === "WOLVERINE").d === 107, "golpe durante a transmissao atualiza o placar");
sent.length = 0;
tv2.command(1, pc, "campeonato", []);
ok(/CAMPEONATO DA ARENA \d{4}-W\d\d .*1\. beto 115 \| 2\. Ana 107 \| 3\. caio 16/.test(chats()[0]), `/campeonato top 5: ${chats()[0]}`);
tv2.ar.until = 0;
tv2.watch(Date.now());
ok(last().ar === null && !tv2.ar && tv2.arEnd > 0, "transmissao sai do ar depois de 45 s");
for (let i = 0; i < 40; i++) tv2.arenaHit({ name: `z${i}` }, "LULA", { dmg: 1 }, Date.now());
ok(!tv2.ar, "pico logo depois respeita cooldown de 3 min");
room.brain.ask = async (sys, user) => (/narracao/.test(user) ? { narracao: "golpe de mestre do <<<beto>>>" } : null);
tv2.arEnd = 0;
tv2.arenaStart(Date.now(), true);
await sleep(10);
ok(last().ar?.nar === "GOLPE DE MESTRE DO BETO", `narrador IA quando responde: ${last().ar?.nar}`);
room.brain.ask = async () => null;
tv2.ar = null;
tv2.hits = [];
tv2.arAt = Date.now() - 10 * 60 * 1000 - 1;
tv2.arEnd = 0;
tv2.s.at = Date.now();
tv2.tick(Date.now(), true);
s = last();
ok(s.ar && s.ar.hits === 0 && s.ar.f.length === 2 && s.ar.f[0].n !== s.ar.f[1].n && s.ar.f[0].hp === 100 && /ARENA VAZIA/.test(s.ar.nar), `agenda de 10 min, arena vazia: ${JSON.stringify(s.ar.f)}`);
ok(/entra ao vivo/.test(arSay().at(-1)), "chat da transmissao agendada");
tv2.ar = null;
tv2.momAt = Date.now();
tv2.s.camp.w = "2000-W01";
sent.length = 0;
tv2.command(1, pa, "campeonato", []);
ok(tv2.s.camp.w === week() && !Object.keys(tv2.s.camp.d).length && tv2.s.campPrev?.n === "beto" && /ninguem bateu.*semana passada: beto \(115\)/.test(chats()[0]), `virada da semana zera e guarda o campeao: ${chats()[0]}`);
ok(new Tv(pl, { camp: { w: week(), d: { x: { n: "x", v: 5 } } } }).campTop(Date.now(), 5)[0]?.v === 5, "campeonato da semana persiste no load");
ok(!new Tv(pl, { camp: { w: "2001-W02", d: { x: { n: "x", v: 5 } } } }).campTop(Date.now(), 5).length, "campeonato de semana velha zera no load");
const ph = { name: "Real" };
tv2.onHit(ph, { t: "a", k: "pf", g: 0, i: 1, d: [0, 0, 1], p: [1, 2, 3], dmg: 12, w: "SOCO" });
ok(tv2.hits.at(-1)?.f === "FLAVIO" && tv2.s.camp.d.real?.v === 12, "onHit no formato do cliente vira golpe da arena");
tv2.onHit({ name: "x" }, { k: "pv", g: 1, i: 0, dmg: 9 });
ok(tv2.hits.length === 1, "aldeao nao conta");

function freshKeep() {
    return { drop: null, rally: null, law: null, don: { v: 7, n: "y" }, rich: null, trips: {}, hits: {} };
}

console.log(fails ? `${fails} falha(s)` : "tudo ok");
process.exit(fails ? 1 : 0);
