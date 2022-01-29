use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Sample, SampleFormat, Stream, StreamConfig};
use std::{thread, time};

fn main() {
    println!("Hello, world!");
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");
    let mut supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!")
        .with_max_sample_rate();
    let sample_format = supported_config.sample_format();
    println!(
        "Attempting to create an output stream with format: {:?}",
        supported_config
    );
    let config = supported_config.into();
    let stream = match sample_format {
        SampleFormat::F32 => Player::get_stream::<f32>(device, &config),
        SampleFormat::I16 => Player::get_stream::<i16>(device, &config),
        SampleFormat::U16 => Player::get_stream::<u16>(device, &config),
    };
    stream.play().unwrap();

    println!("Sleeping for a bit.");
    let one_sec = time::Duration::from_secs(1);
    thread::sleep(one_sec);
    println!("Bye!");
}

struct Player {
    num_channels: u16,
}

impl Player {
    fn get_stream<T: Sample>(device: Device, config: &StreamConfig) -> Stream {
        let mut player = Player {
            num_channels: config.channels,
        };
        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
        device
            .build_output_stream(
                config,
                move |data, cpal| player.write_silence::<T>(data, cpal),
                err_fn,
            )
            .unwrap()
    }

    fn write_silence<T: Sample>(&mut self, data: &mut [T], _: &cpal::OutputCallbackInfo) {
        // We use chunks_mut() to access individual channels:
        // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
        for sample in data.chunks_mut(self.num_channels as usize) {
            for channel_sample in sample.iter_mut() {
                *channel_sample = Sample::from(&0i16);
            }
        }
    }
}
