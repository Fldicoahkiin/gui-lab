//! Where a web module keeps its running application between the page's calls ([`Slot`]), so that a
//! panic in one call does not leave every later call failing.
//!
//! The web build has no unwinding: a panic traps the call it happens in, and nothing that call holds
//! is released. A `RefCell` borrowed for the call stays borrowed, and every later call fails with
//! "already borrowed". A slot holds no borrow: it marks a call as running, and when the next call finds
//! the mark still set after a panic ([`note_panic`], from the panic hook), the trapped call is over and
//! the application goes on from the state the panic left it in.
use std::cell::Cell;

thread_local! {
    static PANICKED: Cell<bool> = const { Cell::new(false) };
}

/// A panic happened (the panic hook calls this before the call traps).
pub fn note_panic() { PANICKED.with(|p| p.set(true)); }

/// One running application, kept for the page's calls.
pub struct Slot<H> {
    host: Cell<Option<Box<H>>>,
    /// The host's place while a call has it (a call that never returned left it here).
    lent: Cell<*mut H>,
}

/// What a call into the slot found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// The application ran the call.
    Ran,
    /// The call before this one panicked; this one ran after it, on the state it left.
    RanAfterPanic,
    /// Nothing has started, or a call is still running (the page called in from inside a call).
    Skipped,
}

impl<H> Default for Slot<H> { fn default() -> Self { Self::new() } }

impl<H> Slot<H> {
    /// An empty slot.
    pub const fn new() -> Self { Self { host: Cell::new(None), lent: Cell::new(std::ptr::null_mut()) } }
    /// Put the application in (the one before it, if any, is dropped).
    pub fn set(&self, h: H) {
        self.host.set(Some(Box::new(h)));
        self.lent.set(std::ptr::null_mut());
    }
    /// Run `f` on the application. A call that the previous call's panic left marked as running is
    /// taken to be over: its application is used again.
    pub fn call<R>(&self, f: impl FnOnce(&mut H) -> R) -> (Option<R>, Call) {
        let mut how = Call::Ran;
        let host = match self.host.take() {
            Some(h) => h,
            None => {
                let p = self.lent.get();
                if p.is_null() || !PANICKED.with(|x| x.replace(false)) { return (None, Call::Skipped); }
                how = Call::RanAfterPanic;
                // SAFETY: `p` came from `Box::into_raw` below, for a call that a panic trapped: on a
                // build without unwinding that call can never run again or drop anything, so nothing
                // else refers to the box. A build with unwinding gets here only from a call that
                // unwound, which ended every borrow it had.
                unsafe { Box::from_raw(p) }
            }
        };
        let raw = Box::into_raw(host);
        self.lent.set(raw);
        // SAFETY: `raw` is the box just let go of; the slot gives it to no one else while `f` runs (a
        // call from inside `f` finds the slot empty and is skipped).
        let r = f(unsafe { &mut *raw });
        // SAFETY: `f` returned, so the borrow above has ended; the box goes back to the slot.
        self.host.set(Some(unsafe { Box::from_raw(raw) }));
        self.lent.set(std::ptr::null_mut());
        (Some(r), how)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_after_a_panicking_one_runs_on_the_state_it_left() {
        let slot: Slot<Vec<u32>> = Slot::new();
        assert_eq!(slot.call(|v| v.len()), (None, Call::Skipped));
        slot.set(vec![1]);
        assert_eq!(slot.call(|v| { v.push(2); v.len() }), (Some(2), Call::Ran));
        // A handler panics part way through (here it unwinds; on the web it traps).
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| note_panic()));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| slot.call(|v| { v.push(3); panic!("in a handler") })));
        std::panic::set_hook(hook);
        assert!(r.is_err());
        // The next call runs, on the state the panic left, and the one after it runs as usual.
        assert_eq!(slot.call(|v| v.clone()), (Some(vec![1, 2, 3]), Call::RanAfterPanic));
        assert_eq!(slot.call(|v| v.len()), (Some(3), Call::Ran));
    }

    #[test]
    fn a_call_from_inside_a_call_is_skipped() {
        let slot: Slot<u32> = Slot::new();
        slot.set(7);
        let (r, how) = slot.call(|n| { *n += 1; slot.call(|m| *m).1 });
        assert_eq!((r, how), (Some(Call::Skipped), Call::Ran));
        assert_eq!(slot.call(|n| *n).0, Some(8));
    }
}
