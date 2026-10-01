// Smoke test WS da TV URNA contra wrangler dev local. Uso: node tools/ws_tv.mjs [ws://127.0.0.1:8773/ws] [plantao|momento|audiencia|arena [espera_ms]]
// "plantao": conta nova doa 100 (fato grande) e espera o PLANTAO; fica conectado `espera_ms` (screenshot).
// "momento": doa 7, viaja pelo portao e pede /momento; espera o segmento no snapshot e o resumo no chat.
const url = process.argv[2] || "ws://127.0.0.1:8773/ws";
const urg = process.argv[3] === "plantao";
const mom = process.argv[3] === "momento";
let momSnap = null;
let momChat = false;
const ws = new WebSocket(url);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let tv = 0;
let got = false;
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.t === "pl" && m.k === "tv") {
        tv++;
        if (m.urg) got = true;
        if (m.mom) momSnap = m.mom;
        console.log("TV", JSON.stringify(m));
    } else if (m.t === "chat" && (m.n === "TV URNA" || m.id === 0)) {
        if (/^MOMENTO DO DIA:/.test(m.m)) momChat = true;
        console.log("CHAT", m.n, ":", m.m);
    }
};
await new Promise((r) => (ws.onopen = r));
if (process.argv[3] === "arena") {
    // golpes falsos no formato do cliente (steve.rs) ate o pico; espera AO VIVO DA ARENA e o /campeonato; fica `espera_ms`
    ws.send(JSON.stringify({ t: "hello", n: `soco${Date.now() % 100000}` }));
    await sleep(1500);
    let ar = null;
    let camp = "";
    const prev = ws.onmessage;
    ws.onmessage = (e) => {
        prev(e);
        const m = JSON.parse(e.data);
        if (m.t === "pl" && m.k === "tv" && m.ar) ar = m.ar;
        if (m.t === "chat" && /^CAMPEONATO DA ARENA/.test(m.m)) camp = m.m;
    };
    const F = [[0, 3], [0, 0], [4, 0], [0, 3]];
    for (let i = 0; i < 40 && !ar; i++) {
        const [g, k] = F[i % F.length];
        ws.send(JSON.stringify({ t: "a", k: g === 0 ? "pf" : "hit", g, i: k, d: [0, 0, 1], p: [64, 21, 300], dmg: 6 + (i % 5) * 3, w: i % 3 ? "SOCO" : "ESPADADA" }));
        await sleep(130);
    }
    for (let i = 0; i < 20 && !ar; i++) await sleep(250);
    ws.send(JSON.stringify({ t: "chat", m: "/campeonato" }));
    for (let i = 0; i < 20 && !camp; i++) await sleep(250);
    const okA = !!ar && ar.f.length === 2 && ar.top.length >= 1 && !!camp;
    console.log(okA ? `arena no ar: ${JSON.stringify(ar.f)} | ${camp}` : "sem arena/campeonato");
    const until = Date.now() + +(process.argv[4] || 0);
    for (let i = 0; Date.now() < until; i++) {
        const [g, k] = F[i % F.length];
        ws.send(JSON.stringify({ t: "a", k: g === 0 ? "pf" : "hit", g, i: k, d: [0, 0, 1], p: [64, 21, 300], dmg: 8, w: "SOCO" }));
        await sleep(900);
    }
    ws.close();
    process.exit(okA ? 0 : 1);
}
if (process.argv[3] === "audiencia") {
    // fica parado na frente do telao: espera AUDIENCIA >= 1 no snapshot; segue conectado `espera_ms`
    ws.send(JSON.stringify({ t: "hello", n: `ibope${Date.now() % 100000}` }));
    await sleep(1000);
    let seen = null;
    const prev = ws.onmessage;
    ws.onmessage = (e) => {
        prev(e);
        const m = JSON.parse(e.data);
        if (m.t === "pl" && m.k === "tv" && m.aud >= 1) seen = m;
    };
    for (let i = 0; i < 20 && !seen; i++) {
        ws.send(JSON.stringify({ t: "p", p: [64, 20, 250], y: 1.57 }));
        await sleep(500);
    }
    console.log(seen ? `audiencia ${seen.aud} recorde ${seen.rec} (${seen.recd})` : "sem audiencia");
    const until = Date.now() + +(process.argv[4] || 0);
    while (Date.now() < until) {
        ws.send(JSON.stringify({ t: "p", p: [64, 20, 250], y: 1.57 }));
        await sleep(1000);
    }
    ws.close();
    process.exit(seen ? 0 : 1);
}
if (mom) {
    ws.send(JSON.stringify({ t: "hello", n: `mom${Date.now() % 100000}` }));
    await sleep(1500);
    ws.send(JSON.stringify({ t: "chat", m: "/doar 7" }));
    ws.send(JSON.stringify({ t: "pl", k: "term_trip", d: "lab" }));
    await sleep(4000);
    ws.send(JSON.stringify({ t: "chat", m: "/momento" }));
    for (let i = 0; i < 20 && !(momSnap && momChat); i++) await sleep(500);
    const ok = !!momSnap && momSnap.items.length >= 3 && momChat;
    console.log(ok ? `momento no ar: ${momSnap.items.length} itens` : "sem momento");
    await sleep(+(process.argv[4] || 0));
    ws.close();
    process.exit(ok ? 0 : 1);
}
if (urg) {
    ws.send(JSON.stringify({ t: "hello", n: `rep${Date.now() % 100000}` }));
    await sleep(1500);
    ws.send(JSON.stringify({ t: "chat", m: "/doar 100" }));
    for (let i = 0; i < 20 && !got; i++) await sleep(500);
    console.log(got ? "plantao no ar" : "sem plantao");
    await sleep(+(process.argv[4] || 0));
    ws.close();
    process.exit(got ? 0 : 1);
}
ws.send(JSON.stringify({ t: "hello", n: "bot" }));
await sleep(2500);
ws.send(JSON.stringify({ t: "chat", m: "/noticia" }));
await sleep(2500);
ws.send(JSON.stringify({ t: "chat", m: "/tv" }));
await sleep(1000);
ws.send(JSON.stringify({ t: "pl", k: "tv_aovivo" }));
await sleep(1500);
console.log(`snapshots tv: ${tv}`);
ws.close();
process.exit(tv ? 0 : 1);
