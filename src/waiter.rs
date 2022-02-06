use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub struct Waiter<F: Fn() -> f64> {
    end: f64,
    get_current_time: F,
}

impl<F: Fn() -> f64> Waiter<F> {
    pub fn new(ms: f64, get_current_time: F) -> Self {
        Self {
            end: get_current_time() + ms,
            get_current_time,
        }
    }
}

impl<F: Fn() -> f64> Future for Waiter<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if (self.get_current_time)() >= self.end {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
