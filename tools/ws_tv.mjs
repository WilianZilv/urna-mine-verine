// Smoke test WS da TV URNA contra wrangler dev local. Uso: node tools/ws_tv.mjs [ws://127.0.0.1:8773/ws]
const url = process.argv[2] || "ws://127.0.0.1:8773/ws";
const ws = new WebSocket(url);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let tv = 0;
ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.t === "pl" && m.k === "tv") {
        tv++;
        console.log("TV", JSON.stringify(m));
    } else if (m.t === "chat" && (m.n === "TV URNA" || m.id === 0)) console.log("CHAT", m.n, ":", m.m);
};
await new Promise((r) => (ws.onopen = r));
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
