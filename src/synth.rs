const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Copy, Clone)]
pub enum Waveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

impl Default for Waveform {
    fn default() -> Self {
        Waveform::Sine
    }
}

#[derive(Copy, Clone, Default)]
pub struct AudioShape {
    pub waveform: Waveform,
    pub frequency: f64,
    pub volume: u8,
}

pub struct AudioShapeSynthesizer {
    sample_rate: usize,
    pos_in_wave: f64,
    volume: u8,
    wave_delta_per_sample: f64,
    is_active: bool,
    target: AudioShape,
}

impl Iterator for AudioShapeSynthesizer {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        let volume_scale = self.volume as f64 / std::u8::MAX as f64;
        let value = self.base_value() * volume_scale;

        self.pos_in_wave = (self.pos_in_wave + self.wave_delta_per_sample) % 1.0;
        self.move_to_target_volume();

        Some(value)
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn triangle_wave(t: f64) -> f64 {
    if t <= 0.25 {
        lerp(0.0, 1.0, t / 0.25)
    } else if t <= 0.75 {
        lerp(1.0, -1.0, (t - 0.25) / 0.5)
    } else {
        lerp(-1.0, 0.0, (t - 0.75) / 0.25)
    }
}

fn rectangle_wave(duty_cycle: f64, t: f64) -> f64 {
    if t < duty_cycle {
        1.0
    } else {
        -1.0
    }
}

impl AudioShapeSynthesizer {
    fn base_value(&self) -> f64 {
        match self.target.waveform {
            Waveform::Sine => (self.pos_in_wave * TWO_PI).sin(),
            Waveform::Square => rectangle_wave(0.5, self.pos_in_wave),
            Waveform::Triangle => triangle_wave(self.pos_in_wave),
            Waveform::Sawtooth => {
                if self.pos_in_wave <= 0.5 {
                    lerp(0.0, 1.0, self.pos_in_wave / 0.5)
                } else {
                    lerp(-1.0, 0.0, (self.pos_in_wave - 0.5) / 0.5)
                }
            }
        }
    }

    fn calculate_wave_delta_per_sample(sample_rate: usize, frequency: f64) -> f64 {
        if frequency == 0.0 {
            0.0
        } else {
            let samples_per_wave = sample_rate as f64 / frequency;
            1.0 / samples_per_wave
        }
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
            is_active: true,
        }
    }

    pub fn make_inactive(&mut self) {
        self.is_active = false;
        self.target.volume = 0;
    }

    pub fn has_finished_playing(&self) -> bool {
        !self.is_active && self.volume == 0
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

#[cfg(test)]
mod tests {
    use crate::synth::{lerp, triangle_wave};

    #[test]
    fn test_lerp_works() {
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);

        assert_eq!(lerp(10.0, 0.0, 0.0), 10.0);
        assert_eq!(lerp(10.0, 0.0, 1.0), 0.0);
        assert_eq!(lerp(10.0, 0.0, 0.5), 5.0);
    }

    #[test]
    fn test_triangle_wave_works() {
        assert_eq!(triangle_wave(0.0), 0.0);
        assert_eq!(triangle_wave(0.25), 1.0);
        assert_eq!(triangle_wave(0.5), 0.0);
        assert_eq!(triangle_wave(0.75), -1.0);
        assert_eq!(triangle_wave(1.0), 0.0);
    }
}
