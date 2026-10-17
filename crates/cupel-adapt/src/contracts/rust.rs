// ABOUTME: `tool.rust-contracts` (ADR-113): the `contracts` crate's violations in a Rust test's panic, read from the
// ABOUTME: libtest output the baseline's runner keeps; the run turns on `debug_*` contracts through debug assertions.
use cupel_core::ContractKind;

use super::{Found, Library, Source};

/// The `contracts` crate: `#[requires]`, `#[ensures]`, `#[invariant]` and their `debug_` and
/// `test_` forms panic with `<Pre-condition|Post-condition|Invariant (as …)> of <function>
/// violated: <description>: <condition>` at the attribute.
pub static RUST_CONTRACTS: Library = Library {
    id: "tool.rust-contracts",
    lang: "rust",
    package: "contracts",
    manifests: &["Cargo.toml"],
    // `debug_*` contracts are `debug_assert!`s: on for every profile a test builds with.
    switches: &[
        ("CARGO_PROFILE_DEV_DEBUG_ASSERTIONS", "true"),
        ("CARGO_PROFILE_TEST_DEBUG_ASSERTIONS", "true"),
    ],
    read,
};

inventory::submit!(crate::ToolProducer {
    id: "tool.rust-contracts",
    register: |registry| super::register(registry, &RUST_CONTRACTS),
});

/// What each message prefix says the contract is.
const PREFIXES: [(&str, ContractKind); 4] = [
    ("Pre-condition of ", ContractKind::Precondition),
    ("Post-condition of ", ContractKind::Postcondition),
    ("Invariant (as pre-condition) of ", ContractKind::Invariant),
    ("Invariant (as post-condition) of ", ContractKind::Invariant),
];

/// Every contract panic in one failed test's `output`: a `thread '…' panicked at <file>:<line>:<col>:`
/// line followed by the crate's message. The panic's place is the contract's attribute.
fn read(output: &str, _: &Source<'_>) -> Vec<Found> {
    let lines: Vec<&str> = output.lines().collect();
    lines
        .iter()
        .zip(lines.iter().skip(1))
        .filter_map(|(panic, message)| {
            let (file, line) = panicked_at(panic)?;
            let (kind, rest) = PREFIXES
                .iter()
                .find_map(|(prefix, kind)| Some((*kind, message.strip_prefix(prefix)?)))?;
            let (function, said) = rest.split_once(" violated")?;
            Some(Found {
                kind,
                function: Some(function.to_owned()),
                message: said.trim_start_matches(':').trim().to_owned(),
                file: Some(file),
                line,
            })
        })
        .collect()
}

/// The file and line of a panic header, `thread 'name' (id) panicked at src/lib.rs:4:18:`.
fn panicked_at(line: &str) -> Option<(String, u32)> {
    let (_, at) = line.strip_prefix("thread '")?.split_once(" panicked at ")?;
    let at = at.strip_suffix(':')?;
    let mut parts = at.rsplitn(3, ':');
    let _column = parts.next()?;
    let line = parts.next()?.parse().ok()?;
    Some((parts.next()?.to_owned(), line))
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;
    use crate::report::libtest;

    /// Each failed test of the capture by name, with what the reader found in its output.
    fn captured() -> Vec<(String, Vec<Found>)> {
        let output = Capture::of("contracts/rust-contracts", "ledger").file("output.txt");
        let mut read: Vec<(String, Vec<Found>)> = libtest::parse(&output)
            .into_iter()
            .filter_map(|test| {
                let found = read(test.output.as_deref()?, &|_| None);
                Some((test.name, found))
            })
            .collect();
        read.sort_by(|a, b| a.0.cmp(&b.0));
        read
    }

    fn found(kind: ContractKind, function: &str, message: &str, line: u32) -> Vec<Found> {
        vec![Found {
            kind,
            function: Some(function.to_owned()),
            message: message.to_owned(),
            file: Some("src/lib.rs".to_owned()),
            line,
        }]
    }

    #[test]
    fn every_contract_panic_of_the_capture_is_read_with_its_kind_function_and_place() {
        assert_eq!(
            captured(),
            [
                (
                    "tests::halves_one".to_owned(),
                    found(
                        ContractKind::Postcondition,
                        "half",
                        "half of a positive amount is positive: ret > 0",
                        10
                    )
                ),
                (
                    "tests::posts_three_entries".to_owned(),
                    found(
                        ContractKind::Invariant,
                        "post",
                        "an account holds two entries: self.entries.len() <= 2",
                        25
                    )
                ),
                (
                    "tests::shares_nothing_among_no_parts".to_owned(),
                    found(
                        ContractKind::Precondition,
                        "share",
                        "a share needs at least one part: parts > 0",
                        4
                    )
                ),
            ]
        );
    }

    #[test]
    fn the_capture_exhibits_every_construct_of_the_conformance_inventory() {
        let found: Vec<Found> = captured()
            .into_iter()
            .flat_map(|(_, found)| found)
            .collect();
        assert_eq!(
            super::super::tests::missed_constructs(&found),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_panic_that_is_no_contract_and_a_contract_without_a_description_read_as_they_are() {
        let output = "thread 'tests::sums' panicked at src/lib.rs:9:5:\n\
                      assertion `left == right` failed\n\
                      thread 'tests::scales' panicked at src/lib.rs:3:1:\n\
                      Post-condition of scale violated: ret < 100\n";
        assert_eq!(
            read(output, &|_| None),
            [Found {
                kind: ContractKind::Postcondition,
                function: Some("scale".to_owned()),
                message: "ret < 100".to_owned(),
                file: Some("src/lib.rs".to_owned()),
                line: 3,
            }]
        );
    }
}
