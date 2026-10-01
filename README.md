# URNA-MINE-VERINE

Clone de Minecraft em Rust. Vila voxel com villagers, um **clube de house** tocando
(com escudo mágico), uma **briga generalizada** no meio da praça (Lula, Flávio Bolsonaro,
Renan Santos e Wolverine, todos com bandeira) e uma **urna eletrônica gigante** flutuando,
soltando laser e explodindo tudo que não está protegido pelo escudo.

Zero assets externos: texturas, modelos e efeitos são gerados em código na inicialização.
A música vem do **telão do clube**, que toca YouTube via `yt-dlp` + `ffmpeg`.

---

## Rodar (passo a passo)

1. Rust instalado (`rustup`, testado com rustc 1.96).
2. `ffmpeg` no PATH e `yt-dlp` (`uv tool install yt-dlp`; sem ele o jogo tenta `uvx yt-dlp`).
3. No diretório do projeto:

```powershell
cargo run --release
```

4. Clique na janela para capturar o mouse. Você nasce no topo da torre de observação
   (sul da vila): **clube à esquerda**, **briga no centro**, **urna à direita**.
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
| `1`-`9` / roda | escolher bloco da hotbar |
| `K` | chamar Wolverine agora |
| `Y` | tocar no telão o link do YouTube copiado (navegador: cola numa caixa) |
| `T` / `Enter` | chat (Enter envia, Esc cancela) |
| `R` | resetar mundo (regenera vila, NPCs) |
| `M` | mutar |
| `H` | mostrar/esconder ajuda |
| `Tab` / `Esc` | soltar mouse |

**Celular** (abre o link no navegador, deita o celular): metade esquerda = joystick, arrastar na
direita = olhar, botões `PULA` / `BATE` / `POE` / `VOA`, `CHAT` e `TELAO` no canto, toque na
hotbar escolhe bloco. Entra em tela cheia no primeiro toque.

---

## O que tem na tela

### Estilo Minecraft
- Mundo 128×48×128 em chunks 16×16, face culling, **ambient occlusion por vértice**,
  sombreamento por face (topo claro, laterais e base mais escuras).
- Atlas 16×16 procedural (`FilterMode::Nearest`): grama, terra, pedra, tábua, tronco,
  folha, pedregulho (Voronoi), vidro, tijolo, cascalho, lã, neon, bedrock, parede do clube.
- Terreno plano na vila, colinas com árvores em volta, 8 casas com telhado escalonado,
  janelas de vidro e porta virada pra praça. Caminhos de cascalho e praça de pedregulho.
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

### Urna eletrônica (anda pelo mapa inteiro)
- Caricata: corpo bege, tela vira **rosto** (olhos seguem o alvo, sobrancelha brava, boca
  abre ao carregar), teclado numérico, `BRANCO` / `CORRIGE` / `CONFIRMA`, faixa JUSTIÇA ELEITORAL.
- **Braços e pernas procedurais com peso**: IK de dois ossos, pé plantado no terreno a cada
  passo (tremor + som de pisada), corpo em mola amortecida (balança, inclina, coice no tiro),
  luvas de boxe socando o ar e apontando pro alvo quando carrega.
- Anda pra pontos aleatórios do mapa (fora do escudo) destruindo tudo.
- Ciclo: escolhe alvo → gira → carrega (olho vermelho cresce) → dispara.
- Alvos: lutadores, pontos aleatórios da vila, o clube (sempre refletido pelo escudo),
  villagers andando e, depois de 30s, ocasionalmente o jogador.
- 18% de chance de **rajada** (5 tiros rápidos). 15% dos tiros são `CONFIRMA!!!` (raio 5.5).
- Explosão: cratera esférica com borda irregular, detritos com a cor do bloco, bola de fogo,
  tremor de câmera proporcional à distância, knockback em lutadores/villagers/jogador.

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
- Clube: `x 8..36`, `z 48..80`. Praça: centro `(64, 64)`, raio 13.
  Urna: `(108, 28, 64.5)`. Torre/spawn: `(64, 30, 105.5)`.
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
