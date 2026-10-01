//! Terminal Interdimensional: painel de partidas (vila + jogos do Hub) e portões de viagem (server/terminal.js).

use super::Place;
use crate::world::World;

pub struct Terminal {}

impl Terminal {
    pub fn new() -> Self {
        Terminal {}
    }
}

impl Place for Terminal {}

pub fn build(w: &mut World) {
    super::clear_lot(w, crate::layout::TERMINAL, 20, crate::world::STONE);
}
