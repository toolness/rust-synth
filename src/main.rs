use clap::{AppSettings, ArgEnum, Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use cpal::Stream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

mod note;
mod player;
mod synth;
mod tracks;

use note::{MidiNote, MAJOR_SCALE, MINOR_HARMONIC_SCALE, OCTAVE};
use player::Player;
use synth::AudioShape;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(global_setting(AppSettings::PropagateVersion))]
#[clap(global_setting(AppSettings::UseLongFormatForHelpSubcommand))]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Plays a scale (default C4 Major).
    Scale {
        /// e.g. C4, A#2, Bb5
        note: Option<String>,
        #[clap(arg_enum)]
        scale: Option<Scale>,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Scale {
    Major,
    MinorHarmonic,
}

fn build_stream(shapes_mutex: Arc<Mutex<[AudioShape]>>) -> Stream {
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
    match sample_format {
        SampleFormat::F32 => Player::get_stream::<f32>(device, &config, shapes_mutex),
        SampleFormat::I16 => Player::get_stream::<i16>(device, &config, shapes_mutex),
        SampleFormat::U16 => Player::get_stream::<u16>(device, &config, shapes_mutex),
    }
}

fn play_scale(tonic: MidiNote, scale: Scale) {
    let shapes_mutex = Arc::new(Mutex::new([
        AudioShape {
            frequency: tonic.frequency(),
            volume: 128,
        },
        AudioShape {
            frequency: (tonic - OCTAVE).frequency(),
            volume: 32,
        },
    ]));
    let stream = build_stream(shapes_mutex.clone());
    stream.play().unwrap();

    let one_beat = Duration::from_millis(500);
    let mut note: MidiNote = tonic;

    let base_scale = match scale {
        Scale::Major => MAJOR_SCALE,
        Scale::MinorHarmonic => MINOR_HARMONIC_SCALE,
    };

    for semitones in base_scale
        .iter()
        .copied()
        .chain(base_scale.iter().rev().map(|s| -s))
    {
        thread::sleep(one_beat);
        note += semitones;
        let mut shapes = shapes_mutex.lock().unwrap();
        shapes[0].frequency = note.frequency();
        shapes[1].frequency = (note - OCTAVE).frequency();
    }

    thread::sleep(one_beat);

    // Avoid popping.
    let mut shapes = shapes_mutex.lock().unwrap();
    shapes[0].volume = 0;
    shapes[1].volume = 0;
    thread::sleep(Duration::from_millis(250));
}

fn main() {
    let cli = Args::parse();
    match &cli.command {
        Commands::Scale { note, scale } => {
            let tonic: MidiNote = if let Some(note_str) = note {
                if let Ok(note) = MidiNote::parse(note_str) {
                    note
                } else {
                    println!("Unable to parse note '{}'!", note_str);
                    std::process::exit(1);
                }
            } else {
                "C4".try_into().unwrap()
            };
            play_scale(tonic, scale.unwrap_or(Scale::Major));
        }
    }
}
