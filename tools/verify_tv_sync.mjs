// Teste headless (mudo, sem screenshot): dois clientes entram com ~25 s de diferença e o telão
// do clube tem que estar no mesmo ponto do vídeo (getCurrentTime do player do YouTube).
import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SITE = process.argv[2] || "https://urna-mine-verine.wilianzilv.workers.dev";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const procs = [];

// Um Chrome (perfil próprio) por cliente: o YouTube lembra a posição por perfil e mascararia o bug.
async function open(name, port) {
    procs.push(spawn(CHROME, ["--headless=new", "--mute-audio", "--autoplay-policy=no-user-gesture-required", `--remote-debugging-port=${port}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-cdp-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" }));
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
    };
    const cmd = (method, params = {}, sessionId) => new Promise((r) => {
        const i = ++id;
        pending.set(i, r);
        setTimeout(() => pending.has(i) && (pending.delete(i), r({ timeout: method })), 15000);
        ws.send(JSON.stringify({ id: i, method, params, sessionId }));
    });
    const { targetId } = await cmd("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
    await cmd("Page.enable", {}, sessionId);
    await cmd("Page.navigate", { url: `${SITE}/?nome=${name}&cam=69,23.2,160,3.14159,0.12` }, sessionId);
    return {
        ws,
        click: async () => { for (const type of ["mousePressed", "mouseReleased"]) await cmd("Input.dispatchMouseEvent", { type, x: 640, y: 360, button: "left", clickCount: 1 }, sessionId); },
        pos: async () => {
            const r = await cmd("Runtime.evaluate", { expression: `(() => { try { const p = YT.get("yt"); return JSON.stringify({ t: p.getCurrentTime(), st: p.getPlayerState(), dur: p.getDuration() }); } catch (e) { return JSON.stringify({ err: String(e) }); } })()`, returnByValue: true }, sessionId);
            return r.result ? JSON.parse(r.result.value) : r;
        },
    };
}

const a = await open("syncA", 9343);
await sleep(15000);
await a.click();
await sleep(25000);
console.log("A antes do B:", await a.pos());
const b = await open("syncB", 9344);
await sleep(15000);
await b.click();
await sleep(10000);
const [pa, pb] = await Promise.all([a.pos(), b.pos()]);
console.log("A:", pa, "B:", pb);
const ok = pa.st === 1 && pb.st === 1 && Math.abs(pa.t - pb.t) < 3 && pb.t > 20;
console.log(ok ? "OK: mesma linha do tempo" : "FALHOU");
for (const c of [a, b]) c.ws.close();
for (const p of procs) p.kill();
process.exit(ok ? 0 : 1);
