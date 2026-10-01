// Preview de compartilhamento: meta tags nas pÃ¡ginas, og.png 1200x630 < 300 KB, kit de imprensa e boot do jogo headless (mudo).
// Uso: node tools/test_press.mjs [--no-boot]
// Serve URNA_WEB (padrÃ£o web/) em SHOT_HTTP_PORT (8775); Chrome CDP em SHOT_CDP_PORT (9375).
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, readFileSync, existsSync, statSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, extname } from "node:path";

const SITE = "https://urna-mine-verine.wilianzilv.workers.dev";
const WEB = process.env.URNA_WEB || "web";
let fails = 0;
const must = (ok, msg) => { console.log(`${ok ? "ok  " : "FAIL"} ${msg}`); if (!ok) fails++; };

function metas(html) {
    const head = html.slice(0, html.indexOf("</head>"));
    const out = {};
    for (const m of head.matchAll(/<meta\s+([^>]+?)\/?>/g)) {
        const a = Object.fromEntries([...m[1].matchAll(/([\w:-]+)="([^"]*)"/g)].map((x) => [x[1], x[2]]));
        const k = a.property || a.name;
        if (k) out[k] = a.content;
    }
    out.icon = head.match(/<link rel="icon" href="([^"]+)"/)?.[1];
    out.title = head.match(/<title>([^<]+)<\/title>/)?.[1];
    return out;
}

const FULL = ["description", "theme-color", "og:type", "og:locale", "og:url", "og:title", "og:description", "og:image", "og:image:width", "og:image:height", "twitter:card", "twitter:title", "twitter:description", "twitter:image"];
const SMALL = ["description", "theme-color", "og:type", "og:locale", "og:url", "og:title", "og:description", "og:image", "twitter:card"];
const PAGES = [
    ["index.html", "/", FULL],
    ["embed/index.html", "/embed/", SMALL],
    ["embed/bolsa.html", "/embed/bolsa.html", SMALL],
    ["embed/leis.html", "/embed/leis.html", SMALL],
    ["embed/tv.html", "/embed/tv.html", SMALL],
    ["hub-templates/index.html", "/hub-templates/", SMALL],
    ...(existsSync(join(WEB, "press")) ? [["press/index.html", "/press/", FULL]] : []),
];
for (const [file, path, need] of PAGES) {
    const f = join(WEB, file);
    if (!existsSync(f)) { must(false, `${file} existe`); continue; }
    const m = metas(readFileSync(f, "utf8"));
    const miss = need.filter((k) => !m[k]);
    must(!miss.length, `${file}: meta ${need.length} tags${miss.length ? ` (faltam ${miss.join(", ")})` : ""}`);
    must(!!m.title, `${file}: <title>`);
    must(m["og:image"] === `${SITE}/og.png`, `${file}: og:image absoluto`);
    must(m["og:url"] === `${SITE}${path}`, `${file}: og:url ${m["og:url"]}`);
    must(m["og:locale"] === "pt_BR" && m["og:type"] === "website", `${file}: og:locale pt_BR, og:type website`);
    must(m["twitter:card"] === "summary_large_image", `${file}: twitter:card summary_large_image`);
    must(m.icon === "/favicon.svg", `${file}: favicon /favicon.svg`);
    if (need === FULL) must(m["og:image:width"] === "1200" && m["og:image:height"] === "630", `${file}: og:image 1200x630 declarado`);
}
must(existsSync(join(WEB, "favicon.svg")) && readFileSync(join(WEB, "favicon.svg"), "utf8").includes("<svg"), "favicon.svg Ã© SVG");

const og = readFileSync(join(WEB, "og.png"));
const isPng = og.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])) && og.toString("latin1", 12, 16) === "IHDR";
const [w, h] = isPng ? [og.readUInt32BE(16), og.readUInt32BE(20)] : [0, 0];
must(isPng, "og.png Ã© PNG");
must(w === 1200 && h === 630, `og.png ${w}x${h}`);
must(og.length < 300 * 1024, `og.png ${(og.length / 1024).toFixed(0)} KB < 300 KB`);

const pressDir = join(WEB, "press");
if (existsSync(pressDir)) {
    const html = readFileSync(join(pressDir, "index.html"), "utf8");
    const files = readdirSync(pressDir);
    must(files.length === 1 && files[0] === "index.html", `press: sÃ³ index.html (${files.join(", ")})`);
    must(html.includes('src="/og.png"'), "press: usa /og.png");
    for (const s of ["/skill.md", "/api/world", "/embed/", "/hub", "&lt;iframe src=", "twitter.com/intent/tweet", 'lang="en"']) must(html.includes(s), `press: menciona ${s}`);
    must(!/<script[^>]+src="https?:/.test(html), "press: sem script externo");
}

if (!process.argv.includes("--no-boot")) {
    const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
    const PORT = +(process.env.SHOT_CDP_PORT || 9375), HTTP = +(process.env.SHOT_HTTP_PORT || 8775);
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".png": "image/png", ".jpg": "image/jpeg" };
    const server = createServer((req, res) => {
        const p = decodeURIComponent(req.url.split("?")[0]).replace(/\/$/, "/index.html");
        const f = join(WEB, p);
        if (!existsSync(f) || statSync(f).isDirectory()) { res.writeHead(404); return res.end(); }
        res.writeHead(200, { "content-type": types[extname(f)] || "application/octet-stream" });
        res.end(readFileSync(f));
    }).listen(HTTP);
    const chrome = spawn(CHROME, ["--headless=new", "--mute-audio", `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "urna-press-"))}`, "--window-size=1280,720", "--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--no-first-run", "about:blank"], { stdio: "ignore" });
    let ver;
    for (let i = 0; i < 40 && !ver; i++) {
        await sleep(250);
        ver = await fetch(`http://127.0.0.1:${PORT}/json/version`).then((r) => r.json()).catch(() => null);
    }
    const ws = new WebSocket(ver.webSocketDebuggerUrl);
    await new Promise((r) => (ws.onopen = r));
    let id = 0;
    const pending = new Map(), errors = [];
    ws.onmessage = (e) => {
        const m = JSON.parse(e.data);
        if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result || m.error); pending.delete(m.id); }
        if (m.method === "Runtime.exceptionThrown") errors.push(`exception: ${m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text}`);
        if (m.method === "Runtime.consoleAPICalled" && m.params.type === "error") errors.push(`console.error: ${m.params.args.map((a) => a.value ?? a.description).join(" ")}`);
        if (m.method === "Log.entryAdded" && m.params.entry.level === "error") errors.push(`log: ${m.params.entry.text} ${m.params.entry.url || ""}`);
    };
    // Servidor estÃ¡tico: /api/* e /ws sÃ³ existem no Worker. Aviso de textura GL apagada vem do wasm commitado antigo.
    const known = (e) => /\/api\/|ws:\/\/[^ ]+\/ws'|already deleted texture/.test(e);
    const clean = (path) => {
        const bad = errors.filter((e) => !known(e)), skip = errors.length - bad.length;
        must(!bad.length, `${path}: sem erros no console${skip ? ` (${skip} esperados ignorados: backend ausente / wasm antigo)` : ""}${bad.length ? `\n     ${bad.join("\n     ")}` : ""}`);
    };
    for (const p of ["/", "/og.png", "/favicon.svg", "/embed/", "/hub-templates/", "/press/"]) {
        const r = await fetch(`http://127.0.0.1:${HTTP}${p}`).catch(() => ({ status: 0 }));
        must(r.status === 200, `HTTP ${r.status} ${p}`);
    }
    const cmd = (method, params = {}, sessionId) => new Promise((r) => {
        const i = ++id;
        pending.set(i, r);
        setTimeout(() => { if (pending.delete(i)) r({ timeout: method }); }, 15000);
        ws.send(JSON.stringify({ id: i, method, params, sessionId }));
    });
    const { targetId } = await cmd("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await cmd("Target.attachToTarget", { targetId, flatten: true });
    await cmd("Runtime.enable", {}, sessionId);
    await cmd("Page.enable", {}, sessionId);
    await cmd("Log.enable", {}, sessionId);
    const ev = async (expression) => (await cmd("Runtime.evaluate", { expression, returnByValue: true }, sessionId)).result?.value;
    for (const path of ["/press/", "/embed/"]) {
        if (!existsSync(join(WEB, path, "index.html"))) continue;
        errors.length = 0;
        await cmd("Page.navigate", { url: `http://127.0.0.1:${HTTP}${path}` }, sessionId);
        await sleep(2500);
        const broken = await ev(`[...document.images].filter((i) => !i.complete || !i.naturalWidth).map((i) => i.src)`);
        must(Array.isArray(broken) && !broken.length, `${path}: imagens carregadas${broken?.length ? ` (quebradas: ${broken.join(", ")})` : ""}`);
        clean(path);
    }
    errors.length = 0;
    await cmd("Page.navigate", { url: `http://127.0.0.1:${HTTP}/?nome=presstest` }, sessionId);
    await sleep(14000);
    const st = JSON.parse(await ev(`JSON.stringify({ wasm: typeof wasm_exports === "object" && !!wasm_exports, w: glcanvas.width, h: glcanvas.height, title: document.title, og: document.querySelector('meta[property="og:image"]')?.content })`));
    must(st.wasm, "/: wasm instanciado");
    must(st.w > 0 && st.h > 0, `/: canvas ${st.w}x${st.h}`);
    must(st.og === `${SITE}/og.png`, "/: og:image no DOM");
    clean("/");
    ws.close();
    chrome.kill();
    server.close();
}
console.log(fails ? `\n${fails} FALHA(S)` : "\nTUDO OK");
process.exit(fails ? 1 : 0);
