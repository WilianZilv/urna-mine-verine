//! Banco Central da Vila: ranking, ledger na fachada, caixa eletrônico e poupança fictícia (server/banco.js).

use super::Place;
use crate::world::World;

pub struct Banco {}

impl Banco {
    pub fn new() -> Self {
        Banco {}
    }
}

impl Place for Banco {}

pub fn build(w: &mut World) {
    super::clear_lot(w, crate::layout::BANCO, 20, crate::world::STONE);
}
