// Screenshot headless e mudo de uma pagina (demo do hub), com erros do console.
// node tools/shot_demo.mjs <url> <out.png> [waitMs] [mobile]   env EVAL="js" (+EVAL_WAIT ms) roda antes da foto
//   env HOLD="KeyS:83:s:3000" (+HOLD_WAIT ms) segura uma tecla de verdade (ex.: ?hub=<id> e andar de re pro arco)
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
const [url, out, waitMs = "8000", mobile = ""] = process.argv.slice(2);
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9340 + Math.floor(Math.random() * 50);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "isl-cdp-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) { await sleep(250); ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null); }
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.method === "Runtime.consoleAPICalled" && ["error", "warning"].includes(m.params.type)) console.log("console:", m.params.args.map((a) => a.value ?? a.description).join(" "));
    if (m.method === "Runtime.exceptionThrown") console.log("EXC:", JSON.stringify(m.params.exceptionDetails).slice(0, 600));
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });
const { targetId } = await cmd("Target.createTarget", { url: "about:blank" });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Runtime.enable", {}, sessionId);
await cmd("Page.enable", {}, sessionId);
if (mobile) await cmd("Emulation.setDeviceMetricsOverride", { width: 412, height: 860, deviceScaleFactor: 2.6, mobile: true }, sessionId);
await cmd("Page.navigate", { url }, sessionId);
await sleep(+waitMs);
if (process.env.HOLD) {
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) { await cmd("Input.dispatchMouseEvent", { type, x: 640, y: 360, button: "left", clickCount: 1 }, sessionId); await sleep(80); }
    await sleep(1500);
    const [code, vk, key, ms] = process.env.HOLD.split(":");
    await cmd("Input.dispatchKeyEvent", { type: "keyDown", key, code, windowsVirtualKeyCode: +vk, text: key }, sessionId);
    await sleep(+ms);
    await cmd("Input.dispatchKeyEvent", { type: "keyUp", key, code, windowsVirtualKeyCode: +vk }, sessionId);
    await sleep(+(process.env.HOLD_WAIT || 10000));
}
if (process.env.EVAL) { await cmd("Runtime.evaluate", { expression: process.env.EVAL }, sessionId); await sleep(+(process.env.EVAL_WAIT || 6000)); }
const r = await cmd("Runtime.evaluate", { expression: "JSON.stringify({st: document.getElementById('st')?.textContent, p: window.Ilha && window.Ilha.player.pos})", returnByValue: true }, sessionId);
console.log("state:", r?.result?.value);
const s = await cmd("Page.captureScreenshot", { format: "png" }, sessionId);
writeFileSync(out, Buffer.from(s.data, "base64"));
console.log("screenshot:", out);
chrome.kill();
process.exit(0);
