# Cata Voto - template canvas 2D do URNA GAME HUB

Jogo arcade de 30 s (pegar cédulas, fugir da TNT) em canvas 2D puro, sem dependências, com o SDK do Urna
já plugado: nome e cor do jogador, carteira fictícia (passaporte), placar no fim (`UrnaPortal.event("score")`),
conquistas no chat (`"achievement"`) e botão "VOLTAR PRA VILA" (`UrnaPortal.exit()`). Fora do Urna roda como
convidado. Jogar: https://urna-mine-verine.wilianzilv.workers.dev/hub-templates/canvas2d/

Arquivos: `index.html` (tag do SDK + tela de fim) e `game.js` (o jogo todo). Licença MIT.

## 5 passos pra virar um arco no Hub

1. **Copie esta pasta** (`index.html` + `game.js`) pro teu projeto.
2. **Troque o nome/id**: escolha um id `[a-z0-9-]`, 2-32 letras (ex.: `meu-jogo`). Troque `cata-voto` em
   `data-portal-id` (index.html) e em `PORTAL_ID` (game.js), e o `<title>`. Esse id + nome vão no manifest.
3. **Publique em HTTPS grátis**, numa origem só tua (o arquivo do passo 4 fica na RAIZ do site):
   Cloudflare Pages (`npx wrangler pages deploy <pasta> --project-name <id>`), Netlify
   (`npx netlify-cli deploy --prod --dir <pasta>`) ou GitHub Pages de usuário (`<user>.github.io`, com `.nojekyll`).
4. **Sirva o challenge**: o registro devolve `{"urna_portal":1,"portals":[{"id":"<id>","challenge":"upp-..."}]}`;
   salve EXATAMENTE isso em `<pasta>/.well-known/urna-portal.json`, publique de novo e confira com
   `curl -s https://<teu-site>/.well-known/urna-portal.json`.
5. **Registre** seguindo https://urna-mine-verine.wilianzilv.workers.dev/hub.txt (helper `urna-hub.mjs prepare`
   + `publish`, ou curl: register -> POST /api/portals -> verify -> activate). Manifest + comandos prontos:
   https://urna-mine-verine.wilianzilv.workers.dev/hub-templates/manifest-generator/
   Nunca coloque o token de criador dentro da pasta publicada.
