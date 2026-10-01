# FINAL-README: botar a vila no ar (de graça, na nuvem)

Tudo roda no **Cloudflare** (plano grátis): o site com o jogo **e** o servidor multiplayer,
no mesmo link. Nada roda no seu PC.

> Por que não Vercel? A Vercel não segura conexão WebSocket aberta (multiplayer em tempo real).
> O Cloudflare segura (Durable Objects) e também hospeda o site. Um lugar só, um comando só.

## Subir (primeira vez) — 3 passos

Precisa de Node.js instalado (você já tem: `node -v`).

1. Crie conta grátis em https://dash.cloudflare.com/sign-up
2. No PowerShell, na pasta do projeto, faça login (abre o navegador, clica em "Allow"):

```powershell
cd d:\Projetos\urna-mine-verine
npx wrangler login
```

3. Publique:

```powershell
npx wrangler deploy
```

No fim ele mostra o link, tipo `https://urna-mine-verine.SEU-USUARIO.workers.dev`.
**Pronto.** Manda esse link pra galera. Cada um digita o nome e entra na mesma vila.

Dica: `https://...workers.dev/?nome=FULANO` já entra direto com o nome.

## Atualizar depois de mexer no código

```powershell
powershell -ExecutionPolicy Bypass -File tools\build_web.ps1
npx wrangler deploy
```

## Jogar a versão nativa (Windows) na mesma vila

Crie `assets\server.txt` com o link do deploy na primeira linha, e rode:

```powershell
cargo run --release
```

Sem `assets\server.txt` (ou `$env:MP_URL`) o nativo roda offline, sozinho.

## Como funciona (resumo)

- `web/` é o site: `index.html` + `urna.wasm` (o jogo em Rust compilado) + `urna.js` (som, YouTube, rede).
- `server/worker.js` é o servidor: repassa mensagens entre jogadores, guarda as mudanças do mundo
  (blocos, crateras) pra quem chega depois, e escolhe um **host** (o primeiro a entrar), que simula
  lutadores, urna e villagers e manda pra todo mundo. Se o host sair, o próximo assume.
- `wrangler.toml` amarra tudo.

## Limites do plano grátis

- Durable Objects grátis: 100 mil "requests" por dia. Cada 20 mensagens recebidas = 1 request.
  O jogo manda ~7 mensagens/s por jogador. Na prática: **5 jogadores ≈ 14 h/dia**, 10 jogadores ≈ 7 h/dia.
- Passou do limite: o multiplayer para até 21h (horário de Brasília, meia-noite UTC) e o jogo
  continua offline pra cada um. Nada é cobrado sem você cadastrar cartão.
- Mundo fica na memória: se todo mundo sair por um tempo, a vila volta zerada.

## Testar local antes de subir (opcional)

```powershell
npx wrangler dev
```

Abra http://127.0.0.1:8787 (e o nativo com `$env:MP_URL = "http://127.0.0.1:8787"`).
