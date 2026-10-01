// Cata Voto - template canvas 2D do URNA GAME HUB (licenca MIT, tudo desenhado em codigo).
// 30 segundos: pegue as cedulas com a urna (setas/A-D, mouse ou dedo) e fuja da TNT.
"use strict";

const PORTAL_ID = "cata-voto"; // igual ao data-portal-id do index.html e ao "id" do manifest
const ROUND = 30;
const U = window.UrnaPortal; // undefined se o SDK nao carregou: o jogo segue igual

const cv = document.getElementById("c"), g = cv.getContext("2d");
const endEl = document.getElementById("end"), resultEl = document.getElementById("result");
const exitBtn = document.getElementById("exit");

// estado exposto em window.game pra depurar no console (e pros testes)
const game = window.game = {
    state: "play", score: 0, time: ROUND, px: 0.5, items: [], fx: [], shake: 0,
    name: "convidado", color: "#00e5ff", coins: null, connected: false, achievements: [],
};
const keys = new Set();
let W = 0, H = 0, S = 0, spawnT = 0, pointerX = null;

function fit() {
    const dpr = Math.min(2, devicePixelRatio || 1);
    W = innerWidth; H = innerHeight; S = Math.min(W, H) / 12;
    cv.width = W * dpr; cv.height = H * dpr;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
}
addEventListener("resize", fit);
fit();

// ------------------------------------------------------------ Urna: sessao, passaporte, eventos
// O servidor aceita 1 conquista a cada 10 s: guardamos numa fila e mandamos espacado.
const achQueue = [];
let achLast = 0;
function achieve(text) {
    if (game.achievements.includes(text)) return;
    game.achievements.push(text);
    game.fx.push({ x: 0.5, y: 0.3, text: "conquista: " + text, t: 2.2, big: true });
    achQueue.push(text);
}
function flushAchievements(now) {
    if (!achQueue.length || now - achLast < 11000) return;
    achLast = now;
    const text = achQueue.shift();
    if (U) U.event("achievement", text);
}
async function refreshWallet() {
    if (!U || !game.connected) return;
    const p = await U.passport();
    if (p && p.ok && p.wallet) game.coins = p.wallet.coins;
}
if (U) U.connect({ portalId: PORTAL_ID }).then((s) => {
    if (!s) return; // standalone: fica como convidado
    game.connected = true;
    game.name = String(s.player.name || "jogador").slice(0, 24);
    if (/^#[0-9a-f]{6}$/i.test(s.player.color || "")) game.color = s.player.color;
    exitBtn.style.display = "inline-block";
    refreshWallet();
});
exitBtn.onclick = () => U && U.exit();

// ------------------------------------------------------------ input
addEventListener("keydown", (e) => {
    keys.add(e.key.toLowerCase());
    if ((e.key === " " || e.key === "Enter") && game.state === "over") restart();
});
addEventListener("keyup", (e) => keys.delete(e.key.toLowerCase()));
cv.addEventListener("pointerdown", (e) => { pointerX = e.clientX / W; });
cv.addEventListener("pointermove", (e) => { if (e.buttons || e.pointerType === "mouse") pointerX = e.clientX / W; });
cv.addEventListener("pointerup", (e) => { if (e.pointerType !== "mouse") pointerX = null; });
document.getElementById("again").onclick = restart;

function restart() {
    Object.assign(game, { state: "play", score: 0, time: ROUND, px: 0.5, items: [], fx: [], shake: 0 });
    endEl.style.display = "none";
}

function finish() {
    game.state = "over";
    game.items = [];
    if (U) U.event("score", game.score);
    if (game.score >= 30) achieve("30 votos numa apuracao");
    resultEl.textContent = `${game.name}: ${game.score} votos` + (game.connected ? "" : " (convidado - jogue dentro do Urna pra valer moedas ficticias)");
    endEl.style.display = "flex";
    setTimeout(refreshWallet, 2000);
}

// ------------------------------------------------------------ simulacao
const KINDS = [["voto", 0.72, 1], ["ouro", 0.08, 5], ["tnt", 0.2, -3]];
function spawn() {
    let r = Math.random(), k = KINDS[0];
    for (const kk of KINDS) { if ((r -= kk[1]) <= 0) { k = kk; break; } }
    game.items.push({ x: 0.06 + Math.random() * 0.88, y: -0.05, k: k[0], pts: k[2], v: 0.22 + Math.random() * 0.18 + (ROUND - game.time) * 0.008, rot: Math.random() * 6 });
}

function update(dt) {
    game.shake = Math.max(0, game.shake - dt);
    for (const f of game.fx) { f.t -= dt; f.y -= dt * 0.08; }
    game.fx = game.fx.filter((f) => f.t > 0);
    if (game.state !== "play") return;

    game.time -= dt;
    if (game.time <= 0) { game.time = 0; return finish(); }

    const dir = (keys.has("arrowright") || keys.has("d") ? 1 : 0) - (keys.has("arrowleft") || keys.has("a") ? 1 : 0);
    if (dir) { game.px += dir * 0.9 * dt; pointerX = null; }
    else if (pointerX !== null) game.px += Math.max(-1.2 * dt, Math.min(1.2 * dt, pointerX - game.px));
    game.px = Math.max(0.05, Math.min(0.95, game.px));

    spawnT -= dt;
    if (spawnT <= 0) { spawn(); spawnT = Math.max(0.22, 0.5 - (ROUND - game.time) * 0.008); }

    const boxY = 1 - 2.2 * S / H, half = (1.1 * S) / W;
    for (const it of game.items) {
        it.y += it.v * dt;
        it.rot += dt * 2;
        if (!it.gone && it.y > boxY - 0.6 * S / H && it.y < boxY + 0.3 * S / H && Math.abs(it.x - game.px) < half + 0.4 * S / W) {
            it.gone = true;
            game.score = Math.max(0, game.score + it.pts);
            game.fx.push({ x: it.x, y: boxY - 0.08, text: (it.pts > 0 ? "+" : "") + it.pts, t: 0.9, bad: it.pts < 0 });
            if (it.k === "tnt") game.shake = 0.35;
            if (it.k === "ouro") achieve("cedula de ouro");
            if (game.score >= 1) achieve("primeiro voto");
            if (game.score >= 15) achieve("15 votos");
        }
    }
    game.items = game.items.filter((it) => !it.gone && it.y < 1.1);
}

// ------------------------------------------------------------ desenho
function drawBallot(x, y, rot, gold) {
    g.save(); g.translate(x, y); g.rotate(Math.sin(rot) * 0.4);
    g.fillStyle = gold ? "#ffd23d" : "#f4f1e8";
    g.fillRect(-0.35 * S, -0.45 * S, 0.7 * S, 0.9 * S);
    g.strokeStyle = gold ? "#a07400" : "#8a8a9a"; g.lineWidth = 2;
    for (let i = 0; i < 3; i++) { g.strokeRect(-0.25 * S, (-0.3 + i * 0.25) * S, 0.14 * S, 0.14 * S); }
    g.strokeStyle = gold ? "#7a3cff" : "#1b7a3d"; g.lineWidth = 3;
    g.beginPath(); g.moveTo(-0.24 * S, -0.04 * S); g.lineTo(-0.16 * S, 0.04 * S); g.lineTo(0.2 * S, -0.08 * S); g.stroke();
    g.restore();
}
function drawTnt(x, y, rot) {
    g.save(); g.translate(x, y); g.rotate(Math.sin(rot) * 0.3);
    g.fillStyle = "#d8262f"; g.fillRect(-0.42 * S, -0.42 * S, 0.84 * S, 0.84 * S);
    g.fillStyle = "#f4f1e8"; g.fillRect(-0.42 * S, -0.14 * S, 0.84 * S, 0.28 * S);
    g.fillStyle = "#222"; g.font = `bold ${0.24 * S}px system-ui, sans-serif`; g.textAlign = "center"; g.textBaseline = "middle";
    g.fillText("TNT", 0, 0.01 * S);
    g.strokeStyle = "#ffb03d"; g.lineWidth = 2; g.beginPath(); g.moveTo(0, -0.42 * S); g.quadraticCurveTo(0.2 * S, -0.62 * S, 0.1 * S, -0.72 * S); g.stroke();
    g.restore();
}
function drawUrna(x, y) {
    g.fillStyle = "#3a3f4b"; g.fillRect(x - 1.1 * S, y - 0.5 * S, 2.2 * S, 1.5 * S);
    g.fillStyle = "#555c6b"; g.fillRect(x - 1.1 * S, y - 0.62 * S, 2.2 * S, 0.2 * S);
    g.fillStyle = "#111"; g.fillRect(x - 0.6 * S, y - 0.58 * S, 1.2 * S, 0.08 * S);
    g.fillStyle = "#9fd8c2"; g.fillRect(x - 0.9 * S, y - 0.25 * S, 1.0 * S, 0.6 * S);
    g.fillStyle = game.color; g.fillRect(x - 0.9 * S, y + 0.5 * S, 1.8 * S, 0.12 * S);
    for (let r = 0; r < 3; r++) for (let c = 0; c < 3; c++) { g.fillStyle = "#cfd3db"; g.fillRect(x + (0.25 + c * 0.24) * S, y + (-0.22 + r * 0.22) * S, 0.17 * S, 0.15 * S); }
    g.fillStyle = "#3dff8a"; g.fillRect(x + 0.25 * S, y + 0.4 * S, 0.65 * S, 0.08 * S);
    g.fillStyle = "#fff"; g.font = `bold ${0.32 * S}px system-ui, sans-serif`; g.textAlign = "center"; g.textBaseline = "bottom";
    g.fillText(game.name, x, y - 0.75 * S);
}

function draw() {
    g.save();
    if (game.shake > 0) g.translate((Math.random() - 0.5) * 14, (Math.random() - 0.5) * 14);
    const sky = g.createLinearGradient(0, 0, 0, H);
    sky.addColorStop(0, "#1b1035"); sky.addColorStop(1, "#3a1d5c");
    g.fillStyle = sky; g.fillRect(-20, -20, W + 40, H + 40);
    g.fillStyle = "#24163f"; g.fillRect(-20, H - 0.9 * S, W + 40, S + 20);

    for (const it of game.items) {
        if (it.k === "tnt") drawTnt(it.x * W, it.y * H, it.rot);
        else drawBallot(it.x * W, it.y * H, it.rot, it.k === "ouro");
    }
    drawUrna(game.px * W, H - 2.2 * S);

    g.textBaseline = "middle";
    for (const f of game.fx) {
        g.globalAlpha = Math.min(1, f.t * 2);
        g.fillStyle = f.big ? "#ffd23d" : f.bad ? "#ff5a5a" : "#3dff8a";
        g.font = `bold ${(f.big ? 0.42 : 0.55) * S}px system-ui, sans-serif`; g.textAlign = "center";
        g.fillText(f.text, f.x * W, f.y * H);
    }
    g.globalAlpha = 1;

    g.fillStyle = "#fff"; g.textAlign = "left"; g.font = `bold ${Math.max(16, 0.4 * S)}px system-ui, sans-serif`;
    g.fillText(`VOTOS ${game.score}`, 14, 26);
    g.textAlign = "right";
    g.fillText(`${Math.ceil(game.time)}s`, W - 14, 26);
    g.font = `${Math.max(13, 0.28 * S)}px system-ui, sans-serif`;
    g.textAlign = "left";
    const wallet = game.coins === null ? (game.connected ? "carteira: ..." : "fora do Urna") : `carteira: ${game.coins} moedas ficticias`;
    g.fillText(`${game.name} - ${wallet}`, 14, 52);
    g.restore();
}

let last = performance.now();
function frame(t) {
    const dt = Math.min(0.05, (t - last) / 1000);
    last = t;
    update(dt);
    flushAchievements(Date.now());
    draw();
    requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
