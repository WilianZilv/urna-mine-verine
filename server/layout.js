// Planta da cidade (espelho de src/layout.rs). Muda aqui = muda la; WORLD_VERSION sobe quando as
// coordenadas mudam (o servidor descarta o log de obras salvo com a planta antiga).
export const WORLD_VERSION = 2;
export const WORLD = 320;
export const G = 20;

export const PLAZA = [160, 160];
export const PLAZA_R = 24;
export const RING_R = 62;
export const ARENA = { x0: 112, x1: 208, z0: 24, z1: 92 };
// Placar holografico da arena (src/arena_panel.rs): pilares em x 108..110, z 45..71.
export const ARENA_BOARD = { x0: 107, x1: 111, z0: 44, z1: 72 };
export const CLUB = { x0: 46, x1: 74, z0: 144, z1: 176, shield: [60, 22, 160], r: 22 };
export const LAB = { x0: 244, x1: 262, z0: 148, z1: 172, dome: [253.5, G, 160.5], r: 16, panelX: 236 };
export const MODZ = { x0: 245, x1: 261, z0: 111, z1: 127 };
export const HUB = { x0: 241, x1: 271, z0: 191, z1: 205, dome: [256.5, G, 198.5], r: [17, 11, 10] };
export const TOWER = [160, 30, 265];
export const SKATE = { x0: 228, x1: 272, z0: 244, z1: 280 };
// Lugares funcionais (src/places/*.rs + server/places.js): Congresso e Bolsa na avenida dos poderes (norte),
// TV ao sul das casas, Banco Central e Terminal na avenida GTA.
export const CONGRESSO = { x0: 50, x1: 86, z0: 80, z1: 116 };
export const BOLSA = { x0: 234, x1: 270, z0: 70, z1: 102 };
export const TV = { x0: 44, x1: 84, z0: 266, z1: 292 };
export const BANCO = { x0: 184, x1: 214, z0: 222, z1: 252 };
export const TERMINAL = { x0: 184, x1: 220, z0: 266, z1: 294 };
// Escola de Agentes (src/places/escola.rs LOT): sul das casas; trilha x 114..116 da rua das casas ate a porta norte.
export const ESCOLA = { x0: 100, x1: 130, z0: 266, z1: 292 };
const ESCOLA_PATH = [114, 116, 243, 267];
// Ringue da Vila (src/places/ringue.rs LOT/LONA/PATH): noroeste, acima do Congresso; lona [x0, z0, x1, z1] elevada 2 blocos;
// trilha [x0, x1, z0, z1] sobe da Avenida dos Poderes. Lote inteiro e escudo (shielded): ninguem quebra nem poe bloco.
export const RINGUE = { x0: 56, x1: 92, z0: 38, z1: 70, lona: [68, 48, 79, 59], path: [90, 92, 71, 95] };
const NEW_PLACES = [CONGRESSO, BOLSA, TV, BANCO, TERMINAL, ESCOLA, RINGUE];
// [x0, z0, x1, z1] (igual layout::ROADS / PATHS)
export const ROADS = [[158, 92, 162, 137], [158, 183, 162, 263], [75, 158, 137, 162], [183, 158, 243, 162], [44, 238, 157, 242], [163, 257, 227, 261], [87, 96, 233, 100]];
export const PATHS = [[239, 119, 241, 157], [242, 118, 245, 120], [241, 163, 242, 190], [61, 243, 63, 265], [163, 236, 183, 238], [199, 262, 201, 265]];
export const HOUSES = [[50, 228], [68, 228], [86, 228], [104, 228], [122, 228], [50, 246], [68, 246], [86, 246], [104, 246], [122, 246], [166, 250]];
export const PLACAR = [120, 131, 200, 133];
export const BILLBOARDS = [[136, 136], [184, 136], [136, 184], [184, 184], [112, 150]];

// Missao "visitar"
export const SPOTS = { praca: PLAZA, clube: [60, 160], lab: [253, 160], torre: [160, 265], arena: [160, 58], skate: [250, 262], casas: [100, 240], congresso: [78, 98], bolsa: [244, 86], tv: [64, 262], banco: [190, 237], terminal: [202, 280], escola: [115, 279] };

// [x0, x1, z0, z1]: nenhuma obra da IA (pedido de jogador ou autonoma) cobre clube, lab+domo+painel, zona de mods, hub ou placar da arena.
export const LANDMARKS = [
    [CLUB.x0 - 2, CLUB.x1 + 2, CLUB.z0 - 2, CLUB.z1 + 2],
    [LAB.panelX - 1, LAB.x1 + 8, LAB.z0 - 4, LAB.z1 + 5],
    [MODZ.x0 - 2, MODZ.x1 + 2, MODZ.z0 - 2, MODZ.z1 + 2],
    [HUB.x0 - 2, HUB.x1 + 2, HUB.z0 - 3, HUB.z1 + 4],
    [ARENA_BOARD.x0, ARENA_BOARD.x1, ARENA_BOARD.z0, ARENA_BOARD.z1],
    ...NEW_PLACES.map((p) => [p.x0 - 2, p.x1 + 2, p.z0 - 2, p.z1 + 2]),
];

// Onde a IA nao constroi sozinha: marcos + ruas, trilhas, casas, torre, skate, arena, placar, outdoors.
export const PROTECTED = [
    ...LANDMARKS,
    ...[...ROADS, ...PATHS].map(([x0, z0, x1, z1]) => [x0 - 1, x1 + 1, z0 - 1, z1 + 1]),
    [ESCOLA_PATH[0] - 1, ESCOLA_PATH[1] + 1, ESCOLA_PATH[2] - 1, ESCOLA_PATH[3] + 1],
    [RINGUE.path[0] - 1, RINGUE.path[1] + 1, RINGUE.path[2] - 1, RINGUE.path[3] + 1],
    ...HOUSES.map(([x, z]) => [x - 1, x + 7, z - 1, z + 7]),
    [156, 163, 262, 269],
    [SKATE.x0 - 2, SKATE.x1 + 2, SKATE.z0 - 2, SKATE.z1 + 2],
    [ARENA.x0, ARENA.x1, ARENA.z0, ARENA.z1],
    [PLACAR[0], PLACAR[2], PLACAR[1], PLACAR[3]],
    ...BILLBOARDS.map(([x, z]) => [x - 4, x + 4, z - 4, z + 4]),
];

const overlaps = (lo, hi, list) => list.some(([x0, x1, z0, z1]) => lo[0] <= x1 && hi[0] >= x0 && lo[2] <= z1 && hi[2] >= z0);
export const onLandmarkBox = (lo, hi) => overlaps(lo, hi, LANDMARKS);

// Caixa [lo, hi] invade praca (raio 30), anel ou area protegida?
export function blocked(lo, hi) {
    const dx = Math.max(lo[0] - PLAZA[0], 0, PLAZA[0] - hi[0]), dz = Math.max(lo[2] - PLAZA[1], 0, PLAZA[1] - hi[2]);
    const near = Math.hypot(dx, dz);
    const far = Math.hypot(Math.max(Math.abs(lo[0] - PLAZA[0]), Math.abs(hi[0] - PLAZA[0])), Math.max(Math.abs(lo[2] - PLAZA[1]), Math.abs(hi[2] - PLAZA[1])));
    if (near < PLAZA_R + 6 || (near < RING_R + 3 && far > RING_R - 3)) return true;
    return overlaps(lo, hi, PROTECTED);
}

// Escudos (igual src/shield.rs): domo do lab, domo do hub + piso do corredor.
export function shielded(p) {
    if (!Array.isArray(p) || p.length < 3) return false;
    const [x, y, z] = p.map(Number);
    if (x >= HUB.x0 && x <= HUB.x1 && z >= HUB.z0 && z <= HUB.z1) return true;
    if (x >= RINGUE.x0 && x <= RINGUE.x1 && z >= RINGUE.z0 && z <= RINGUE.z1) return true;
    const domes = [[LAB.dome, [LAB.r, LAB.r, LAB.r]], [HUB.dome, HUB.r]];
    return domes.some(([c, r]) => ((x + 0.5 - c[0]) / r[0]) ** 2 + (Math.max(0, y + 0.5 - c[1]) / r[1]) ** 2 + ((z + 0.5 - c[2]) / r[2]) ** 2 < 1);
}

// Texto pro prompt das IAs.
export const PLACES =
    `MUNDO: voxels ${WORLD}x48x${WORLD}. x e z de 0 a ${WORLD - 1}, y de 0 a 47. O chao e y=${G} (primeira camada de ar; blocos em y<${G} sao terreno). ` +
    `Ruas de cascalho com meio-fio ligam tudo; morros com floresta nas bordas. LUGARES: ` +
    `PRACA central (${PLAZA[0]},${G},${PLAZA[1]}) raio ${PLAZA_R} com fonte, bancos, a urna eletronica gigante e o placar das eleicoes no lado norte; anel de rua raio ${RING_R} em volta. ` +
    `ARENA DOS GIGANTES ao norte (x ${ARENA.x0}..${ARENA.x1}, z ${ARENA.z0}..${ARENA.z1}): Lula, Flavio Bolsonaro, Renan Santos, Wolverine, urna x GODZILHA. ` +
    `CLUBE de house a oeste (x ${CLUB.x0}..${CLUB.x1}, z ${CLUB.z0}..${CLUB.z1}) com escudo (centro ${CLUB.shield.join(",")} raio ${CLUB.r}; explosoes nao afetam dentro). ` +
    `DISTRITO DA CIENCIA a leste: laboratorio (x ${LAB.x0}..${LAB.x1}, z ${LAB.z0}..${LAB.z1}) com domo, zona de mods ao norte (x ${MODZ.x0}..${MODZ.x1}, z ${MODZ.z0}..${MODZ.z1}), game hub ao sul (x ${HUB.x0}..${HUB.x1}, z ${HUB.z0}..${HUB.z1}). ` +
    `AVENIDA DOS PODERES (z 96..100, x 87..233) passa no sul da arena: CONGRESSO DA VILA na ponta oeste (x ${CONGRESSO.x0}..${CONGRESSO.x1}, z ${CONGRESSO.z0}..${CONGRESSO.z1}, leis votadas que mudam o jogo) e BOLSA DE VALORES na ponta leste (x ${BOLSA.x0}..${BOLSA.x1}, z ${BOLSA.z0}..${BOLSA.z1}). ` +
    `TV URNA NEWS ao sul das casas (x ${TV.x0}..${TV.x1}, z ${TV.z0}..${TV.z1}). BANCO CENTRAL (x ${BANCO.x0}..${BANCO.x1}, z ${BANCO.z0}..${BANCO.z1}) e TERMINAL INTERDIMENSIONAL (x ${TERMINAL.x0}..${TERMINAL.x1}, z ${TERMINAL.z0}..${TERMINAL.z1}) na avenida GTA. ` +
    `ESCOLA DE AGENTES ao sul das casas (x ${ESCOLA.x0}..${ESCOLA.x1}, z ${ESCOLA.z0}..${ESCOLA.z1}; trilha x ${ESCOLA_PATH[0]}..${ESCOLA_PATH[1]} vem da rua das casas), onde um robo professor ensina a criar mods colando /skill.md no agente. ` +
    `RINGUE DA VILA a noroeste, acima do Congresso (x ${RINGUE.x0}..${RINGUE.x1}, z ${RINGUE.z0}..${RINGUE.z1}; trilha x ${RINGUE.path[0]}..${RINGUE.path[1]} sobe da avenida dos poderes): boxe PvP entre jogadores, melhor de 3, /ringue entra na fila; lote protegido. ` +
    `CASAS a sudoeste (rua z 238..242, x 44..157). TORRE ao sul (${TOWER.join(",")}) na avenida GTA (z 257..261). SKATE PARK a sudeste (x ${SKATE.x0}..${SKATE.x1}, z ${SKATE.z0}..${SKATE.z1}). Jogadores nascem no sul da praca.`;
