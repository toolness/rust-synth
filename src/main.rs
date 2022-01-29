use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Sample, SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};
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
    let mut frequency = 261.63; // Middle C, taken from https://pages.mtu.edu/~suits/notefreqs.html
    let shape = Arc::new(Mutex::new(AudioShape { frequency }));
    let stream = match sample_format {
        SampleFormat::F32 => Player::get_stream::<f32>(device, &config, shape.clone()),
        SampleFormat::I16 => Player::get_stream::<i16>(device, &config, shape.clone()),
        SampleFormat::U16 => Player::get_stream::<u16>(device, &config, shape.clone()),
    };
    stream.play().unwrap();

    let one_beat = time::Duration::from_millis(500);

    for semitones in [2, 2, 1, 2, 2, 2, 1].iter() {
        thread::sleep(one_beat);

        for _ in 0..*semitones {
            // https://www.reddit.com/r/musictheory/comments/kyv9nd/how_many_hz_are_there_between_two_semitones/
            frequency *= 2.0f64.powf(1.0 / 12.0);
        }
        *shape.lock().unwrap() = AudioShape { frequency };
    }

    thread::sleep(one_beat);

    println!("Bye!");
}

#[derive(Copy, Clone)]
struct AudioShape {
    frequency: f64,
}

struct Player {
    num_channels: u16,
    sample_rate: usize,
    shape: Arc<Mutex<AudioShape>>,
    latest_pos_in_wave: f64,
}

impl Player {
    fn get_stream<T: Sample>(
        device: Device,
        config: &StreamConfig,
        shape: Arc<Mutex<AudioShape>>,
    ) -> Stream {
        let mut player = Player {
            num_channels: config.channels,
            sample_rate: config.sample_rate.0 as usize,
            shape,
            latest_pos_in_wave: 0.0,
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
        let shape = *self.shape.lock().unwrap();
        let samples_per_wave = self.sample_rate as f64 / shape.frequency;
        let wave_delta_per_sample = 1.0 / samples_per_wave;

        // We use chunks_mut() to access individual channels:
        // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
        for sample in data.chunks_mut(self.num_channels as usize) {
            let value = (self.latest_pos_in_wave * 2.0 * std::f64::consts::PI).sin() as f32;
            let sample_value = Sample::from(&value);

            for channel_sample in sample.iter_mut() {
                *channel_sample = sample_value;
            }
            self.latest_pos_in_wave = (self.latest_pos_in_wave + wave_delta_per_sample) % 1.0;
        }
    }
}
