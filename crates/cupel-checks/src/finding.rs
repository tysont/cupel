// ABOUTME: Builds findings the way every native check reports them: about one symbol (ID from its structural hash,
// ABOUTME: location from its span) or about another subject (ID from the subject), with repro and suppress commands.
use cupel_core::syntax::{Span, SymbolDef, TestConventions};
use cupel_core::{
    Action, ActionKind, CheckId, Evidence, Finding, FindingId, Location, Manifest, Severity,
    SiteHash, Symbol, SymbolHash, Tier,
};
use serde::Serialize;
use serde_json::Value;

/// What a check says about one symbol; [`SymbolFinding::finding`] adds the rest.
pub struct SymbolFinding<'a, E> {
    pub check: &'a CheckId,
    pub kind: &'static str,
    pub symbol: &'a SymbolDef,
    pub hash: &'a SymbolHash,
    pub severity: Severity,
    pub tier: Tier,
    pub title: String,
    pub evidence: E,
    pub action: ActionKind,
    pub summary: String,
}

impl<'a, E: Serialize> SymbolFinding<'a, E> {
    /// A `warn` finding of `kind` from the check `manifest` describes, about `symbol`, with the
    /// action the manifest maps `kind` to; `described` gives it a title and an action summary.
    pub fn new(
        manifest: &'a Manifest,
        kind: &'static str,
        symbol: &'a SymbolDef,
        hash: &'a SymbolHash,
        evidence: E,
    ) -> Self {
        Self {
            check: &manifest.id,
            kind,
            symbol,
            hash,
            severity: Severity::Warn,
            tier: manifest.tier,
            title: String::new(),
            evidence,
            action: manifest
                .findings
                .iter()
                .find(|spec| spec.kind == kind)
                .map_or(ActionKind::Review, |spec| spec.action),
            summary: String::new(),
        }
    }

    /// This finding with `title`, and `summary` saying what its action asks.
    pub fn described(self, title: String, summary: String) -> Self {
        Self {
            title,
            summary,
            ..self
        }
    }

    /// This finding at `severity`.
    pub fn at(self, severity: Severity) -> Self {
        Self { severity, ..self }
    }

    /// The finding, identified by `FindingId::compute(check, kind, hash, None)` and reproduced by
    /// running only this check on only this symbol.
    pub fn finding(self) -> Finding {
        let span = self.symbol.span;
        self.located(None, &span)
    }

    /// The finding about one site of the symbol, at `span` in the symbol's file, identified by
    /// `FindingId::compute(check, kind, hash, Some(site))`.
    pub fn at_site(self, site: &SiteHash, span: &Span) -> Finding {
        self.located(Some(site), span)
    }

    fn located(self, site: Option<&SiteHash>, span: &Span) -> Finding {
        let id = FindingId::compute(self.check, self.kind, self.hash, site);
        let evidence = match serde_json::to_value(&self.evidence) {
            Ok(Value::Object(fields)) => fields,
            _ => serde_json::Map::new(),
        };
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cupel check --only {} --symbols {}", self.check, self.hash),
            id,
            check: self.check.clone(),
            kind: self.kind.to_owned(),
            severity: self.severity,
            confidence: 1.0,
            tier: self.tier,
            symbol: Some(Symbol {
                display: self.symbol.display.clone(),
                hash: self.hash.clone(),
                lang: self.symbol.lang.clone(),
            }),
            location: Some(Location {
                file: self.symbol.file.clone(),
                line: span.line,
                col: Some(span.col),
                end_line: span.end_line,
                end_col: Some(span.end_col),
            }),
            title: self.title,
            detail: None,
            evidence: Evidence(evidence),
            action: Action {
                kind: self.action,
                priority: 2,
                summary: self.summary,
                hints: serde_json::Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// The location of `span` in `file`.
pub fn located(file: &str, span: &Span) -> Location {
    Location {
        file: file.to_owned(),
        line: span.line,
        col: Some(span.col),
        end_line: span.end_line,
        end_col: Some(span.end_col),
    }
}

/// `finding` about `symbol` with the action hint `nearest_test_file`: the first test file
/// `conventions` pairs with the symbol's file, when there is one.
pub fn with_nearest_test(
    mut finding: Finding,
    symbol: &SymbolDef,
    conventions: Option<&TestConventions>,
) -> Finding {
    if let Some(test) =
        conventions.and_then(|conventions| conventions.test_files_for(&symbol.file).first())
    {
        finding
            .action
            .hints
            .insert("nearest_test_file".to_owned(), Value::String(test.clone()));
    }
    finding
}

/// What a check says about a subject that is not one symbol: a report, a benchmark, a fuzz
/// target, a dependency. [`SubjectFinding::finding`] adds the rest.
pub struct SubjectFinding {
    pub kind: &'static str,
    /// What the finding is about, for its ID (`FindingId::for_subject`): stable across runs and
    /// line moves.
    pub subject: String,
    pub severity: Severity,
    pub confidence: f64,
    pub title: String,
    /// What the finding's action asks.
    pub summary: String,
    pub evidence: Value,
    pub location: Option<Location>,
    pub repro: String,
}

impl SubjectFinding {
    /// The finding of the check `manifest` describes, with the action the manifest maps its kind
    /// to, at priority 1 when it fails the check and 2 otherwise.
    pub fn finding(self, manifest: &Manifest) -> Finding {
        let id = FindingId::for_subject(&manifest.id, self.kind, &self.subject);
        let evidence = match self.evidence {
            Value::Object(fields) => fields,
            _ => serde_json::Map::new(),
        };
        let action = manifest
            .findings
            .iter()
            .find(|spec| spec.kind == self.kind)
            .map_or(ActionKind::Review, |spec| spec.action);
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: self.repro,
            id,
            check: manifest.id.clone(),
            kind: self.kind.to_owned(),
            severity: self.severity,
            confidence: self.confidence,
            tier: manifest.tier,
            symbol: None,
            location: self.location,
            title: self.title,
            detail: None,
            evidence: Evidence(evidence),
            action: Action {
                kind: action,
                priority: if self.severity == Severity::Fail {
                    1
                } else {
                    2
                },
                summary: self.summary,
                hints: serde_json::Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use cupel_core::{CostClass, FactKind, FindingKindSpec, Stability};
    use serde_json::json;

    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            id: CheckId::from("x.subject"),
            version: 1,
            tier: Tier::try_from(4).unwrap(),
            cost: CostClass::Deep,
            needs: vec![FactKind::FuzzResults],
            wants: vec![],
            findings: vec![FindingKindSpec {
                kind: "x.crash".to_owned(),
                action: ActionKind::FixBug,
                summary: String::new(),
                evidence: serde_json::Map::new(),
            }],
            metrics: vec![],
            settings: Vec::new(),
            stability: Stability::Experimental,
            summary: String::new(),
        }
    }

    fn said(kind: &'static str, severity: Severity) -> SubjectFinding {
        SubjectFinding {
            kind,
            subject: "fuzz/first".to_owned(),
            severity,
            confidence: 0.9,
            title: "first crashes".to_owned(),
            summary: "Fix it.".to_owned(),
            evidence: json!({"target": "first"}),
            location: Some(located(
                "src/lib.rs",
                &Span {
                    line: 3,
                    col: 5,
                    end_line: 3,
                    end_col: 12,
                },
            )),
            repro: "cupel check --only x.subject".to_owned(),
        }
    }

    #[test]
    fn a_subject_finding_is_identified_by_its_subject_and_acts_as_the_manifest_says() {
        let manifest = manifest();
        let finding = said("x.crash", Severity::Fail).finding(&manifest);
        let id = FindingId::for_subject(&manifest.id, "x.crash", "fuzz/first");
        assert_eq!(
            serde_json::to_value(&finding).unwrap(),
            json!({
                "id": id.to_string(),
                "check": "x.subject",
                "kind": "x.crash",
                "severity": "fail",
                "confidence": 0.9,
                "tier": 4,
                "symbol": null,
                "location": {"file": "src/lib.rs", "line": 3, "col": 5, "end_line": 3, "end_col": 12},
                "title": "first crashes",
                "detail": null,
                "evidence": {"target": "first"},
                "action": {"kind": "fix_bug", "priority": 1, "summary": "Fix it.", "hints": {}},
                "repro": "cupel check --only x.subject",
                "suppress": format!("cupel suppress {id} --reason '…'"),
                "new_since_baseline": false,
                "suppressed": false,
            })
        );
    }

    #[test]
    fn a_kind_outside_the_manifest_is_reviewed_and_a_warning_is_priority_two() {
        let finding = said("x.other", Severity::Warn).finding(&manifest());
        assert_eq!(
            (finding.action.kind, finding.action.priority),
            (ActionKind::Review, 2)
        );
    }
}
