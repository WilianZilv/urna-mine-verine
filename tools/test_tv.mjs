// Teste offline do server/tv.js com sala/economia falsas e brain null. Uso: node tools/test_tv.mjs
import { Tv } from "../server/tv.js";
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

console.log(fails ? `${fails} falha(s)` : "tudo ok");
process.exit(fails ? 1 : 0);
