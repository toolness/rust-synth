use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Sample, SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

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
    let shape_mutex = Arc::new(Mutex::new(AudioShape {
        frequency,
        volume: 255,
    }));
    let stream = match sample_format {
        SampleFormat::F32 => Player::get_stream::<f32>(device, &config, shape_mutex.clone()),
        SampleFormat::I16 => Player::get_stream::<i16>(device, &config, shape_mutex.clone()),
        SampleFormat::U16 => Player::get_stream::<u16>(device, &config, shape_mutex.clone()),
    };
    stream.play().unwrap();

    let one_beat = Duration::from_millis(500);

    for semitones in [2, 2, 1, 2, 2, 2, 1].iter() {
        thread::sleep(one_beat);

        // https://en.wikipedia.org/wiki/Equal_temperament
        frequency *= 2.0f64.powf(*semitones as f64 / 12.0);

        shape_mutex.lock().unwrap().frequency = frequency;
    }

    thread::sleep(one_beat);

    // Avoid popping.
    shape_mutex.lock().unwrap().volume = 0;
    thread::sleep(Duration::from_millis(100));

    println!("Bye!");
}

#[derive(Copy, Clone)]
struct AudioShape {
    frequency: f64,
    volume: u8,
}

struct Player {
    num_channels: u16,
    sample_rate: usize,
    shape_mutex: Arc<Mutex<AudioShape>>,
    shape: AudioShape,
    latest_pos_in_wave: f64,
    latest_volume: u8,
}

impl Player {
    fn get_stream<T: Sample>(
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
