use std::fs::File;

use counter::Counter;

/// The trace file `name` in the directory `CUPEL_TRACE_DIR` names, when it is set.
fn trace(name: &str) -> Option<File> {
    let dir = std::env::var("CUPEL_TRACE_DIR").ok()?;
    Some(File::create(std::path::Path::new(&dir).join(name)).unwrap())
}

#[test]
fn counts_then_finishes() {
    let mut counter = Counter::new(trace("counts.jsonl"));
    counter.inc();
    counter.inc();
    counter.finish();
    assert_eq!(counter.count(), 2);
}

#[test]
fn bumps() {
    let mut counter = Counter::new(trace("bumps.jsonl"));
    counter.inc();
    counter.bump();
    assert_eq!(counter.count(), 3);
}

#[test]
fn restarts() {
    let mut counter = Counter::starting_at(5, trace("restarts.jsonl"));
    counter.inc();
    assert_eq!(counter.count(), 6);
}

#[test]
fn tears() {
    if let Some(dir) = std::env::var_os("CUPEL_TRACE_DIR") {
        std::fs::write(std::path::Path::new(&dir).join("torn.jsonl"), "{\"state\": ").unwrap();
    }
}
