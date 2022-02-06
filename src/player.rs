use cpal::traits::DeviceTrait;
use cpal::{Device, Sample, Stream, StreamConfig};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use waker_fn::waker_fn;

use crate::synth::AudioShape;
use crate::tracks::Tracks;

pub type PlayerProgram = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct Player {
    num_channels: u16,
    tracks: Tracks,
    program: PlayerProgram,
}

thread_local! {
    static CURRENT_TIME: RefCell<u64> = RefCell::new(0);
}

fn get_current_time() -> u64 {
    CURRENT_TIME.with(|value| *value.borrow())
}

struct Waiter {
    end: u64,
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

    pub fn wait(ms: u64) -> impl Future<Output = ()> {
        return Waiter {
            end: get_current_time() + ms,
        };
    }

    fn run_program(&mut self) {
        let waker = waker_fn(|| {});
        let mut context = Context::from_waker(&waker);
        match self.program.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => {
                return value;
            }
            std::task::Poll::Pending => {
                println!("PENDING");
            }
        }
    }

    fn write_audio<T: Sample>(&mut self, data: &mut [T], _: &cpal::OutputCallbackInfo) {
        // Try to update the tracks, but don't block, because we're
        // running in an extremely time-sensitive audio thread.
        self.tracks.try_to_update();

        // We use chunks_mut() to access individual channels:
        // https://github.com/RustAudio/cpal/blob/master/examples/beep.rs#L127
        for sample in data.chunks_mut(self.num_channels as usize) {
            let sample_value = Sample::from(&(self.tracks.next().unwrap() as f32));

            for channel_sample in sample.iter_mut() {
                *channel_sample = sample_value;
            }
        }

        println!("Running program.");
        self.run_program();
        CURRENT_TIME.with(|value| {
            *value.borrow_mut() += 1;
        });
    }
}
