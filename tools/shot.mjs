// Screenshots headless (mudo) do jogo: serve web/ (ou URNA_WEB), clica no canvas e aperta teclas.
// Uso: node tools/shot.mjs "<query extra>" <saida-prefixo> [mobile] [teclas separadas por vírgula]
// Cada tecla vira um PNG `<prefixo>-<tecla>.png` (sem teclas: `<prefixo>.png`).
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, extname } from "node:path";

const [extra = "", out = join(tmpdir(), "urna-shot"), mode = "", keyList = ""] = process.argv.slice(2);
const mobile = mode === "mobile";
const keys = keyList ? keyList.split(",") : [];
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
// Portas trocaveis pra rodar varios em paralelo; SHOT_URL (ex.: wrangler dev) pula o servidor estatico.
const PORT = +(process.env.SHOT_CDP_PORT || 9341), HTTP = +(process.env.SHOT_HTTP_PORT || 8741);
const BASE = process.env.SHOT_URL || `http://127.0.0.1:${HTTP}`;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };
const server = process.env.SHOT_URL ? { close() { } } : createServer((req, res) => {
    const f = join(process.env.URNA_WEB || "web", decodeURIComponent(req.url.split("?")[0]).replace(/^\/$/, "/index.html"));
    if (!existsSync(f)) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { "content-type": types[extname(f)] || "application/octet-stream" });
    res.end(readFileSync(f));
}).listen(HTTP);

const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-shot-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) {
    await sleep(250);
    ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null);
}
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
    if (m.method === "Runtime.consoleAPICalled" && process.env.SHOT_LOG) console.log("console:", m.params.args.map((a) => a.value).join(" "));
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => {
    const i = ++id;
    pending.set(i, r);
    setTimeout(() => { if (pending.delete(i)) r({ timeout: method }); }, 15000);
    ws.send(JSON.stringify({ id: i, method, params, sessionId }));
});

const { targetId } = await cmd("Target.createTarget", { url: "about:blank" });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Page.enable", {}, sessionId);
await cmd("Runtime.enable", {}, sessionId);
if (mobile) {
    await cmd("Emulation.setDeviceMetricsOverride", { width: 915, height: 412, deviceScaleFactor: 2, mobile: true }, sessionId);
    await cmd("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 }, sessionId);
} else {
    await cmd("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false }, sessionId);
}
await cmd("Page.navigate", { url: `${BASE}/?nome=${process.env.SHOT_NAME || "shot"}&${extra}` }, sessionId);
await sleep(+(process.env.SHOT_WAIT || 14000));
const save = async (f) => {
    const shot = await cmd("Page.captureScreenshot", { format: "png" }, sessionId);
    writeFileSync(f, Buffer.from(shot.data, "base64"));
    console.log("screenshot:", f);
};
const key = async (k, type) => {
    const code = /^\d$/.test(k) ? `Digit${k}` : k.length === 1 ? `Key${k.toUpperCase()}` : k;
    await cmd("Input.dispatchKeyEvent", { type, key: k, code, windowsVirtualKeyCode: k.length === 1 ? k.toUpperCase().charCodeAt(0) : 0 }, sessionId);
};
if (!keys.length) await save(`${out}.png`);
else {
    if (!mobile) {
        for (const type of ["mousePressed", "mouseReleased"]) await cmd("Input.dispatchMouseEvent", { type, x: 640, y: 360, button: "left", clickCount: 1 }, sessionId);
        await sleep(1500);
    }
    for (const k of keys) {
        await key(k, "keyDown");
        await sleep(120);
        await key(k, "keyUp");
        await sleep(1200);
        await save(`${out}-${k}.png`);
    }
}
ws.close();
chrome.kill();
server.close();
process.exit(0);
