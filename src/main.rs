use beat::{Beat, BeatCounter, TimeSignature};
use clap::{AppSettings, ArgEnum, Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::SampleFormat;

mod beat;
mod dummy_waker;
mod note;
mod player;
mod synth;
mod synth_registry;
mod waiter;

use note::{MidiNote, MAJOR_SCALE, MINOR_HARMONIC_SCALE, OCTAVE};
use player::{Player, PlayerProgram, PlayerProxy};
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
        #[clap(long)]
        /// Beats per minute (default 60).
        bpm: Option<u64>,
        #[clap(long)]
        /// Play two scales, the second an octave above the first.
        octaves: bool,
    },
    /// Plays a siren sound.
    Siren {},
    /// Outputs a siren sound to a WAV file.
    SirenWav {},
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Scale {
    Major,
    MinorHarmonic,
}

fn build_stream(program: PlayerProgram) -> PlayerProxy {
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
        SampleFormat::F32 => Player::get_stream::<f32>(device, &config, program),
        SampleFormat::I16 => Player::get_stream::<i16>(device, &config, program),
        SampleFormat::U16 => Player::get_stream::<u16>(device, &config, program),
    }
}

async fn siren_program() {
    for _ in 0..5 {
        Player::wait(500.0).await;
        let mut shape = Player::new_shape(AudioShape {
            frequency: 440.0,
            volume: 128,
        });
        Player::wait(500.0).await;
        shape.set_frequency(400.0);
        Player::wait(250.0).await;
    }
}

async fn scale_program(tonic: MidiNote, scale: Scale, bpm: u64, octaves: bool) {
    if octaves {
        Player::start_program(Box::pin(play_scale(tonic + OCTAVE, scale, bpm)));
    }
    play_scale(tonic, scale, bpm).await;
}

async fn play_scale(tonic: MidiNote, scale: Scale, bpm: u64) {
    let beat_counter = BeatCounter {
        bpm,
        time_signature: TimeSignature(4, Beat::Quarter),
    };
    let mut note: MidiNote = tonic;
    let mut shape = Player::new_shape(AudioShape {
        frequency: note.frequency(),
        volume: 127,
    });

    let base_scale = match scale {
        Scale::Major => MAJOR_SCALE,
        Scale::MinorHarmonic => MINOR_HARMONIC_SCALE,
    };

    let ms_per_quarter_note = beat_counter.duration_in_millis(Beat::Quarter);

    for semitones in base_scale
        .iter()
        .copied()
        .chain(base_scale.iter().rev().map(|s| -s))
    {
        Player::wait(ms_per_quarter_note).await;
        note += semitones;
        shape.set_frequency(note.frequency());
    }

    Player::wait(ms_per_quarter_note).await;
}

fn main() {
    let cli = Args::parse();
    match &cli.command {
        &Commands::SirenWav {} => {
            let filename = "siren.wav";
            Player::write_wav(filename, Box::pin(siren_program()));
            println!("Wrote {}.", filename);
        }
        Commands::Siren {} => {
            let player = build_stream(Box::pin(siren_program()));
            player.play_until_finished();
        }
        Commands::Scale {
            note,
            scale,
            bpm,
            octaves,
        } => {
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
            let player = build_stream(Box::pin(scale_program(
                tonic,
                scale.unwrap_or(Scale::Major),
                bpm.unwrap_or(60),
                *octaves,
            )));
            player.play_until_finished();
        }
    }
}
