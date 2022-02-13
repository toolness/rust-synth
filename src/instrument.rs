use crate::{
    beat::{Beat, BeatCounter, BeatSettings},
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
    pub fn new(beat_settings: BeatSettings, max_volume: u8) -> Self {
        Instrument {
            beat_counter: BeatCounter::new(beat_settings),
            shape: Player::new_shape(AudioShape::default()),
            max_volume,
        }
    }

    async fn play_note_impl<N: MidiNoteLike>(&mut self, note: N, length: Beat, release_ms: f64) {
        self.shape
            .set_frequency(note.into_midi_note_or_panic().frequency());
        self.shape.set_volume(self.max_volume);
        Player::wait(self.beat_counter.increment(length) - release_ms).await;
        if release_ms > 0.0 {
            self.shape.set_volume(0);
            Player::wait(release_ms).await;
        }
    }

    pub async fn play_note<N: MidiNoteLike>(&mut self, note: N, length: Beat) {
        self.play_note_impl(note, length, PAUSE_MS).await;
    }

    pub async fn play_note_without_release<N: MidiNoteLike>(&mut self, note: N, length: Beat) {
        self.play_note_impl(note, length, 0.0).await;
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
        self.shape.set_volume(0);
        Player::wait(self.beat_counter.increment(length)).await;
    }

    pub fn total_measures(&self) -> f64 {
        self.beat_counter.total_measures()
    }
}
