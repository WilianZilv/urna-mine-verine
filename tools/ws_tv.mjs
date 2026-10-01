// Smoke test WS da TV URNA contra wrangler dev local. Uso: node tools/ws_tv.mjs [ws://127.0.0.1:8773/ws] [plantao [espera_ms]]
// "plantao": conta nova doa 100 (fato grande) e espera o PLANTAO; fica conectado `espera_ms` (screenshot).
const url = process.argv[2] || "ws://127.0.0.1:8773/ws";
const urg = process.argv[3] === "plantao";
const ws = new WebSocket(url);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let tv = 0;
let got = false;
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.t === "pl" && m.k === "tv") {
        tv++;
        if (m.urg) got = true;
        console.log("TV", JSON.stringify(m));
    } else if (m.t === "chat" && (m.n === "TV URNA" || m.id === 0)) console.log("CHAT", m.n, ":", m.m);
};
await new Promise((r) => (ws.onopen = r));
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
