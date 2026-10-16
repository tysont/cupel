// ABOUTME: Tests for the run controls threaded through checks and producers: the cooperative
// ABOUTME: `Cancellation` token behind `--timeout`, the `--events` JSON-lines sink, and the seed.
use std::io::{self, Write};
use std::time::{Duration, Instant};

use cupel_core::{
    Cancellation, CheckId, CheckStatus, Error, Event, EventSink, ExitCode, JsonlSink, RunId,
    Status, random_seed,
};

#[test]
fn a_fresh_token_is_not_cancelled_until_cancel_is_called_on_any_clone() {
    let token = Cancellation::new();
    let clone = token.clone();
    assert!(!token.is_cancelled());
    assert!(token.checkpoint().is_ok());
    clone.cancel();
    assert!(token.is_cancelled());
    assert_eq!(token.checkpoint(), Err(Error::Cancelled));
}

#[test]
fn a_deadline_cancels_once_it_passes() {
    let past = Cancellation::with_deadline(
        Instant::now()
            .checked_sub(Duration::from_millis(1))
            .unwrap(),
    );
    let future = Cancellation::with_deadline(Instant::now() + Duration::from_secs(3600));
    assert_eq!([past.is_cancelled(), future.is_cancelled()], [true, false]);
}

#[test]
fn wait_completes_when_nothing_cancels_it() {
    let token = Cancellation::new();
    // (completed, cancelled)
    assert_eq!(
        (token.wait(Duration::from_millis(5)), token.is_cancelled()),
        (true, false)
    );
}

#[test]
fn wait_returns_early_when_the_deadline_passes() {
    let started = Instant::now();
    let token = Cancellation::with_deadline(started + Duration::from_millis(20));
    // (completed, cancelled)
    assert_eq!(
        (token.wait(Duration::from_secs(30)), token.is_cancelled()),
        (false, true)
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn wait_returns_early_when_another_thread_cancels() {
    let token = Cancellation::new();
    let canceller = token.clone();
    let started = Instant::now();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        canceller.cancel();
    });
    // (completed, cancelled)
    assert_eq!(
        (token.wait(Duration::from_secs(30)), token.is_cancelled()),
        (false, true)
    );
    handle.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
}

/// Records what reaches it and how often it was flushed.
#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    flushes: usize,
}

impl Write for &mut Recorder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

fn events() -> Vec<Event> {
    vec![
        Event::Plan {
            run: RunId::generate(),
            checks: 1,
            est_ms: 900,
        },
        Event::CheckEnd {
            id: CheckId::from("static.crap"),
            status: CheckStatus::Pass,
            duration_ms: 12,
        },
        Event::RunEnd {
            status: Status::Pass,
            exit_code: ExitCode::Pass,
        },
    ]
}

/// Emits every one of `events()` to `sink`, in order.
fn emit_all(sink: &mut dyn EventSink) {
    for event in events() {
        sink.emit(event);
    }
}

#[test]
fn jsonl_sink_writes_one_flushed_line_per_event_in_order() {
    let mut recorder = Recorder::default();
    let mut sink = JsonlSink::new(&mut recorder);
    emit_all(&mut sink);
    assert_eq!(sink.error(), None);
    drop(sink);
    let text = String::from_utf8(recorder.bytes).unwrap();
    let parsed: Vec<Event> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(parsed.len(), 3);
    assert!(text.ends_with('\n') && !text.contains("\n\n"));
    assert!(matches!(
        parsed[0],
        Event::Plan {
            checks: 1,
            est_ms: 900,
            ..
        }
    ));
    assert_eq!(parsed[1..], events()[1..]);
    assert_eq!(
        recorder.flushes, 3,
        "each event is flushed as it is emitted"
    );
}

/// Accepts nothing.
struct Broken {
    writes: usize,
}

impl Write for &mut Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        Err(io::Error::other("disk full"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn jsonl_sink_stops_at_the_first_write_error_and_reports_it() {
    let mut broken = Broken { writes: 0 };
    let mut sink = JsonlSink::new(&mut broken);
    emit_all(&mut sink);
    assert_eq!(sink.error(), Some("disk full"));
    drop(sink);
    assert_eq!(broken.writes, 1);
}

#[test]
fn random_seeds_are_non_zero_and_differ() {
    let seeds: Vec<u64> = (0..8).map(|_| random_seed()).collect();
    assert!(seeds.iter().all(|seed| *seed != 0));
    let mut unique = seeds.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), seeds.len(), "{seeds:?}");
}

#[test]
fn a_token_under_another_ends_with_it_or_at_its_own_deadline_and_never_ends_it() {
    let parent = Cancellation::new();
    let open = parent.under(None);
    let past = parent.under(Some(Instant::now()));
    open.cancel();
    // (parent, open, past) after cancelling `open`, then a fresh one after cancelling the parent.
    let first = (
        parent.is_cancelled(),
        open.is_cancelled(),
        past.is_cancelled(),
    );
    let fresh = parent.under(None);
    parent.cancel();
    assert_eq!((first, fresh.is_cancelled()), ((false, true, true), true));
}
