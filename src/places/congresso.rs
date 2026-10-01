//! Congresso da Vila: jogadores votam leis que mudam o jogo por alguns minutos (server/congresso.js).

use super::{Laws, Place};
use crate::world::World;

pub struct Congresso {
    laws: Laws,
}

impl Congresso {
    pub fn new() -> Self {
        Congresso { laws: Laws::default() }
    }

    pub fn laws(&self) -> Laws {
        self.laws
    }
}

impl Place for Congresso {}

pub fn build(w: &mut World) {
    super::clear_lot(w, crate::layout::CONGRESSO, 20, crate::world::STONE);
}
