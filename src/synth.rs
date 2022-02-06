const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Copy, Clone)]
pub struct AudioShape {
    pub frequency: f64,
    pub volume: u8,
}

pub struct AudioShapeSynthesizer {
    sample_rate: usize,
    pos_in_wave: f64,
    volume: u8,
    wave_delta_per_sample: f64,
    target: AudioShape,
}

impl Iterator for AudioShapeSynthesizer {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        let volume_scale = self.volume as f64 / std::u8::MAX as f64;
        let value = (self.pos_in_wave * TWO_PI).sin() * volume_scale;

        self.pos_in_wave = (self.pos_in_wave + self.wave_delta_per_sample) % 1.0;
        self.move_to_target_volume();

        Some(value)
    }
}

impl AudioShapeSynthesizer {
    fn calculate_wave_delta_per_sample(sample_rate: usize, frequency: f64) -> f64 {
        let samples_per_wave = sample_rate as f64 / frequency;
        1.0 / samples_per_wave
    }

    pub fn new(target: AudioShape, sample_rate: usize) -> Self {
        Self {
            sample_rate,
            pos_in_wave: 0.0,
            volume: 0,
            target,
            wave_delta_per_sample: Self::calculate_wave_delta_per_sample(
                sample_rate,
                target.frequency,
            ),
        }
    }

    pub fn get_target(&self) -> AudioShape {
        self.target
    }

    pub fn update_target(&mut self, target: AudioShape) {
        self.target = target;
        self.wave_delta_per_sample =
            Self::calculate_wave_delta_per_sample(self.sample_rate, self.target.frequency);
    }

    fn move_to_target_volume(&mut self) {
        let target = self.target.volume;
        if self.volume == target {
            return;
        }
        if self.volume < target {
            self.volume += 1;
        } else {
            self.volume -= 1;
        }
    }
}
