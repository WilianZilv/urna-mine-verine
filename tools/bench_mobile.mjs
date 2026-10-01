// Benchmark headless (mudo) em emulação de celular: serve web/ local, CPU 6x mais lenta, lê window.urnaProf.
// Uso: node tools/bench_mobile.mjs [query extra, ex: "q=low"] [segundos]
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, extname } from "node:path";

const extra = process.argv[2] || "";
const secs = +(process.argv[3] || 15);
const desktop = extra.includes("desktop");
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9338, HTTP = 8737;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };
const server = createServer((req, res) => {
    const f = join("web", decodeURIComponent(req.url.split("?")[0]).replace(/^\/$/, "/index.html"));
    if (!existsSync(f)) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { "content-type": types[extname(f)] || "application/octet-stream" });
    res.end(readFileSync(f));
}).listen(HTTP);

const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-bench-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
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
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });

const { targetId } = await cmd("Target.createTarget", { url: "about:blank" });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Page.enable", {}, sessionId);
if (!desktop) {
    await cmd("Emulation.setDeviceMetricsOverride", { width: 412, height: 915, deviceScaleFactor: 2.6, mobile: true }, sessionId);
    await cmd("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 }, sessionId);
    await cmd("Emulation.setCPUThrottlingRate", { rate: 6 }, sessionId);
} else {
    await cmd("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false }, sessionId);
}
await cmd("Page.navigate", { url: `http://127.0.0.1:${HTTP}/?nome=bench&debug=1&${extra}` }, sessionId);
await sleep(desktop ? 15000 : 40000);
const eval_ = async (expr) => (await cmd("Runtime.evaluate", { expression: expr, returnByValue: true }, sessionId)).result?.value;
console.log("touch:", await eval_(`navigator.maxTouchPoints + " coarse=" + matchMedia("(pointer: coarse)").matches + " dpr=" + devicePixelRatio + " msaa=" + (glcanvas.getContext("webgl") || glcanvas.getContext("webgl2"))?.getContextAttributes().antialias`));
const samples = [];
for (let i = 0; i < secs; i++) {
    await sleep(1000);
    const p = await eval_("JSON.stringify(window.urnaProf || null)");
    if (p && p !== "null") samples.push(JSON.parse(p));
}
const shot = await cmd("Page.captureScreenshot", { format: "png" }, sessionId);
const png = join(tmpdir(), `urna-bench${desktop ? "-desktop" : ""}.png`);
writeFileSync(png, Buffer.from(shot.data, "base64"));
if (!samples.length) console.log("sem dados de profiler");
else {
    const avg = (f) => samples.reduce((s, x) => s + f(x), 0) / samples.length;
    const last = samples[samples.length - 1];
    console.log(`fps ${avg((x) => x.fps).toFixed(1)}  calls ${avg((x) => x.calls).toFixed(0)}  verts ${(avg((x) => x.verts) / 1000).toFixed(0)}k  cubes ${avg((x) => x.cubes).toFixed(0)}  chunks ${avg((x) => x.chunks).toFixed(0)}  texts ${avg((x) => x.texts).toFixed(0)}  screen ${last.w}x${last.h}  ${last.extra}`);
    for (const k of Object.keys(last.ms)) console.log(`  ${k.padStart(10)} ${avg((x) => x.ms[k]).toFixed(2)} ms`);
}
console.log("screenshot:", png);
ws.close();
chrome.kill();
server.close();
process.exit(0);
