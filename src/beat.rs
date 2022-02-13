pub const FOUR_FOUR: TimeSignature = TimeSignature(4, Beat::Quarter);

#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum Beat {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
}

impl Beat {
    pub fn divisor(&self) -> u64 {
        match self {
            Beat::Whole => 1,
            Beat::Half => 2,
            Beat::Quarter => 4,
            Beat::Eighth => 8,
            Beat::Sixteenth => 16,
            Beat::ThirtySecond => 32,
            Beat::SixtyFourth => 64,
        }
    }
}

#[derive(Copy, Clone)]
pub struct TimeSignature(pub u64, pub Beat);

impl TimeSignature {
    #[allow(dead_code)]
    pub fn beats_per_measure(&self) -> u64 {
        self.0
    }

    pub fn beat_unit(&self) -> Beat {
        self.1
    }
}

#[derive(Copy, Clone)]
pub struct BeatCounter {
    bpm: u64,
    time_signature: TimeSignature,
}

impl BeatCounter {
    pub fn new(bpm: u64, time_signature: TimeSignature) -> Self {
        BeatCounter {
            bpm,
            time_signature,
        }
    }

    pub fn beats(&self, length: Beat) -> f64 {
        let beat_unit_divisor = self.time_signature.beat_unit().divisor();
        let length_divisor = length.divisor();
        beat_unit_divisor as f64 / length_divisor as f64
    }

    pub fn duration_in_millis(&self, length: Beat) -> f64 {
        let beats_per_second = 60.0 / self.bpm as f64;
        let ms_per_beat = beats_per_second * 1000.0;
        ms_per_beat * self.beats(length)
    }
}
