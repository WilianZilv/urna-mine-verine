// E2E ao vivo do Universo: avatar mod + skin, passaporte, presenca, grant/spend. node tools/verify_universe.mjs
// Tokens de criador ficam em %TEMP% (nunca no repo): urna-universe-token.txt (avatar) e urna-hub-token.txt (dono do portal demo).
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const SITE = process.env.SITE || "https://urna-mine-verine.wilianzilv.workers.dev";
const PORTAL = "chuva-de-votos";
const TMP = process.env.TEMP || "/tmp";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let fails = 0;
const must = (ok, what, extra = "") => {
    console.log(`${ok ? "PASS" : "FAIL"}: ${what}${extra ? " " + extra : ""}`);
    if (!ok) fails++;
};
async function api(method, path, body, token) {
    const r = await fetch(SITE + path, { method, headers: { "content-type": "application/json", ...(token ? { authorization: `Bearer ${token}` } : {}) }, body: body === undefined ? undefined : JSON.stringify(body) });
    let j = null;
    try { j = await r.json(); } catch (e) { }
    return { s: r.status, j };
}
async function creator(file, name) {
    const f = join(TMP, file);
    if (existsSync(f)) return readFileSync(f, "utf8").trim();
    const r = await api("POST", "/api/mods/register", { name: `${name}-${Date.now() % 100000}` });
    writeFileSync(f, r.j.token);
    return r.j.token;
}
/// Sobe (ou versiona) e ativa um mod de exemplo; devolve o id usado.
async function publish(file, token) {
    const pkg = JSON.parse(readFileSync(new URL(`../examples/mods/${file}`, import.meta.url), "utf8"));
    let r = await api("POST", "/api/mods/validate", pkg);
    must(r.j?.ok, `validate ${file}`, JSON.stringify(r.j?.errors || []));
    const meta = await api("GET", `/api/mods/${pkg.manifest.id}`);
    const me = await api("GET", "/api/mods/me", undefined, token);
    const mine = me.j?.mods?.some((m) => m.id === pkg.manifest.id);
    if (meta.s === 404) r = await api("POST", "/api/mods", pkg, token);
    else if (mine) {
        const latest = meta.j.mod.latest;
        if (latest !== pkg.manifest.version) {
            const [a, b, c] = latest.split(".").map(Number);
            pkg.manifest.version = `${a}.${b}.${c + 1}`;
        }
        r = latest === pkg.manifest.version ? { s: 201 } : await api("PUT", `/api/mods/${pkg.manifest.id}/versions`, pkg, token);
    } else {
        pkg.manifest.id = `${pkg.manifest.id}-${Date.now() % 10000}`;
        r = await api("POST", "/api/mods", pkg, token);
    }
    must(r.s === 201, `upload ${pkg.manifest.id}`, r.s === 201 ? "" : JSON.stringify(r.j));
    r = await api("POST", `/api/mods/${pkg.manifest.id}/activate`, {}, token);
    must(r.j?.ok && r.j.kind === pkg.manifest.kind, `activate ${pkg.manifest.id} (kind ${r.j?.kind})`, r.j?.ok ? "" : JSON.stringify(r.j));
    return pkg.manifest.id;
}
function client(name) {
    const ws = new WebSocket(SITE.replace("https", "wss") + "/ws");
    const c = { ws, name, inbox: [] };
    ws.onmessage = (e) => c.inbox.push(JSON.parse(e.data));
    c.open = new Promise((r) => (ws.onopen = () => { ws.send(JSON.stringify({ t: "hello", n: name })); r(); }));
    c.send = (m) => ws.send(JSON.stringify(m));
    c.wait = async (pred, ms = 6000) => {
        const t0 = Date.now();
        while (Date.now() - t0 < ms) {
            const i = c.inbox.findIndex(pred);
            if (i >= 0) return c.inbox.splice(i, 1)[0];
            await sleep(50);
        }
        return null;
    };
    return c;
}

const uniTok = await creator("urna-universe-token.txt", "urna-universe");
const hubTok = readFileSync(join(TMP, "urna-hub-token.txt"), "utf8").trim();

// 1) avatar mod + item mod (item do mesmo criador do portal demo, pra allowlist)
const avId = await publish("astronauta.json", uniTok);
const itemId = await publish("voto-dourado.json", hubTok);
let r = await api("PUT", `/api/portals/${PORTAL}/items`, { items: [itemId, avId] }, hubTok);
must(r.j?.approved?.includes(itemId) && r.j.rejected.some((x) => x.id === avId), "allowlist: item aprovado, avatar recusado", JSON.stringify(r.j?.rejected || r.j));
r = await api("PUT", `/api/portals/${PORTAL}/items`, { items: [itemId] }, uniTok);
must(r.s === 403, "allowlist: nao-dono recusado");
r = await api("GET", "/api/universe/avatars");
must(r.j?.avatars?.some((a) => a.id === avId), "lista publica de avatares");

// 2) dois jogadores na vila; P1 veste a skin, P2 ve o "p" com av carimbado pelo servidor
const tag = Date.now().toString(36).slice(-5);
const p1 = client(`uvA${tag}`), p2 = client(`uvB${tag}`);
await Promise.all([p1.open, p2.open]);
must(!!(await p1.wait((m) => m.t === "uv_list")), "join recebe uv_list");
p1.send({ t: "uv_set", mod: avId });
const me = await p1.wait((m) => m.t === "uv_me");
must(me?.me?.startsWith(`${avId}@`), "uv_set confirmado", me?.me);
p1.send({ t: "p", p: [64, 21, 100], y: 0, c: 0, av: "forjado@9.9.9" });
const seen = await p2.wait((m) => m.t === "p" && m.id && m.av);
must(!!seen?.av && seen.av === me?.me, "remoto recebe p com av do servidor", seen?.av);
p2.send({ t: "p", p: [65, 21, 100], y: 0, c: 0, av: "forjado@9.9.9" });
const forged = await p1.wait((m) => m.t === "p" && m.p?.[0] === 65);
must(forged && !forged.av, "av forjado por cliente sem skin e removido");
p2.send({ t: "uv_get", mod: avId });
const pk = await p2.wait((m) => m.t === "uv_pkg");
must(pk?.pkg?.manifest?.kind === "avatar", "uv_get devolve pacote do avatar");

// 3) portal: token com avatar + ctok so pra pagina; passaporte
p1.send({ t: "hub", k: "enter", p: PORTAL, col: "#ff8800", ch: "steve" });
p2.send({ t: "hub", k: "enter", p: PORTAL, col: "#00ffaa", ch: "steve" });
const s1 = await p1.wait((m) => m.t === "hub_s"), s2 = await p2.wait((m) => m.t === "hub_s");
const c1 = await p1.wait((m) => m.t === "uv_ctok");
must(!!s1?.tok && !!s2?.tok && !!c1?.ctok, "sessao de portal + token de confirmacao");
r = await api("GET", `/api/passport?token=${encodeURIComponent(s1.tok)}`);
must(r.j?.ok && r.j.avatar?.manifest?.id === avId && r.j.avatar.avatar?.height > 1, "passaporte com avatar completo", `${r.j?.avatar_ref} wallet=${r.j?.wallet?.coins}`);
must(r.j?.limits?.grant_items?.some((x) => x.id === itemId), "passaporte lista itens concedíveis");
r = await api("GET", `/api/passport?token=bad`);
must(r.s === 401, "passaporte sem token valido = 401");

// 4) presenca: os dois se veem; update repassado; limite de tamanho
const pres = (tok) => {
    const ws = new WebSocket(`${SITE.replace("https", "wss")}/api/portals/${PORTAL}/presence?token=${encodeURIComponent(tok)}`);
    const o = { ws, inbox: [] };
    ws.onmessage = (e) => o.inbox.push(JSON.parse(e.data));
    o.open = new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
    o.wait = async (pred, ms = 5000) => { const t0 = Date.now(); while (Date.now() - t0 < ms) { const i = o.inbox.findIndex(pred); if (i >= 0) return o.inbox.splice(i, 1)[0]; await sleep(40); } return null; };
    return o;
};
const a = pres(s1.tok);
await a.open;
const wa = await a.wait((m) => m.t === "welcome");
const b = pres(s2.tok);
await b.open;
const wb = await b.wait((m) => m.t === "welcome");
must(wb?.players?.some((p) => p.name === p1.name && p.avatar === me.me), "B ve A no welcome (com avatar)");
must(!!(await a.wait((m) => m.t === "join" && m.player.name === p2.name)), "A recebe join de B");
a.ws.send(JSON.stringify({ t: "u", s: { x: 0.42, anim: "run", junk: { deep: 1 } } }));
const u = await b.wait((m) => m.t === "u");
must(u?.s?.x === 0.42 && u.s.anim === "run" && !("junk" in u.s), "update repassado e saneado", JSON.stringify(u?.s));
a.ws.send(JSON.stringify({ t: "u", s: { big: "x".repeat(600) } }));
for (let i = 0; i < 40; i++) a.ws.send(JSON.stringify({ t: "u", s: { i } }));
await sleep(800);
const got = b.inbox.filter((m) => m.t === "u").length;
must(got > 0 && got < 30, "rajada limitada pelo servidor", `${got}/40 repassadas`);
const st = await api("GET", `/api/portals/${PORTAL}`);
must(st.j?.portal?.players >= 2, "contador do arco inclui presenca", `players=${st.j?.portal?.players}`);
b.ws.close();
must(!!(await a.wait((m) => m.t === "leave")), "A recebe leave de B");
must(!!wa, "A recebeu welcome");

// 5) grant: dentro do orcamento ok; duplicado e acima do orcamento recusados; item da allowlist
const key = `e2e-${tag}`;
r = await api("POST", "/api/universe/grant", { token: s1.tok, coins: 5, reason: "teste e2e", key });
must(r.j?.ok && r.j.granted.coins === 5, "grant 5 moedas", JSON.stringify(r.j));
r = await api("POST", "/api/universe/grant", { token: s1.tok, coins: 5, reason: "teste e2e", key });
must(r.s === 409 && r.j.error === "duplicate", "grant duplicado recusado (idempotencia)");
r = await api("POST", "/api/universe/grant", { token: s1.tok, coins: 60, reason: "demais", key: key + "b" });
must(r.s === 429 && r.j.error === "budget", "grant acima do orcamento diario recusado", JSON.stringify(r.j));
r = await api("POST", "/api/universe/grant", { token: s1.tok, item: itemId, n: 2, reason: "trofeu", key: key + "i" });
must(r.j?.ok && r.j.inventory?.some((x) => x.id === itemId && x.n === 2), "grant item da allowlist");
must(!!(await p1.wait((m) => m.t === "uv_bag" && m.items.some((x) => x.id === itemId))), "inventario do jogo recebe o item (uv_bag)");
r = await api("POST", "/api/universe/grant", { token: s1.tok, item: itemId, n: 2, reason: "trofeu", key: key + "j" });
must(r.s === 429, "item acima do daily_cap recusado");
r = await api("POST", "/api/universe/grant", { token: s1.tok, item: avId, reason: "x", key: key + "k" });
must(r.s === 403, "item fora da allowlist recusado");

// 6) spend: sem confirmacao = 403; com o ctok da pagina = ok
r = await api("POST", "/api/universe/spend", { token: s1.tok, coins: 2, reason: "vida", key: key + "s" });
must(r.s === 403 && r.j.error === "confirm_required", "spend sem confirmacao do jogador recusado");
r = await api("POST", "/api/universe/spend", { ctok: c1.ctok, coins: 2, reason: "vida extra", key: key + "s" });
must(r.j?.ok && r.j.spent.coins === 2, "spend confirmado", JSON.stringify(r.j));
r = await api("POST", "/api/universe/spend", { ctok: c1.ctok, item: itemId, n: 1, reason: "usar trofeu", key: key + "t" });
must(r.j?.ok && r.j.inventory.find((x) => x.id === itemId)?.n === 1, "spend de item confirmado");
r = await api("GET", `/api/universe/receipt?token=${encodeURIComponent(s1.tok)}&key=${key}`);
must(r.j?.ok && r.j.kind === "grant", "recibo do grant");

a.ws.close();
p1.ws.close();
p2.ws.close();
console.log(fails ? `${fails} FALHA(S)` : "TUDO OK");
process.exit(fails ? 1 : 0);
