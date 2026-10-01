# URNA-MINE-VERINE

Clone de Minecraft em Rust. Vila voxel com villagers, um **clube de house** tocando
(com escudo mágico), uma **briga generalizada** no meio da praça (Lula, Flávio Bolsonaro,
Renan Santos e Wolverine, todos com bandeira) e uma **urna eletrônica gigante** flutuando,
soltando laser e explodindo tudo que não está protegido pelo escudo.

Zero assets externos: texturas, modelos e efeitos são gerados em código na inicialização.
A música vem do **telão do clube**, que toca YouTube via `yt-dlp` + `ffmpeg`.

**Um link pra qualquer agente de IA** (Cursor, Claude Code, Codex...): cola
`Le https://urna-mine-verine.wilianzilv.workers.dev/skill.md e faz o que ela diz` — ele pergunta se tu quer
criar um mod, conectar teu jogo ao Hub ou só jogar, e faz o resto (`server/skill.js`, cópia em `docs/SKILL.md`).

---

## Rodar (passo a passo)

1. Rust instalado (`rustup`, testado com rustc 1.96).
2. `ffmpeg` no PATH e `yt-dlp` (`uv tool install yt-dlp`; sem ele o jogo tenta `uvx yt-dlp`).
3. No diretório do projeto:

```powershell
cargo run --release
```

4. Clique na janela para capturar o mouse. Você nasce na **praça central** (fonte + placar);
   ruas em cruz + anel levam a **arena** (N), **clube** (O), **ciência** (L), casas/torre/skate (S).
5. Wolverine cai do céu aos 20s (ou aperte `K`).

Primeiro build: ~30s. Depois: instantâneo.

### Multiplayer e navegador

Deploy grátis na nuvem (Cloudflare): ver **[FINAL-README.md](FINAL-README.md)**.

- Só pede nome; chat por texto (`T` ou `Enter`). Tudo sincronizado: jogadores, blocos, crateras,
  tiros da urna, lutadores, villagers, placar, telão (`Y` troca pra todo mundo).
- Nativo entra no servidor com `assets/server.txt` (ou `$env:MP_URL`); sem isso roda offline.
  `$env:URNA_NOME` / `assets/nome.txt` pula a tela de nome. No navegador: `?nome=FULANO`.
- Navegador: `powershell -ExecutionPolicy Bypass -File tools\build_web.ps1` gera `web/urna.wasm`.
  O telão vira um iframe do YouTube projetado na parede (som liga no primeiro clique).

### Contagem regressiva das eleições

Placar gigante de blocos no norte da praça: **FALTAM xD HH:MM:SS PRAS ELEICOES 2026**
(1º turno, 04/10/2026 08:00 de Brasília). Na abertura: fogos, urna enlouquecida (rajada de
`CONFIRMA!!!` por 2 min) e banner. Depois mostra o tempo até fechar (17:00).
Testar: `$env:ELEICAO_EM_SEGUNDOS = "30"; cargo run --release`.

### Telão do clube (YouTube)

Por padrão toca https://www.youtube.com/watch?v=dPaWD5E7xMM (vídeo + áudio, em loop).
Enquanto carrega, toca um house sintetizado (124 BPM) de fallback.

- Trocar em jogo: copie qualquer link do YouTube e aperte `Y`.
- Trocar o padrão: `$env:TELAO_URL = "https://..."; cargo run --release`
  ou primeira linha de `assets/telao.txt`. Arquivo local/URL direta também funciona.
- Pipeline (`src/telao.rs`): `yt-dlp -g` (≤360p) → `ffmpeg` áudio f32 no mixer cpal +
  `ffmpeg` vídeo 640×360 RGBA 24fps em textura, sincronizado pelo relógio do áudio.

---

## Controles

| Tecla | Ação |
| --- | --- |
| `WASD` | andar |
| `Espaço` | pular (subir no voo) |
| `Shift` | correr |
| `F` | alterna voo (`Ctrl` desce) |
| Mouse | olhar |
| Botão esquerdo | quebrar bloco / socar lutador ou villager |
| Botão direito | colocar bloco |
| `1`-`9` / roda | escolher slot da hotbar (Steve) |
| `E` / `I` | inventário do Steve |
| `K` | chamar Wolverine agora |
| `Y` | tocar no telão o link do YouTube copiado (navegador: cola numa caixa) |
| `T` / `Enter` | chat (Enter envia, Esc cancela) |
| `R` | resetar mundo (regenera vila, NPCs) |
| `M` | mutar |
| `H` | mostrar/esconder ajuda |
| `Tab` / `Esc` | soltar mouse |

### Personagens (`C` ou botão `PERS`)

| # | Personagem | Jogabilidade |
| --- | --- | --- |
| 1 | Steve Survival | 10 corações, dano de queda/explosão/fogo/PvP, morre e renasce na torre, sem voo; hotbar com picareta, machado, pá, espada, arco + flechas, TNT e isqueiro; segura `Esq` pra minerar (rachadura; ferramenta certa ~8x mais rápida) e o bloco vai pro inventário |
| 2 | Steve Criativo | voa (`F`), quebra na hora, blocos infinitos, sem dano |
| 3 | Skatista (estilo Skate 3) | flick-it no mouse/arrasto (velocidade do flick = altura do pop): baixo→cima ollie, baixo→diagonal kickflip/heelflip, baixo→lado shove-it, baixo→lado→cima 360 shove-it, baixo→lado→diagonal varial, esq→baixo→cima-dir 360 flip/laser, invertido = nollie; `W` rema, `S` freia/powerslide, `A/D` carve (no ar gira, no grind equilibra), `Q/E` grab (mouse escolhe indy/melon/nose/tail/stalefish), `Shift` manual/nose manual com equilíbrio, grind 50-50/5-0/nosegrind/boardslide em muro ou quina; fakie, aterrissagem limpa/sketchy/bail, câmera baixa estilo Skate 3 |
| 4 | Bandido (estilo GTA 3) | terceira pessoa, 12 armas (punho, taco, pistola, uzi, escopeta, AK-47, M16, sniper com zoom, lança-foguete, lança-chamas, granada, molotov); sedã da 1ª missão perto da torre (`F` entra/sai, `Espaço` freio de mão, atropela) |
| 5 | Arma de portal (inspirada em Portal) | primeira pessoa, não quebra bloco; `Esq` portal azul, `Dir` laranja (celular: `AZUL`/`LARANJA`); só em face plana 1x2 (parede, chão, teto), o novo substitui o antigo da mesma cor; cada portal mostra a vista do par; jogador, cubos, partículas e villagers arremessados atravessam com momento preservado; `Q` cria cubo companheiro, `E` (`CUBO`) pega/solta |
| 6 | Encanador (paródia SM64) | `Espaço` pulo simples/duplo/triplo, `Shift` agacha → long jump/mortal pra trás, `E` soco-soco-chute (correndo = mergulho), `Shift` no ar = sentada com onda de choque; o MARIO (NPC, `src/mario.rs`) cai do céu num cano aos ~6s e briga com os candidatos (renasce em 45s) |
| 7 | Wolverine (`src/wolvie.rs`) | câmera no ombro; `X` garras (snikt); `Esq` combo de 4 (direção muda o golpe), segurar `Esq`/`F` pesado, `S+F` gancho que lança + combo aéreo, `F` no ar mergulho, `Shift+F` tornado; `E` bote com trava (`Tab`/botão do meio) e monta no alvo (urna/kaiju inclusive) pra esfaquear no tempo do anel; `Dir` defende (na hora certa reflete laser), `Ctrl`/`V` rola, segurar `Espaço` na parede escala; `R` fúria berserker com a barra cheia; fator de cura, roupa rasga e mostra o adamantium; PvP |

**Portais (`src/portal.rs`):** cada jogador tem seu par (os meus azul/laranja, os dos outros em outras
cores) e todo portal do mapa funciona pra qualquer um. A vista é renderizada de uma câmera virtual
atravessando o par num render target de baixa resolução, com near plane oblíquo no portal de saída,
e amostrada na posição de tela (vira janela); só os 2 mais próximos visíveis (1 no celular) ganham
vista, o resto e os portais dentro da vista mostram redemoinho. Colocação vai pelo log de mundo
(`"w"` `k:"portal"`), então quem entra depois vê; portais de quem saiu somem. Limites: recursão 1
nível, partículas transparentes/rótulos não aparecem na vista.

**Steve:** `1`-`9`/roda escolhem o slot; `E` ou `I` abre o inventário (criativo: grade com todos os
blocos, clica pra pôr no slot; survival: clica em dois slots pra trocar). Arco: segura `Dir` e solta
(flecha com gravidade, crava no bloco). Isqueiro: `Dir` na TNT acende (pavio de 4s piscando) ou põe
fogo que apaga sozinho e acende TNT vizinha; explosão derruba TNT perto em cadeia. TNT, fogo e
blocos vão pelos eventos de mundo (quem entra depois vê igual); flechas, dano PvP e item na mão vão
na mensagem de posição. Celular: toca no slot, `...` abre o inventário, segura `BATE` pra minerar,
segura `POE` com o arco.

Tudo original (sem código/asset de Skate 3, GTA ou Euphoria). As mecânicas do skate seguem o design documentado pelo projeto de engenharia reversa [SK8-ENGINE/skate-3-rust-engine](https://github.com/SK8-ENGINE/skate-3-rust-engine), reimplementadas do zero (aquele repo não tem licença, então nenhum código foi copiado).

### Agente IA no chat (`/comando`)

No chat, qualquer mensagem começando com `/` vai pra uma **fila** no servidor (1 pedido por jogador,
máx. 12). A IA (OpenAI) responde com operações de uma whitelist que o servidor valida e transmite
pra todos em ordem: construir caixas/esferas de blocos, explodir, banner, fogos, cor do céu, urna
furiosa, teleportar jogador, chamar Wolverine, trocar o telão. A fala da IA aparece no guardião do
escudo. Ex.: `/constroi uma piramide de neon na praca`, `/ceu roxo e fogos`, `/explode a casa do lado da torre`.
Precisa da chave: `npx wrangler secret put OPENAI_API_KEY` (modelo opcional: `OPENAI_MODEL`).

### Economia da vila (moedas FICTÍCIAS)

**As moedas são de brinquedo: sem dinheiro real, sem cripto, sem doação real, sem valor fora do jogo.**
Estado fica no Durable Object (`server/economy.js`): cofre da IA, carteira por nome de jogador
(conta nova ganha 100 — única emissão de moeda), loja, missões, 4 outdoors e um **ledger público**
(últimas 200 entradas: quem, o quê, quanto e o porquê da IA). Carteira é por nome, sem senha.

- `/saldo`, `/banco` (ajuda), `/doar n` (pro cofre), `/pagar nome n` (conta com 10+ min), `/loja`,
  `/comprar item` (fogos, ceu, faixa, estatua, raiva, wolverine), `/missao` (quebrar/construir
  blocos, acertar NPCs ou visitar um lugar; paga do cofre), `/anuncio texto n` (outdoor perto da praça;
  a IA aprova/recusa). Outros `/` continuam indo pro agente IA.
- A cada ~4 min com gente online, a IA revisa a cidade: ajusta preços, posta o porquê no ledger e
  pode bancar fogos/céu/construção com o cofre (sempre sobra 300).
- A IA só sugere números e textos; o servidor limita tudo (preço ±50% por revisão, recompensa
  proporcional e ≤ 10% do cofre, nunca move carteira sem comando do dono). Sem `OPENAI_API_KEY`
  usa preços/missões por regra fixa.
- No jogo: saldo e missão no topo, `L` (celular: `BANCO`) abre o ledger, movimentos aparecem no chat.
- `/pedido descricao`: a IA orça em moedas (mín. 20 + 1 por 100 blocos, máx. 1000); `/aceito` paga o
  cofre e executa (fila prioritária). `/votar ideia` sugere a próxima obra da IA.
- **Cérebro grátis**: sem chave, usa Workers AI (binding `AI` no `wrangler.toml`; `llama-3.3-70b` pra
  construir/moderar, `llama-3.1-8b` pro fundo). Com `OPENAI_API_KEY` usa OpenAI. Tudo passa por
  `server/brain.js`: fila com prioridade, 8 chamadas/min, ~8000 neurons/dia; se falhar, regra fixa.
- **IA viva 24h**: alarme do DO roda a cada 4 min (online) / 15 min (vazio). Ela ajusta preços, escreve
  pensamentos no ledger, atualiza o **outdoor da IA** (ao lado do clube) e constrói obras pequenas
  pagas pelo cofre (até 8000 blocos/dia, fora da praça/clube/lab/caminhos). Obras da IA ficam salvas.
  Texto público passa por filtro: nada de dinheiro real, pix, cripto, chave, senha ou link.

**Celular** (abre o link no navegador, deita o celular): metade esquerda = joystick, arrastar na
direita = olhar, botões `PULA` / `BATE` / `POE` / `VOA`, `CHAT` e `TELAO` no canto, toque na
hotbar escolhe bloco. Entra em tela cheia no primeiro toque.

---

## O que tem na tela

### Estilo Minecraft
- Mundo 320×48×320 em chunks 16×16, face culling, **ambient occlusion por vértice**,
  sombreamento por face (topo claro, laterais e base mais escuras).
- Atlas 16×16 procedural (`FilterMode::Nearest`): grama, terra, pedra, tábua, tronco,
  folha, pedregulho (Voronoi), vidro, tijolo, cascalho, lã, neon, bedrock, parede do clube.
- Terreno plano na cidade, colinas/floresta nas bordas, 10 casas com telhado escalonado na
  rua residencial. Ruas de pedra com meio-fio, postes e placas de bairro; praça com fonte e bancos.
- Sol, nuvens andando, mira, hotbar, quebrar/colocar bloco, física AABB do jogador.

### Clube de house (oeste)
- Pista de LED 16×20 piscando no beat, palco, cabine do DJ com toca-discos girando,
  caixas de som com cones pulsando no kick, globo espelhado e canhões de luz varrendo.
- DJ villager balançando a cabeça, 28 villagers dançando (pulo + braços no tempo),
  cada um com bandeira: `EU ME RENDO`, `QUE SE FODA`, `SO CURTINDO`, `TO NEM AI`, `PAZ E HOUSE`.
- **Escudo mágico**: esfera translúcida (raio 22) que **reflete os lasers** da urna
  (feixe azul refletido + faíscas + som). Explosões nunca removem blocos dentro do escudo.
- **Telão** na parede oeste atrás do palco tocando YouTube (título e status abaixo).
- **VIPs** com nome dourado: os 10 ministros do STF de toga preta dançando na frente da pista
  (Fachin, Gilmar, Cármen Lúcia, Toffoli, Fux, Moraes, Nunes Marques, Mendonça, Zanin, Dino),
  **Vorcaro** e **Lulinha** no palco ao lado do DJ (`actors::spawn_guests`).
- Volume da música aumenta conforme você se aproxima do clube.
- **Dentro do clube a guerra some**: sons de fora do escudo (explosões, laser, socos) não tocam
  e a tela não treme. A luz faz transição de dia pra balada (escuro roxo + pulso colorido no beat).
- **Guardião do escudo (eu, a IA que fez o jogo)**: figura gigante flutuando sobre o clube,
  cabeça de monitor CRT com três olhos, cabelo elétrico, auréola de código, cauda de dados e
  **seis braços** sustentando o escudo com feixes. Encara a urna; quando um laser bate no
  escudo os olhos ficam vermelhos e abre um sorriso maníaco. Solta falas rotativas.

### Laboratório (leste)
Prédio de vidro onde 5 **robôs cientistas** circulam entre estações estudando um **cérebro
humano holográfico** girando (neurônios disparando), com painéis de hologramas e leituras.

**Pesquisa de verdade (honesta):** o servidor (`server/lab.js`) busca artigos **reais já publicados**
no [Europe PMC](https://europepmc.org) (API pública, sem chave) — cérebro primeiro, depois outros
sistemas do corpo — e a IA só **resume o abstract** em pt-BR (título, achado em 1-2 linhas, revista,
ano, link DOI). Não faz experimento, não inventa resultado; sem IA disponível mostra só título +
revista. Guarda os últimos 50 achados. Ciclo a cada 30 min com fundo, 2 h sem fundo (máx. 40/dia).
- `/lab` último achado · `/pesquisa tema` põe tema na fila (30 moedas) · `/doarlab n` doa pro fundo.
- O **fundo do lab é moeda fictícia** do jogo: cada tema pesquisado gasta 25 do fundo (aparece no
  ledger). Nenhum dinheiro real passa pelo jogo. Quem quiser ajudar pesquisa de verdade: doe direto
  pra [Brain & Behavior Research Foundation](https://bbrfoundation.org/donate) ou
  [Instituto D'Or](https://www.idor.org/).
- **Painel holográfico** gigante na entrada (oeste) com estatísticas, os 3 últimos achados e os links.
- **Dra. Sinapse-9**, androide cientista guardiã: mantém uma **cúpula de energia** sobre o lab (a
  urna não entra). Dá pra derrubar (400 HP, volta em 45 s); a cúpula continua.

### Zona de mods (norte do lab) — modding ao vivo por agentes de IA
Qualquer agente (Claude, Cursor, GPT...) lê [`/modding.txt`](https://urna-mine-verine.wilianzilv.workers.dev/modding.txt),
monta um **pacote JSON declarativo** (modelo de caixas com partes/pivôs, animações por keyframe,
comportamento = primitivas da whitelist com parâmetros limitados, sons do synth, itens/blocos de paleta),
registra um token (`POST /api/mods/register`), sobe (`POST /api/mods`), versiona (`PUT /api/mods/:id/versions`)
e ativa (`POST /api/mods/:id/activate`): o mod aparece **na hora** pra todo mundo na praça ao norte do lab
(painel com os mods ativos, spawn pads, pedestais de itens). Mod é **só dado, nunca código** (schema estrito,
limites, filtro de texto + moderação IA). Entidades de mod são matáveis (vida no host, renascem) e podem
derrubar moedas fictícias. Docs: `docs/MODDING.md`, schema em `/modding.json`, exemplo
`examples/mods/king-kong.json` (King Kong fica ativo como vitrine). Código: `server/mods.js`, `src/mods.rs`.

### Game Hub (corredor ao sul do lab) — portais pra jogos da web
Quem portou um jogo pra web conecta ele como **arco-portal** pelo **Urna Portal Protocol v1**: o agente lê
[`/hub.txt`](https://urna-mine-verine.wilianzilv.workers.dev/hub.txt), registra o portal com o mesmo token do
modding (`POST /api/portals`, versionado com ativar/rollback), prova que é dono da origem servindo o challenge em
`/.well-known/urna-portal.json` (`POST /api/portals/:id/verify`) e ativa. Entrar no arco abre o jogo num **iframe
sandbox** em tela cheia com token de sessão assinado (HMAC, 10 min, `GET /api/portals/verify`) entregue por
`postMessage` com origem conferida; Esc/voltar sempre devolve pra vila. Score vira poucas moedas fictícias
(limitado), conquistas/chat caem no chat. SDK: `/sdk/urna-portal.js`; demo "Ilha do Por do Sol" em
`urna-hub-demo.wilianzilv.workers.dev` (`server/hub-demo/`). Segredo: `wrangler secret put HUB_SECRET` (sem ele, o DO
gera e guarda um). Docs `docs/HUB.md`, schema `/hub.json`. Código: `server/hub.js`, `src/hub.rs`, `web/hub.js`.

### A briga (praça central)
Todos com bandeira nas costas, nome e barra de vida flutuando, placar no topo (vida + KOs).

| Lutador | Visual | Bandeira |
| --- | --- | --- |
| Lula | camisa vermelha, cabelo e barba grisalhos | vermelha/branca `LULA` |
| Flávio Bolsonaro | camisa verde, calça azul | verde/amarela `FLAVIO` |
| Renan Santos | camiseta preta, óculos | azul/laranja `RENAN` |
| Wolverine | uniforme amarelo/azul clássico, máscara com "orelhas", 3 garras por mão | amarela/azul `SNIKT` |

Os três humanos têm **atributos idênticos** (100 HP, mesma velocidade e dano). Só o
Wolverine é diferente.

### Urna eletrônica (arena ao norte)
- Caricata: corpo bege, tela vira **rosto** (olhos seguem o alvo, sobrancelha brava, boca
  abre ao carregar), teclado numérico, `BRANCO` / `CORRIGE` / `CONFIRMA`, faixa JUSTIÇA ELEITORAL.
- **Braços e pernas procedurais com peso**: IK de dois ossos, pé plantado no terreno a cada
  passo (tremor + som de pisada), corpo em mola amortecida (balança, inclina, coice no tiro),
  luvas de boxe socando o ar e apontando pro alvo quando carrega.
- Anda pra pontos aleatórios da arena aberta (fora do escudo) destruindo tudo.
- Ciclo: escolhe alvo → gira → carrega (olho vermelho cresce) → dispara.
- Alvos: lutadores, pontos aleatórios da vila, o clube (sempre refletido pelo escudo),
  villagers andando e, depois de 30s, ocasionalmente o jogador.
- 18% de chance de **rajada** (5 tiros rápidos). 15% dos tiros são `CONFIRMA!!!` (raio 5.5).
- Explosão: cratera esférica com borda irregular, detritos com a cor do bloco, bola de fogo,
  tremor de câmera proporcional à distância, knockback em lutadores/villagers/jogador.

### Bolsonaro Voador (céu da vila, `src/voador.rs`)
- Homenagem ao jogo mobile de 2016: o Jair de um universo paralelo que **voa e atira laser
  pelos olhos**. Terno, faixa presidencial verde-amarela, cabelo grisalho repartido, capa.
- Rota em função do tempo sincronizado (curvas inclinadas, mergulho a cada 26s); o host escolhe
  alvos (jogadores, lutadores, villagers, a urna) e os tiros quebram bloco como os da urna.
- Falas satíricas ("TALKEY?", "ESSA URNA AI NAO E AUDITAVEL!"). Matável: 900 HP, barra de
  chefão por perto, despenca e volta em 60s (+$2500 pro bandido que derrubar).

### GODZILHA (kaiju rival da urna, `src/kaiju.rs`)
- Paródia blocky do tamanho da urna: pisa (tremor), ruge, dá rabada e solta o **bafo atômico azul**
  (placas acendem do rabo pra cabeça) que quebra bloco (`shot` com `by: 9`). Caça a urna (ela revida),
  às vezes jogadores/NPCs/voador; não entra no escudo do clube nem no domo do lab. 1500 HP, volta em 90s (+$4000).

- **URNA AIRSHIP** (`src/zeppelin.rs`): zepelim rígido de 72 blocos em volta lenta no céu (relógio compartilhado), bombardeia em fileira, metralha quem voa perto (`by: 10`, escudos seguram); 3000 HP, pega fogo por seções, cai e explode, volta em 3 min (+$6000).

---

## Sistema de combate

Implementado em `src/actors.rs` (`update_fighters`).

### Base (todos)
- **IA de alvo**: escolhe o oponente mais próximo com ruído aleatório, reavalia a cada 2–4.5s.
- **Movimento**: anda até o alcance, circula o oponente (strafe) enquanto espera o cooldown,
  separação para não sobrepor.
- **Combo encadeado** com janela de 1.2s: `Jab → Cross → Finisher` (uppercut com
  lançamento pra cima). Cada golpe tem duração, frame ativo, multiplicador de dano,
  knockback horizontal/vertical e hitstun próprios.
- **Hit confirm** só no frame ativo, exige alcance (2.1) e estar de frente pro alvo.
- **Hitstun** interrompe o ataque do oponente.
- **Esquiva**: ao perceber um golpe começando, chance de sidestep invulnerável (0.28s).
- **Defesa**: se parado e de frente, chance de bloquear (25% do dano, 30% do knockback).
- **Especial "VOADORA"**: a 3–8 blocos, pulo com investida; acerta em contato.
- **KO**: cai pra trás, volta após 5s no ponto inicial. KOs contam no placar.

### Wolverine
| Mecânica | Valor |
| --- | --- |
| HP | 160 |
| Velocidade | 5.2 (humanos 3.4) |
| Velocidade de ataque | ×1.35 |
| Combo | 4 golpes: garra direita → garra esquerda → **golpe duplo em X** → uppercut |
| Fator de cura | +3 HP/s sempre, +13 HP/s se 2.5s sem apanhar |
| Berserker | abaixo de 35% HP: 6s com velocidade/ataque ×1.5 e dano ×1.4, tinta vermelha, cooldown 18s |
| Bote | salto mais longo (14 de impulso) + **dano em área ao aterrissar** |
| Esquiva | 40% (humanos 12%) |
| Defesa | 25% (humanos 15%) |
| KO | volta em 3.5s (humanos 5s) |
| Entrada | cai do céu na praça, onda de choque empurra quem estiver perto |
| Urna | toma 60% do dano de explosão |

---

## Áudio (`src/audio.rs` + `src/synth.rs`)

- Mixer próprio em **cpal**: vozes (efeitos/loop) + buffer de streaming do telão.
- **House loop** (fallback) 8 compassos, 124 BPM: kick 4×4 com pitch envelope, clap no 2 e 4
  (3 micro-rajadas + cauda), hi-hat fechado em semicolcheias e aberto no contratempo,
  baixo no contratempo saturado, stabs de acorde em serra desafinada com filtro passa-baixa
  envelopado (progressão Am7 – Fmaj7 – Dm7 – Em7), pad suave e **sidechain** do kick.
- Efeitos: explosão, laser (sweep), soco, garra (metal), SNIKT, deflexão do escudo.
  Volume atenuado pela distância até o jogador.

---

## Arquitetura

```
src/
  main.rs     loop principal, input, câmera, áudio, render 3D/2D, HUD, labels
  world.rs    voxels, geração (terreno, casas, clube, torre, árvores), meshing com AO, raycast DDA
  atlas.rs    atlas de texturas procedural + cor média por tile
  batch.rs    batch de cubos transformados na CPU (1 draw call para todos os personagens)
  models.rs   humanoide (Steve), villager, bandeira animada
  actors.rs   villagers (dança/andar/arremesso) e lutadores (IA + combate)
  urna.rs     urna (modelo + IA de tiro), laser, escudo, explosão, partículas
  club.rs     decoração animada do clube no beat
  player.rs   jogador 1ª pessoa (AABB, voo, knockback)
  steve.rs    Steve survival/criativo: vida, mineração, arco, TNT, fogo, PvP
  items.rs    itens (blocos + ferramentas), tempo de quebra, ícones pixel-art
  inventory.rs hotbar e tela do inventário (E/I)
  synth.rs    síntese do house e dos efeitos (amostras f32)
  audio.rs    mixer cpal (vozes + stream do telão, relógio de áudio)
  telao.rs    player YouTube do telão (yt-dlp + ffmpeg → textura + áudio)
  net.rs      WebSocket nativo (tungstenite numa thread)
  mp.rs       protocolo multiplayer (snapshots do host, tiros, eventos)
  eleicao.rs  contagem regressiva gigante + evento da abertura das urnas
  *_web.rs    versões navegador de audio/telao/net; web.rs = ponte com web/urna.js
web/          site (index.html, urna.js, mq_js_bundle.js, urna.wasm)
server/       worker.js: Cloudflare Worker + Durable Object (sala multiplayer)
tools/
  post_thread.py  posta os clipes em thread no X (`uv run tools\post_thread.py --reply-to <ID>`, chaves no `.env`)
```

### Fluxo por frame
1. Input (mouse capturado, teclado, hotbar).
2. `Player::update` → raycast de seleção → quebrar/colocar/socar.
3. `update_villagers`, `update_fighters` (gera eventos `Ev`: hit, texto, tremor, banner).
4. `Urna::update` → `urna::fire` → refletido no escudo **ou** `explode` (remove voxels,
   marca chunks sujos) → `blast_fighters` / `blast_villagers` / knockback no jogador.
5. Eventos → sons posicionais + textos flutuantes.
6. Remesh de até 8 chunks sujos por frame.
7. Render: chunks → batch opaco (NPCs, urna, clube, partículas) → wireframe de seleção →
   batch transparente (luzes, lasers) → bolas de fogo → escudo → 2D (labels, HUD).

### Decisões técnicas
- **macroquad 0.4** (sem feature `audio`, conflita com cpal): janela, 3D imediato, compila rápido.
  Capacidade de draw call aumentada (`60000` vértices / `100000` índices) via `conf::Conf`.
- Meshes de chunk divididos em blocos de ≤16000 vértices (índices `u16`).
- Personagens são cubos transformados na CPU (`Mat4`) em um único buffer, em vez de
  `push_model_matrix` por membro (que quebraria o batching em centenas de draw calls).
- `profile.dev` com `opt-level=1` e dependências em `opt-level=3` (debug jogável).

### Coordenadas
- `Y` pra cima, chão da vila em `y = 20` (`world::G`).
- Todas as posições de marcos ficam em `src/layout.rs` (espelho no servidor: `server/layout.js`).
- Praça: centro `(160, 160)`, raio 24, anel viário r62. Spawn `(160.5, 21, 178.5)`.
  Clube ~`(60, 160)`, lab `(253, 160)` (mods ao N, hub ao S), arena `(160, 58)`,
  torre `(160, 265)` + avenida GTA, skate `(228..272, 244..280)`.

```
            ARENA (urna x kaiju)
                    |
  CLUBE ---- ( PRAÇA + anel ) ---- CIÊNCIA (mods/lab/hub)
                    |
   CASAS ------ TORRE + avenida ---- SKATE
        floresta/colinas nas bordas
```
- Yaw dos modelos: frente local `+Z`, `yaw = atan2(dir.x, dir.z)`.

---

## Ajustes rápidos

| O quê | Onde |
| --- | --- |
| BPM / progressão / mix do house | `synth.rs` (`BPM`, `chords`, `roots`, `mix`) |
| Frequência e alvos da urna | `Urna::update`, closure de alvo em `main.rs` |
| Raio da explosão | `urna::fire` |
| Raio do escudo | `world::SHIELD_R` |
| Atributos dos lutadores | `Fighter::new`, `Atk::*` em `actors.rs` |
| Frases das bandeiras | `actors::FLAG_TEXTS` |
| Quantidade de dançarinos | `spawn_villagers` |
| Tempo até o Wolverine | `time > 20.0` em `main.rs` |
