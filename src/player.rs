use cpal::traits::DeviceTrait;
use cpal::{Device, Sample, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

use crate::synth::AudioShape;
use crate::tracks::Tracks;

pub struct Player {
    num_channels: u16,
    tracks: Tracks,
}

impl Player {
    pub fn get_stream<T: Sample>(
        device: Device,
        config: &StreamConfig,
        shape_mutex: Arc<Mutex<[AudioShape]>>,
    ) -> Stream {
        let tracks = Tracks::new(shape_mutex, config.sample_rate.0 as usize);
        let mut player = Player {
            num_channels: config.channels,
            tracks,
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

    fn write_audio<T: Sample>(&mut self, data: &mut [T], _: &cpal::OutputCallbackInfo) {
        // Try to update the tracks, but don't block, because we're
        // running in an extremely time-sensitive audio thread.
        self.tracks.try_to_update();

        // We use chunks_mut() to access individual channels:
        // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
        for sample in data.chunks_mut(self.num_channels as usize) {
            let sample_value = Sample::from(&(self.tracks.next().unwrap() as f32));

            for channel_sample in sample.iter_mut() {
                *channel_sample = sample_value;
            }
        }
    }
}
