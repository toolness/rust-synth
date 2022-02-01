use cpal::traits::DeviceTrait;
use cpal::{Device, Sample, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Copy, Clone)]
pub struct AudioShape {
    pub frequency: f64,
    pub volume: u8,
}

struct AudioShapeState {
    pos_in_wave: f64,
    volume: u8,
}

impl AudioShapeState {
    fn advance_pos_in_wave(&mut self, amount: f64) {
        self.pos_in_wave = (self.pos_in_wave + amount) % 1.0;
    }

    fn move_to_volume(&mut self, target: u8) {
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
    sample_rate: usize,
    shape_mutex: Arc<Mutex<AudioShape>>,
    shape: AudioShape,
    shape_state: AudioShapeState,
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
            sample_rate: config.sample_rate.0 as usize,
            shape_mutex,
            shape,
            shape_state: AudioShapeState {
                pos_in_wave: 0.0,
                volume: 0,
            },
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
            self.shape = *shape
        }
    }

    fn write_audio<T: Sample>(&mut self, data: &mut [T], _: &cpal::OutputCallbackInfo) {
        self.try_to_update_shape();
        let samples_per_wave = self.sample_rate as f64 / self.shape.frequency;
        let wave_delta_per_sample = 1.0 / samples_per_wave;

        // We use chunks_mut() to access individual channels:
        // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
        for sample in data.chunks_mut(self.num_channels as usize) {
            let volume_scale = self.shape_state.volume as f64 / std::u8::MAX as f64;
            let value = ((self.shape_state.pos_in_wave * TWO_PI).sin() * volume_scale) as f32;
            let sample_value = Sample::from(&value);

            for channel_sample in sample.iter_mut() {
                *channel_sample = sample_value;
            }
            self.shape_state.advance_pos_in_wave(wave_delta_per_sample);
            self.shape_state.move_to_volume(self.shape.volume);
        }
    }
}
