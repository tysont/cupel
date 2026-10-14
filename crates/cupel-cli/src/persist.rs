// ABOUTME: The CLI's access to the fact store: opening it for verbs (failures are usage errors) and the
// ABOUTME: persistence hook that records every run and mirrors the suppressions file (ADR-045 §8, §10).
use cupel_core::{Envelope, Writer};
use cupel_store::{Store, StoreError};

use crate::error::CupelError;
use crate::suppress;

/// The subject and argument of a store failure in a verb.
const STORE: &str = ".cupel/cupel.db";

/// The store, opened for reading and writing.
pub fn open(writer: &Writer) -> Result<Store, CupelError> {
    Store::open(writer).map_err(|error| unusable(&error))
}

/// The store for reading: opened read-only by a dry run, which creates nothing, and normally
/// otherwise. `None` only for a dry run in a repository without a store.
pub fn open_for_reading(writer: &Writer) -> Result<Option<Store>, CupelError> {
    if writer.is_recording() {
        Store::open_existing(writer).map_err(|error| unusable(&error))
    } else {
        open(writer).map(Some)
    }
}

/// The usage error for a store that cannot be used.
pub fn unusable(error: &StoreError) -> CupelError {
    CupelError::bad_argument(
        STORE,
        None,
        format!("Cannot use the fact store: {error}"),
        "Run `cupel doctor`; .cupel/ must be a writable directory.".to_owned(),
    )
}

/// Records `envelope` in the store and mirrors `.cupel/suppressions.toml` into it. The error is
/// for one stderr line; it never changes the run's outcome (ADR-045 §8).
pub fn record(writer: &Writer, envelope: &Envelope) -> Result<(), String> {
    let store = Store::open(writer).map_err(|error| error.to_string())?;
    store
        .record_run(envelope)
        .map_err(|error| error.to_string())?;
    if let Ok(suppressions) = suppress::load(writer) {
        store
            .replace_suppressions(suppressions.entries())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
