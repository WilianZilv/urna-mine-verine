// Teste offline do server/tv.js com sala/economia falsas e brain null. Uso: node tools/test_tv.mjs
import { Tv, urgent } from "../server/tv.js";
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

console.log(fails ? `${fails} falha(s)` : "tudo ok");
process.exit(fails ? 1 : 0);
