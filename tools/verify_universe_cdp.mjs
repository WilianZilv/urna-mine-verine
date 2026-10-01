// Teste headless (mudo): guarda-roupa aplica skin (servidor carimba "av"), remoto com skin aparece, demo do hub mostra o avatar.
// Precisa do mod "astronauta" ativo (node tools/verify_universe.mjs antes).
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SITE = "https://urna-mine-verine.wilianzilv.workers.dev";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9338;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let fails = 0;
const must = (ok, what, extra = "") => {
    console.log(`${ok ? "PASS" : "FAIL"}: ${what}${extra ? " " + extra : ""}`);
    if (!ok) fails++;
};
const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-cdp-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) {
    await sleep(250);
    ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null);
}
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map(), contexts = [];
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.method === "Runtime.executionContextCreated") contexts.push(m.params.context);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });
const shot = async (sid, name) => {
    const s = await cmd("Page.captureScreenshot", { format: "png" }, sid);
    const f = join(tmpdir(), name);
    writeFileSync(f, Buffer.from(s.data, "base64"));
    console.log("screenshot:", f);
};

// observador WS: ve o "p" do jogador do navegador
const name = `uvcdp${Date.now().toString(36).slice(-4)}`;
const watch = new WebSocket(SITE.replace("https", "wss") + "/ws");
const seen = [];
let ids = new Map();
watch.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.t === "welcome") for (const [i, n] of m.players) ids.set(i, n);
    if (m.t === "join") ids.set(m.id, m.n);
    if (m.t === "p" && ids.get(m.id) === name) seen.push(m);
};
await new Promise((r) => (watch.onopen = r));
watch.send(JSON.stringify({ t: "hello", n: "uvwatch" }));

const { targetId } = await cmd("Target.createTarget", { url: `${SITE}/?nome=${name}` });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Page.enable", {}, sessionId);
await sleep(20000);
const key = async (k, code, vk, text) => {
    await cmd("Input.dispatchKeyEvent", { type: "keyDown", key: k, code, windowsVirtualKeyCode: vk, text }, sessionId);
    await sleep(40);
    await cmd("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: vk }, sessionId);
    await sleep(80);
};
const click = async (x, y) => {
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
        await cmd("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 }, sessionId);
        await sleep(60);
    }
};
must(seen.length > 0 && !seen.at(-1).av, "navegador sem skin no comeco");

// 1) C abre o menu; card 1 do guarda-roupa = primeira skin (astronauta). Layout = Universe::cards (1280x720, 2 cards).
await key("c", "KeyC", 67, "c");
await sleep(600);
await shot(sessionId, "urna-uv-wardrobe.png");
const vp = (await cmd("Runtime.evaluate", { expression: "[innerWidth, innerHeight]", returnByValue: true }, sessionId)).result.value;
const [sw, sh] = vp, cw = Math.min((sw - 24) / 2, 170);
await click(sw / 2 + 4 + cw / 2, sh / 2 - Math.min(sh * 0.36, 200) / 2 - 108 + 24);
await sleep(2500);
const last = seen.at(-1);
must(last?.av?.startsWith("astronauta@"), "guarda-roupa: servidor carimba skin no p", last?.av);

// 2) remotos com skin em volta do jogador -> desenhados com o avatar
const bots = [];
if (last) {
    const [x, y, z] = last.p;
    const fx = Math.cos(last.y), fz = Math.sin(last.y);
    for (const [d, side] of [[5, 0], [7, 2], [7, -2]]) {
        const [dx, dz] = [fx * d - fz * side, fz * d + fx * side];
        const b = new WebSocket(SITE.replace("https", "wss") + "/ws");
        await new Promise((r) => (b.onopen = r));
        b.send(JSON.stringify({ t: "hello", n: `astro${bots.length}` }));
        await sleep(200);
        b.send(JSON.stringify({ t: "uv_set", mod: "astronauta" }));
        bots.push({ b, p: [x + dx, y, z + dz] });
    }
    for (let i = 0; i < 20; i++) {
        for (const { b, p } of bots) b.send(JSON.stringify({ t: "p", p: [p[0], p[1], p[2] + (i % 2) * 0.3], y: last.y + Math.PI, c: 0 }));
        await sleep(150);
    }
    await shot(sessionId, "urna-uv-remotes.png");
}
for (const { b } of bots) b.close();

// 3) portal demo: sessao real (mesmo nome -> mesma skin) num overlay igual ao web/hub.js, na nossa origem
const tokWs = new WebSocket(SITE.replace("https", "wss") + "/ws");
const tokMsgs = [];
tokWs.onmessage = (e) => tokMsgs.push(JSON.parse(e.data));
await new Promise((r) => (tokWs.onopen = r));
tokWs.send(JSON.stringify({ t: "hello", n: name }));
await sleep(500);
tokWs.send(JSON.stringify({ t: "hub", k: "enter", p: "chuva-de-votos", col: "#ff8800", ch: "steve" }));
let hs;
for (let i = 0; i < 60 && !hs; i++) { await sleep(100); hs = tokMsgs.find((m) => m.t === "hub_s"); }
must(!!hs?.tok, "sessao de portal pro demo");
await cmd("Page.navigate", { url: `${SITE}/hub.txt` }, sessionId);
await sleep(1500);
await cmd("Runtime.enable", {}, sessionId);
const harness = `(() => {
    const info = ${JSON.stringify({ url: hs?.url, origin: hs?.url && new URL(hs.url).origin, tok: hs?.tok, p: "chuva-de-votos" })};
    document.body.innerHTML = "";
    const f = document.createElement("iframe");
    f.setAttribute("sandbox", "allow-scripts allow-same-origin");
    f.style.cssText = "position:fixed;inset:0;width:100%;height:100%;border:0";
    f.src = info.url;
    document.body.appendChild(f);
    const player = JSON.parse(atob(info.tok.split(".")[1].replace(/-/g, "+").replace(/_/g, "/"))).player;
    addEventListener("message", (e) => {
        if (e.source === f.contentWindow && e.data?.type === "upp:hello")
            f.contentWindow.postMessage({ type: "upp:session", v: 1, portalId: info.p, token: info.tok, player, returnUrl: location.origin }, info.origin);
    });
    return info.url;
})()`;
const hr = await cmd("Runtime.evaluate", { expression: harness, returnByValue: true }, sessionId);
console.log("demo url:", hr?.result?.value || JSON.stringify(hr));
await sleep(9000);
await shot(sessionId, "urna-uv-demo.png");
tokWs.close();
const frame = contexts.find((c) => c.origin.includes("urna-hub-demo") && c.auxData?.isDefault);
must(!!frame, "overlay do portal aberto", frame?.origin);
if (frame) {
    const r = await cmd("Runtime.evaluate", { expression: "JSON.stringify({av: !!av, online, me: me.name, others: others.size})", returnByValue: true, contextId: frame.id }, sessionId);
    console.log("demo:", r?.result?.value);
    must(r?.result?.value?.includes('"av":true'), "demo recebeu avatar do passaporte");
}

watch.close();
ws.close();
chrome.kill();
console.log(fails ? `${fails} FALHA(S)` : "TUDO OK");
process.exit(fails ? 1 : 0);
