use cpal::traits::DeviceTrait;
use cpal::{Device, Sample, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Copy, Clone)]
pub struct AudioShape {
    pub frequency: f64,
    pub volume: u8,
}

pub struct Player {
    num_channels: u16,
    sample_rate: usize,
    shape_mutex: Arc<Mutex<AudioShape>>,
    shape: AudioShape,
    latest_pos_in_wave: f64,
    latest_volume: u8,
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
            latest_pos_in_wave: 0.0,
            latest_volume: 0,
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

    fn move_to_target_volume(&mut self, target: u8) {
        if self.latest_volume == target {
            return;
        }
        if self.latest_volume < target {
            self.latest_volume += 1;
        } else {
            self.latest_volume -= 1;
        }
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
            let volume_scale = self.latest_volume as f64 / std::u8::MAX as f64;
            let value = ((self.latest_pos_in_wave * TWO_PI).sin() * volume_scale) as f32;
            let sample_value = Sample::from(&value);

            for channel_sample in sample.iter_mut() {
                *channel_sample = sample_value;
            }
            self.latest_pos_in_wave = (self.latest_pos_in_wave + wave_delta_per_sample) % 1.0;
            self.move_to_target_volume(self.shape.volume);
        }
    }
}
