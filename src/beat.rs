pub const THREE_FOUR: TimeSignature = TimeSignature(3, Beat::Quarter);
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

    pub fn sixty_fourth_beats(&self) -> u64 {
        64 / self.divisor()
    }
}

#[derive(Copy, Clone)]
pub struct TimeSignature(pub u64, pub Beat);

impl TimeSignature {
    pub fn beats_per_measure(&self) -> u64 {
        self.0
    }

    pub fn beat_unit(&self) -> Beat {
        self.1
    }
}

#[derive(Copy, Clone)]
pub struct BeatSettings {
    pub bpm: u64,
    pub time_signature: TimeSignature,
}

impl BeatSettings {
    pub fn new(bpm: u64, time_signature: TimeSignature) -> Self {
        Self {
            bpm,
            time_signature,
        }
    }

    fn beats_in_duration(&self, length: Beat) -> f64 {
        let beat_unit_divisor = self.time_signature.beat_unit().divisor();
        let length_divisor = length.divisor();
        beat_unit_divisor as f64 / length_divisor as f64
    }

    pub fn duration_in_millis(&self, length: Beat) -> f64 {
        let beats_per_second = 60.0 / self.bpm as f64;
        let ms_per_beat = beats_per_second * 1000.0;
        ms_per_beat * self.beats_in_duration(length)
    }

    pub fn measure_in_millis(&self) -> f64 {
        self.duration_in_millis(self.time_signature.beat_unit())
            * self.time_signature.beats_per_measure() as f64
    }
}

#[derive(Copy, Clone)]
pub struct BeatCounter {
    settings: BeatSettings,
    sixty_fourth_beats: u64,
}

impl BeatCounter {
    pub fn new(settings: BeatSettings) -> Self {
        BeatCounter {
            settings,
            sixty_fourth_beats: 0,
        }
    }

    /// Increment the counter by the given length, returning the
    /// length's duration in milliseconds.
    pub fn increment(&mut self, length: Beat) -> f64 {
        self.sixty_fourth_beats += length.sixty_fourth_beats();
        self.settings.duration_in_millis(length)
    }

    pub fn total_beats(&self) -> f64 {
        self.sixty_fourth_beats as f64
            / self
                .settings
                .time_signature
                .beat_unit()
                .sixty_fourth_beats() as f64
    }

    pub fn total_measures(&self) -> f64 {
        self.total_beats() / self.settings.time_signature.beats_per_measure() as f64
    }

    pub fn total_millis(&self) -> f64 {
        self.total_measures() * self.settings.measure_in_millis()
    }
}

#[cfg(test)]
mod tests {
    use crate::beat::Beat;

    use super::{BeatCounter, BeatSettings, FOUR_FOUR};

    #[test]
    fn test_beat_settings_works() {
        let bs = BeatSettings::new(60, FOUR_FOUR);
        assert_eq!(bs.beats_in_duration(Beat::Quarter), 1.0);
        assert_eq!(bs.beats_in_duration(Beat::Half), 2.0);
        assert_eq!(bs.beats_in_duration(Beat::Eighth), 0.5);
        assert_eq!(bs.measure_in_millis(), 4000.0);
    }

    #[test]
    fn test_beat_counter_works() {
        let bs = BeatSettings::new(60, FOUR_FOUR);
        let mut bc = BeatCounter::new(bs);
        assert_eq!(bc.total_millis(), 0.0);
        assert_eq!(bc.increment(Beat::Quarter), 1000.0);
        assert_eq!(bc.total_measures(), 0.25);
        assert_eq!(bc.total_millis(), 1000.0);

        bc.increment(Beat::Half);
        bc.increment(Beat::Eighth);
        bc.increment(Beat::Eighth);

        assert_eq!(bc.total_measures(), 1.0);
        assert_eq!(bc.total_millis(), 4000.0);
    }
}
