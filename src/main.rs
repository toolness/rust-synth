use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Sample, SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{ops, thread};

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

// https://www.inspiredacoustics.com/en/MIDI_note_numbers_and_center_frequencies
const A4_MIDI_NOTE: i8 = 69;

// https://en.wikipedia.org/wiki/Equal_temperament
const A4_FREQUENCY: f64 = 440.0;
const SEMITONES_PER_OCTAVE: i8 = 12;

const SEMITONE: Semitones = Semitones(1);
const TONE: Semitones = Semitones(2);
const MAJOR_SCALE: [Semitones; 7] = [TONE, TONE, SEMITONE, TONE, TONE, TONE, SEMITONE];
const MINOR_HARMONIC_SCALE: [Semitones; 7] =
    [TONE, SEMITONE, TONE, TONE, SEMITONE, Semitones(3), SEMITONE];

#[derive(Copy, Clone, PartialEq, Debug)]
struct MidiNote(i8);

impl MidiNote {
    fn frequency(&self) -> f64 {
        // Keep in mind that every MIDI note represents a semitone.
        let semitones_from_a4: f64 = self.0 as f64 - A4_MIDI_NOTE as f64;
        return A4_FREQUENCY * 2.0f64.powf(semitones_from_a4 as f64 / SEMITONES_PER_OCTAVE as f64);
    }

    fn try_from_chars(
        note: char,
        accidental: Option<char>,
        octave: char,
    ) -> Result<MidiNote, MidiNoteParseError> {
        let note_semitones_from_a = match note {
            'C' => Semitones(-9),
            'D' => Semitones(-7),
            'E' => Semitones(-5),
            'F' => Semitones(-4),
            'G' => Semitones(-2),
            'A' => Semitones(0),
            'B' => Semitones(2),
            _ => return Err(MidiNoteParseError::InvalidNoteCharacter),
        };
        let accidental_semitone_offset = match accidental {
            Some('#') => Semitones(1),
            Some('b') => Semitones(-1),
            None => Semitones(0),
            _ => return Err(MidiNoteParseError::InvalidAccidentalCharacter),
        };
        let octaves_from_4 = match octave {
            '0' => -4,
            '1' => -3,
            '2' => -2,
            '3' => -1,
            '4' => 0,
            '5' => 1,
            '6' => 2,
            '7' => 3,
            '8' => 4,
            '9' => 5,
            _ => return Err(MidiNoteParseError::InvalidOctaveCharacter),
        };

        Ok(MidiNote(A4_MIDI_NOTE)
            + note_semitones_from_a
            + accidental_semitone_offset
            + Semitones(octaves_from_4 * SEMITONES_PER_OCTAVE))
    }
}

#[derive(Debug, PartialEq)]
enum MidiNoteParseError {
    InvalidLength,
    InvalidNoteCharacter,
    InvalidAccidentalCharacter,
    InvalidOctaveCharacter,
}

impl TryFrom<&str> for MidiNote {
    type Error = MidiNoteParseError;

    fn try_from(value: &str) -> Result<MidiNote, MidiNoteParseError> {
        match &value.chars().collect::<Vec<char>>()[..] {
            &[note, accidental, octave] => MidiNote::try_from_chars(note, Some(accidental), octave),
            &[note, octave] => MidiNote::try_from_chars(note, None, octave),
            _ => Err(MidiNoteParseError::InvalidLength),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{MidiNote, MidiNoteParseError};

    #[test]
    fn test_a4_works() {
        assert_eq!("A4".try_into(), Ok(MidiNote(69)));
    }

    #[test]
    fn test_c4_works() {
        assert_eq!("C4".try_into(), Ok(MidiNote(60)));
    }

    #[test]
    fn test_weird_notes_work() {
        let f4: MidiNote = "F4".try_into().unwrap();
        assert_eq!("E#4".try_into(), Ok(f4));

        let b3: MidiNote = "B3".try_into().unwrap();
        assert_eq!("Cb4".try_into(), Ok(b3));
    }

    #[test]
    fn test_sharps_work() {
        assert_eq!("C#4".try_into(), Ok(MidiNote(61)));
    }

    #[test]
    fn test_flats_work() {
        assert_eq!("Bb4".try_into(), Ok(MidiNote(70)));
    }

    #[test]
    fn test_octaves_work() {
        assert_eq!("A0".try_into(), Ok(MidiNote(21)));
        assert_eq!("G9".try_into(), Ok(MidiNote(127)));
    }

    fn try_parse(value: &'static str) -> Result<MidiNote, MidiNoteParseError> {
        value.try_into()
    }

    #[test]
    fn test_invalid_length_error() {
        assert_eq!(try_parse("A"), Err(MidiNoteParseError::InvalidLength));
        assert_eq!(try_parse("Ab4k"), Err(MidiNoteParseError::InvalidLength));
    }

    #[test]
    fn test_invalid_note_character() {
        assert_eq!(
            try_parse("Z4"),
            Err(MidiNoteParseError::InvalidNoteCharacter)
        );
    }

    #[test]
    fn test_invalid_accidental_character() {
        assert_eq!(
            try_parse("Ak4"),
            Err(MidiNoteParseError::InvalidAccidentalCharacter)
        );
    }

    #[test]
    fn test_invalid_octave_character() {
        assert_eq!(
            try_parse("Ap"),
            Err(MidiNoteParseError::InvalidOctaveCharacter)
        );
    }
}

#[derive(Copy, Clone)]
struct Semitones(i8);

impl ops::Neg for Semitones {
    type Output = Semitones;

    fn neg(self) -> Self::Output {
        Semitones(-self.0)
    }
}

impl ops::Neg for &Semitones {
    type Output = Semitones;

    fn neg(self) -> Self::Output {
        Semitones(-self.0)
    }
}

impl ops::Add<Semitones> for MidiNote {
    type Output = MidiNote;

    fn add(self, rhs: Semitones) -> MidiNote {
        // TODO: We shouldn't let this go outside the range of MIDI notes.
        MidiNote(self.0 + rhs.0)
    }
}

impl ops::AddAssign<Semitones> for MidiNote {
    fn add_assign(&mut self, rhs: Semitones) {
        self.0 = self.0 + rhs.0;
    }
}

impl ops::Sub<Semitones> for MidiNote {
    type Output = MidiNote;

    fn sub(self, rhs: Semitones) -> MidiNote {
        // TODO: We shouldn't let this go outside the range of MIDI notes.
        MidiNote(self.0 - rhs.0)
    }
}

impl ops::SubAssign<Semitones> for MidiNote {
    fn sub_assign(&mut self, rhs: Semitones) {
        self.0 = self.0 - rhs.0;
    }
}

fn main() {
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

    let down: Vec<Semitones> = MINOR_HARMONIC_SCALE.iter().rev().map(|s| -s).collect();
    for semitones in MAJOR_SCALE.iter().chain(down.iter()) {
        thread::sleep(one_beat);
        note += *semitones;
        shape_mutex.lock().unwrap().frequency = note.frequency();
    }

    thread::sleep(one_beat);

    // Avoid popping.
    shape_mutex.lock().unwrap().volume = 0;
    thread::sleep(Duration::from_millis(250));

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
