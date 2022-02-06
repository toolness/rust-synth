// https://stackoverflow.com/a/63264582
use std::task::{RawWaker, RawWakerVTable, Waker};

static DUMMY_VTABLE: RawWakerVTable =
    RawWakerVTable::new(dummy_clone, dummy_wake, dummy_wake_by_ref, dummy_drop);

unsafe fn dummy_clone(ptr: *const ()) -> RawWaker {
    RawWaker::new(ptr, &DUMMY_VTABLE)
}

unsafe fn dummy_wake(_ptr: *const ()) {}

unsafe fn dummy_wake_by_ref(_ptr: *const ()) {}

unsafe fn dummy_drop(_ptr: *const ()) {}

pub fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(RawWaker::new(&(), &DUMMY_VTABLE)) }
}
