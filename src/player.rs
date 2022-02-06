use cpal::traits::DeviceTrait;
use cpal::{Device, Sample, Stream, StreamConfig, StreamInstant};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::dummy_waker::dummy_waker;
use crate::synth::{AudioShape, AudioShapeSynthesizer};
use crate::tracks::Tracks;

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
}

pub struct Player {
    num_channels: u16,
    sample_rate: usize,
    tracks: Tracks,
    program: PlayerProgram,
    latest_instant: Option<StreamInstant>,
}

thread_local! {
    static CURRENT_SAMPLE_RATE: RefCell<Option<usize>> = RefCell::new(None);
    static CURRENT_TIME: RefCell<f64> = RefCell::new(0.0);
    static CURRENT_SYNTHS: RefCell<SynthRegistry> = RefCell::new(SynthRegistry::new());
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

    pub fn set_volume(&mut self, volume: u8) {
        CURRENT_SYNTHS.with(|registry| {
            registry
                .borrow_mut()
                .map
                .entry(self.id)
                .and_modify(|synth| {
                    synth.update_target(AudioShape {
                        volume,
                        ..synth.get_target()
                    })
                });
        })
    }

    pub async fn finish(mut self) {
        self.set_volume(0);
        Player::wait(50.0).await;
    }
}

impl Drop for AudioShapeProxy {
    fn drop(&mut self) {
        CURRENT_SYNTHS.with(|registry| {
            registry.borrow_mut().map.remove(&self.id);
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
        shape_mutex: Arc<Mutex<[AudioShape]>>,
        program: PlayerProgram,
    ) -> Stream {
        let tracks = Tracks::new(shape_mutex, config.sample_rate.0 as usize);
        let mut player = Player {
            num_channels: config.channels,
            tracks,
            program,
            latest_instant: None,
            sample_rate: config.sample_rate.0 as usize,
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

    fn run_program(&mut self) {
        let waker = dummy_waker();
        let mut context = Context::from_waker(&waker);
        match self.program.as_mut().poll(&mut context) {
            std::task::Poll::Ready(_) => {}
            std::task::Poll::Pending => {}
        }
    }

    fn write_audio<T: Sample>(&mut self, data: &mut [T], info: &cpal::OutputCallbackInfo) {
        let latest_instant = info.timestamp().playback;
        if let Some(previous_instant) = self.latest_instant {
            CURRENT_TIME.with(|value| {
                *value.borrow_mut() += latest_instant
                    .duration_since(&previous_instant)
                    .unwrap()
                    .as_millis() as f64;
            });
        } else {
            CURRENT_SAMPLE_RATE.with(|value| {
                *value.borrow_mut() = Some(self.sample_rate);
            });
        }
        self.latest_instant = Some(latest_instant);

        self.run_program();

        // Try to update the tracks, but don't block, because we're
        // running in an extremely time-sensitive audio thread.
        self.tracks.try_to_update();

        CURRENT_SYNTHS.with(|registry| {
            let mut mut_registry = registry.borrow_mut();

            // We use chunks_mut() to access individual channels:
            // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
            for sample in data.chunks_mut(self.num_channels as usize) {
                let mut value = 0.0;
                for (_id, synth) in mut_registry.map.iter_mut() {
                    value += synth.next().unwrap();
                }
                value += self.tracks.next().unwrap();
                let sample_value = Sample::from(&(value as f32));

                for channel_sample in sample.iter_mut() {
                    *channel_sample = sample_value;
                }
            }
        });
    }
}
