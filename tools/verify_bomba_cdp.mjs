// Teste headless (mudo, sem screenshot): BOMBADINHO NPC entra na arena e bomba; tecla 8 vira o personagem e solta bomba;
// um segundo Chrome (cliente não-host) recebe o NPC pelo snapshot.
// node tools/verify_bomba_cdp.mjs   (BASE=http://127.0.0.1:8774 com wrangler dev, ou o site publicado)
import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const BASE = process.env.BASE || "http://127.0.0.1:8774";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const errors = [];

// Um Chrome por cliente (aba de fundo não roda o loop do jogo).
async function open(port, nome) {
    const proc = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${port}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-bomba-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
    let ver;
    for (let i = 0; i < 40 && !ver; i++) {
        await sleep(250);
        ver = await fetch(`http://127.0.0.1:${port}/json/version`).then((r) => r.json()).catch(() => null);
    }
    const ws = new WebSocket(ver.webSocketDebuggerUrl);
    await new Promise((r) => (ws.onopen = r));
    let id = 0;
    const pending = new Map();
    ws.onmessage = (e) => {
        const m = JSON.parse(e.data);
        if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
        if (m.method === "Runtime.exceptionThrown") errors.push(m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text);
        if (m.method === "Runtime.consoleAPICalled" && m.params.type === "error") errors.push(m.params.args.map((a) => a.value ?? a.description).join(" "));
    };
    const raw = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });
    const { targetId } = await raw("Target.createTarget", { url: `${BASE}/?nome=${nome}&debug=1` });
    const { sessionId } = await raw("Target.attachToTarget", { targetId, flatten: true });
    const cmd = (method, params = {}) => raw(method, params, sessionId);
    await cmd("Runtime.enable");
    const extra = async () => JSON.parse((await cmd("Runtime.evaluate", { expression: "JSON.stringify(window.urnaProf||null)", returnByValue: true })).result?.value || "null");
    const key = async (k, code, vk, text) => {
        await cmd("Input.dispatchKeyEvent", { type: "keyDown", key: k, code, windowsVirtualKeyCode: vk, text });
        await sleep(60);
        await cmd("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: vk });
        await sleep(80);
    };
    const close = () => { ws.close(); proc.kill(); };
    return { cmd, extra, key, close };
}

const field = (p, k) => Number((p?.extra || "").match(new RegExp(`${k}=(\\d+)`))?.[1] ?? -1);
const fails = [];
const check = (ok, msg) => { console.log(`${ok ? "ok  " : "FAIL"} ${msg}`); if (!ok) fails.push(msg); };

const host = await open(9341, "cdpbomba");
// NPC: nasce aos ~10s (host = primeiro cliente) e começa a bombar
let p, maxBooms = 0, sawBomb = false, sawNpc = false;
for (let t = 0; t < 75; t++) {
    await sleep(1000);
    p = await host.extra();
    sawNpc ||= field(p, "npc") === 1;
    sawBomb ||= field(p, "bombs") > 0 || field(p, "flames") > 0;
    maxBooms = Math.max(maxBooms, field(p, "booms"));
    if (t % 10 === 0) console.log(t, p?.extra);
    if (sawNpc && maxBooms > 0 && t > 30) break;
}
console.log("extra:", p?.extra, "fps:", p?.fps);
check(sawNpc, "BOMBADINHO NPC apareceu");
check(sawBomb, "NPC pos bomba");
check(maxBooms > 0, `explosoes do host (${maxBooms})`);
check((p?.fps ?? 0) > 5, `fps ok (${p?.fps})`);

// Personagem 8: C abre menu, 8 escolhe; clique captura o mouse (C solta); E troca tipo; F solta bomba
await host.key("c", "KeyC", 67, "c");
await sleep(300);
await host.key("8", "Digit8", 56, "8");
await sleep(1500);
check(field(await host.extra(), "me") === 1, "tecla 8 vira BOMBADINHO");
await host.cmd("Input.dispatchMouseEvent", { type: "mousePressed", x: 640, y: 360, button: "left", clickCount: 1 });
await host.cmd("Input.dispatchMouseEvent", { type: "mouseReleased", x: 640, y: 360, button: "left", clickCount: 1 });
await sleep(1500);
check(field(await host.extra(), "grab") === 1, "clique captura o mouse");
await host.key("e", "KeyE", 69, "e");
await sleep(1500);
p = await host.extra();
check(/kind=FOGO/.test(p?.extra || ""), "E troca tipo pra FOGO");
const before = field(p, "bombs");
await host.key("f", "KeyF", 70, "f");
let placed = false;
for (let i = 0; i < 20 && !placed; i++) {
    await sleep(150);
    p = await host.extra();
    placed = field(p, "bombs") > before || field(p, "flames") > 0;
}
check(placed, "F solta bomba do jogador");

// Segundo cliente (não-host): NPC chega pelo snapshot
const cli = await open(9342, "cdpbomba2");
let p2;
for (let i = 0; i < 40 && field(p2, "npc") !== 1; i++) {
    await sleep(1000);
    p2 = await cli.extra();
}
console.log("cliente 2:", p2?.extra);
check(field(p2, "npc") === 1, "cliente nao-host ve o BOMBADINHO");
p = await host.extra();
console.log("host:", p?.extra, "fps:", p?.fps);
check((p?.fps ?? 0) > 5, `fps ok depois das explosoes (${p?.fps})`);
check(errors.length === 0, `sem erro no console (${errors.slice(0, 3).join(" / ")})`);
host.close();
cli.close();
console.log(fails.length ? `FALHOU: ${fails.length}` : "TUDO OK");
process.exit(fails.length ? 1 : 0);
