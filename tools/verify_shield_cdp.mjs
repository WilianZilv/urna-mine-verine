// Teste headless (mudo): /hub.txt no chat abre aba nova; tiro no domo do lab/hub é refletido.
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SITE = "https://urna-mine-verine.wilianzilv.workers.dev";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9337;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-cdp-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) {
    await sleep(250);
    ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null);
}
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map(), created = [];
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
    if (m.method === "Target.targetCreated") created.push(m.params.targetInfo);
    if (m.method === "Target.targetInfoChanged") { const t = created.find((x) => x.targetId === m.params.targetInfo.targetId); if (t) Object.assign(t, m.params.targetInfo); }
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });

await cmd("Target.setDiscoverTargets", { discover: true });
const { targetId } = await cmd("Target.createTarget", { url: `${SITE}/?nome=cdpbot` });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Page.enable", {}, sessionId);
await sleep(20000);
const key = async (k, code, vk, text) => {
    await cmd("Input.dispatchKeyEvent", { type: "keyDown", key: k, code, windowsVirtualKeyCode: vk, text }, sessionId);
    await sleep(40);
    await cmd("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: vk }, sessionId);
    await sleep(60);
};

// Tiros (cliente WS separado) no domo do lab e do hub: devem refletir, não explodir.
const bot = new WebSocket(SITE.replace("https", "wss") + "/ws");
await new Promise((r) => (bot.onopen = r));
bot.send(JSON.stringify({ t: "hello", n: "cdpshot" }));
await sleep(800);
bot.send(JSON.stringify({ t: "w", k: "shot", o: [64, 50, 64], h: [103, 20, 64], r: 5.5, d: false, by: 9 }));
bot.send(JSON.stringify({ t: "w", k: "shot", o: [64, 50, 88], h: [106, 20, 88], r: 5.5, d: false, by: 9 }));
await sleep(500);
const shot = await cmd("Page.captureScreenshot", { format: "png" }, sessionId);
writeFileSync(join(tmpdir(), "urna-shield.png"), Buffer.from(shot.data, "base64"));
bot.close();

const before = created.length;
await key("Enter", "Enter", 13, "\r");
await sleep(300);
for (const ch of "/hub.txt") await key(ch, "", ch.charCodeAt(0), ch);
await key("Enter", "Enter", 13, "\r");
await sleep(2500);
const tabs = created.slice(before).filter((t) => t.type === "page").map((t) => t.url);
console.log("new tabs:", tabs);
console.log(tabs.some((u) => u.includes("/hub.txt")) ? "PASS: /hub.txt abriu aba nova" : "FAIL: nenhuma aba /hub.txt");
console.log("screenshot:", join(tmpdir(), "urna-shield.png"));
ws.close();
chrome.kill();
process.exit(0);
