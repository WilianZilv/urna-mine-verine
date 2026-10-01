// "Jogo externo" de demonstracao do Game Hub, numa origem separada (urna-hub-demo.*.workers.dev):
// serve a Chuva de Votos e o /.well-known/urna-portal.json que prova dono da origem.
// Deploy: npx wrangler deploy -c server/hub-demo/wrangler.toml --var CHALLENGE:upp-...
import page from "../../web/hub/demo.html";
import plain from "../../web/hub/plain.html";
import auto3d from "../../web/hub/auto3d.html";

export default {
    async fetch(req, env) {
        const url = new URL(req.url);
        if (url.pathname === "/.well-known/urna-portal.json") {
            return Response.json({ urna_portal: 1, portals: [{ id: env.PORTAL_ID, challenge: env.CHALLENGE }] }, { headers: { "cache-control": "no-store" } });
        }
        if (url.pathname === "/" || url.pathname === "/index.html") {
            return new Response(page, { headers: { "content-type": "text/html; charset=utf-8" } });
        }
        // jogo 2D sem SDK nenhum: testa a presenca que o overlay do Urna desenha sozinho
        if (url.pathname === "/sem-sdk") return new Response(plain, { headers: { "content-type": "text/html; charset=utf-8" } });
        // jogo three.js com so a tag <script data-auto>: testa o modo auto do SDK
        if (url.pathname === "/auto-3d") return new Response(auto3d, { headers: { "content-type": "text/html; charset=utf-8" } });
        return new Response("not found", { status: 404 });
    },
};
