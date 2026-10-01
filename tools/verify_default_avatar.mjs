// E2E ao vivo (Chrome headless MUDO): jogador SEM skin entra na Ilha e vai com o avatar embutido do personagem
// (urna:steve). Confere passaporte, modelo 3D na ilha (UrnaAvatarThree) e busto no card do overlay.
// node tools/verify_default_avatar.mjs
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SITE = process.env.SITE || "https://urna-mine-verine.wilianzilv.workers.dev";
const PORTAL = "chuva-de-votos";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9343;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let fails = 0;
const must = (ok, what, extra = "") => {
    console.log(`${ok ? "PASS" : "FAIL"}: ${what}${extra ? " " + extra : ""}`);
    if (!ok) fails++;
};

async function session(name, col, ch) {
    const w = new WebSocket(SITE.replace("https", "wss") + "/ws");
    const inbox = [];
    w.onmessage = (e) => inbox.push(JSON.parse(e.data));
    await new Promise((r) => (w.onopen = r));
    w.send(JSON.stringify({ t: "hello", n: name }));
    await sleep(400);
    w.send(JSON.stringify({ t: "uv_set", mod: "" }));
    await sleep(400);
    w.send(JSON.stringify({ t: "hub", k: "enter", p: PORTAL, col, ch }));
    for (let i = 0; i < 60; i++) { await sleep(100); const h = inbox.find((m) => m.t === "hub_s"); if (h) return { w, h }; }
    throw new Error("sem hub_s");
}

const tag = Date.now().toString(36).slice(-4);
const sa = await session(`steve${tag}`, "#ff4f8b", "steve");
const pass = await fetch(`${SITE}/api/passport?token=${encodeURIComponent(sa.h.tok)}`).then((r) => r.json());
must(/^urna:steve@/.test(pass.avatar_ref) && pass.avatar?.model?.parts?.length === 6, "passaporte sem skin traz urna:steve", pass.avatar_ref);
const sk = await session(`skate${tag}`, "#00e5ff", "skatista");
const pk = await fetch(`${SITE}/api/passport?token=${encodeURIComponent(sk.h.tok)}`).then((r) => r.json());
must(/^urna:skatista@/.test(pk.avatar_ref) && !!pk.avatar, "personagem skatista -> urna:skatista", pk.avatar_ref);
const pub = await fetch(`${SITE}/api/mods/${encodeURIComponent("urna:steve")}/versions/1.0.0`);
must(pub.ok, "GET /api/mods/urna:steve/versions/1.0.0 publico", String(pub.status));

const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-av-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
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
async function tab({ w, h }, name) {
    const { targetId } = await cmd("Target.createTarget", { url: `${SITE}/hub.txt`, newWindow: true });
    const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
    const t = { name, w, sid: sessionId };
    await cmd("Runtime.enable", {}, sessionId);
    await cmd("Page.enable", {}, sessionId);
    await sleep(1200);
    await ev(t, `(async () => {
        document.open(); document.write("<!DOCTYPE html><html><head><meta charset='utf-8'></head><body style='margin:0;background:#0d0519'></body></html>"); document.close();
        window.miniquad_add_plugin = () => {};
        for (const s of ["/hub.js", "/sdk/urna-avatar-canvas2d.js", "/presence.js"]) await new Promise((ok, no) => { const e = document.createElement("script"); e.src = s; e.onload = ok; e.onerror = no; document.head.appendChild(e); });
        UrnaHubOverlay.open(${JSON.stringify({ url: h.url, origin: h.origin, tok: h.tok, p: PORTAL, name: h.name, ret: h.ret })});
    })()`);
    return t;
}
const A = await tab(sa, sa.h.name);
const B = await tab(sk, sk.h.name);
await sleep(14000);
await ev(A, "Ilha.player.camYaw += Math.PI", true);
await sleep(1500);
const va = JSON.parse((await ev(A, `JSON.stringify({ st: document.getElementById("st").textContent, ref: UrnaPortal.session && UrnaPortal.session.player.avatar })`, true)) || "{}");
must(/avatar do passaporte/.test(va.st || ""), "ilha: A renderiza avatar do passaporte (Steve) via UrnaAvatarThree", JSON.stringify(va));
const bust = await ev(B, `!!document.querySelector("#upp-pres .pp-card:not(.me) .pp-bust canvas")`);
must(bust === true, "B ve busto do Steve de A no card do overlay");
const s1 = await cmd("Page.captureScreenshot", { format: "png" }, A.sid);
writeFileSync(join(tmpdir(), "urna-default-steve-A.png"), Buffer.from(s1.data, "base64"));
const s2 = await cmd("Page.captureScreenshot", { format: "png" }, B.sid);
writeFileSync(join(tmpdir(), "urna-default-steve-B.png"), Buffer.from(s2.data, "base64"));
console.log("screenshots:", join(tmpdir(), "urna-default-steve-A.png"), join(tmpdir(), "urna-default-steve-B.png"));
sa.w.close(); sk.w.close(); cdp.close(); chrome.kill();
console.log(fails ? `${fails} FALHA(S)` : "TUDO OK");
process.exit(fails ? 1 : 0);
