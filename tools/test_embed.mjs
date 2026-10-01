// /api/world + widgets de web/embed/ contra o wrangler dev local (Chrome headless MUDO, CDP 9374).
//   node tools/test_embed.mjs            (BASE=http://127.0.0.1:8774 SHOTS=D:\tmp\urna-wt\shots)
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const BASE = process.env.BASE || "http://127.0.0.1:8774";
const OTHER = BASE.replace("127.0.0.1", "localhost");
const SHOTS = process.env.SHOTS || "D:\\tmp\\urna-wt\\shots";
const CHROME = process.env.CHROME || "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const PORT = 9374;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let fails = 0;
const must = (ok, what, extra = "") => {
    console.log(`${ok ? "PASS" : "FAIL"}: ${what}${extra ? " " + extra : ""}`);
    if (!ok) fails++;
};
mkdirSync(SHOTS, { recursive: true });

// 1) jogador online pra ter estado vivo (snapshots dos lugares, boletim da TV)
const ws = new WebSocket(BASE.replace("http", "ws") + "/ws");
await new Promise((r, j) => ((ws.onopen = r), (ws.onerror = j)));
ws.send(JSON.stringify({ t: "hello", n: "embedbot" }));
await sleep(1500);

// 2) shape + CORS + cache
const r1 = await fetch(BASE + "/api/world", { headers: { origin: "https://example.com" } });
const w = await r1.json();
must(r1.status === 200, "GET /api/world 200");
must(r1.headers.get("access-control-allow-origin") === "*", "CORS *");
must(/max-age=10/.test(r1.headers.get("cache-control") || ""), "cache-control max-age=10", r1.headers.get("cache-control"));
must(typeof w.at === "number" && w.online >= 1, "at + online >= 1", `online=${w.online}`);
must(typeof w.treasury === "number", "treasury numero", String(w.treasury));
must(Array.isArray(w.laws) && w.laws.every((l) => l.id && l.n && l.left >= 0), "laws [{id,n,d,left}]", JSON.stringify(w.laws));
must(Array.isArray(w.bolsa?.tickers) && w.bolsa.tickers.length > 0 && w.bolsa.tickers.every((t) => t.length === 3 && typeof t[0] === "string" && typeof t[1] === "number" && typeof t[2] === "number"), "bolsa.tickers [s,p,d]", JSON.stringify(w.bolsa?.tickers?.slice(0, 3)));
must(typeof w.bolsa?.mood === "number", "bolsa.mood");
must(Array.isArray(w.tv?.headlines) && w.tv.headlines.length <= 3, "tv.headlines <= 3", JSON.stringify(w.tv?.headlines));
must(Array.isArray(w.banco?.rich) && w.banco.rich.length <= 5 && w.banco.rich.every((e) => e.length === 2), "banco.rich top5 [nome,total]", JSON.stringify(w.banco?.rich));
must(typeof w.terminal?.trips_today === "number", "terminal.trips_today", String(w.terminal?.trips_today));
must(Array.isArray(w.tour?.guides) && w.tour.guides.length <= 5, "tour.guides <= 5");
must(w.lab === null || typeof w.lab?.title === "string", "lab null ou {title}", JSON.stringify(w.lab));
const raw = JSON.stringify(w);
must(!/tok|secret|"ip"|password|key/i.test(raw.replace(/"(n|d|title|headlines|ticker|urgent|alert)":"[^"]*"/g, "")), "sem campo sensivel (tok/secret/ip/key)");
const r2 = await fetch(BASE + "/api/world");
must((await r2.json()).at === w.at, "cache 5 s no DO (mesmo at)");
const opt = await fetch(BASE + "/api/world", { method: "OPTIONS" });
must(opt.status === 204 && opt.headers.get("access-control-allow-origin") === "*", "OPTIONS 204 + CORS");
console.log("world:", raw.slice(0, 600));

// lei aprovada (quorum = metade de quem ta online) aparece no /api/world quando o cache vence
ws.send(JSON.stringify({ t: "chat", m: "/lei turbo" }));
await sleep(5500);
const w3 = await (await fetch(BASE + "/api/world")).json();
must(w3.laws.some((l) => l.id === "turbo" && l.left > 0 && l.left <= 300), "lei votada aparece com segundos restantes", JSON.stringify(w3.laws));

// 3) widgets no Chrome headless
const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-embed-"))}`, "--no-first-run", "about:blank"], { stdio: "ignore" });
let ver;
for (let i = 0; i < 40 && !ver; i++) { await sleep(250); ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null); }
const cdp = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((r) => (cdp.onopen = r));
let id = 0;
const pending = new Map(), errs = new Map();
cdp.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
    const bad = m.method === "Runtime.exceptionThrown" ? m.params.exceptionDetails?.exception?.description || m.params.exceptionDetails?.text
        : m.method === "Runtime.consoleAPICalled" && m.params.type === "error" ? m.params.args.map((a) => a.value ?? a.description).join(" ")
        : m.method === "Log.entryAdded" && m.params.entry.level === "error" ? `${m.params.entry.text} ${m.params.entry.url || ""}` : null;
    if (bad && m.sessionId) errs.get(m.sessionId)?.push(bad);
};
const cmd = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); cdp.send(JSON.stringify({ id: i, method, params, sessionId })); });

async function page(url, w, h, reduce = false) {
    const { targetId } = await cmd("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
    errs.set(sessionId, []);
    for (const d of ["Runtime", "Log", "Page"]) await cmd(`${d}.enable`, {}, sessionId);
    await cmd("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 1, mobile: false }, sessionId);
    if (reduce) await cmd("Emulation.setEmulatedMedia", { features: [{ name: "prefers-reduced-motion", value: "reduce" }] }, sessionId);
    await cmd("Page.navigate", { url }, sessionId);
    await sleep(2500);
    const ev = async (expression) => (await cmd("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, sessionId))?.result?.value;
    const shot = async (name) => {
        const s = await cmd("Page.captureScreenshot", { format: "png" }, sessionId);
        const f = join(SHOTS, name);
        writeFileSync(f, Buffer.from(s.data, "base64"));
        console.log("screenshot:", f);
    };
    return { ev, shot, errs: errs.get(sessionId), close: () => cmd("Target.closeTarget", { targetId }) };
}

const check = async (name, url, w, h, test, reduce) => {
    const p = await page(url, w, h, reduce);
    const v = await p.ev(test);
    must(v === true, `${name}: renderizou`, String(v));
    must(p.errs.length === 0, `${name}: sem erro no console`, JSON.stringify(p.errs));
    await p.shot(`embed_${name}.png`);
    await p.close();
};

const BOLSA_OK = `document.querySelectorAll("#board .row .s").length > 0 && /IVILA/.test(document.getElementById("ix").textContent) && /jogar na vila/.test(document.getElementById("play").textContent)`;
const TV_OK = `!/sintonizando/.test(document.getElementById("hl").textContent) && document.getElementById("crawltxt").textContent.length > 10`;
await check("bolsa", `${BASE}/embed/bolsa.html`, 480, 220, BOLSA_OK);
await check("tv", `${BASE}/embed/tv.html`, 480, 200, TV_OK);
// ?api= cross-origin (localhost -> 127.0.0.1): prova o CORS no navegador e o link do site trocado
await check("bolsa_api", `${OTHER}/embed/bolsa.html?api=${encodeURIComponent(BASE)}`, 360, 260, `${BOLSA_OK} && document.getElementById("play").href === ${JSON.stringify(BASE + "/")}`);
await check("tv_still", `${BASE}/embed/tv.html`, 480, 200, `${TV_OK} && getComputedStyle(document.querySelector("#crawl span")).animationName === "none"`, true);

const LEIS_OK = `document.querySelectorAll(".law").length > 0 && /^\\d+:\\d\\d$/.test(document.querySelector(".law .t").textContent)`;
await check("leis", `${BASE}/embed/leis.html`, 480, 220, LEIS_OK);
const LIGHT = `document.documentElement.className === "light" && getComputedStyle(document.body).backgroundColor === "rgb(251, 248, 255)"`;
await check("bolsa_light", `${BASE}/embed/bolsa.html?theme=light`, 480, 220, `${BOLSA_OK} && ${LIGHT}`);
await check("tv_light", `${BASE}/embed/tv.html?theme=light`, 480, 200, `${TV_OK} && ${LIGHT}`);
await check("leis_light", `${BASE}/embed/leis.html?theme=light`, 480, 220, `${LEIS_OK} && ${LIGHT}`);

// galeria: 3 previews, snippets com a origem, tema/largura mudam iframe + snippet, botao copiar
await cmd("Browser.grantPermissions", { origin: BASE, permissions: ["clipboardReadWrite", "clipboardSanitizedWrite"] });
const g = await page(`${BASE}/embed/`, 1100, 1250);
await sleep(1500);
const g1 = await g.ev(`JSON.stringify({ n: document.querySelectorAll(".card iframe").length, snip: [...document.querySelectorAll("textarea")].map((t) => t.value) })`);
must(/"n":3/.test(g1) && ["bolsa", "tv", "leis"].every((k) => g1.includes(`src=\\"${BASE}/embed/${k}.html\\"`)), "galeria: 3 previews + snippets com a origem", g1.slice(0, 300));
await g.shot("embed_gallery.png");
const g2 = await g.ev(`(async () => {
    const t = document.getElementById("theme"), w = document.getElementById("width");
    t.value = "light"; t.dispatchEvent(new Event("change"));
    w.value = "320"; w.dispatchEvent(new Event("change"));
    document.querySelector("#card-tv button").click();
    await new Promise((r) => setTimeout(r, 300));
    const ta = document.querySelector("#card-tv textarea").value;
    return JSON.stringify({ src: document.querySelector("#card-bolsa iframe").src, ta, btn: document.querySelector("#card-tv button").textContent, clip: await navigator.clipboard.readText().catch((e) => "ERR " + e) });
})()`);
must(/theme=light/.test(g2) && /width=\\"320\\"/.test(g2), "galeria: tema claro + largura no iframe e no snippet", g2);
must(/"btn":"copiado!"/.test(g2) && /"clip":"<iframe src=\\"[^"]+tv\.html\?theme=light/.test(g2), "galeria: copiar poe o snippet no clipboard", g2.slice(-200));
await sleep(2500);
await g.shot("embed_gallery_light.png");
must(g.errs.length === 0, "galeria: sem erro no console", JSON.stringify(g.errs));
await g.close();

ws.close();
cdp.close();
chrome.kill();
console.log(fails ? `${fails} FALHA(S)` : "TUDO OK");
process.exit(fails ? 1 : 0);
