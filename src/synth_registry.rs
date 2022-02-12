use std::collections::HashMap;

use crate::synth::AudioShapeSynthesizer;

pub struct SynthRegistry {
    total_samples: usize,
    latest_id: usize,
    map: HashMap<usize, AudioShapeSynthesizer>,
}

impl SynthRegistry {
    pub fn new() -> Self {
        Self {
            total_samples: 0,
            latest_id: 0,
            map: HashMap::new(),
        }
    }

    pub fn remove_finished_synths(&mut self) {
        self.map.retain(|_id, synth| {
            return !synth.has_finished_playing();
        });
    }

    pub fn modify<F: FnOnce(&mut AudioShapeSynthesizer)>(&mut self, id: usize, f: F) {
        self.map.entry(id).and_modify(f);
    }

    pub fn insert(&mut self, synth: AudioShapeSynthesizer) -> usize {
        self.latest_id += 1;
        let id = self.latest_id;
        let prev_value = self.map.insert(id, synth);
        assert!(prev_value.is_none());
        return id;
    }

    pub fn get_total_samples(&self) -> usize {
        self.total_samples
    }

    pub fn next_sample(&mut self) -> f64 {
        let mut value = 0.0;
        for (_id, synth) in self.map.iter_mut() {
            value += synth.next().unwrap();
        }
        self.total_samples += 1;
        value
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
