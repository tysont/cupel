// ABOUTME: Renders an envelope for stdout in each built format: pretty JSON, JSON lines, and text.
// ABOUTME: Every renderer takes the already-paged envelope; SARIF (P6.1) is added beside them here.
mod text;

#[cfg(test)]
pub(crate) mod fixture;

use cupel_core::Envelope;

use crate::format::Rendering;

/// The stdout text of `envelope` in `rendering`. `color` applies to text only.
pub fn render(envelope: &Envelope, rendering: Rendering, color: bool) -> String {
    match rendering {
        Rendering::Json => json(envelope),
        Rendering::Jsonl => jsonl(envelope),
        Rendering::Text => text::render(envelope, color),
    }
}

/// The envelope as pretty JSON in struct-field order, ending with a newline.
fn json(envelope: &Envelope) -> String {
    let mut out = serde_json::to_string_pretty(envelope).expect("envelopes serialize");
    out.push('\n');
    out
}

/// A header line holding the envelope with `"findings": []`, then one line per finding
/// (ADR-019).
fn jsonl(envelope: &Envelope) -> String {
    let header = Envelope {
        findings: Vec::new(),
        ..envelope.clone()
    };
    let mut out = serde_json::to_string(&header).expect("envelopes serialize");
    out.push('\n');
    for finding in &envelope.findings {
        out.push_str(&serde_json::to_string(finding).expect("findings serialize"));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde_json::Value;

    use super::*;

    #[test]
    fn json_is_the_pretty_envelope_in_field_order_with_a_trailing_newline() {
        let out = json(&fixture::envelope());
        assert!(out.ends_with("}\n") && !out.ends_with("\n\n"));
        let back: Envelope = serde_json::from_str(&out).expect("round-trips");
        assert_eq!(back, fixture::envelope());
        insta::assert_snapshot!(out);
    }

    #[test]
    fn jsonl_is_a_header_line_then_one_line_per_finding() {
        let out = jsonl(&fixture::envelope());
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 4, "{out}");
        assert!(out.ends_with('\n'));
        let header: Envelope = serde_json::from_str(lines[0]).expect("the header is an envelope");
        assert!(header.findings.is_empty());
        assert_eq!(header.total_findings, 3);
        insta::assert_snapshot!(out);
    }

    #[test]
    fn jsonl_of_an_envelope_without_findings_is_only_the_header() {
        let mut envelope = fixture::envelope();
        envelope.findings.clear();
        let out = jsonl(&envelope);
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn text_renders_every_field_but_evidence_bodies() {
        insta::assert_snapshot!(text::render(&fixture::envelope(), false));
    }

    #[test]
    fn text_lists_each_oracle_with_its_definition_state_and_verification() {
        let out = text::render(&fixture::envelope(), false);
        let block: Vec<&str> = out
            .lines()
            .skip_while(|line| !line.starts_with("oracles"))
            .take(3)
            .collect();
        assert_eq!(
            block,
            [
                "oracles  2",
                "  bc  command `bc -l <<< {input}` · inputs 1 · changed · recorded \
                 2026-10-09T08:00:00Z by run_01JA2B3C4D5E6F7G8H9JKMNPQR",
                "  pg  ref v1.2.0 · verified · recorded 2026-10-10T16:00:00Z by \
                 run_01JA2B3C4D5E6F7G8H9JKMNPQS",
            ]
        );
    }

    #[test]
    fn text_in_color_adds_only_escape_codes() {
        let plain = text::render(&fixture::envelope(), false);
        let colored = text::render(&fixture::envelope(), true);
        assert!(colored.contains("\u{1b}["), "color is on");
        assert!(!plain.contains('\u{1b}'), "color is off");
        assert_eq!(strip_escapes(&colored), plain);
        insta::assert_snapshot!(colored);
    }

    fn strip_escapes(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    /// Reassembles a JSON-lines rendering into one envelope value.
    fn reassemble(jsonl: &str) -> Value {
        let mut lines = jsonl.lines();
        let mut header: Value = serde_json::from_str(lines.next().expect("a header")).unwrap();
        let findings: Vec<Value> = lines.map(|l| serde_json::from_str(l).unwrap()).collect();
        header["findings"] = Value::Array(findings);
        header
    }

    fn arbitrary_envelope() -> impl Strategy<Value = Envelope> {
        let finding = (
            any::<u32>(),
            prop_oneof![
                Just(cupel_core::Severity::Fail),
                Just(cupel_core::Severity::Warn),
                Just(cupel_core::Severity::Info),
            ],
            "[a-z]{1,8}\\.[a-z]{1,8}",
            ".*",
            proptest::option::of(".*"),
        )
            .prop_map(|(n, severity, check, title, detail)| {
                let mut finding = fixture::finding(n % 1000, &check, "kind.x", severity);
                finding.title = title;
                finding.detail = detail;
                finding
            });
        (
            proptest::collection::vec(finding, 0..8),
            any::<bool>(),
            ".*",
        )
            .prop_map(|(findings, truncated, summary)| {
                let mut envelope = fixture::envelope();
                envelope.total_findings = findings.len() as u64 + u64::from(truncated);
                envelope.truncated = truncated;
                envelope.findings = findings;
                envelope.next_actions[0].summary = summary;
                envelope
            })
    }

    proptest! {
        #[test]
        fn jsonl_header_and_lines_reassemble_into_the_json_envelope(
            envelope in arbitrary_envelope()
        ) {
            let whole: Value = serde_json::from_str(&json(&envelope)).unwrap();
            let lines = jsonl(&envelope);
            prop_assert_eq!(lines.lines().count(), envelope.findings.len() + 1);
            prop_assert_eq!(reassemble(&lines), whole);
        }
    }
}
