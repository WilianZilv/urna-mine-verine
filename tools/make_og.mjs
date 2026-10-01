// Imagens de divulgação a partir de screenshots headless (mudo) do jogo de verdade.
// Uso: node tools/make_og.mjs [og|press|all]   (padrão: all)
//   og    -> web/og.png 1200x630 (screenshot + título por cima, PNG indexado < 300 KB)
//   press -> web/press/*.jpg 1280x720 (6 lugares)
// Serve URNA_WEB (padrão web/) em SHOT_HTTP_PORT (8775); Chrome CDP em SHOT_CDP_PORT (9375).
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, extname } from "node:path";
import { deflateSync } from "node:zlib";

const what = process.argv[2] || "all";
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = +(process.env.SHOT_CDP_PORT || 9375), HTTP = +(process.env.SHOT_HTTP_PORT || 8775);
const WEB = process.env.URNA_WEB || "web";
const WAIT = +(process.env.SHOT_WAIT || 15000);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const types = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm" };
const server = createServer((req, res) => {
    const f = join(WEB, decodeURIComponent(req.url.split("?")[0]).replace(/^\/$/, "/index.html"));
    if (!existsSync(f) || statSync(f).isDirectory()) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { "content-type": types[extname(f)] || "application/octet-stream" });
    res.end(readFileSync(f));
}).listen(HTTP);

// x,y,z,yaw,pitch (yaw 0 = +x, PI/2 = +z)
const OG_CAM = process.env.OG_CAM || "185,30,96,3.1416,-0.12";
const PRESS = {
    avenida: "160,40,104,0,-0.25",
    congresso: "100,30,98,3.1416,-0.25",
    bolsa: "222,30,88,0,-0.25",
    tv: "64,28,240,1.5708,-0.1",
    banco: "165,26,237,0,-0.05",
    terminal: "202,27,256,1.5708,0.05",
};

const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-og-"))}`, "--window-size=1200,630", "--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) {
    await sleep(250);
    ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null);
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
    setTimeout(() => { if (pending.delete(i)) r({ timeout: method }); }, 30000);
    ws.send(JSON.stringify({ id: i, method, params, sessionId }));
});
const { targetId } = await cmd("Target.createTarget", { url: "about:blank" });
const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
await cmd("Page.enable", {}, sessionId);
await cmd("Runtime.enable", {}, sessionId);
// O modo sobrevivência desliga o voo do ?cam= e o jogador cai; com o relógio do jogo quase parado a câmera fica onde foi posta.
await cmd("Page.addScriptToEvaluateOnNewDocument", { source: `
document.addEventListener("DOMContentLoaded", () => {
    if (typeof importObject === "undefined") return;
    const t0 = Date.now();
    importObject.env.now = () => t0 / 1e3 + (Date.now() - t0) / 1e3 * ${+(process.env.SHOT_TIME_SCALE || 0.002)};
});` }, sessionId);
const evaluate = async (expression) => (await cmd("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, sessionId)).result?.value;
const viewport = (width, height) => cmd("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false }, sessionId);

const key = async (k) => {
    const code = /^\d$/.test(k) ? `Digit${k}` : `Key${k.toUpperCase()}`;
    for (const type of ["keyDown", "keyUp"]) {
        await cmd("Input.dispatchKeyEvent", { type, key: k, code, windowsVirtualKeyCode: k.toUpperCase().charCodeAt(0) }, sessionId);
        await sleep(150);
    }
};

// Sem parâmetro de HUD no jogo: viewport mais alto e recorte entre o banner de boas-vindas (0.28*altura) e a barra de baixo.
async function shoot(cam, w, h, format = "png", quality) {
    const H = Math.ceil((h + 150) / 0.72);
    await viewport(w, H);
    await cmd("Page.navigate", { url: `http://127.0.0.1:${HTTP}/?nome=press&q=high&cam=${cam}` }, sessionId);
    await sleep(WAIT);
    for (const type of ["mousePressed", "mouseReleased"]) await cmd("Input.dispatchMouseEvent", { type, x: w / 2, y: H / 2, button: "left", clickCount: 1 }, sessionId);
    await sleep(800);
    await key("h");
    await key("9");
    await sleep(1500);
    const top = Math.round(0.28 * H + 30 + (H - 120 - (0.28 * H + 30) - h) / 2);
    const shot = await cmd("Page.captureScreenshot", { format, quality, clip: { x: 0, y: top, width: w, height: h, scale: 1 } }, sessionId);
    return Buffer.from(shot.data, "base64");
}

function crc32(buf) {
    let c, crc = ~0;
    for (const b of buf) {
        c = (crc ^ b) & 0xff;
        for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
        crc = (crc >>> 8) ^ c;
    }
    return ~crc >>> 0;
}
function chunk(type, data) {
    const len = Buffer.alloc(4), crc = Buffer.alloc(4), td = Buffer.concat([Buffer.from(type), data]);
    len.writeUInt32BE(data.length);
    crc.writeUInt32BE(crc32(td));
    return Buffer.concat([len, td, crc]);
}
function indexedPng(w, h, palette, idx) {
    const ihdr = Buffer.alloc(13);
    ihdr.writeUInt32BE(w, 0);
    ihdr.writeUInt32BE(h, 4);
    ihdr.set([8, 3, 0, 0, 0], 8);
    const raw = Buffer.alloc((w + 1) * h);
    for (let y = 0; y < h; y++) raw.set(idx.subarray(y * w, (y + 1) * w), y * (w + 1) + 1);
    return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("PLTE", Buffer.from(palette)), chunk("IDAT", deflateSync(raw, { level: 9 })), chunk("IEND", Buffer.alloc(0))]);
}

// Composição no próprio Chrome (canvas): screenshot + vinheta + título; devolve pixels quantizados (median cut, 256 cores).
const COMPOSE = (shotB64, w, h) => `(async () => {
    const img = new Image();
    img.src = "data:image/png;base64,${shotB64}";
    await img.decode();
    const c = document.createElement("canvas");
    c.width = ${w}; c.height = ${h};
    const g = c.getContext("2d");
    g.drawImage(img, 0, 0);
    const grad = g.createLinearGradient(0, 0, 0, ${h});
    grad.addColorStop(0, "rgba(13,5,25,0)");
    grad.addColorStop(0.45, "rgba(13,5,25,0.1)");
    grad.addColorStop(1, "rgba(13,5,25,0.92)");
    g.fillStyle = grad;
    g.fillRect(0, 0, ${w}, ${h});
    g.textAlign = "left";
    g.textBaseline = "alphabetic";
    g.font = "900 92px 'Courier New', monospace";
    g.lineJoin = "round";
    g.lineWidth = 12;
    g.strokeStyle = "#0d0519";
    g.strokeText("URNA-MINE-VERINE", 56, ${h - 150});
    g.fillStyle = "#ffe14d";
    g.fillText("URNA-MINE-VERINE", 56, ${h - 150});
    g.font = "bold 34px 'Courier New', monospace";
    g.lineWidth = 8;
    for (const [t, y, col] of [["A vila satírica de blocos no navegador.", ${h - 96}, "#ffffff"], ["Grátis · moedas fictícias · mods feitos por IA", ${h - 50}, "#7dfcb5"]]) {
        g.strokeText(t, 58, y);
        g.fillStyle = col;
        g.fillText(t, 58, y);
    }
    const d = g.getImageData(0, 0, ${w}, ${h}).data, n = ${w * h};
    const q = (v) => v >> 3;
    const hist = new Map();
    for (let i = 0; i < n; i++) {
        const k = (q(d[i * 4]) << 10) | (q(d[i * 4 + 1]) << 5) | q(d[i * 4 + 2]);
        hist.set(k, (hist.get(k) || 0) + 1);
    }
    let boxes = [[...hist.entries()]];
    const range = (b, s) => { let lo = 31, hi = 0; for (const [k] of b) { const v = (k >> s) & 31; if (v < lo) lo = v; if (v > hi) hi = v; } return hi - lo; };
    while (boxes.length < 256) {
        let bi = -1, best = 0, bs = 0;
        boxes.forEach((b, i) => { if (b.length < 2) return; const w = b.reduce((a, e) => a + e[1], 0); for (const s of [10, 5, 0]) { const r = range(b, s) * Math.sqrt(w); if (r > best) { best = r; bi = i; bs = s; } } });
        if (bi < 0) break;
        const b = boxes[bi].sort((a, c) => ((a[0] >> bs) & 31) - ((c[0] >> bs) & 31));
        const tot = b.reduce((a, e) => a + e[1], 0);
        let acc = 0, cut = 1;
        for (; cut < b.length - 1; cut++) { acc += b[cut - 1][1]; if (acc >= tot / 2) break; }
        boxes.splice(bi, 1, b.slice(0, cut), b.slice(cut));
    }
    const pal = [], map = new Map();
    boxes.forEach((b, i) => {
        let r = 0, gg = 0, bb = 0, t = 0;
        for (const [k, c] of b) { r += ((k >> 10) & 31) * c; gg += ((k >> 5) & 31) * c; bb += (k & 31) * c; t += c; map.set(k, i); }
        pal.push(Math.round(r / t * 8.226), Math.round(gg / t * 8.226), Math.round(bb / t * 8.226));
    });
    const idx = new Uint8Array(n);
    for (let i = 0; i < n; i++) idx[i] = map.get((q(d[i * 4]) << 10) | (q(d[i * 4 + 1]) << 5) | q(d[i * 4 + 2]));
    const b64 = (u) => { let s = ""; for (let i = 0; i < u.length; i += 0x8000) s += String.fromCharCode(...u.subarray(i, i + 0x8000)); return btoa(s); };
    return JSON.stringify({ pal: b64(new Uint8Array(pal)), idx: b64(idx) });
})()`;

if (what === "og" || what === "all") {
    const shot = await shoot(OG_CAM, 1200, 630);
    if (process.env.OG_RAW) writeFileSync(process.env.OG_RAW, shot);
    await cmd("Page.navigate", { url: "about:blank" }, sessionId);
    await sleep(500);
    const r = JSON.parse(await evaluate(COMPOSE(shot.toString("base64"), 1200, 630)));
    const png = indexedPng(1200, 630, Buffer.from(r.pal, "base64"), Buffer.from(r.idx, "base64"));
    writeFileSync(join("web", "og.png"), png);
    console.log(`web/og.png ${(png.length / 1024).toFixed(0)} KB`);
}
if (what === "press" || what === "all") {
    mkdirSync(join("web", "press"), { recursive: true });
    for (const [name, cam] of Object.entries(PRESS)) {
        const jpg = await shoot(cam, 1280, 720, "jpeg", 80);
        writeFileSync(join("web", "press", `${name}.jpg`), jpg);
        console.log(`web/press/${name}.jpg ${(jpg.length / 1024).toFixed(0)} KB`);
    }
}
ws.close();
chrome.kill();
server.close();
process.exit(0);
