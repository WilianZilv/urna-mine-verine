#!/usr/bin/env node
// Urna Game Hub helper (UPP v1). Zero dependencies, Node 18+. Docs: https://urna-mine-verine.wilianzilv.workers.dev/hub.txt
//
//   node urna-hub.mjs prepare --id my-game --name "My Game" --url https://my-game.pages.dev/ --dir dist [--version 1.0.0] [--colors "#ff3d7f,#ffd23d"] [--desc "..."]
//       -> creator token (saved in ~/.urna-creator-token), registers the portal (or a new version), writes <dir>/.well-known/urna-portal.json
//   (now build + deploy <dir> so https://<origin>/.well-known/urna-portal.json is online)
//   node urna-hub.mjs publish --id my-game [--version 1.0.0]     -> verify origin + activate + check it is live
//   node urna-hub.mjs status --id my-game                         -> owner view (challenge, versions, next step)
//   node urna-hub.mjs unpublish --id my-game                      -> take the arch down (history kept)
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir, userInfo } from "node:os";
import { join } from "node:path";

const SITE = process.env.URNA_SITE || "https://urna-mine-verine.wilianzilv.workers.dev";
const TOKEN_FILE = process.env.URNA_TOKEN_FILE || join(homedir(), ".urna-creator-token");
const [cmd, ...rest] = process.argv.slice(2);
const arg = {};
for (let i = 0; i < rest.length; i++) if (rest[i].startsWith("--")) arg[rest[i].slice(2)] = rest[i + 1]?.startsWith("--") || rest[i + 1] === undefined ? "1" : rest[++i];
const die = (m) => { console.error("ERROR:", m); process.exit(1); };

async function api(method, path, body, token) {
    const r = await fetch(SITE + path, { method, headers: { "content-type": "application/json", ...(token ? { authorization: `Bearer ${token}` } : {}) }, body: body ? JSON.stringify(body) : undefined });
    const text = await r.text();
    let j = null;
    try { j = JSON.parse(text); } catch (e) { }
    return { s: r.status, j: j || { ok: false, error: text.slice(0, 200) } };
}

async function token() {
    if (process.env.URNA_TOKEN) return process.env.URNA_TOKEN;
    if (existsSync(TOKEN_FILE)) return readFileSync(TOKEN_FILE, "utf8").trim();
    let base = (arg.creator || userInfo().username || "agent").replace(/[^A-Za-z0-9_-]/g, "").slice(0, 16) || "agent";
    if (base.length < 3) base += "-dev";
    for (let i = 0; i < 4; i++) {
        const name = i ? `${base}-${Math.floor(Math.random() * 9000 + 1000)}` : base;
        const r = await api("POST", "/api/mods/register", { name });
        if (r.j.token) {
            writeFileSync(TOKEN_FILE, r.j.token, { mode: 0o600 });
            console.log(`creator "${name}" registered; token saved in ${TOKEN_FILE} (keep it private)`);
            return r.j.token;
        }
        if (r.s !== 409) die(`register failed: ${r.s} ${JSON.stringify(r.j)}`);
    }
    die("could not pick a free creator name (use --creator <name>)");
}

const id = arg.id;
if (!["prepare", "publish", "status", "unpublish"].includes(cmd) || !id) die("usage: node urna-hub.mjs prepare|publish|status|unpublish --id <portal-id> ... (see header)");
const tok = await token();

if (cmd === "prepare") {
    if (!arg.url || !arg.dir) die("prepare needs --url https://... and --dir <folder that gets deployed>");
    const colors = (arg.colors || "#7a3cff,#00e5ff").split(",").map((c) => c.trim());
    const manifest = { name: arg.name || id, version: arg.version || "1.0.0", url: arg.url, description: arg.desc || "", thumbnail: { colors } };
    let r = await api("GET", `/api/portals/${id}`, null, tok);
    if (r.s === 404) {
        r = await api("POST", "/api/portals", { id, ...manifest }, tok);
        if (r.s !== 201) die(`create failed: ${r.s} ${r.j.error}`);
        console.log(`portal "${id}" created (v${manifest.version})`);
    } else if (r.s === 200 && !r.j.portal.challenge) {
        die(`portal id "${id}" belongs to someone else: pick another --id`);
    } else if (r.s === 200 && !r.j.portal.versions.some((v) => v.version === manifest.version)) {
        r = await api("PUT", `/api/portals/${id}/versions`, manifest, tok);
        if (r.s !== 201) die(`new version failed: ${r.s} ${r.j.error} (bump --version)`);
        console.log(`new version ${manifest.version} registered`);
    } else if (r.s !== 200) die(`${r.s} ${r.j.error}`);
    const p = r.j.portal;
    const dir = join(arg.dir, ".well-known");
    const file = join(dir, "urna-portal.json");
    mkdirSync(dir, { recursive: true });
    let doc = { urna_portal: 1, portals: [] };
    try { doc = JSON.parse(readFileSync(file, "utf8")); } catch (e) { }
    doc.urna_portal = 1;
    doc.portals = [...(Array.isArray(doc.portals) ? doc.portals : []).filter((x) => x.id !== id), { id, challenge: p.challenge }];
    writeFileSync(file, JSON.stringify(doc, null, 1) + "\n");
    console.log(`wrote ${file}\nNEXT: deploy "${arg.dir}" so ${p.well_known.url} serves it, then: node urna-hub.mjs publish --id ${id}`);
}

if (cmd === "publish") {
    let r = await api("POST", `/api/portals/${id}/verify`, null, tok);
    if (r.s !== 200) die(`verification failed: ${r.j.error}\nexpected at ${r.j.well_known?.url}: ${JSON.stringify(r.j.well_known?.body)}`);
    console.log(`origin verified: ${r.j.portal.verified_origin}`);
    r = await api("POST", `/api/portals/${id}/activate`, arg.version ? { version: arg.version } : {}, tok);
    if (r.s !== 200) die(`activate failed: ${r.s} ${r.j.error}`);
    const live = (await api("GET", "/api/portals")).j.portals?.find((x) => x.id === id);
    if (!live) die("activated but not listed as live (check /api/portals)");
    console.log(`LIVE: arch "${live.current.name}" v${live.current.version} in the Game Hub at ${SITE} (walk south from the lab)`);
}

if (cmd === "status") console.log(JSON.stringify((await api("GET", `/api/portals/${id}`, null, tok)).j, null, 1));

if (cmd === "unpublish") {
    const r = await api("DELETE", `/api/portals/${id}`, null, tok);
    if (r.s !== 200) die(`${r.s} ${r.j.error}`);
    console.log(`portal "${id}" unpublished (re-activate any time with publish)`);
}
