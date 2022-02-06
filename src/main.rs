use beat::{Beat, BeatCounter, TimeSignature};
use clap::{AppSettings, ArgEnum, Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use cpal::Stream;
use std::sync::{Arc, Mutex};

mod beat;
mod dummy_waker;
mod note;
mod player;
mod synth;
mod tracks;

use note::{MidiNote, MAJOR_SCALE, MINOR_HARMONIC_SCALE, OCTAVE};
use player::{Player, PlayerProgram};
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
        /// Beats per minute (default 60).
        bpm: Option<u64>,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Scale {
    Major,
    MinorHarmonic,
}

async fn program() {
    loop {
        Player::wait(500.0).await;
        let mut shape = Player::new_shape(AudioShape {
            frequency: 440.0,
            volume: 128,
        });
        Player::wait(500.0).await;
        shape.set_frequency(400.0);
        Player::wait(250.0).await;
        shape.finish().await;
    }
}

fn build_stream(shapes_mutex: Arc<Mutex<[AudioShape]>>, program: PlayerProgram) -> Stream {
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
        SampleFormat::F32 => Player::get_stream::<f32>(device, &config, shapes_mutex, program),
        SampleFormat::I16 => Player::get_stream::<i16>(device, &config, shapes_mutex, program),
        SampleFormat::U16 => Player::get_stream::<u16>(device, &config, shapes_mutex, program),
    }
}

fn play_scale(tonic: MidiNote, scale: Scale, bpm: u64) {
    let shapes_mutex = Arc::new(Mutex::new([
        AudioShape {
            frequency: tonic.frequency(),
            volume: 0,
        },
        AudioShape {
            frequency: (tonic - OCTAVE).frequency(),
            volume: 0,
        },
    ]));
    let stream = build_stream(shapes_mutex.clone(), Box::pin(program()));
    stream.play().unwrap();

    let beat_counter = BeatCounter {
        bpm,
        time_signature: TimeSignature(4, Beat::Quarter),
    };
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
        beat_counter.sleep(Beat::Quarter);
        note += semitones;
        let mut shapes = shapes_mutex.lock().unwrap();
        let mut curr_shape_note = note;
        for shape in shapes.iter_mut() {
            shape.frequency = curr_shape_note.frequency();
            curr_shape_note = curr_shape_note - OCTAVE;
        }
    }

    beat_counter.sleep(Beat::Quarter);

    // Avoid popping.
    shapes_mutex.lock().unwrap().iter_mut().for_each(|shape| {
        shape.volume = 0;
    });
    beat_counter.sleep(Beat::Sixteenth);
}

fn main() {
    let cli = Args::parse();
    match &cli.command {
        Commands::Scale { note, scale, bpm } => {
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
            play_scale(tonic, scale.unwrap_or(Scale::Major), bpm.unwrap_or(60));
        }
    }
}
