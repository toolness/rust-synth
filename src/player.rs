use cpal::traits::DeviceTrait;
use cpal::{Device, Sample, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

use crate::synth::{AudioShape, AudioShapeSynthesizer};

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
