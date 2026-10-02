// Ringue no navegador headless (MUDO, sem screenshot): a pagina nasce em cima da lona olhando pro placar (?cam=),
// entra na fila sozinha, um bot WS manda /ringue, a luta comeca: a pagina tem que obedecer o teleporte pro corner,
// tomar um nocaute pelo "pv" do bot e nao soltar erro no console. Uso: node tools/verify_ring_cdp.mjs [site]
import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SITE = process.argv[2] || "http://127.0.0.1:8791";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9347;
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
    if (m.method === "Runtime.consoleAPICalled" && m.params.type === "error") errors.push(m.params.args.map((a) => a.value ?? a.description).join(" "));};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });

const name = "cdpring" + Date.now().toString(36).slice(-3);
const { targetId } = await cmd("Target.createTarget", { url: `${SITE}/?nome=${name}&cam=${process.env.CAM || process.env.FCAM || "74,22,57,-1.5708,0.25"}` });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Runtime.enable", {}, sessionId);
await sleep(15000);
if (process.env.CAM) {
    // Comparacao: so carrega numa camera qualquer e conta os erros do console
    await sleep(8000);
    console.log("console errors (CAM):", errors.length ? [...new Set(errors)] : "nenhum");
    chrome.kill();
    process.exit(0);
}

const bot = new WebSocket(SITE.replace(/^http/, "ws") + "/ws");
await new Promise((r) => (bot.onopen = r));
let me = 0, page = 0, pagePos = null;
const snaps = [], said = [];
bot.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.t === "welcome") {
        me = m.id;
        page = (m.players || []).find((p) => p[1] === name)?.[0] || 0;
    }
    if (m.t === "p" && m.id === page) pagePos = m.p;
    if (m.t === "pl" && m.k === "ringue" && !m.a) snaps.push(m);
    if (m.t === "chat" && m.n === "RINGUE") said.push(m.m);
};
bot.send(JSON.stringify({ t: "hello", n: "cdpringbot" }));
await sleep(1500);
const queued = snaps.at(-1)?.q?.includes(name) || snaps.at(-1)?.king?.[0] === name;
bot.send(JSON.stringify({ t: "p", p: [100, 20, 100], y: 0, c: 0 }));
bot.send(JSON.stringify({ t: "chat", m: "/ringue" }));
let started = false, cornered = false, ko = false;
for (let t = 0; t < 40 && !(started && cornered); t++) {
    await sleep(250);
    const s = snaps.at(-1);
    started ||= s?.ph === "count" && s.f.some((f) => f[0] === page);
    cornered = !!pagePos && started && Math.hypot(pagePos[0] - 69.5, pagePos[2] - 49.5) < 1.5;
}
// Bot vai pro outro corner, espera o LUTA e nocauteia a pagina pelo "pv" (Steve com espada)
const BC = [78.5, 22, 58.5];
for (let t = 0; t < 30 && snaps.at(-1)?.ph !== "fight"; t++) {
    bot.send(JSON.stringify({ t: "p", p: BC, y: 0, c: 0 }));
    await sleep(200);
}
for (let i = 0; i < 4; i++) {
    bot.send(JSON.stringify({ t: "p", p: BC, y: 0, c: 0, pv: [[page, 7.2, [0, 0, -1]]] }));
    await sleep(200);
}
await sleep(800);
ko = snaps.some((s) => s.ph === "break" && s.f.find((f) => f[0] === me)?.[3] === 1);
// Painel e decoracao rodando alguns segundos com a luta no ar
await sleep(4000);
bot.send(JSON.stringify({ t: "chat", m: "/ringue sair" }));
await sleep(800);
bot.close();
console.log("pagina:", page, "| na fila pela lona:", queued, "| luta comecou:", started, "| teleporte pro corner:", cornered, pagePos, "| nocaute:", ko);
console.log("chat RINGUE:", said);
console.log("console errors:", errors.length ? errors : "nenhum");
const ok = page && queued && started && cornered && ko && errors.length === 0;
console.log(ok ? "PASS: ringue no navegador ok" : "FAIL");
ws.close();
chrome.kill();
process.exit(ok ? 0 : 1);
