//! Áudio no navegador: clipes PCM viram AudioBuffers no WebAudio (web/urna.js).

use crate::web::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub type Clip = Arc<Vec<f32>>;

pub struct Audio {
    pub rate: u32,
    clips: RefCell<HashMap<usize, u32>>,
}

impl Audio {
    pub fn new() -> Self {
        Audio { rate: unsafe { urna_audio_rate() }, clips: RefCell::new(HashMap::new()) }
    }

    pub fn play(&self, data: &Clip, vol: f32, looped: bool) -> u64 {
        let key = Arc::as_ptr(data) as usize;
        let clip = *self.clips.borrow_mut().entry(key).or_insert_with(|| unsafe { urna_audio_clip(data.as_ptr(), data.len(), self.rate) });
        unsafe { urna_audio_play(clip, vol, looped as i32) as u64 }
    }

    pub fn set_volume(&self, id: u64, vol: f32) {
        unsafe { urna_audio_volume(id as u32, vol) }
    }
}
