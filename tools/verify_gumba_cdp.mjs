// Teste headless (mudo, sem screenshot): página vira host, um bot WS lê o snapshot e confere os COGUMAUs
// ("gb": 6 andando na arena) e que não houve erro no console. Uso: node tools/verify_gumba_cdp.mjs [site]
import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SITE = process.argv[2] || "http://127.0.0.1:8811";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9341;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-cdp-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) {
    await sleep(250);
    ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null);
}
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map(), errors = [];
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
    if (m.method === "Runtime.exceptionThrown") errors.push(m.params.exceptionDetails.text + " " + (m.params.exceptionDetails.exception?.description || ""));
    if (m.method === "Runtime.consoleAPICalled" && m.params.type === "error") errors.push(m.params.args.map((a) => a.value ?? a.description).join(" "));
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });

const { targetId } = await cmd("Target.createTarget", { url: `${SITE}/?nome=cdpgumba` });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Runtime.enable", {}, sessionId);
await sleep(15000);

const bot = new WebSocket(SITE.replace(/^http/, "ws") + "/ws");
await new Promise((r) => (bot.onopen = r));
const snaps = [];
bot.onmessage = (e) => {
    try {
        const m = JSON.parse(e.data);
        if (m.t === "s" && Array.isArray(m.gb)) snaps.push(m);
    } catch {}
};
bot.send(JSON.stringify({ t: "hello", n: "cdpgumbabot" }));
await sleep(6000);

// Tiro de jogador no meio de um COGUMAU: o host tem que matar (vida do grupo 13 some do "n" cheio)
const first = snaps.at(-1);
let killed = false;
if (first) {
    const [x, y, z] = first.gb[0];
    bot.send(JSON.stringify({ t: "a", k: "hit", g: 13, i: 0, d: [0, -1, 0], p: [x, y + 0.85, z], dmg: 50, w: "PISAO" }));
    await sleep(1500);
    killed = snaps.slice(-3).some((s) => (s.n || []).some((e) => e[0] === 13 && e[1] === 0 && e[3] > 0));
}
// Informativo: o ENCANADOR (NPC) caça e pisa os outros sozinho
const hunt = Number(process.env.HUNT_SECS || 0);
if (hunt) {
    const from = snaps.length;
    await sleep(hunt * 1000);
    const dead = new Set(snaps.slice(from).flatMap((s) => (s.n || []).filter((e) => e[0] === 13 && e[1] !== 0 && e[3] > 0).map((e) => e[1])));
    console.log(`cogumaus 1-5 derrubados em ${hunt}s sem o bot:`, [...dead]);
}
bot.close();

const last = snaps.at(-1);
const moved = first && last && first !== last && first.gb.some((g, k) => Math.hypot(g[0] - last.gb[k][0], g[2] - last.gb[k][2]) > 0.3);
console.log("snapshots com gb:", snaps.length, "| cogumaus:", last?.gb.length, "| exemplo:", JSON.stringify(last?.gb[1]));
console.log("console errors:", errors.length ? errors : "nenhum");
const ok = snaps.length > 0 && last.gb.length === 6 && moved && killed && errors.length === 0;
console.log(ok ? "PASS: 6 COGUMAUs patrulhando, pisão mata, sem erro" : `FAIL: snaps=${snaps.length} moved=${moved} killed=${killed} errors=${errors.length}`);
ws.close();
chrome.kill();
process.exit(ok ? 0 : 1);
