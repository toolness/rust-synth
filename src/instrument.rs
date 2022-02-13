use crate::{
    beat::{Beat, BeatCounter},
    note::MidiNoteLike,
    player::{AudioShapeProxy, Player},
    synth::AudioShape,
};

// Amount of time to pause between notes (when not slurring)
const PAUSE_MS: f64 = 50.0;

#[derive(Clone)]
pub struct Instrument {
    beat_counter: BeatCounter,
    shape: AudioShapeProxy,
    max_volume: u8,
}

impl Instrument {
    pub fn new(beat_counter: BeatCounter, max_volume: u8) -> Self {
        Instrument {
            beat_counter,
            shape: Player::new_shape(AudioShape::default()),
            max_volume,
        }
    }

    pub async fn play_note<N: MidiNoteLike>(&mut self, note: N, length: Beat) {
        let ms = self.beat_counter.duration_in_millis(length);
        self.shape
            .set_frequency(note.into_midi_note_or_panic().frequency());
        self.shape.set_volume(self.max_volume);
        Player::wait(ms - PAUSE_MS).await;
        self.shape.set_volume(0);
        Player::wait(PAUSE_MS).await;
    }

    pub async fn play_note_without_release<N: MidiNoteLike>(&mut self, note: N, length: Beat) {
        let ms = self.beat_counter.duration_in_millis(length);
        self.shape
            .set_frequency(note.into_midi_note_or_panic().frequency());
        self.shape.set_volume(self.max_volume);
        Player::wait(ms).await;
    }

    pub async fn play_chord<N: MidiNoteLike>(&mut self, notes: &[N], length: Beat) {
        for note in notes.iter().skip(1) {
            let mut instrument = self.clone();
            let midi_note = (*note).into_midi_note_or_panic();
            Player::start_program(async move {
                instrument.play_note(midi_note, length).await;
            });
        }
        let first_note = (*notes.get(0).unwrap()).into_midi_note_or_panic();
        self.play_note(first_note, length).await;
    }

    pub async fn rest(&mut self, length: Beat) {
        let ms = self.beat_counter.duration_in_millis(length);
        self.shape.set_volume(0);
        Player::wait(ms).await;
    }
}
