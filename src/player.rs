use cpal::traits::DeviceTrait;
use cpal::{Device, Sample, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Copy, Clone)]
pub struct AudioShape {
    pub frequency: f64,
    pub volume: u8,
}

struct AudioShapeSynthesizer {
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

    fn new(target: AudioShape, sample_rate: usize) -> Self {
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

    fn update_target(&mut self, target: AudioShape) {
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

pub struct Player {
    num_channels: u16,
    shape_mutex: Arc<Mutex<AudioShape>>,
    synth: AudioShapeSynthesizer,
}

impl Player {
    pub fn get_stream<T: Sample>(
        device: Device,
        config: &StreamConfig,
        shape_mutex: Arc<Mutex<AudioShape>>,
    ) -> Stream {
        let shape = *shape_mutex.lock().unwrap();
        let mut player = Player {
            num_channels: config.channels,
            shape_mutex,
            synth: AudioShapeSynthesizer::new(shape, config.sample_rate.0 as usize),
        };
        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
        device
            .build_output_stream(
                config,
                move |data, cpal| player.write_audio::<T>(data, cpal),
                err_fn,
            )
            .unwrap()
    }

    /// Try to update the shape from our mutex, but don't block, because we're
    /// running in an extremely time-sensitive audio thread.
    fn try_to_update_shape(&mut self) {
        if let Ok(shape) = self.shape_mutex.try_lock() {
            self.synth.update_target(*shape);
        }
    }

    fn write_audio<T: Sample>(&mut self, data: &mut [T], _: &cpal::OutputCallbackInfo) {
        self.try_to_update_shape();

        // We use chunks_mut() to access individual channels:
        // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
        for sample in data.chunks_mut(self.num_channels as usize) {
            let sample_value = Sample::from(&(self.synth.next().unwrap() as f32));

            for channel_sample in sample.iter_mut() {
                *channel_sample = sample_value;
            }
        }
    }
}
