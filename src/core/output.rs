//! Library output sink: the one place every library write goes through.
//!
//! casket is a library, but its scan, control and exploit helpers were written like
//! small programs: they print progress and results straight to stdout. That is what
//! forces a consumer to either duplicate the protocol work or re-parse text.
//!
//! This module keeps the default behaviour (text goes to stdout) and makes the text
//! interceptable: a consumer takes a [`Capture`], runs the operation, and reads the
//! text back. No call site changes its logic, no format string changes its meaning.
//!
//! ```
//! use casket::core::output;
//!
//! let cap = output::begin_capture();
//! output::emit_line("scanning 10.0.0.5");
//! let text = cap.take();
//! assert_eq!(text, "scanning 10.0.0.5\n");
//! ```
//!
//! Captures are process-wide and serialised: two concurrent captures would interleave
//! their text, so the second [`begin_capture`] waits for the first to finish. A
//! scanner probing one host at a time fits that model; a server fanning out to many
//! hosts at once should use the typed return values instead of the text.

use std::sync::{Mutex, MutexGuard, PoisonError};

static SLOT: Mutex<()> = Mutex::new(());
static BUFFER: Mutex<Option<String>> = Mutex::new(None);

fn slot() -> MutexGuard<'static, ()> {
    SLOT.lock().unwrap_or_else(PoisonError::into_inner)
}

fn buffer() -> MutexGuard<'static, Option<String>> {
    BUFFER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An active capture. Hold it for the duration of the operation, then [`take`](Capture::take).
///
/// While it lives, nothing else can start a capture (they wait), so one operation's text
/// never mixes with another's.
#[must_use = "a capture does nothing unless you take() the text back"]
pub struct Capture {
    _slot: MutexGuard<'static, ()>,
}

impl Capture {
    /// Stop capturing and return everything written since the capture began.
    pub fn take(self) -> String {
        buffer().take().unwrap_or_default()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // A capture abandoned without take() must not leave stale text behind for the
        // next consumer to inherit.
        *buffer() = None;
    }
}

/// Start capturing library output. The returned guard serialises other captures.
pub fn begin_capture() -> Capture {
    let guard = slot();
    *buffer() = Some(String::new());
    Capture { _slot: guard }
}

/// True while a capture is active.
#[must_use]
pub fn capturing() -> bool {
    buffer().is_some()
}

/// Write text with no trailing newline.
pub fn emit(text: &str) {
    let mut slot = buffer();
    let Some(buf) = slot.as_mut() else {
        drop(slot);
        print!("{text}");
        return;
    };
    buf.push_str(text);
}

/// Write one line.
pub fn emit_line(text: &str) {
    let mut slot = buffer();
    let Some(buf) = slot.as_mut() else {
        drop(slot);
        println!("{text}");
        return;
    };
    buf.push_str(text);
    buf.push('\n');
}

/// `println!`, but through the sink. Replace `println!` with this and nothing else.
#[macro_export]
macro_rules! casket_outln {
    ($($arg:tt)*) => {
        $crate::core::output::emit_line(&format!($($arg)*))
    };
}

/// `print!`, but through the sink.
#[macro_export]
macro_rules! casket_out {
    ($($arg:tt)*) => {
        $crate::core::output::emit(&format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_round_trip() {
        let cap = begin_capture();
        assert!(capturing());
        emit("no newline");
        emit_line(" and a line");
        assert_eq!(cap.take(), "no newline and a line\n");
        assert!(!capturing());
    }

    #[test]
    fn capture_starts_empty_and_stops_on_take() {
        let cap = begin_capture();
        assert_eq!(cap.take(), "");
        assert!(!capturing());
    }

    #[test]
    fn abandoned_capture_leaves_nothing_behind() {
        {
            let _cap = begin_capture();
            emit_line("dropped");
        }
        assert!(!capturing());
        let cap = begin_capture();
        emit_line("clean");
        assert_eq!(cap.take(), "clean\n");
    }
}
