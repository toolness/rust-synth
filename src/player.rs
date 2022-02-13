use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, Sample, Stream, StreamConfig};
use hound;
use std::cell::{RefCell, RefMut};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::task::Context;
use std::thread::sleep;
use std::time::Duration;

use crate::dummy_waker::dummy_waker;
use crate::synth::{AudioShape, AudioShapeSynthesizer};
use crate::synth_registry::SynthRegistry;
use crate::waiter::Waiter;

type PinnedPlayerProgram = Pin<Box<dyn Future<Output = ()> + Send>>;

pub const WAV_CHANNELS: u16 = 1;

pub const WAV_SAMPLE_RATE: u32 = 44100;

thread_local! {
    static CURRENT_SAMPLE_RATE: RefCell<Option<usize>> = RefCell::new(None);
    static CURRENT_TIME: RefCell<f64> = RefCell::new(0.0);
    static CURRENT_SYNTHS: RefCell<SynthRegistry> = RefCell::new(SynthRegistry::new());
    static NEW_PROGRAMS: RefCell<Vec<PinnedPlayerProgram>> = RefCell::new(vec![]);
}

pub trait PlayerProgram: Future<Output = ()> + Send + 'static {}

impl<P: Future<Output = ()> + Send + 'static> PlayerProgram for P {}

fn get_current_time() -> f64 {
    CURRENT_TIME.with(|value| *value.borrow())
}

pub struct PlayerProxy {
    stream: Stream,
    receiver: Receiver<()>,
}

impl PlayerProxy {
    fn wait_until_finished(&mut self) {
        self.receiver.recv().unwrap();
        // The audio thread has finished generating audio, but it may still
        // need to be played, so give a bit of time for that.
        sleep(Duration::from_millis(250));
    }

    pub fn play_until_finished(mut self) {
        self.stream.play().unwrap();
        self.wait_until_finished();
    }
}

pub struct AudioShapeProxy {
    id: usize,
}

impl AudioShapeProxy {
    fn new(shape: AudioShape) -> Self {
        let sample_rate = CURRENT_SAMPLE_RATE.with(|value| value.borrow().unwrap());
        let id = CURRENT_SYNTHS.with(|registry| {
            let mut mut_registry = registry.borrow_mut();
            let synth = AudioShapeSynthesizer::new(shape, sample_rate);
            return mut_registry.insert(synth);
        });
        AudioShapeProxy { id }
    }

    pub fn set_frequency(&mut self, frequency: f64) {
        CURRENT_SYNTHS.with(|registry| {
            registry.borrow_mut().modify(self.id, |synth| {
                synth.update_target(AudioShape {
                    frequency,
                    ..synth.get_target()
                })
            });
        })
    }

    pub fn set_volume(&mut self, volume: u8) {
        CURRENT_SYNTHS.with(|registry| {
            registry.borrow_mut().modify(self.id, |synth| {
                synth.update_target(AudioShape {
                    volume,
                    ..synth.get_target()
                })
            });
        })
    }
}

impl Clone for AudioShapeProxy {
    fn clone(&self) -> Self {
        let shape = CURRENT_SYNTHS
            .with(|registry| registry.borrow_mut().get_shape(&self.id))
            .unwrap();
        Self::new(shape)
    }
}

impl Drop for AudioShapeProxy {
    fn drop(&mut self) {
        CURRENT_SYNTHS.with(|registry| {
            registry.borrow_mut().modify(self.id, |synth| {
                synth.make_inactive();
            });
        })
    }
}

pub struct Player {
    num_channels: u16,
    sample_rate: usize,
    programs: Vec<PinnedPlayerProgram>,
    total_samples: usize,
    sender: Option<SyncSender<()>>,
    is_finished: bool,
}

impl Player {
    pub fn write_wav<F: AsRef<Path>, P: PlayerProgram>(filename: F, program: P) {
        let spec = hound::WavSpec {
            channels: WAV_CHANNELS,
            sample_rate: WAV_SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(filename, spec).unwrap();
        let mut player = Player {
            num_channels: spec.channels,
            programs: vec![Box::pin(program)],
            total_samples: 0,
            sample_rate: spec.sample_rate as usize,
            sender: None,
            is_finished: false,
        };
        player.write_wav_audio(&mut writer);
        writer.finalize().unwrap();
    }

    pub fn get_stream<T: Sample, P: PlayerProgram>(
        device: Device,
        config: &StreamConfig,
        program: P,
    ) -> PlayerProxy {
        let (sender, receiver) = sync_channel(1);
        let mut player = Player {
            num_channels: config.channels,
            programs: vec![Box::pin(program)],
            total_samples: 0,
            sample_rate: config.sample_rate.0 as usize,
            sender: Some(sender),
            is_finished: false,
        };
        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
        let stream = device
            .build_output_stream(
                config,
                move |data, cpal| player.write_audio::<T>(data, cpal),
                err_fn,
            )
            .unwrap();
        return PlayerProxy { stream, receiver };
    }

    pub fn wait(ms: f64) -> impl Future<Output = ()> {
        Waiter::new(ms, get_current_time)
    }

    pub fn new_shape(shape: AudioShape) -> AudioShapeProxy {
        AudioShapeProxy::new(shape)
    }

    pub fn start_program<P: Future<Output = ()> + Send + 'static>(program: P) {
        NEW_PROGRAMS.with(|programs| {
            programs.borrow_mut().push(Box::pin(program));
        });
    }

    fn process_new_programs(&mut self) {
        NEW_PROGRAMS.with(|programs| {
            let mut mut_programs = programs.borrow_mut();
            while let Some(program) = mut_programs.pop() {
                self.programs.push(program);
            }
        });
    }

    fn execute_programs(&mut self) {
        let waker = dummy_waker();
        let mut context = Context::from_waker(&waker);
        let mut i = 0;
        while i < self.programs.len() {
            let program = self.programs.get_mut(i).unwrap();
            match program.as_mut().poll(&mut context) {
                std::task::Poll::Ready(_) => {
                    self.programs.remove(i);
                }
                std::task::Poll::Pending => {
                    i += 1;
                }
            }
            self.process_new_programs();
        }
    }

    fn init_thread_locals(&mut self) {
        CURRENT_SAMPLE_RATE.with(|value| {
            *value.borrow_mut() = Some(self.sample_rate);
        });
        CURRENT_TIME.with(|value| {
            *value.borrow_mut() = 0.0;
        });
    }

    fn increment_total_samples(&mut self, amount: usize) {
        self.total_samples += amount;
        CURRENT_TIME.with(|value| {
            *value.borrow_mut() = (self.total_samples as f64 / self.sample_rate as f64) * 1000.0;
        });
    }

    fn check_finished(&mut self, mut_registry: &mut RefMut<SynthRegistry>) {
        mut_registry.remove_finished_synths();

        if mut_registry.is_empty() && self.programs.len() == 0 && !self.is_finished {
            if let Some(sender) = &self.sender {
                if let Ok(_) = sender.send(()) {
                    self.is_finished = true;
                }
            } else {
                self.is_finished = true;
            }
        }
    }

    fn generate_samples<F: FnOnce(&mut RefMut<SynthRegistry>)>(&mut self, f: F) {
        self.execute_programs();
        let mut num_samples = 0;

        CURRENT_SYNTHS.with(|registry| {
            let mut mut_registry = registry.borrow_mut();
            let start_samples = mut_registry.get_total_samples();

            f(&mut mut_registry);

            num_samples = mut_registry.get_total_samples() - start_samples;
            self.check_finished(&mut mut_registry);
        });

        self.increment_total_samples(num_samples);
    }

    fn samples_per_program_loop(&self) -> usize {
        let samples_per_ms = self.sample_rate / 1000;
        samples_per_ms / 2
    }

    fn write_wav_audio<W: std::io::Write + std::io::Seek>(
        &mut self,
        writer: &mut hound::WavWriter<W>,
    ) {
        assert_eq!(self.num_channels, 1);
        let num_samples = self.samples_per_program_loop();
        self.init_thread_locals();

        while !self.is_finished {
            self.generate_samples(|registry| {
                for _ in 0..num_samples {
                    let value = registry.next_sample();
                    writer.write_sample(value as f32).unwrap();
                }
            });
        }

        // Write about a quarter-second of silence.
        for _ in 0..(self.sample_rate / 4) {
            writer.write_sample(0.0).unwrap();
        }
    }

    fn write_audio<T: Sample>(&mut self, data: &mut [T], _info: &cpal::OutputCallbackInfo) {
        if self.total_samples == 0 {
            self.init_thread_locals();
        }

        let num_channels = self.num_channels as usize;
        for chunk in data.chunks_mut(self.samples_per_program_loop() * num_channels) {
            self.generate_samples(|registry| {
                // We use chunks_mut() to access individual channels:
                // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
                for sample in chunk.chunks_mut(num_channels) {
                    let value = registry.next_sample();
                    let sample_value = Sample::from(&(value as f32));

                    for channel_sample in sample.iter_mut() {
                        *channel_sample = sample_value;
                    }
                }
            });
        }
    }
}
