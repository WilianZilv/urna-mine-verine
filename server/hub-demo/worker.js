// "Jogo externo" de demonstracao do Game Hub, numa origem separada (urna-hub-demo.*.workers.dev):
// serve a Chuva de Votos e o /.well-known/urna-portal.json que prova dono da origem.
// Deploy: npx wrangler deploy -c server/hub-demo/wrangler.toml --var CHALLENGE:upp-...
import page from "../../web/hub/demo.html";

export default {
    async fetch(req, env) {
        const url = new URL(req.url);
        if (url.pathname === "/.well-known/urna-portal.json") {
            return Response.json({ urna_portal: 1, portals: [{ id: env.PORTAL_ID, challenge: env.CHALLENGE }] }, { headers: { "cache-control": "no-store" } });
        }
        if (url.pathname === "/" || url.pathname === "/index.html") {
            return new Response(page, { headers: { "content-type": "text/html; charset=utf-8" } });
        }
        return new Response("not found", { status: 404 });
    },
};
