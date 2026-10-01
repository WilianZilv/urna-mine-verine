// Teste headless (mudo) do clipe: serve web/, joga ~15s, aperta B e espera o vídeo cair na pasta de download.
// Uso: node tools/test_clip.mjs [q=high|q=low] [pasta-download] [mobile]
// q=high: buffer rolante (salva na hora). q=low: grava os próximos 10s.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, mkdirSync, readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, extname } from "node:path";

const [q = "q=high", dir = mkdtempSync(join(tmpdir(), "urna-clip-")), mode = ""] = process.argv.slice(2);
const mobile = mode === "mobile";
mkdirSync(dir, { recursive: true });
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = +(process.env.CLIP_CDP_PORT || 9372), HTTP = +(process.env.CLIP_HTTP_PORT || 8772);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };
const server = createServer((req, res) => {
    const f = join("web", decodeURIComponent(req.url.split("?")[0]).replace(/^\/$/, "/index.html"));
    if (!existsSync(f) || statSync(f).isDirectory()) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { "content-type": types[extname(f)] || "application/octet-stream" });
    res.end(readFileSync(f));
}).listen(HTTP);

const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-clip-prof-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
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
    if (m.method === "Runtime.consoleAPICalled" && /error/i.test(m.params.type)) console.log("console:", m.params.args.map((a) => a.value || a.description).join(" "));
    if (m.method === "Runtime.exceptionThrown") console.log("exception:", m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text);
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => {
    const i = ++id;
    pending.set(i, r);
    setTimeout(() => { if (pending.delete(i)) r({ timeout: method }); }, 15000);
    ws.send(JSON.stringify({ id: i, method, params, sessionId }));
});

await cmd("Browser.setDownloadBehavior", { behavior: "allow", downloadPath: dir });
await cmd("Browser.grantPermissions", { origin: `http://127.0.0.1:${HTTP}`, permissions: ["clipboardReadWrite", "clipboardSanitizedWrite"] });
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
// Tom de teste no master do jogo (primeiro GainNode criado) pra conferir que o áudio entra no clipe.
await cmd("Page.addScriptToEvaluateOnNewDocument", { source: "const _cg = AudioContext.prototype.createGain; AudioContext.prototype.createGain = function () { const g = _cg.call(this); if (!window.__master) window.__master = g; return g; };" }, sessionId);
if (process.env.CLIP_WEBM) await cmd("Page.addScriptToEvaluateOnNewDocument", { source: "const _its = MediaRecorder.isTypeSupported; MediaRecorder.isTypeSupported = (t) => !t.startsWith('video/mp4') && _its(t);" }, sessionId);
await cmd("Page.navigate", { url: `http://127.0.0.1:${HTTP}/?nome=clip&${q}` }, sessionId);
await sleep(3000);
if (mobile) {
    for (const type of ["touchStart", "touchEnd"]) await cmd("Input.dispatchTouchEvent", { type, touchPoints: type === "touchStart" ? [{ x: 700, y: 200 }] : [] }, sessionId);
} else {
    for (const type of ["mousePressed", "mouseReleased"]) await cmd("Input.dispatchMouseEvent", { type, x: 640, y: 360, button: "left", clickCount: 1 }, sessionId);
}
await cmd("Runtime.evaluate", { expression: "{ const c = window.__master.context, o = c.createOscillator(), g = c.createGain(); g.gain.value = 0.3; o.connect(g).connect(window.__master); o.start(); }" }, sessionId);
await sleep(+(process.env.CLIP_PLAY_MS || 12000));
const t0 = Date.now();
if (mobile) {
    // Botão CLIPE: r = 412*0.085 = 35.02, centro (r*7.7+8, sh*0.3)
    const r = Math.max(412 * 0.085, 26);
    const pt = { x: r * 7.7 + 8, y: 412 * 0.3 };
    for (const type of ["touchStart", "touchEnd"]) { await cmd("Input.dispatchTouchEvent", { type, touchPoints: type === "touchStart" ? [pt] : [] }, sessionId); await sleep(100); }
} else {
    for (const type of ["keyDown", "keyUp"]) { await cmd("Input.dispatchKeyEvent", { type, key: "b", code: "KeyB", windowsVirtualKeyCode: 66 }, sessionId); await sleep(120); }
}
let file = null;
for (let i = 0; i < 60 && !file; i++) {
    await sleep(500);
    file = readdirSync(dir).find((f) => /^urna-clip-\d{6}\.(webm|mp4)$/.test(f));
}
await sleep(1500);
const clip = await cmd("Runtime.evaluate", { expression: "navigator.clipboard.readText().catch(e => 'ERR ' + e)", awaitPromise: true }, sessionId);
console.log(file ? `file: ${join(dir, file)} ${statSync(join(dir, file)).size} bytes after ${((Date.now() - t0) / 1000).toFixed(1)}s` : "NO FILE");
console.log("clipboard:", clip.result?.value);
ws.close();
chrome.kill();
server.close();
process.exit(file ? 0 : 1);
