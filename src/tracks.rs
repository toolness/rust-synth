use std::sync::{Arc, Mutex};

use crate::synth::{AudioShape, AudioShapeSynthesizer};

pub struct Tracks {
    shapes_mutex: Arc<Mutex<[AudioShape]>>,
    synths: Vec<AudioShapeSynthesizer>,
}

impl Tracks {
    pub fn new(shapes_mutex: Arc<Mutex<[AudioShape]>>, sample_rate: usize) -> Self {
        let shapes = shapes_mutex.lock().unwrap();
        let mut synths = Vec::with_capacity(shapes.len());
        for shape in shapes.iter() {
            let synth = AudioShapeSynthesizer::new(*shape, sample_rate);
            synths.push(synth);
        }
        drop(shapes);
        Tracks {
            shapes_mutex,
            synths,
        }
    }

    pub fn try_to_update(&mut self) {
        if let Ok(shapes) = self.shapes_mutex.try_lock() {
            for (shape, synth) in shapes.iter().zip(self.synths.iter_mut()) {
                synth.update_target(*shape);
            }
        }
    }
}

impl Iterator for Tracks {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        let mut value = 0.0;
        for synth in self.synths.iter_mut() {
            value += synth.next().unwrap();
        }
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::Tracks;

    #[test]
    fn test_it_works() {
        let mut tracks = Tracks::new(Arc::new(Mutex::new([])), 8000);
        tracks.try_to_update();
        assert_eq!(tracks.next(), Some(0.0));
    }
}
