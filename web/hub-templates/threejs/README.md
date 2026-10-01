# Pula Urna - template three.js do URNA GAME HUB

Jogo 3D de 30 s (pular de plataforma em plataforma) num único `index.html`. three.js (MIT) vem da CDN jsDelivr
por import map, nada copiado. SDK do Urna já plugado com `data-auto` (os outros jogadores do Urna aparecem na
cena) + `UrnaPortal.auto.attach/pose`, nome e cor do jogador, carteira fictícia (passaporte), placar no fim
(`UrnaPortal.event("score")`), conquistas no chat e botão "VOLTAR PRA VILA". Fora do Urna roda como convidado.
Jogar: https://urna-mine-verine.wilianzilv.workers.dev/hub-templates/threejs/

Licença MIT.

## 5 passos pra virar um arco no Hub

1. **Copie esta pasta** (`index.html`) pro teu projeto.
2. **Troque o nome/id**: escolha um id `[a-z0-9-]`, 2-32 letras (ex.: `meu-jogo`). Troque `pula-urna` em
   `data-portal-id` e em `PORTAL_ID` (os dois no index.html), e o `<title>`. Esse id + nome vão no manifest.
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

Dica: troque a versão do three.js no import map se precisar (o modo auto do SDK funciona com qualquer r1xx).
