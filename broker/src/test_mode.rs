//! Marks only the in-process broker thread as a test broker.

use std::cell::Cell;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn run(body: impl FnOnce()) {
    ACTIVE.with(|state| state.set(true));
    body();
    ACTIVE.with(|state| state.set(false));
}

pub(crate) fn is_active() -> bool {
    cfg!(test) || ACTIVE.with(Cell::get)
}
