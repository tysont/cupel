// ABOUTME: Builds findings about one symbol the way every native check reports them: stable ID from the
// ABOUTME: structural hash, location from the symbol span, and repro and suppress commands.
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
