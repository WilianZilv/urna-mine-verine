// FPS headless (mudo) nas câmeras dos lugares (Avenida dos Poderes + distrito sul), via window.urnaProf (?debug=1).
// Uso: node tools/perf_places.mjs [qualidades separadas por vírgula, ex: "low,high" ("" = auto)] [segundos por câmera] [mobile]
// Serve web/ (ou URNA_WEB) em SHOT_HTTP_PORT; Chrome CDP em SHOT_CDP_PORT. Conta também os avisos de textura GL apagada.
// "rt repaints/s" = glGenerateMipmap por segundo (todo painel em render target gera mipmaps ao repintar).
// PERF_CAMS=bolsa,tv filtra câmeras; PERF_SHOTS=<pasta> salva um PNG por câmera.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, extname } from "node:path";

const [qList = "low,high", secsArg = "8", mode = ""] = process.argv.slice(2);
const qs = qList.split(",");
const secs = +secsArg;
const mobile = mode === "mobile";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = +(process.env.SHOT_CDP_PORT || 9371), HTTP = +(process.env.SHOT_HTTP_PORT || 8771);
// x,y,z,yaw,pitch (yaw 0 = +x, PI/2 = +z)
const CAMS = {
    congresso: "100,24,98.5,3.1416,0",
    avenida: "160,40,104,0,-0.25",
    bolsa: "222,24,86,0,0",
    banco: "170,26,237.5,0,0.1",
    terminal: "200.5,27,254,1.5708,0",
    tv: "64.5,26,242,1.5708,0.1",
    sul: "150,45,240,0.3,-0.3",
    bolsa_longe: "180,30,86,0,0",
    tv_longe: "64.5,30,195,1.5708,0",
};
const only = process.env.PERF_CAMS ? process.env.PERF_CAMS.split(",") : Object.keys(CAMS);
// A/B intercalado: PERF_WEBS="antes=D:\\a,depois=D:\\b" (cada pasta = cópia de web/ com seu urna.wasm); PERF_ROUNDS repete.
const webs = (process.env.PERF_WEBS || `web=${process.env.URNA_WEB || "web"}`).split(",").map((s) => s.split("="));
const rounds = +(process.env.PERF_ROUNDS || 1);
let webDir = webs[0][1];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };
const server = createServer((req, res) => {
    const f = join(webDir, decodeURIComponent(req.url.split("?")[0]).replace(/^\/$/, "/index.html"));
    if (!existsSync(f)) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { "content-type": types[extname(f)] || "application/octet-stream" });
    res.end(readFileSync(f));
}).listen(HTTP);

const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-perf-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) {
    await sleep(250);
    ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null);
}
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0, glWarn = 0;
const glSrc = new Set();
const pending = new Map();
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
    if (m.method === "Runtime.consoleAPICalled") {
        const t = m.params.args.map((a) => a.value).join(" ");
        if (/already deleted/.test(t)) glWarn++;
        if (t.startsWith("GLDEL ")) glSrc.add(t.slice(6));
    }
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
// Quem apagou a textura que depois foi bindada: primeira função do jogo na pilha do glDeleteTextures.
await cmd("Page.addScriptToEvaluateOnNewDocument", { source: `
window.__gl = { mip: 0 };
for (const C of [WebGLRenderingContext, WebGL2RenderingContext]) {
    const mip = C.prototype.generateMipmap;
    C.prototype.generateMipmap = function (...a) { __gl.mip++; return mip.apply(this, a); };
}
document.addEventListener("DOMContentLoaded", () => {
    const env = importObject.env, del = env.glDeleteTextures, by = {};
    env.glDeleteTextures = (n, p) => {
        const line = new Error().stack.split("\\n").map((l) => l.replace(/^.*?[.]wasm[.]/, "")).find((l) => l.includes("urna_mine_verine")) || "?";
        const fn = line.replace(/ [(].*$/, "").replace(/17h[0-9a-f]{16}E/, "").replace(/_ZN\\d+|[$]LT[$]|[$]GT[$]|[$]u20[$]|[$]u7b[$]|[$]u7d[$]/g, " ").trim().slice(0, 90);
        for (const t of getArray(p, Int32Array, n)) by[t] = fn;
        return del(n, p);
    };
    const err = console.error.bind(console);
    console.error = (...a) => { const m = String(a[0]).match(/already deleted texture ID (\\d+)/); if (m) console.log("GLDEL " + (by[m[1]] || "?")); err(...a); };
});` }, sessionId);
if (mobile) {
    await cmd("Emulation.setDeviceMetricsOverride", { width: 915, height: 412, deviceScaleFactor: 2, mobile: true }, sessionId);
    await cmd("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 }, sessionId);
    await cmd("Emulation.setCPUThrottlingRate", { rate: 4 }, sessionId);
} else {
    await cmd("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false }, sessionId);
}
const eval_ = async (expr) => (await cmd("Runtime.evaluate", { expression: expr, returnByValue: true }, sessionId)).result?.value;
const avg = (xs, f) => xs.reduce((s, x) => s + f(x), 0) / Math.max(1, xs.length);
const rows = [];
for (let round = 0; round < rounds; round++) for (const q of qs) for (const name of only) for (const [web, dir] of webs) {
    webDir = dir;
    glWarn = 0;
    glSrc.clear();
    await cmd("Page.navigate", { url: `http://127.0.0.1:${HTTP}/?nome=perf&debug=1&cam=${CAMS[name]}${q ? `&q=${q}` : ""}` }, sessionId);
    await sleep(+(process.env.PERF_WARMUP || 12000));
    const samples = [];
    const gl0 = await eval_("JSON.stringify(__gl)"), t0 = Date.now();
    for (let i = 0; i < secs; i++) {
        await sleep(1000);
        const p = await eval_("JSON.stringify(window.urnaProf || null)");
        if (p && p !== "null") samples.push(JSON.parse(p));
    }
    const [a, b] = [JSON.parse(gl0), JSON.parse(await eval_("JSON.stringify(__gl)"))];
    const span = (Date.now() - t0) / 1000;
    if (process.env.PERF_SHOTS) {
        const shot = await cmd("Page.captureScreenshot", { format: "png" }, sessionId);
        writeFileSync(join(process.env.PERF_SHOTS, `${web}-${q || "auto"}-${name}.png`), Buffer.from(shot.data, "base64"));
    }
    const r = { web, q: q || "auto", cam: name, fps: avg(samples, (x) => x.fps), rt: avg(samples, (x) => x.ms["lab rt"]), actors: avg(samples, (x) => x.ms.actors), cubes: avg(samples, (x) => x.cubes), texts: avg(samples, (x) => x.texts), gl: glWarn, mips: (b.mip - a.mip) / span };
    rows.push(r);
    console.log(`${r.web.padEnd(6)} ${r.q.padEnd(5)} ${r.cam.padEnd(10)} fps ${r.fps.toFixed(1).padStart(5)}  rt ${r.rt.toFixed(2).padStart(6)}ms  actors ${r.actors.toFixed(2).padStart(6)}ms  cubes ${r.cubes.toFixed(0).padStart(5)}  texts ${r.texts.toFixed(0).padStart(4)}  rt repaints/s ${r.mips.toFixed(1).padStart(5)}  glwarn ${r.gl}${glSrc.size ? ` (apagada em ${[...glSrc].join(", ")})` : ""}`);
}
for (const q of qs) for (const [web] of webs) {
    const sel = rows.filter((r) => r.q === (q || "auto") && r.web === web);
    console.log(`media ${web.padEnd(6)} ${(q || "auto").padEnd(5)} fps ${avg(sel, (r) => r.fps).toFixed(1)}  rt ${avg(sel, (r) => r.rt).toFixed(2)}ms  actors ${avg(sel, (r) => r.actors).toFixed(2)}ms  cubes ${avg(sel, (r) => r.cubes).toFixed(0)}  rt repaints/s ${avg(sel, (r) => r.mips).toFixed(1)}  glwarn ${sel.reduce((s, r) => s + r.gl, 0)}`);
}
ws.close();
chrome.kill();
server.close();
process.exit(0);
