use std::ops;

// https://www.inspiredacoustics.com/en/MIDI_note_numbers_and_center_frequencies
const A4_MIDI_NOTE: i8 = 69;

// https://en.wikipedia.org/wiki/Equal_temperament
const A4_FREQUENCY: f64 = 440.0;
const SEMITONES_PER_OCTAVE: i8 = 12;

pub const SEMITONE: Semitones = Semitones(1);
pub const TONE: Semitones = Semitones(2);
pub const OCTAVE: Semitones = Semitones(SEMITONES_PER_OCTAVE);
pub const MAJOR_SCALE: [Semitones; 7] = [TONE, TONE, SEMITONE, TONE, TONE, TONE, SEMITONE];
pub const MINOR_HARMONIC_SCALE: [Semitones; 7] =
    [TONE, SEMITONE, TONE, TONE, SEMITONE, Semitones(3), SEMITONE];

pub trait MidiNoteLike: TryInto<MidiNote> + Copy {
    fn into_midi_note_or_panic(self) -> MidiNote;
}

impl<T: TryInto<MidiNote, Error = E> + Copy, E: std::fmt::Debug> MidiNoteLike for T {
    fn into_midi_note_or_panic(self) -> MidiNote {
        self.try_into().unwrap()
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct MidiNote(i8);

impl MidiNote {
    pub fn frequency(&self) -> f64 {
        // Keep in mind that every MIDI note represents a semitone.
        let semitones_from_a4: f64 = self.0 as f64 - A4_MIDI_NOTE as f64;
        return A4_FREQUENCY * 2.0f64.powf(semitones_from_a4 as f64 / SEMITONES_PER_OCTAVE as f64);
    }

    pub fn parse<T: AsRef<str>>(value: &T) -> Result<MidiNote, MidiNoteParseError> {
        value.as_ref().try_into()
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
pub enum MidiNoteParseError {
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
    use super::{MidiNote, MidiNoteParseError};

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
pub struct Semitones(i8);

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
