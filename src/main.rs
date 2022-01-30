use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

mod note;
mod player;

fn main() {
    use note::{MidiNote, MAJOR_SCALE, MINOR_HARMONIC_SCALE};
    use player::{AudioShape, Player};

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
    let tonic: MidiNote = "C4".try_into().unwrap();
    let shape_mutex = Arc::new(Mutex::new(AudioShape {
        frequency: tonic.frequency(),
        volume: 255,
    }));
    let stream = match sample_format {
        SampleFormat::F32 => Player::get_stream::<f32>(device, &config, shape_mutex.clone()),
        SampleFormat::I16 => Player::get_stream::<i16>(device, &config, shape_mutex.clone()),
        SampleFormat::U16 => Player::get_stream::<u16>(device, &config, shape_mutex.clone()),
    };
    stream.play().unwrap();

    let one_beat = Duration::from_millis(500);
    let mut note: MidiNote = tonic;

    for semitones in MAJOR_SCALE
        .iter()
        .copied()
        .chain(MINOR_HARMONIC_SCALE.iter().rev().map(|s| -s))
    {
        thread::sleep(one_beat);
        note += semitones;
        shape_mutex.lock().unwrap().frequency = note.frequency();
    }

    thread::sleep(one_beat);

    // Avoid popping.
    shape_mutex.lock().unwrap().volume = 0;
    thread::sleep(Duration::from_millis(250));

    println!("Bye!");
}
