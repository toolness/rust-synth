use cpal::traits::DeviceTrait;
use cpal::{Device, Sample, Stream, StreamConfig, StreamInstant};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::task::{Context, Poll};
use std::thread::sleep;
use std::time::Duration;

use crate::dummy_waker::dummy_waker;
use crate::synth::{AudioShape, AudioShapeSynthesizer};

pub type PlayerProgram = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct SynthRegistry {
    latest_id: usize,
    map: HashMap<usize, AudioShapeSynthesizer>,
}

impl SynthRegistry {
    pub fn new() -> Self {
        Self {
            latest_id: 0,
            map: HashMap::new(),
        }
    }

    pub fn remove_finished_synths(&mut self) {
        self.map.retain(|_id, synth| {
            return !synth.has_finished_playing();
        });
    }
}

pub struct PlayerProxy {
    pub stream: Stream,
    receiver: Receiver<()>,
}

impl PlayerProxy {
    pub fn wait_until_finished(&mut self) {
        self.receiver.recv().unwrap();
        // The audio thread has finished generating audio, but it may still
        // need to be played, so give a bit of time for that.
        sleep(Duration::from_millis(250));
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
            registry
                .borrow_mut()
                .map
                .entry(self.id)
                .and_modify(|synth| {
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
            registry
                .borrow_mut()
                .map
                .entry(self.id)
                .and_modify(|synth| {
                    synth.make_inactive();
                });
        })
    }
}

struct Waiter {
    end: f64,
}

impl Future for Waiter {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if get_current_time() >= self.end {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
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
        return Waiter {
            end: get_current_time() + ms,
        };
    }

    pub fn new_shape(shape: AudioShape) -> AudioShapeProxy {
        let sample_rate = CURRENT_SAMPLE_RATE.with(|value| value.borrow().unwrap());
        let id = CURRENT_SYNTHS.with(|registry| {
            let mut mut_registry = registry.borrow_mut();
            let synth = AudioShapeSynthesizer::new(shape, sample_rate);
            mut_registry.latest_id += 1;
            let id = mut_registry.latest_id;
            mut_registry.map.insert(id, synth);
            return id;
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

    fn run_programs(&mut self) {
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

    fn write_audio<T: Sample>(&mut self, data: &mut [T], info: &cpal::OutputCallbackInfo) {
        let latest_instant = info.timestamp().playback;
        if let Some(previous_instant) = self.latest_instant {
            CURRENT_TIME.with(|value| {
                // Bizzarely, the latest instant can sometimes be *before* our
                // previous one, so we need to check here.
                if let Some(duration) = latest_instant.duration_since(&previous_instant) {
                    *value.borrow_mut() += duration.as_millis() as f64;
                }
            });
        } else {
            CURRENT_SAMPLE_RATE.with(|value| {
                *value.borrow_mut() = Some(self.sample_rate);
            });
        }
        self.latest_instant = Some(latest_instant);

        self.run_programs();

        CURRENT_SYNTHS.with(|registry| {
            let mut mut_registry = registry.borrow_mut();

            // We use chunks_mut() to access individual channels:
            // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
            for sample in data.chunks_mut(self.num_channels as usize) {
                let mut value = 0.0;
                for (_id, synth) in mut_registry.map.iter_mut() {
                    value += synth.next().unwrap();
                }
                let sample_value = Sample::from(&(value as f32));

                for channel_sample in sample.iter_mut() {
                    *channel_sample = sample_value;
                }
            }

            mut_registry.remove_finished_synths();

            if mut_registry.map.is_empty() && self.programs.len() == 0 && !self.sent_finished_signal
            {
                if let Ok(_) = self.sender.send(()) {
                    self.sent_finished_signal = true;
                }
            }
        });
    }
}
