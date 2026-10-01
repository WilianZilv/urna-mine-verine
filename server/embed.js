// "Mundo vivo": GET /api/world (resumo publico do DO pros widgets de web/embed/). So dado que o jogo ja mostra
// pra qualquer um (nada de token, IP, segredo de criador). Cache de CACHE_MS no DO: embed pesado sai barato.
const CACHE_MS = 5000;
const HEADERS = {
    "content-type": "application/json; charset=utf-8",
    "access-control-allow-origin": "*",
    "access-control-allow-methods": "GET, OPTIONS",
    "cache-control": "public, max-age=10",
};
const cache = new WeakMap();

const str = (v, n = 200) => String(v ?? "").replace(/[\u0000-\u001f]/g, " ").slice(0, n);
const num = (v) => (Number.isFinite(+v) ? +v : 0);
const today = () => new Date().toISOString().slice(0, 10);
// Um lugar quebrado vira null em vez de derrubar o resumo inteiro.
const part = (fn) => {
    try {
        return fn() ?? null;
    } catch (e) {
        return null;
    }
};

export function build(room, now = Date.now()) {
    const pl = room.places;
    return {
        at: now,
        online: part(() => [...room.clients.values()].filter((c) => c.name).length) ?? 0,
        treasury: part(() => num(room.eco?.s?.tr)),
        laws: part(() => (pl?.congresso?.active?.(now) || []).map((l) => ({
            id: l.id,
            n: str(l.n, 40),
            d: str(l.d, 80),
            left: Math.max(0, Math.round((num(pl.congresso.s?.laws?.[l.id]?.until) - now) / 1000)),
        }))) ?? [],
        bolsa: part(() => {
            const s = pl?.bolsa?.snapshot?.();
            if (!s) return null;
            const tickers = [...(s.tk || []), ...(s.mk || [])].map((t) => [str(t.s, 8), Math.round(num(t.p) * 100) / 100, num(t.d)]);
            return { tickers, mood: num(s.mood), ix: num(s.ix), alert: str(s.alert) || undefined };
        }),
        tv: part(() => {
            const s = pl?.tv?.snap?.();
            if (!s) return null;
            return { headlines: (s.h || []).slice(0, 3).map((h) => str(h)), ticker: str(s.tk, 200) || undefined, urgent: s.urg?.text ? str(s.urg.text) : undefined };
        }),
        banco: part(() => ({ rich: (pl?.banco?.rich?.() || []).slice(0, 5).map((r) => [str(r.n, 16), Math.round(num(r.t))]) })),
        terminal: part(() => {
            const s = pl?.terminal?.s;
            if (!s) return null;
            const trips = s.day === today() ? Object.values(s.trips || {}).reduce((a, n) => a + num(n), 0) : 0;
            return { trips_today: trips, total: num(s.total) };
        }),
        tour: part(() => ({ guides: (pl?.tour?.s?.mural || []).slice(0, 5).map((n) => str(n, 16)) })),
        lab: part(() => {
            const x = room.lab?.s?.f?.at?.(-1);
            if (!x) return null;
            return { title: str(x.pt || x.ti, 200), journal: str(x.j, 80), year: str(x.y, 4), url: /^https:\/\//.test(x.u) ? str(x.u, 200) : undefined, topic: str(x.topic, 40) };
        }),
    };
}

export function world(room, req) {
    if (req.method === "OPTIONS") return new Response(null, { status: 204, headers: HEADERS });
    if (req.method !== "GET" && req.method !== "HEAD") return new Response("use GET", { status: 405, headers: HEADERS });
    const now = Date.now();
    let c = cache.get(room);
    if (!c || now - c.at >= CACHE_MS) {
        c = { at: now, body: JSON.stringify(build(room, now)) };
        cache.set(room, c);
    }
    return new Response(req.method === "HEAD" ? null : c.body, { headers: HEADERS });
}
