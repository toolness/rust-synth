use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, Sample, Stream, StreamConfig, StreamInstant};
use std::cell::{RefCell, RefMut};
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::task::Context;
use std::thread::sleep;
use std::time::Duration;

use crate::dummy_waker::dummy_waker;
use crate::synth::{AudioShape, AudioShapeSynthesizer};
use crate::synth_registry::SynthRegistry;
use crate::waiter::Waiter;

pub type PlayerProgram = Pin<Box<dyn Future<Output = ()> + Send>>;

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

pub struct Player {
    num_channels: u16,
    sample_rate: usize,
    programs: Vec<PlayerProgram>,
    latest_instant: Option<StreamInstant>,
    sender: SyncSender<()>,
    sent_finished_signal: bool,
}

thread_local! {
    static CURRENT_SAMPLE_RATE: RefCell<Option<usize>> = RefCell::new(None);
    static CURRENT_TIME: RefCell<f64> = RefCell::new(0.0);
    static CURRENT_SYNTHS: RefCell<SynthRegistry> = RefCell::new(SynthRegistry::new());
    static NEW_PROGRAMS: RefCell<Vec<PlayerProgram>> = RefCell::new(vec![]);
}

fn get_current_time() -> f64 {
    CURRENT_TIME.with(|value| *value.borrow())
}

pub struct AudioShapeProxy {
    id: usize,
}

impl AudioShapeProxy {
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

impl Player {
    pub fn get_stream<T: Sample>(
        device: Device,
        config: &StreamConfig,
        program: PlayerProgram,
    ) -> PlayerProxy {
        let (sender, receiver) = sync_channel(1);
        let mut player = Player {
            num_channels: config.channels,
            programs: vec![program],
            latest_instant: None,
            sample_rate: config.sample_rate.0 as usize,
            sender,
            sent_finished_signal: false,
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
        let sample_rate = CURRENT_SAMPLE_RATE.with(|value| value.borrow().unwrap());
        let id = CURRENT_SYNTHS.with(|registry| {
            let mut mut_registry = registry.borrow_mut();
            let synth = AudioShapeSynthesizer::new(shape, sample_rate);
            return mut_registry.insert(synth);
        });
        AudioShapeProxy { id }
    }

    pub fn start_program(program: PlayerProgram) {
        NEW_PROGRAMS.with(|programs| {
            programs.borrow_mut().push(program);
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

    fn increment_current_time(&mut self, amount: f64) {
        CURRENT_TIME.with(|value| {
            *value.borrow_mut() += amount;
        });
    }

    fn check_finished(&mut self, mut_registry: &mut RefMut<SynthRegistry>) {
        mut_registry.remove_finished_synths();

        if mut_registry.is_empty() && self.programs.len() == 0 && !self.sent_finished_signal {
            if let Ok(_) = self.sender.send(()) {
                self.sent_finished_signal = true;
            }
        }
    }

    fn write_audio<T: Sample>(&mut self, data: &mut [T], info: &cpal::OutputCallbackInfo) {
        let latest_instant = info.timestamp().playback;
        if let Some(previous_instant) = self.latest_instant {
            // Bizzarely, the latest instant can sometimes be *before* our
            // previous one, so we need to check here.
            if let Some(duration) = latest_instant.duration_since(&previous_instant) {
                self.increment_current_time(duration.as_millis() as f64);
            }
        } else {
            self.init_thread_locals();
        }
        self.latest_instant = Some(latest_instant);

        self.execute_programs();

        CURRENT_SYNTHS.with(|registry| {
            let mut mut_registry = registry.borrow_mut();

            // We use chunks_mut() to access individual channels:
            // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
            for sample in data.chunks_mut(self.num_channels as usize) {
                let value = mut_registry.next_sample();
                let sample_value = Sample::from(&(value as f32));

                for channel_sample in sample.iter_mut() {
                    *channel_sample = sample_value;
                }
            }

            self.check_finished(&mut mut_registry);
        });
    }
}
