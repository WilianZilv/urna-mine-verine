//! Planta da cidade: todo marco do mapa sai daqui (o servidor espelha em server/layout.js).
//! Os prédios antigos (clube, lab, hub, zona de mods, torre) mantêm o desenho interno do mapa 128x128;
//! cada distrito só é transladado pelo seu delta (`*_D`). Coisas novas usam coordenada absoluta.
//!
//! ```text
//!   z=0 ┌──────────────────────── 320 ────────────────────────┐
//!       │ floresta/morros          ARENA (160,58)    floresta  │
//!       │                              │ (rua N)               │
//!       │                 ╭──── anel r62 ────╮      MODS       │
//!  CLUB ├────(rua O)──────┤   PRAÇA (160,160) ├──(rua L)── LAB │
//!       │                 ╰────────┬─────────╯      GAME HUB   │
//!       │  CASAS ═══(rua das casas)╡ (rua S)                   │
//!       │                          ╞═══ avenida ═══ SKATE      │
//!       │                        TORRE                         │
//!       └──────────────────────────────────────────────────────┘
//! ```

use crate::world::G;
use macroquad::prelude::*;

/// Lado do mapa (x e z).
pub const SIZE: i32 = 320;

const fn shift(v: Vec3, d: IVec2) -> Vec3 {
    vec3(v.x + d.x as f32, v.y, v.z + d.y as f32)
}

// ---------------------------------------------------------------- Praça central

pub const PLAZA: Vec2 = vec2(160.0, 160.0);
pub const PLAZA_R: f32 = 24.0;
/// Delta das coisas que ficavam em volta da praça antiga (64,64): placar, outdoors.
pub const PLAZA_D: IVec2 = ivec2(96, 96);
pub const fn plaza(v: Vec3) -> Vec3 {
    shift(v, PLAZA_D)
}
pub fn plaza_center() -> Vec3 {
    vec3(PLAZA.x, G as f32, PLAZA.y)
}
/// Spawn dos jogadores: lado sul da praça, olhando pro norte (fonte, urna, placar).
pub const SPAWN: Vec3 = vec3(160.5, G as f32 + 1.0, 178.5);
/// Urna gigante nasce no norte da praça e anda pra arena.
pub const URNA_SPAWN: Vec3 = vec3(160.0, G as f32, 146.0);
/// Outdoors da economia (cantos entre as ruas, virados pra praça) e o da IA (rua oeste).
pub const BILLBOARDS: [(f32, f32); 4] = [(136.0, 136.0), (184.0, 136.0), (136.0, 184.0), (184.0, 184.0)];
pub const AI_BOARD: (f32, f32) = (112.0, 150.0);

// ---------------------------------------------------------------- Distritos

/// Clube de house (oeste). Escudo/pista/DJ saem dos consts do world (já transladados).
pub const CLUB_D: IVec2 = ivec2(38, 96);
pub const fn club(v: Vec3) -> Vec3 {
    shift(v, CLUB_D)
}
/// Distrito da ciência (leste): lab + painel holográfico; zona de mods ao norte; game hub ao sul.
pub const LAB_D: IVec2 = ivec2(150, 96);
pub const MODZ_D: IVec2 = ivec2(150, 80);
pub const HUB_D: IVec2 = ivec2(150, 110);
pub const fn lab(v: Vec3) -> Vec3 {
    shift(v, LAB_D)
}
pub const fn modz(v: Vec3) -> Vec3 {
    shift(v, MODZ_D)
}
pub const fn hub(v: Vec3) -> Vec3 {
    shift(v, HUB_D)
}
/// Torre de observação + casa do carro (sul, fim da rua S).
pub const TOWER_D: IVec2 = ivec2(96, 160);
pub const fn tower(v: Vec3) -> Vec3 {
    shift(v, TOWER_D)
}
/// Sedã da missão GTA: parado na avenida, ao lado da torre.
pub const CAR_SPAWN: (f32, f32) = (166.5, 259.5);
/// Casas da rua residencial: (x0, z0, porta no lado sul). Porta sempre virada pra rua.
pub const HOUSES: [(i32, i32, bool); 10] = [
    (50, 228, true),
    (68, 228, true),
    (86, 228, true),
    (104, 228, true),
    (122, 228, true),
    (50, 246, false),
    (68, 246, false),
    (86, 246, false),
    (104, 246, false),
    (122, 246, false),
];
/// Casa ao lado da torre (porta pro norte; a parede sul é alvo dos testes de portal).
pub const TOWER_HOUSE: (i32, i32, bool) = (166, 250, false);
/// Arena aberta (norte) pra briga dos gigantes: urna x GODZILHA, lutadores, Wolverine.
pub const ARENA: Vec2 = vec2(160.0, 58.0);
pub const ARENA_HALF: Vec2 = vec2(48.0, 34.0);
pub fn arena_center() -> Vec3 {
    vec3(ARENA.x, G as f32, ARENA.y)
}
pub fn in_arena(p: Vec2, margin: f32) -> bool {
    (p - ARENA).abs().cmple(ARENA_HALF - Vec2::splat(margin)).all()
}
pub fn arena_point(margin: f32) -> Vec2 {
    let h = ARENA_HALF - Vec2::splat(margin);
    ARENA + vec2(macroquad::rand::gen_range(-h.x, h.x), macroquad::rand::gen_range(-h.y, h.y))
}
/// Pista de skate (sudeste): (x0, z0, x1, z1).
pub const SKATE: (i32, i32, i32, i32) = (228, 244, 272, 280);

// ---------------------------------------------------------------- Lugares funcionais (src/places/*)

/// Retângulo de um lugar (x0, z0, x1, z1), inclusivo. Cada módulo em src/places/ constrói dentro do seu.
pub type Rect = (i32, i32, i32, i32);
/// Congresso da Vila: ponta oeste da Avenida dos Poderes; entrada leste (x 86, z 96..100).
pub const CONGRESSO: Rect = (50, 80, 86, 116);
/// Bolsa de Valores: ponta leste da Avenida dos Poderes; entrada oeste (x 234, z 96..100).
pub const BOLSA: Rect = (234, 70, 270, 102);
/// TV URNA NEWS: sul das casas; entrada norte (z 266, x 60..64), trilha vem da rua das casas.
pub const TV: Rect = (44, 266, 84, 292);
/// Banco Central: leste da rua S, norte da avenida GTA; entrada oeste (x 184, z 235..239).
pub const BANCO: Rect = (184, 222, 214, 252);
/// Terminal Interdimensional: sul da avenida GTA; entrada norte (z 266, x 198..202).
pub const TERMINAL: Rect = (184, 266, 220, 294);

// ---------------------------------------------------------------- Ruas

/// Ruas retas (x0, z0, x1, z1): miolo de cascalho, meio-fio de pedra, postes de luz.
pub const ROADS: [(i32, i32, i32, i32); 7] = [
    (158, 92, 162, 137),  // N: praça -> arena
    (158, 183, 162, 263), // S: praça -> torre
    (75, 158, 137, 162),  // O: praça -> clube
    (183, 158, 243, 162), // L: praça -> painel -> entrada do lab
    (44, 238, 157, 242),  // rua das casas
    (163, 257, 227, 261), // avenida GTA: rua S -> skate
    (87, 96, 233, 100),   // Avenida dos Poderes: Congresso <- topo do anel -> Bolsa
];
pub const RING_R: f32 = 62.0;
/// Trilhas de cascalho sem meio-fio: rua L -> zona de mods; casas -> TV; rua S -> banco; avenida -> terminal.
/// (A trilha do hub é do hub.rs.)
pub const PATHS: [(i32, i32, i32, i32); 5] = [(239, 119, 241, 157), (242, 118, 245, 120), (61, 243, 63, 265), (163, 236, 183, 238), (199, 262, 201, 265)];
/// Trilha do hub sai da rua L (z) até o corredor.
pub const HUB_PATH_Z0: i32 = 163;

/// Placas de rua (posição no chão, texto).
pub const SIGNS: [(f32, f32, &str); 13] = [
    (155.5, 102.5, "< CONGRESSO DA VILA"),
    (165.5, 102.5, "BOLSA DE VALORES >"),
    (59.5, 244.5, "v TV URNA NEWS"),
    (164.5, 234.5, "BANCO CENTRAL >   v TERMINAL"),
    (164.5, 134.5, "^ ARENA DOS GIGANTES"),
    (164.5, 186.5, "v TORRE / CASAS / AVENIDA"),
    (134.5, 155.5, "< CLUB DO HOUSE"),
    (186.5, 155.5, "DISTRITO DA CIENCIA >"),
    (236.5, 155.5, "^ ZONA DE MODS   v GAME HUB"),
    (154.5, 236.5, "< RUA DAS CASAS"),
    (164.5, 255.5, "AVENIDA GTA > SKATE"),
    (224.5, 255.5, "SKATE PARK"),
    (164.5, 95.5, "ARENA - AQUI OS GIGANTES BRIGAM"),
];

/// Retângulos ocupados (x0, z0, x1, z1): sem árvore nem poste.
pub const FOOTPRINTS: [(i32, i32, i32, i32); 15] = [
    (CONGRESSO.0 - 2, CONGRESSO.1 - 2, CONGRESSO.2 + 2, CONGRESSO.3 + 2),
    (BOLSA.0 - 2, BOLSA.1 - 2, BOLSA.2 + 2, BOLSA.3 + 2),
    (TV.0 - 2, TV.1 - 2, TV.2 + 2, TV.3 + 2),
    (BANCO.0 - 2, BANCO.1 - 2, BANCO.2 + 2, BANCO.3 + 2),
    (TERMINAL.0 - 2, TERMINAL.1 - 2, TERMINAL.2 + 2, TERMINAL.3 + 2),
    (40, 138, 86, 182),   // clube + escudo
    (226, 104, 284, 214), // lab, domo, painel, mods, hub
    (40, 222, 134, 258),  // casas
    (150, 246, 178, 272), // torre + casa
    (224, 240, 276, 284), // skate
    (106, 18, 214, 98),   // arena
    (118, 128, 202, 135), // placar
    (128, 128, 192, 192), // praça + outdoors
    (104, 144, 120, 156), // outdoor da IA
    (130, 230, 150, 236), // folga
];

/// Fora de rua/trilha/praça/distrito, com folga `m` blocos.
pub fn free(x: i32, z: i32, m: i32) -> bool {
    let inr = |&(x0, z0, x1, z1): &(i32, i32, i32, i32)| x >= x0 - m && x <= x1 + m && z >= z0 - m && z <= z1 + m;
    let d = vec2(x as f32 + 0.5, z as f32 + 0.5).distance(PLAZA);
    !(ROADS.iter().any(inr) || PATHS.iter().any(inr) || FOOTPRINTS.iter().any(inr) || (d - RING_R).abs() < 2.5 + m as f32)
}

/// Placas com nome dos distritos (texto projetado).
pub fn labels(out: &mut Vec<crate::extras::Label>, eye: Vec3) {
    for &(x, z, t) in &SIGNS {
        let p = vec3(x, G as f32 + 3.6, z);
        if p.distance(eye) < 40.0 {
            out.push(crate::extras::Label { pos: p, text: t.into(), size: 18.0, color: Color::new(1.0, 0.95, 0.75, 1.0) });
        }
    }
}
