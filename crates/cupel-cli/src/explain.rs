// ABOUTME: The `explain` verb (design §6.5): a stored finding with its source excerpt and test hints, or a
// ABOUTME: check's last stored result with its findings and metrics. Reads the fact store; never re-runs.
use std::path::Path;

use cupel_core::{
    CheckId, Envelope, Excerpt, ExcerptLine, ExitCode, ExplainSubject, Explanation, Finding,
    FindingId, Location, RunId, Status, Writer,
};
use serde_json::Value;

use crate::error::CupelError;
use crate::persist;
use crate::run::Run;

/// Lines of context shown above and below a location.
const CONTEXT: u32 = 3;

/// Explains `target`: a finding ID (`fnd_…`) or a check ID.
/// What `explain` adds to a finding: an excerpt of its location and the tests its action names.
fn finding_explanation(root: &Path, target: &str, finding: &Finding, run: RunId) -> Explanation {
    let hints = &finding.action.hints;
    Explanation {
        subject: ExplainSubject::Finding,
        target: target.to_owned(),
        run,
        excerpt: finding
            .location
            .as_ref()
            .and_then(|location| excerpt(root, location)),
        covering_tests: hints
            .get("covering_tests")
            .and_then(Value::as_array)
            .map(|tests| {
                tests
                    .iter()
                    .filter_map(|test| test.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        nearest_test_file: hints
            .get("nearest_test_file")
            .and_then(Value::as_str)
            .map(str::to_owned),
        metrics: Vec::new(),
    }
}

pub fn run(run: &Run, writer: &Writer, target: &str) -> Result<Envelope, CupelError> {
    let store = persist::open(writer)?;
    let unknown = |what: &str| {
        CupelError::bad_argument(
            "TARGET",
            Some(target),
            format!("No stored run produced {what} `{target}`"),
            "Explain a finding ID (fnd_…) or a check ID from a run Cupel recorded in this \
             repository; explain reads only the fact store and never runs anything."
                .to_owned(),
        )
    };
    let stored = |error: cupel_store::StoreError| persist::unusable(&error);
    if target.starts_with("fnd_") {
        let id: FindingId = target.parse().map_err(|_| unknown("the finding"))?;
        let (finding, from) = store
            .finding(&id)
            .map_err(stored)?
            .ok_or_else(|| unknown("the finding"))?;
        let explanation = finding_explanation(writer.root(), target, &finding, from);
        return Ok(explained(run, vec![finding], Vec::new(), explanation));
    }
    let check = CheckId::from(target);
    let last = store
        .last_run_for_check(&check)
        .map_err(stored)?
        .ok_or_else(|| unknown("the check"))?;
    let explanation = Explanation {
        subject: ExplainSubject::Check,
        target: target.to_owned(),
        run: last.run,
        excerpt: None,
        covering_tests: Vec::new(),
        nearest_test_file: None,
        metrics: last.metrics,
    };
    Ok(explained(
        run,
        last.findings,
        vec![last.result],
        explanation,
    ))
}

fn explained(
    run: &Run,
    findings: Vec<cupel_core::Finding>,
    checks: Vec<cupel_core::CheckResult>,
    explanation: Explanation,
) -> Envelope {
    let mut envelope = run.reported(Status::Pass, ExitCode::Pass, findings);
    envelope.checks = checks;
    envelope.explanation = Some(explanation);
    envelope
}

/// The lines of `location`'s file around it, when the file is inside `root` and has them.
fn excerpt(root: &Path, location: &Location) -> Option<Excerpt> {
    let path = root.join(&location.file).canonicalize().ok()?;
    if !path.starts_with(root) {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let last = u32::try_from(lines.len()).ok()?;
    let first = location.line.saturating_sub(CONTEXT).max(1);
    let end = location.end_line.saturating_add(CONTEXT).min(last);
    if first > end {
        return None;
    }
    Some(Excerpt {
        file: location.file.clone(),
        lines: (first..=end)
            .map(|line| ExcerptLine {
                line,
                text: lines[(line - 1) as usize].to_owned(),
                focus: (location.line..=location.end_line).contains(&line),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(line: u32, end_line: u32) -> Location {
        Location {
            file: "a.rs".to_owned(),
            line,
            col: None,
            end_line,
            end_col: None,
        }
    }

    /// A temporary root, and its canonical path, holding `a.rs` of `count` numbered lines.
    fn numbered(count: u32) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let text = (1..=count)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(root.join("a.rs"), text).unwrap();
        (dir, root)
    }

    fn shown(excerpt: &Excerpt) -> Vec<(u32, bool)> {
        excerpt.lines.iter().map(|l| (l.line, l.focus)).collect()
    }

    #[test]
    fn an_excerpt_stops_at_the_start_and_end_of_the_file() {
        let (_dir, root) = numbered(4);
        let excerpt = excerpt(&root, &location(1, 4)).unwrap();
        assert_eq!(
            shown(&excerpt),
            [(1, true), (2, true), (3, true), (4, true)]
        );
        assert_eq!(excerpt.lines[0].text, "line 1");
    }

    #[test]
    fn a_location_past_the_end_of_the_file_has_no_excerpt() {
        let (_dir, root) = numbered(4);
        assert_eq!(excerpt(&root, &location(9, 9)), None);
    }
}
