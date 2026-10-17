// ABOUTME: What every race detector's reader yields before normalizing, and the normalizing (ADR-105 §1): frames
// ABOUTME: kept only inside the repository, hash suffixes and IDs masked, accesses ordered, the scope applied.
use std::path::Path;

use cupel_core::{RaceFrame, RaceKind, RaceReport, RaceStack, Scope};

use crate::tree::relative;

/// A report as a detector printed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    pub kind: RaceKind,
    pub test: Option<String>,
    pub message: String,
    pub stacks: Vec<RawStack>,
}

/// A stack as a detector printed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawStack {
    pub access: Option<String>,
    pub frames: Vec<RawFrame>,
}

/// A frame as a detector printed it: its function, when named, and its file (absolute, or
/// relative to the directory the detector ran in) and line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    pub function: Option<String>,
    pub file: String,
    pub line: u32,
}

/// What a detector's run in one project root said.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Read {
    pub reports: Vec<Raw>,
    /// Why the run did not complete, when it did not.
    pub failed: Option<String>,
}

/// Where a detector ran: the repository root and the project root's directory, both canonical.
pub struct Paths<'a> {
    pub root: &'a Path,
    pub dir: &'a Path,
}

impl Raw {
    /// The report as the fact holds it: frames outside the repository dropped, functions
    /// without hash suffixes, the message masked, and the stacks ordered by their first frame.
    pub fn normalized(self, paths: &Paths<'_>) -> RaceReport {
        let mut stacks: Vec<RaceStack> = self
            .stacks
            .into_iter()
            .map(|stack| RaceStack {
                access: stack.access,
                frames: stack
                    .frames
                    .into_iter()
                    .filter_map(|frame| {
                        Some(RaceFrame {
                            file: relative(paths.root, paths.dir, &frame.file)?,
                            function: frame.function.map(|function| unhashed(&function)),
                            line: frame.line,
                        })
                    })
                    .collect(),
            })
            .collect();
        stacks.sort_by(|a, b| order(a).cmp(&order(b)));
        RaceReport {
            kind: self.kind,
            test: self.test,
            message: masked(&self.message),
            stacks,
        }
    }
}

/// What orders a report's stacks: its first frame's place, then its access.
fn order(stack: &RaceStack) -> (Option<(&str, u32)>, Option<&str>) {
    (
        stack
            .frames
            .first()
            .map(|frame| (frame.file.as_str(), frame.line)),
        stack.access.as_deref(),
    )
}

/// Whether `report` touches the scope: a frame in one of its files, or no frame to place it by.
/// An empty scope holds everything.
pub fn in_scope(report: &RaceReport, scope: &Scope) -> bool {
    let mut files = report.stacks.iter().flat_map(|stack| &stack.frames);
    scope.files.is_empty()
        || report.stacks.iter().all(|stack| stack.frames.is_empty())
        || files.any(|frame| scope.files.contains(&frame.file))
}

/// `function` without the `::h<16 hex digits>` suffix rustc's legacy mangling leaves.
fn unhashed(function: &str) -> String {
    match function.rsplit_once("::h") {
        Some((name, hash)) if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) => {
            name.to_owned()
        }
        _ => function.to_owned(),
    }
}

/// `message` with what differs from run to run masked: allocation IDs (`alloc1234` →
/// `alloc`), addresses of eight or more hex digits (`0x…`), and a process ID (`(pid=12)`).
pub fn masked(message: &str) -> String {
    let message = match message.find(" (pid=") {
        Some(at) if message.ends_with(')') => &message[..at],
        _ => message,
    };
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(next) = rest.chars().next() {
        if let Some(tail) = rest.strip_prefix("alloc") {
            out.push_str("alloc");
            rest = tail.trim_start_matches(|c: char| c.is_ascii_digit());
        } else if let Some(tail) = rest.strip_prefix("0x") {
            let digits = tail.len()
                - tail
                    .trim_start_matches(|c: char| c.is_ascii_hexdigit())
                    .len();
            out.push_str(if digits >= 8 {
                "0x…"
            } else {
                &rest[..2 + digits]
            });
            rest = &tail[digits..];
        } else {
            out.push(next);
            rest = &rest[next.len_utf8()..];
        }
    }
    out
}

/// The text after `prefix` on `line` once its leading whitespace is gone.
pub fn after<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    line.trim_start().strip_prefix(prefix)
}

/// `file:line` (or `file:line:col`, or anything after the line number) split into the file
/// and the line.
pub fn place(text: &str) -> Option<(String, u32)> {
    let mut parts = text.trim().splitn(3, ':');
    let file = parts.next()?;
    let line = parts.next()?;
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    Some((file.to_owned(), digits.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_frame(function: &str, file: &str, line: u32) -> RawFrame {
        RawFrame {
            function: Some(function.to_owned()),
            file: file.to_owned(),
            line,
        }
    }

    #[test]
    fn frames_outside_the_repository_go_and_stacks_are_ordered_by_their_first_frame() {
        let raw = Raw {
            kind: RaceKind::DataRace,
            test: Some("TestPublish".to_owned()),
            message: "data race".to_owned(),
            stacks: vec![
                RawStack {
                    access: Some("read".to_owned()),
                    frames: vec![
                        raw_frame("tally.Publish", "/repo/tally.go", 13),
                        raw_frame("testing.tRunner", "/usr/go/src/testing/testing.go", 1690),
                    ],
                },
                RawStack {
                    access: Some("write".to_owned()),
                    frames: vec![raw_frame(
                        "tally::publish::h0123456789abcdef",
                        "tally.go",
                        10,
                    )],
                },
            ],
        };
        let paths = Paths {
            root: Path::new("/repo"),
            dir: Path::new("/repo"),
        };
        let report = raw.normalized(&paths);
        let frames: Vec<(Option<&str>, Option<&str>, &str, u32)> = report
            .stacks
            .iter()
            .flat_map(|stack| {
                stack.frames.iter().map(|frame| {
                    (
                        stack.access.as_deref(),
                        frame.function.as_deref(),
                        frame.file.as_str(),
                        frame.line,
                    )
                })
            })
            .collect();
        assert_eq!(
            frames,
            [
                (Some("write"), Some("tally::publish"), "tally.go", 10),
                (Some("read"), Some("tally.Publish"), "tally.go", 13),
            ]
        );
    }

    #[test]
    fn ids_addresses_and_process_ids_are_masked_but_short_offsets_stay() {
        assert_eq!(
            masked("got alloc16+0x4 at alloc39912 and 0x00c0001121b8"),
            "got alloc+0x4 at alloc and 0x…"
        );
        assert_eq!(masked("data race (pid=83925)"), "data race");
        assert_eq!(masked("allocation of size 4"), "allocation of size 4");
    }

    #[test]
    fn a_report_is_in_scope_by_any_frame_or_when_it_has_none() {
        let report = |file: Option<&str>| RaceReport {
            kind: RaceKind::DataRace,
            test: None,
            message: "data race".to_owned(),
            stacks: vec![RaceStack {
                access: None,
                frames: file
                    .map(|file| RaceFrame {
                        function: None,
                        file: file.to_owned(),
                        line: 1,
                    })
                    .into_iter()
                    .collect(),
            }],
        };
        let scope = Scope::of_files(vec!["a.go".to_owned()]);
        assert!(in_scope(&report(Some("a.go")), &scope));
        assert!(!in_scope(&report(Some("b.go")), &scope));
        assert!(in_scope(&report(None), &scope));
        assert!(in_scope(&report(Some("b.go")), &Scope::default()));
    }

    #[test]
    fn a_place_is_a_file_and_a_line_with_anything_after() {
        assert_eq!(place("src/lib.rs:9:49"), Some(("src/lib.rs".to_owned(), 9)));
        assert_eq!(
            place("src/lib.rs:26:14: 26:29"),
            Some(("src/lib.rs".to_owned(), 26))
        );
        assert_eq!(
            place("/a/tally.go:10 +0x3c"),
            Some(("/a/tally.go".to_owned(), 10))
        );
        assert_eq!(place("<null>"), None);
    }
}
