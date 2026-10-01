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
// [x0, z0, x1, z1] (igual layout::ROADS / PATHS)
export const ROADS = [[158, 92, 162, 137], [158, 183, 162, 263], [75, 158, 137, 162], [183, 158, 243, 162], [44, 238, 157, 242], [163, 257, 227, 261]];
export const PATHS = [[239, 119, 241, 157], [242, 118, 245, 120], [241, 163, 242, 190]];
export const HOUSES = [[50, 228], [68, 228], [86, 228], [104, 228], [122, 228], [50, 246], [68, 246], [86, 246], [104, 246], [122, 246], [166, 250]];
export const PLACAR = [120, 131, 200, 133];
export const BILLBOARDS = [[136, 136], [184, 136], [136, 184], [184, 184], [112, 150]];

// Missao "visitar"
export const SPOTS = { praca: PLAZA, clube: [60, 160], lab: [253, 160], torre: [160, 265], arena: [160, 58], skate: [250, 262], casas: [100, 240] };

// [x0, x1, z0, z1]: nenhuma obra da IA (pedido de jogador ou autonoma) cobre clube, lab+domo+painel, zona de mods, hub ou placar da arena.
export const LANDMARKS = [
    [CLUB.x0 - 2, CLUB.x1 + 2, CLUB.z0 - 2, CLUB.z1 + 2],
    [LAB.panelX - 1, LAB.x1 + 8, LAB.z0 - 4, LAB.z1 + 5],
    [MODZ.x0 - 2, MODZ.x1 + 2, MODZ.z0 - 2, MODZ.z1 + 2],
    [HUB.x0 - 2, HUB.x1 + 2, HUB.z0 - 3, HUB.z1 + 4],
    [ARENA_BOARD.x0, ARENA_BOARD.x1, ARENA_BOARD.z0, ARENA_BOARD.z1],
];

// Onde a IA nao constroi sozinha: marcos + ruas, trilhas, casas, torre, skate, arena, placar, outdoors.
export const PROTECTED = [
    ...LANDMARKS,
    ...[...ROADS, ...PATHS].map(([x0, z0, x1, z1]) => [x0 - 1, x1 + 1, z0 - 1, z1 + 1]),
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
    `CASAS a sudoeste (rua z 238..242, x 44..157). TORRE ao sul (${TOWER.join(",")}) na avenida GTA (z 257..261). SKATE PARK a sudeste (x ${SKATE.x0}..${SKATE.x1}, z ${SKATE.z0}..${SKATE.z1}). Jogadores nascem no sul da praca.`;
