// ABOUTME: Builds findings about one symbol the way every native check reports them: stable ID from the
// ABOUTME: structural hash, location from the symbol span, and repro and suppress commands.
use cupel_core::syntax::SymbolDef;
use cupel_core::{
    Action, ActionKind, CheckId, Evidence, Finding, FindingId, Location, Severity, Symbol,
    SymbolHash, Tier,
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

impl<E: Serialize> SymbolFinding<'_, E> {
    /// The finding, identified by `FindingId::compute(check, kind, hash, None)` and reproduced by
    /// running only this check on only this symbol.
    pub fn finding(self) -> Finding {
        let id = FindingId::compute(self.check, self.kind, self.hash, None);
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
                line: self.symbol.span.line,
                col: Some(self.symbol.span.col),
                end_line: self.symbol.span.end_line,
                end_col: Some(self.symbol.span.end_col),
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
