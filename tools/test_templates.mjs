// Teste headless dos templates do Hub (web/hub-templates): node tools/test_templates.mjs
// 1) standalone: cada template servido de web/ num servidor estatico, sem Urna -> convidado; input via CDP muda o placar.
// 2) dentro do "Urna": uma pagina falsa na origem do Urna (Fetch do CDP) faz o papel do overlay e responde upp:session;
//    confere nome, carteira (passaporte falso), upp:ready, evento de score e upp:exit.
// O SDK vem sempre de web/sdk (interceptado); three.js vem da CDN de verdade (precisa de internet).
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, extname } from "node:path";
import assert from "node:assert/strict";

const CHROME = process.env.CHROME || "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const HTTP = +(process.env.TPL_HTTP_PORT || 8772), CDP = +(process.env.TPL_CDP_PORT || 9372);
const SHOTS = process.env.TPL_SHOTS || "D:\\tmp\\urna-wt\\shots";
const URNA = "https://urna-mine-verine.wilianzilv.workers.dev", GAME = "https://tpl.urna.test";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".mjs": "text/javascript", ".json": "application/json", ".md": "text/markdown" };
mkdirSync(SHOTS, { recursive: true });

function file(root, path) {
    let f = join(root, decodeURIComponent(path.split("?")[0]));
    if (existsSync(f) && statSync(f).isDirectory()) f = join(f, "index.html");
    return existsSync(f) ? f : null;
}
const server = createServer((req, res) => {
    const f = file("web", req.url);
    if (!f) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { "content-type": types[extname(f)] || "application/octet-stream" });
    res.end(readFileSync(f));
}).listen(HTTP, "127.0.0.1");

const harness = (g) => `<!DOCTYPE html><html><head><meta charset="utf-8"><title>urna falso</title></head>
<body style="margin:0;background:#000;overflow:hidden">
<iframe id="f" src="${GAME}/${g}/" sandbox="allow-scripts allow-same-origin allow-pointer-lock" allow="fullscreen; gamepad; autoplay"
  style="display:block;border:0;width:100vw;height:100vh"></iframe>
<script>
window.__msgs = [];
addEventListener("message", (e) => {
  if (e.origin !== "${GAME}") return;
  const m = e.data; __msgs.push(m);
  if (m && m.type === "upp:hello") e.source.postMessage({ type: "upp:session", v: 1, portalId: m.portalId, token: "tok-teste",
    player: { name: "Zeca Teste", color: "#ff3d7f", character: "eleitor" }, returnUrl: location.origin + "/?hub=" + m.portalId,
    verifyUrl: location.origin + "/api/portals/verify" }, e.origin);
});
</script></body></html>`;

const userDir = mkdtempSync(join(tmpdir(), "urna-tpl-"));
const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${CDP}`, `--user-data-dir=${userDir}`,
    "--window-size=1280,720", "--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--no-first-run",
    "--disable-features=IsolateOrigins,site-per-process", "--disable-site-isolation-trials", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) {
    await sleep(250);
    ver = await fetch(`http://127.0.0.1:${CDP}/json/version`).then((r) => r.json()).catch(() => null);
}
assert.ok(ver, "chrome nao abriu o CDP");
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let seq = 0;
const pending = new Map(), handlers = [];
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || { error: m.error }); pending.delete(m.id); }
    else if (m.method) for (const h of handlers) h(m);
};
let sid;
const cmd = (method, params = {}, sessionId = sid) => new Promise((r) => {
    const i = ++seq;
    pending.set(i, r);
    setTimeout(() => { if (pending.delete(i)) { console.log("cdp timeout:", method); r({ error: "timeout " + method }); } }, 20000);
    ws.send(JSON.stringify({ id: i, method, params, sessionId }));
});

const errors = [], contexts = new Map();
handlers.push((m) => {
    const p = m.params;
    if (m.method === "Runtime.exceptionThrown") errors.push("exception: " + (p.exceptionDetails.exception?.description || p.exceptionDetails.text));
    if (m.method === "Runtime.consoleAPICalled" && p.type === "error") errors.push("console.error: " + p.args.map((a) => a.value ?? a.description).join(" "));
    if (m.method === "Log.entryAdded" && p.entry.level === "error") errors.push(`log(${p.entry.source}): ${p.entry.text} ${p.entry.url || ""}`);
    if (m.method === "Runtime.executionContextCreated" && p.context.auxData?.isDefault) contexts.set(p.context.auxData.frameId, p.context);
    if (m.method === "Runtime.executionContextDestroyed") for (const [k, c] of contexts) if (c.id === p.executionContextId) contexts.delete(k);
    if (m.method === "Page.loadEventFired" && loaded) loaded();
    if (m.method === "Fetch.requestPaused") route(p);
});

function fulfill(requestId, code, body, type = "text/html; charset=utf-8") {
    return cmd("Fetch.fulfillRequest", {
        requestId, responseCode: code, body: Buffer.from(body).toString("base64"),
        responseHeaders: [{ name: "content-type", value: type }, { name: "access-control-allow-origin", value: "*" }],
    });
}
function route(p) {
    const u = new URL(p.request.url);
    if (process.env.TPL_DEBUG) console.log("fetch:", p.request.url);
    if (u.origin === GAME) {
        const f = file("web/hub-templates", u.pathname);
        return f ? fulfill(p.requestId, 200, readFileSync(f), types[extname(f)]) : fulfill(p.requestId, 404, "");
    }
    if (u.pathname.startsWith("/sdk/")) {
        const f = file("web", u.pathname);
        return f ? fulfill(p.requestId, 200, readFileSync(f), types[extname(f)]) : fulfill(p.requestId, 404, "");
    }
    if (u.pathname === "/__harness") return fulfill(p.requestId, 200, harness(u.searchParams.get("g")));
    if (u.pathname === "/api/passport") return fulfill(p.requestId, 200, JSON.stringify({
        ok: true, player: { name: "Zeca Teste", color: "#ff3d7f" }, avatar: null, wallet: { coins: 77 }, inventory: [], limits: {},
    }), "application/json");
    return fulfill(p.requestId, 404, "{}", "application/json");
}

const { targetId } = await cmd("Target.createTarget", { url: "about:blank" }, undefined);
({ sessionId: sid } = await cmd("Target.attachToTarget", { targetId, flatten: true }, undefined));
await cmd("Page.enable");
await cmd("Runtime.enable");
await cmd("Log.enable");
await cmd("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false });
await cmd("Fetch.enable", { patterns: [{ urlPattern: `${URNA}/*` }, { urlPattern: `${GAME}/*` }] });

async function ev(expression, contextId) {
    const r = await cmd("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true, ...(contextId ? { contextId } : {}) });
    if (r.exceptionDetails || r.error) throw new Error(`eval falhou: ${expression}: ${JSON.stringify(r.exceptionDetails || r.error)}`);
    return r.result.value;
}
async function waitFor(expression, ms, contextId) {
    for (const t0 = Date.now(); Date.now() - t0 < ms; await sleep(150)) if (await ev(expression, contextId).catch(() => false)) return true;
    throw new Error("timeout esperando: " + expression);
}
const KEYS = { ArrowLeft: 37, ArrowRight: 39, " ": 32 };
async function key(k, type) {
    await cmd("Input.dispatchKeyEvent", { type, key: k, code: k === " " ? "Space" : k, windowsVirtualKeyCode: KEYS[k], text: type === "keyDown" && k === " " ? " " : undefined });
}
async function tap(k, hold = 80) { await key(k, "keyDown"); await sleep(hold); await key(k, "keyUp"); }
async function shot(name) {
    const r = await cmd("Page.captureScreenshot", { format: "png" });
    const f = join(SHOTS, `tpl_${name}.png`);
    writeFileSync(f, Buffer.from(r.data, "base64"));
    console.log("screenshot:", f);
}
let loaded = null;
async function open(url) {
    errors.length = 0;
    const done = new Promise((r) => (loaded = r));
    await cmd("Page.navigate", { url });
    await Promise.race([done, sleep(20000)]);
    loaded = null;
}
/** contextId do documento do jogo dentro do iframe do harness. */
async function gameContext() {
    for (const t0 = Date.now(); Date.now() - t0 < 8000; await sleep(150)) {
        const { frameTree } = await cmd("Page.getFrameTree");
        const child = (frameTree.childFrames || []).map((f) => f.frame).find((f) => f.url.startsWith(GAME));
        const c = child && contexts.get(child.id);
        if (c && c.origin === GAME) return c.id;
    }
    throw new Error("sem contexto do iframe");
}
const relevant = (allowWs) => errors.filter((e) => !/favicon/.test(e) && !(allowWs && /WebSocket/.test(e)));

let failed = 0;
async function test(name, fn) {
    try { await fn(); console.log("ok  ", name); }
    catch (e) { failed++; console.log("FAIL", name, "-", e.message); }
}

const local = `http://127.0.0.1:${HTTP}/hub-templates`;

await test("index lista os dois templates", async () => {
    await open(`${local}/`);
    await waitFor("document.readyState === 'complete'", 5000);
    const hrefs = await ev("[...document.querySelectorAll('a')].map((a) => a.getAttribute('href'))");
    for (const h of ["canvas2d/", "threejs/"]) assert.ok(hrefs.includes(h), "sem link " + h);
    assert.deepEqual(relevant(), []);
});

await test("canvas2d standalone: convidado, loop roda, input pega votos", async () => {
    await open(`${local}/canvas2d/`);
    await waitFor("window.game && game.time < 29.5", 8000);
    const s0 = await ev("({ name: game.name, connected: game.connected, state: game.state, px: game.px, sdk: UrnaPortal.state })");
    assert.equal(s0.connected, false);
    assert.equal(s0.name, "convidado");
    assert.equal(s0.sdk, "standalone");
    assert.equal(s0.state, "play");
    await tap("ArrowLeft", 300);
    assert.ok(await ev("game.px") < s0.px, "seta esquerda nao mexeu a urna");
    // bot: persegue a cedula mais baixa e foge de TNT
    const t0 = Date.now();
    while (Date.now() - t0 < 15000 && (await ev("game.score")) < 3) {
        const dir = await ev(`(() => {
            const it = game.items.filter((i) => i.k !== "tnt" && i.y < 0.95).sort((a, b) => b.y - a.y)[0];
            if (!it) return 0;
            const d = it.x - game.px;
            return Math.abs(d) < 0.02 ? 0 : Math.sign(d);
        })()`);
        if (dir) await tap(dir > 0 ? "ArrowRight" : "ArrowLeft", 60); else await sleep(60);
    }
    const score = await ev("game.score");
    assert.ok(score >= 3, "placar nao subiu: " + score);
    await shot("canvas2d");
    assert.deepEqual(relevant(), []);
});

await test("canvas2d no Urna: sessao, carteira, ready, score, conquista, exit", async () => {
    await open(`${URNA}/__harness?g=canvas2d`);
    await waitFor(`window.__msgs && __msgs.some((m) => m.type === "upp:ready")`, 8000);
    const ctx = await gameContext();
    await waitFor("game.connected && game.coins === 77", 5000, ctx);
    assert.equal(await ev("game.name", ctx), "Zeca Teste");
    assert.equal(await ev("game.color", ctx), "#ff3d7f");
    assert.equal(await ev("getComputedStyle(document.getElementById('exit')).display", ctx), "inline-block");
    await ev("game.score = 31; game.time = 0.05", ctx);
    await waitFor(`__msgs.some((m) => m.type === "upp:event" && m.event.type === "score" && m.event.value === 31)`, 3000);
    await waitFor(`__msgs.some((m) => m.type === "upp:event" && m.event.type === "achievement")`, 3000);
    await shot("canvas2d_urna");
    await ev("document.getElementById('exit').click()", ctx);
    await waitFor(`__msgs.some((m) => m.type === "upp:exit")`, 3000);
    assert.deepEqual(relevant(), []);
});

await test("threejs standalone: convidado, WebGL, pulos contam", async () => {
    await open(`${local}/threejs/`);
    await waitFor("window.game && game.time < 29.5", 20000);
    const s0 = await ev("({ name: game.name, connected: game.connected, sdk: UrnaPortal.state, auto: UrnaPortal.auto.enabled, gl: !!document.querySelector('canvas').getContext('webgl2') })");
    assert.equal(s0.connected, false);
    assert.equal(s0.name, "convidado");
    assert.equal(s0.sdk, "standalone");
    assert.equal(s0.gl, true);
    await tap(" ");
    await waitFor("game.score === 1", 2000);
    await sleep(200);
    await tap(" ");
    await waitFor("game.score === 2", 2000);
    await sleep(700);
    await shot("threejs");
    assert.deepEqual(relevant(), []);
});

await test("threejs no Urna: sessao, carteira, modo auto, score, exit", async () => {
    await open(`${URNA}/__harness?g=threejs`);
    await waitFor(`window.__msgs && __msgs.some((m) => m.type === "upp:ready")`, 20000);
    const ctx = await gameContext();
    await waitFor("window.game && game.connected && game.coins === 77", 8000, ctx);
    assert.equal(await ev("document.getElementById('who').textContent", ctx), "Zeca Teste - carteira: 77 moedas ficticias");
    assert.equal(await ev("UrnaPortal.auto.enabled", ctx), true);
    await sleep(800);
    await shot("threejs_urna");
    await ev("game.score = 7; game.time = 0.05", ctx);
    await waitFor(`__msgs.some((m) => m.type === "upp:event" && m.event.type === "score" && m.event.value === 7)`, 3000);
    await ev("document.getElementById('exit').click()", ctx);
    await waitFor(`__msgs.some((m) => m.type === "upp:exit")`, 3000);
    // presenca abre WebSocket de verdade com token falso: erro esperado aqui, ignorado
    assert.deepEqual(relevant(true), []);
});

ws.close();
chrome.kill();
server.close();
console.log(failed ? `test_templates: ${failed} falha(s)` : "test_templates: ok");
process.exit(failed ? 1 : 0);
