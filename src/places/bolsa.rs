//! Bolsa de Valores da Vila: ações fictícias que reagem à arena e à cidade (server/bolsa.js).

use super::Place;
use crate::world::World;

pub struct Bolsa {}

impl Bolsa {
    pub fn new() -> Self {
        Bolsa {}
    }
}

impl Place for Bolsa {}

pub fn build(w: &mut World) {
    super::clear_lot(w, crate::layout::BOLSA, 20, crate::world::STONE);
}
