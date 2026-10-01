// E2E ao vivo (Chrome headless MUDO): presenca entre jogos.
//  A e B entram na Ilha (SDK data-auto + API explicita): cada um ve o outro em 3D e nos cards do overlay do Urna.
//  C entra num jogo 2D SEM SDK (/sem-sdk na origem do demo): o overlay do Urna (web/presence.js) mostra todo mundo.
//  Reacao/chat rapido do overlay chegam no outro overlay e no avatar 3D da ilha. node tools/verify_presence_cdp.mjs
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SITE = process.env.SITE || "https://urna-mine-verine.wilianzilv.workers.dev";
const PORTAL = "chuva-de-votos";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9342;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let fails = 0;
const must = (ok, what, extra = "") => {
    console.log(`${ok ? "PASS" : "FAIL"}: ${what}${extra ? " " + extra : ""}`);
    if (!ok) fails++;
};
const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-pres-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) { await sleep(250); ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null); }
const cdp = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (cdp.onopen = r));
let id = 0;
const pending = new Map(), contexts = [];
cdp.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.method === "Runtime.executionContextCreated") contexts.push({ ...m.params.context, sid: m.sessionId });
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); cdp.send(JSON.stringify({ id: i, method, params, sessionId })); });
const ev = async (tab, expression, frame = false) => {
    const ctx = frame ? contexts.filter((c) => c.sid === tab.sid && c.origin.includes("urna-hub-demo") && c.auxData?.isDefault).at(-1) : null;
    if (frame && !ctx) return null;
    const r = await cmd("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true, ...(ctx ? { contextId: ctx.id } : {}) }, tab.sid);
    return r?.result?.value;
};
const shot = async (tab, name) => {
    const s = await cmd("Page.captureScreenshot", { format: "png" }, tab.sid);
    const f = join(tmpdir(), name);
    writeFileSync(f, Buffer.from(s.data, "base64"));
    console.log("screenshot:", f);
};

/// Sessao de portal real (mesmo caminho do jogo: WS da vila -> {t:"hub", k:"enter"} -> token assinado).
async function session(name, col, skin) {
    const w = new WebSocket(SITE.replace("https", "wss") + "/ws");
    const inbox = [];
    w.onmessage = (e) => inbox.push(JSON.parse(e.data));
    await new Promise((r) => (w.onopen = r));
    w.send(JSON.stringify({ t: "hello", n: name }));
    await sleep(400);
    if (skin) {
        w.send(JSON.stringify({ t: "uv_set", mod: skin }));
        await sleep(600);
    }
    w.send(JSON.stringify({ t: "hub", k: "enter", p: PORTAL, col, ch: "steve" }));
    for (let i = 0; i < 60; i++) { await sleep(100); const h = inbox.find((m) => m.t === "hub_s"); if (h) return { w, h }; }
    throw new Error("sem hub_s");
}
/// Aba na NOSSA origem com o overlay de verdade (web/hub.js + web/presence.js), sem o wasm.
async function tab(name, col, path, skin) {
    const { w, h } = await session(name, col, skin);
    const { targetId } = await cmd("Target.createTarget", { url: `${SITE}/hub.txt`, newWindow: true });
    const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
    const t = { name, w, sid: sessionId };
    await cmd("Runtime.enable", {}, sessionId);
    await cmd("Page.enable", {}, sessionId);
    await sleep(1200);
    const url = new URL(path, h.url).href;
    const r = await ev(t, `(async () => {
        document.open(); document.write("<!DOCTYPE html><html><head><meta charset='utf-8'></head><body style='margin:0;background:#0d0519'></body></html>"); document.close();
        window.miniquad_add_plugin = () => {};
        for (const s of ["/hub.js", "/sdk/urna-avatar-canvas2d.js", "/presence.js"]) await new Promise((ok, no) => { const e = document.createElement("script"); e.src = s; e.onload = ok; e.onerror = no; document.head.appendChild(e); });
        UrnaHubOverlay.open(${JSON.stringify({ url, origin: new URL(url).origin, tok: h.tok, p: PORTAL, name: h.name, ret: h.ret })});
        return !!document.querySelector("#upp-overlay iframe") && !!document.getElementById("upp-pres");
    })()`);
    must(r === true, `${name}: overlay do portal + camada de presenca abertos`, url);
    return t;
}

const tag = Date.now().toString(36).slice(-4);
// A veste a skin "astronauta" (mod avatar, se ativo): busto no card via UrnaAvatar2D e modelo 3D na ilha.
const A = await tab(`ilhaA${tag}`, "#ff4f8b", "/", "astronauta");
const B = await tab(`ilhaB${tag}`, "#00e5ff", "/");
await sleep(14000);
await ev(A, "Ilha.player.pos.x -= 2.5, Ilha.player.camYaw += 0.6", true);
await sleep(2000);

// 1) ilha: os dois se veem em 3D (SDK auto + API explicita)
for (const [me, other] of [[A, B], [B, A]]) {
    const v = JSON.parse((await ev(me, `JSON.stringify({ st: document.getElementById("st").textContent, auto: UrnaPortal.auto.enabled, n: Ilha.others.size, pos: [...Ilha.others.values()].map((a) => a.root.position.toArray().map((x) => +x.toFixed(1))), inScene: !!Ilha.scene.getObjectByName("urna-auto-players") })`, true)) || "{}");
    must(v.n >= 1 && v.inScene && v.auto, `${me.name} ve ${other.name} em 3D na ilha`, JSON.stringify(v));
}
// 2) overlay do Urna: cards dos dois em cada aba
for (const t of [A, B]) {
    const v = await ev(t, `JSON.stringify({ s: UrnaPresenceOverlay.state, cards: [...document.querySelectorAll("#upp-pres .pp-card .pp-name")].map((e) => e.textContent), count: document.querySelector("#upp-pres .pp-count").textContent })`);
    must(/"players":2/.test(v) && /2 jogadores aqui/.test(v), `${t.name}: overlay mostra 2 jogadores`, v);
}
const bustB = await ev(B, `!!document.querySelector("#upp-pres .pp-card:not(.me) .pp-bust canvas")`);
must(bustB === true, "B ve o busto do avatar do passaporte de A no card (canvas2d)");
// 3) reacao + chat rapido: A manda pelo overlay; B ve voar no overlay e no avatar 3D
await ev(A, `UrnaPresenceOverlay.emote("wave"), UrnaPresenceOverlay.say("oi da ilha!")`);
await sleep(700);
const fxB = await ev(B, `JSON.stringify({ fly: [...document.querySelectorAll("#upp-pres .pp-fly")].map((e) => e.textContent), bubble: [...document.querySelectorAll("#upp-pres .pp-bubble")].map((e) => e.textContent) })`);
must(/oi da ilha!/.test(fxB) && /\u{1F44B}/u.test(fxB), "B ve reacao voando + balao de chat do A no overlay", fxB);
const fx3d = await ev(B, `[...Ilha.others.values()].some((a) => a.fx || a.say)`, true);
must(fx3d === true, "reacao/balao tambem aparece no avatar 3D do A dentro da ilha (SDK)");
await shot(A, "urna-presence-ilha-A.png");
await shot(B, "urna-presence-ilha-B.png");

// 4) jogo SEM SDK: C so existe pelo overlay; A/B veem C nos cards; cliques passam pro iframe
const C = await tab(`semsdkC${tag}`, "#ffd23d", "/sem-sdk");
await sleep(6000);
const vc = await ev(C, `JSON.stringify({ s: UrnaPresenceOverlay.state, count: document.querySelector("#upp-pres .pp-count").textContent, hit: document.elementFromPoint(640, 400).tagName, bar: document.elementFromPoint(...(() => { const b = document.querySelector("#upp-pres .pp-bar button").getBoundingClientRect(); return [b.x + 5, b.y + 5]; })()).tagName, sdk: false })`);
must(/"players":3/.test(vc) && /3 jogadores aqui/.test(vc), "C (jogo sem SDK) ve os 3 no overlay", vc);
must(/"hit":"IFRAME"/.test(vc) && /"bar":"BUTTON"/.test(vc), "clique no meio vai pro jogo; so os botoes do overlay pegam clique");
const sees = await ev(A, `[...document.querySelectorAll("#upp-pres .pp-card")].map((e) => e.textContent).join(" | ")`);
must(sees?.includes(C.name), "A (na ilha) ve C (jogo sem SDK) nos cards", sees);
await ev(C, `UrnaPresenceOverlay.emote("laugh"), UrnaPresenceOverlay.say("sem SDK e eu to aqui")`);
await sleep(700);
await shot(C, "urna-presence-sem-sdk-C.png");
const fxA = await ev(A, `[...document.querySelectorAll("#upp-pres .pp-bubble")].map((e) => e.textContent).join("|")`);
must(fxA?.includes("sem SDK e eu to aqui"), "A recebe chat rapido de C", fxA);
await shot(A, "urna-presence-ilha-A-ve-C.png");

// 5) jogo three.js com SO a tag <script data-auto>: SDK acha renderer/cena/camera sozinho e poe A e B na cena;
//    a camera de D vira um avatar na ilha de A
const D = await tab(`autoD${tag}`, "#7cff6b", "/auto-3d");
await sleep(12000);
const vd = JSON.parse((await ev(D, `JSON.stringify({ n: UrnaPortal.auto.avatars.size, inScene: [...UrnaPortal.auto.avatars.values()].every((a) => a.root.parent && a.root.parent.parent && a.root.parent.parent.isScene), names: [...UrnaPortal.presence.players.values()].map((p) => p.name) })`, true)) || "{}");
must(vd.n >= 2 && vd.inScene, "auto3d (so data-auto): A e B injetados na cena three.js detectada", JSON.stringify(vd));
const va = await ev(A, `JSON.stringify({ n: Ilha.others.size, k: [...UrnaPortal.presence.players.values()].map((p) => p.name + ":" + (p.s && p.s._k)) })`, true);
must(/"n":[2-9]/.test(va) && va.includes(`${D.name}:cam`), "A ve D na ilha (avatar onde a camera de D esta)", va);
await shot(D, "urna-presence-auto3d-D.png");

for (const t of [A, B, C, D]) t.w.close();
cdp.close();
chrome.kill();
console.log(fails ? `${fails} FALHA(S)` : "TUDO OK");
process.exit(fails ? 1 : 0);
