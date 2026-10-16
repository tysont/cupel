// ABOUTME: `Facts`, what one check may read: the facts its manifest declares, shared between checks, plus
// ABOUTME: the base side and range of a range scope. Facts are the only channel from producers to checks.
use std::collections::{BTreeSet, HashMap};

use serde_json::Value;

use crate::base::Range;
use crate::error::{Error, Result};
use crate::fact::{Fact, FactKind};
use crate::producer::{Produced, SharedFact};

/// The facts a check may read: exactly the kinds its manifest declares in `needs` and `wants`.
/// Facts are shared, so one production serves every check that declares the kind. Under a
/// range scope, a check that declares `Check::base_needs` also reads those kinds as they were
/// at the base commit, and the range itself (ADR-056).
pub struct Facts {
    declared: BTreeSet<FactKind>,
    values: HashMap<FactKind, SharedFact>,
    /// The same kinds for the other languages of the family, for a family-wide check
    /// (`Check::spans_family`, ADR-051), in language-ID order.
    siblings: HashMap<FactKind, Vec<SharedFact>>,
    base: HashMap<FactKind, SharedFact>,
    range: Option<Range>,
}

impl Facts {
    /// Facts declaring `kinds`, holding none yet.
    pub fn declaring(kinds: impl IntoIterator<Item = FactKind>) -> Self {
        Self {
            declared: kinds.into_iter().collect(),
            values: HashMap::new(),
            siblings: HashMap::new(),
            base: HashMap::new(),
            range: None,
        }
    }

    /// Adds a produced fact. Refuses kinds the manifest does not declare.
    pub fn insert<F: Fact>(&mut self, fact: F) -> Result<()> {
        self.insert_shared(&Produced::new(fact).share())
    }

    /// Adds a fact the scheduler produced once for several checks. Refuses kinds the manifest
    /// does not declare.
    pub fn insert_shared(&mut self, fact: &SharedFact) -> Result<()> {
        self.check_declared(fact.kind())?;
        self.values.insert(fact.kind(), fact.clone());
        Ok(())
    }

    /// Adds another family member's fact of a declared kind (ADR-051).
    pub fn insert_sibling<F: Fact>(&mut self, fact: F) -> Result<()> {
        self.insert_shared_sibling(&Produced::new(fact).share())
    }

    /// Adds another family member's fact the scheduler produced (ADR-051).
    pub fn insert_shared_sibling(&mut self, fact: &SharedFact) -> Result<()> {
        self.check_declared(fact.kind())?;
        self.siblings
            .entry(fact.kind())
            .or_default()
            .push(fact.clone());
        Ok(())
    }

    /// The fact of type `F` for this language, then each other family member's that was
    /// added (ADR-051). Errors as `get` does when this language's fact is missing.
    pub fn family<F: Fact>(&self) -> Result<Vec<&F>> {
        let own = self.get::<F>()?;
        let siblings = self
            .siblings
            .get(&F::KIND)
            .into_iter()
            .flatten()
            .filter_map(SharedFact::downcast_ref::<F>);
        Ok(std::iter::once(own).chain(siblings).collect())
    }

    /// Adds a fact produced from the base side of the range. Refuses kinds the manifest does
    /// not declare.
    pub fn insert_base_shared(&mut self, fact: &SharedFact) -> Result<()> {
        self.check_declared(fact.kind())?;
        self.base.insert(fact.kind(), fact.clone());
        Ok(())
    }

    /// Records the range the base side comes from.
    pub fn set_range(&mut self, range: Range) {
        self.range = Some(range);
    }

    /// The kinds the manifest declares, in catalog order.
    pub fn declared(&self) -> impl Iterator<Item = FactKind> + '_ {
        self.declared.iter().copied()
    }

    /// The fact of type `F`. Errors when `F` is undeclared or was not produced.
    pub fn get<F: Fact>(&self) -> Result<&F> {
        self.check_declared(F::KIND)?;
        self.values
            .get(&F::KIND)
            .and_then(SharedFact::downcast_ref::<F>)
            .ok_or(Error::AbsentFact { kind: F::KIND })
    }

    /// The fact of `kind` as it serializes, for a reader that does not know its type (a check
    /// plugin, ADR-089 §3); `None` when `kind` is undeclared or was not produced.
    pub fn json(&self, kind: FactKind) -> Option<Value> {
        self.values.get(&kind).map(SharedFact::to_json)
    }

    /// The fact of type `F` as it was at the range's base commit, over the changed files that
    /// existed there. Errors when `F` is undeclared or no base side was produced.
    pub fn base<F: Fact>(&self) -> Result<&F> {
        self.check_declared(F::KIND)?;
        self.base
            .get(&F::KIND)
            .and_then(SharedFact::downcast_ref::<F>)
            .ok_or(Error::AbsentFact { kind: F::KIND })
    }

    /// The range being compared; `None` outside a range scope.
    pub fn range(&self) -> Option<&Range> {
        self.range.as_ref()
    }

    fn check_declared(&self, kind: FactKind) -> Result<()> {
        if self.declared.contains(&kind) {
            Ok(())
        } else {
            Err(Error::UndeclaredFact { kind })
        }
    }
}
