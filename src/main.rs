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
use player::{AudioShapeProxy, Player, PlayerProgram, PlayerProxy};
use synth::AudioShape;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(global_setting(AppSettings::PropagateVersion))]
#[clap(global_setting(AppSettings::UseLongFormatForHelpSubcommand))]
struct Args {
    #[clap(subcommand)]
    command: Commands,
    #[clap(long, short = 'o')]
    /// Output to WAV file.
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
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Scale {
    Major,
    MinorHarmonic,
}

impl Args {
    fn run_program(&self, program: PlayerProgram) {
        if let Some(filename) = &self.output {
            Player::write_wav(filename, program);
            println!("Wrote {}.", filename);
        } else {
            let player = build_stream(program);
            player.play_until_finished();
        }
    }
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

// Amount of time to pause between notes (when not slurring)
const PAUSE_MS: f64 = 50.0;

struct Instrument {
    beat_counter: BeatCounter,
    shape: AudioShapeProxy,
    max_volume: u8,
}

impl Instrument {
    fn new(beat_counter: BeatCounter, max_volume: u8) -> Self {
        Instrument {
            beat_counter,
            shape: Player::new_shape(AudioShape {
                frequency: 440.0,
                volume: 0,
            }),
            max_volume,
        }
    }

    async fn play_note(&mut self, note: &str, length: Beat) {
        let ms = self.beat_counter.duration_in_millis(length);
        let note = MidiNote::try_from(note).unwrap();
        self.shape.set_frequency(note.frequency());
        self.shape.set_volume(self.max_volume);
        Player::wait(ms - PAUSE_MS).await;
        self.shape.set_volume(0);
        Player::wait(PAUSE_MS).await;
    }

    async fn play_note_without_release(&mut self, note: &str, length: Beat) {
        let ms = self.beat_counter.duration_in_millis(length);
        let note = MidiNote::try_from(note).unwrap();
        self.shape.set_frequency(note.frequency());
        self.shape.set_volume(self.max_volume);
        Player::wait(ms).await;
    }

    async fn rest(&mut self, length: Beat) {
        let ms = self.beat_counter.duration_in_millis(length);
        self.shape.set_volume(0);
        Player::wait(ms).await;
    }
}

async fn captain_silver_program() {
    let beats = BeatCounter {
        bpm: 120,
        time_signature: TimeSignature(4, Beat::Quarter),
    };

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
    };

    Player::start_program(Box::pin(right_hand));
    Player::start_program(Box::pin(left_hand));
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
        &Commands::CaptainSilver {} => {
            cli.run_program(Box::pin(captain_silver_program()));
        }
        Commands::Siren {} => {
            cli.run_program(Box::pin(siren_program()));
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
            cli.run_program(Box::pin(scale_program(
                tonic,
                scale.unwrap_or(Scale::Major),
                bpm.unwrap_or(60),
                *octaves,
            )))
        }
    }
}
