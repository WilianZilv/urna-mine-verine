// Teste headless (mudo): câmera no clube olhando o telão; Vorcaro na pista e luz tingida pelo vídeo.
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SITE = process.argv[2] || "https://urna-mine-verine.wilianzilv.workers.dev";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9341;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", "--autoplay-policy=no-user-gesture-required", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-cdp-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) {
    await sleep(250);
    ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null);
}
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map(), thumbs = [];
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
    if (m.method === "Network.responseReceived" && m.params.response.url.includes("ytthumb")) thumbs.push(`${m.params.response.status} ${m.params.response.url}`);
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });

const { targetId } = await cmd("Target.createTarget", { url: "about:blank" });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Network.enable", {}, sessionId);
await cmd("Page.enable", {}, sessionId);
await cmd("Page.navigate", { url: `${SITE}/?nome=cdpvibe&cam=${process.argv[3] || "69,23.2,160,3.14159,0.12"}` }, sessionId);
await sleep(20000);
for (const type of ["mousePressed", "mouseReleased"]) await cmd("Input.dispatchMouseEvent", { type, x: 640, y: 360, button: "left", clickCount: 1 }, sessionId);
await sleep(12000);
const shot = await cmd("Page.captureScreenshot", { format: "png" }, sessionId);
const out = join(tmpdir(), "urna-club-vibe.png");
writeFileSync(out, Buffer.from(shot.data, "base64"));
console.log("thumbs:", thumbs);
console.log("screenshot:", out);
ws.close();
chrome.kill();
process.exit(0);
