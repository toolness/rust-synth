use std::process::Command;

use beat::{Beat, BeatSettings, FOUR_FOUR};
use clap::{AppSettings, ArgEnum, Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::SampleFormat;

mod beat;
mod dummy_waker;
mod instrument;
mod note;
mod player;
mod synth;
mod synth_registry;
mod waiter;

use instrument::Instrument;
use note::{MidiNote, MAJOR_SCALE, MINOR_HARMONIC_SCALE, OCTAVE};
use player::{Player, PlayerProgram, PlayerProxy, WAV_CHANNELS, WAV_SAMPLE_RATE};
use synth::AudioShape;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(global_setting(AppSettings::PropagateVersion))]
#[clap(global_setting(AppSettings::UseLongFormatForHelpSubcommand))]
struct Args {
    #[clap(subcommand)]
    command: Commands,
    #[clap(long, short = 'o', global = true)]
    /// Output to WAV or MP3 file (MP3 requires ffmpeg).
    output: Option<String>,
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
    /// Plays the song "Captain Silver" from pg. 21 of Schaum's Red Book (Alfred).
    CaptainSilver {},
    /// Plays the song "Tune of the Tuna Fish" from pg. 29 of Schaum's Red Book (Alfred).
    Tuna {},
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Scale {
    Major,
    MinorHarmonic,
}

impl Args {
    fn run_program<P: PlayerProgram>(&self, program: P) {
        if let Some(filename) = &self.output {
            let is_mp3 = filename.ends_with(".mp3");
            let wav_filename = if is_mp3 { "temp.wav" } else { &filename };
            Player::write_wav(wav_filename, program);
            if is_mp3 {
                let success = convert_wav_to_mp3(wav_filename, filename);
                std::fs::remove_file(wav_filename).unwrap();
                if !success {
                    std::process::exit(1);
                }
            }
            println!("Wrote {}.", filename);
        } else {
            let player = build_stream(program);
            player.play_until_finished();
        }
    }
}

fn convert_wav_to_mp3(wav_filename: &str, mp3_filename: &String) -> bool {
    let mut success = false;
    let cmd = format!(
        "ffmpeg -y -i {} -ar {} -ac {} -b:a 128k {}",
        wav_filename, WAV_SAMPLE_RATE, WAV_CHANNELS, mp3_filename
    );
    if let Ok(mut child) = Command::new("bash").args(["-c", &cmd]).spawn() {
        if let Ok(ffmpeg_exit_code) = child.wait() {
            if ffmpeg_exit_code.success() {
                success = true;
            } else {
                println!("An error occurred running ffmpeg.");
            }
        } else {
            println!("An error occurred while waiting for ffmpeg to exit.");
        }
    } else {
        println!("Starting ffmpeg failed, you may need to install it.");
    }
    success
}

fn build_stream<P: PlayerProgram>(program: P) -> PlayerProxy {
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
        SampleFormat::F32 => Player::get_stream::<f32, P>(device, &config, program),
        SampleFormat::I16 => Player::get_stream::<i16, P>(device, &config, program),
        SampleFormat::U16 => Player::get_stream::<u16, P>(device, &config, program),
    }
}

async fn tuna_program() {
    let beats = BeatSettings::new(80, FOUR_FOUR);
    let mut left_hand = Instrument::new(beats, 63);
    let mut right_hand = Instrument::new(beats, 63);

    async fn tuna_fish_tuna_fish(right_hand: &mut Instrument, mut left_hand: Instrument) {
        Player::start_program(async move {
            left_hand.play_note("F3", Beat::Half).await;
            left_hand.play_chord(&["A3", "C4"], Beat::Half).await;

            left_hand.play_note("G3", Beat::Half).await;
            left_hand.play_chord(&["Bb3", "C4"], Beat::Half).await;
        });

        right_hand.play_note("C5", Beat::Quarter).await;
        right_hand.play_note("A4", Beat::Quarter).await;
        right_hand.play_note("A4", Beat::Half).await;

        right_hand.play_note("Bb4", Beat::Quarter).await;
        right_hand.play_note("G4", Beat::Quarter).await;
        right_hand.play_note("G4", Beat::Half).await;
    }

    async fn sing_a_tune_of_tuna_fish(right_hand: &mut Instrument, mut left_hand: Instrument) {
        Player::start_program(async move {
            left_hand.play_note("F3", Beat::Half).await;
            left_hand.play_chord(&["A3", "C4"], Beat::Half).await;

            left_hand.play_note("F3", Beat::Half).await;
            left_hand.play_chord(&["A3", "C4"], Beat::Half).await;
        });

        right_hand.play_note("F4", Beat::Quarter).await;
        right_hand.play_note("G4", Beat::Quarter).await;
        right_hand.play_note("A4", Beat::Quarter).await;
        right_hand.play_note("Bb4", Beat::Quarter).await;

        right_hand.play_note("C5", Beat::Quarter).await;
        right_hand.play_note("C5", Beat::Quarter).await;
        right_hand.play_note("C5", Beat::Half).await;
    }

    tuna_fish_tuna_fish(&mut right_hand, left_hand.clone()).await;
    left_hand.sync_beats_with(&right_hand);
    sing_a_tune_of_tuna_fish(&mut right_hand, left_hand.clone()).await;
    left_hand.sync_beats_with(&right_hand);

    assert_eq!(right_hand.total_measures(), 4.0);
    assert_eq!(left_hand.total_measures(), 4.0);
}

async fn captain_silver_program() {
    let beats = BeatSettings::new(120, FOUR_FOUR);

    let right_hand = async move {
        let mut hand = Instrument::new(beats, 63);

        // Measures 1-4
        for _ in 0..5 {
            hand.play_note("E4", Beat::Half).await;
        }
        hand.play_note("E4", Beat::Quarter).await;
        hand.play_note("F4", Beat::Quarter).await;
        hand.play_note("G4", Beat::Whole).await;

        // Measures 5-8
        hand.play_note("F4", Beat::Half).await;
        hand.play_note("F4", Beat::Half).await;
        hand.play_note("D4", Beat::Half).await;
        hand.play_note("D4", Beat::Half).await;
        hand.play_note("G4", Beat::Whole).await;
        hand.play_note("F4", Beat::Whole).await;

        // Measures 9-12 (same as 1-4)
        for _ in 0..5 {
            hand.play_note("E4", Beat::Half).await;
        }
        hand.play_note("E4", Beat::Quarter).await;
        hand.play_note("F4", Beat::Quarter).await;
        hand.play_note("G4", Beat::Whole).await;

        // Measures 13-16
        hand.play_note("F4", Beat::Half).await;
        hand.play_note("F4", Beat::Half).await;
        hand.play_note("D4", Beat::Half).await;
        hand.play_note("D4", Beat::Half).await;
        hand.play_note_without_release("C4", Beat::Whole).await;
        hand.play_note("C4", Beat::Whole).await;

        assert_eq!(hand.total_measures(), 16.0);
    };

    let left_hand = async move {
        let mut hand = Instrument::new(beats, 63);

        // Measures 1-4
        for _ in 0..5 {
            hand.play_note("C3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }
        hand.play_note("C3", Beat::Quarter).await;
        hand.rest(Beat::Quarter).await;
        for _ in 0..2 {
            hand.play_note("E3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }

        // Measures 5-8
        for _ in 0..2 {
            hand.play_note("D3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }
        for _ in 0..2 {
            hand.play_note("F3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }
        for _ in 0..2 {
            hand.play_note("E3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }
        for _ in 0..2 {
            hand.play_note("D3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }

        // Measures 9-12 (same as 1-4)
        for _ in 0..5 {
            hand.play_note("C3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }
        hand.play_note("C3", Beat::Quarter).await;
        hand.rest(Beat::Quarter).await;
        for _ in 0..2 {
            hand.play_note("E3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }

        // Measures 13-16
        for _ in 0..2 {
            hand.play_note("D3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }
        for _ in 0..2 {
            hand.play_note("F3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }
        for _ in 0..2 {
            hand.play_note("E3", Beat::Quarter).await;
            hand.play_note("G3", Beat::Quarter).await;
        }
        hand.play_note("C3", Beat::Whole).await;

        assert_eq!(hand.total_measures(), 16.0);
    };

    Player::start_program(right_hand);
    Player::start_program(left_hand);
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
        Player::start_program(play_scale(tonic + OCTAVE, scale, bpm));
    }
    play_scale(tonic, scale, bpm).await;
}

async fn play_scale(tonic: MidiNote, scale: Scale, bpm: u64) {
    let beat_settings = BeatSettings::new(bpm, FOUR_FOUR);
    let mut note: MidiNote = tonic;
    let mut shape = Player::new_shape(AudioShape {
        frequency: note.frequency(),
        volume: 127,
    });

    let base_scale = match scale {
        Scale::Major => MAJOR_SCALE,
        Scale::MinorHarmonic => MINOR_HARMONIC_SCALE,
    };

    let ms_per_quarter_note = beat_settings.duration_in_millis(Beat::Quarter);

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
        Commands::CaptainSilver {} => {
            cli.run_program(captain_silver_program());
        }
        Commands::Tuna {} => {
            cli.run_program(tuna_program());
        }
        Commands::Siren {} => {
            cli.run_program(siren_program());
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
            cli.run_program(scale_program(
                tonic,
                scale.unwrap_or(Scale::Major),
                bpm.unwrap_or(60),
                *octaves,
            ))
        }
    }
}
