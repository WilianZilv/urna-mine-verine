//! TV URNA NEWS: âncora IA resume o mundo num telão de fachada (server/tv.js).

use super::Place;
use crate::world::World;

pub struct Tv {}

impl Tv {
    pub fn new() -> Self {
        Tv {}
    }
}

impl Place for Tv {}

pub fn build(w: &mut World) {
    super::clear_lot(w, crate::layout::TV, 20, crate::world::STONE);
}
