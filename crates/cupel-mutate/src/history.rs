// ABOUTME: What earlier runs left in the store (ADR-080 §10–11): outcomes reused by cache key, and the order the rest
// ABOUTME: run in — last run's survivors, then mutants of symbols never run, then the others; files kept together.
use std::collections::{BTreeMap, BTreeSet};

use cupel_core::{Mutant, MutantOutcome, Writer};
use cupel_store::{MutantSite, Store};

use crate::plan::Planned;

/// The outcomes the store holds for a plan.
#[derive(Debug, Default)]
pub struct History {
    /// By cache key, for the plan's covered mutants.
    stored: BTreeMap<String, Mutant>,
    /// The last outcome of every site and replacement.
    last: BTreeMap<MutantSite, MutantOutcome>,
}

impl History {
    /// The history of `planned` in the store of `writer`'s repository; empty without a store or
    /// when it cannot be read, so every mutant runs.
    pub fn load(writer: &Writer, planned: &[Planned]) -> Self {
        let Ok(Some(store)) = Store::open_existing(writer) else {
            return Self::default();
        };
        let keys: Vec<String> = planned
            .iter()
            .filter_map(|planned| planned.mutant.cache_key.clone())
            .collect();
        Self {
            stored: store.mutants_by_key(&keys).unwrap_or_default(),
            last: store.last_mutant_outcomes().unwrap_or_default(),
        }
    }

    /// Fills `planned`'s outcome from the store when its key is there; whether it was.
    pub fn reuse(&self, planned: &mut Planned) -> bool {
        let Some(stored) = planned
            .mutant
            .cache_key
            .as_ref()
            .and_then(|key| self.stored.get(key))
        else {
            return false;
        };
        let mutant = &mut planned.mutant;
        mutant.outcome = stored.outcome;
        mutant.tests.clone_from(&stored.tests);
        mutant.killing_tests.clone_from(&stored.killing_tests);
        mutant.duration_ms = stored.duration_ms;
        mutant.reused = true;
        true
    }

    /// The indexes of `to_run` (mutants of `planned`) grouped by file, in the order they run:
    /// files by their first mutant, mutants by priority — last run's survivors, then those of
    /// symbols with no stored outcome, then the rest — then by position.
    pub fn order(&self, planned: &[Planned], to_run: &[usize]) -> Vec<(String, Vec<usize>)> {
        let symbols: BTreeSet<&str> = self
            .last
            .keys()
            .map(|site| site.symbol_hash.as_str())
            .collect();
        let priority = |index: &usize| {
            let mutant = &planned[*index].mutant;
            let hash = mutant.symbol_hash.as_ref().map(ToString::to_string);
            let site = hash.as_ref().map(|hash| MutantSite {
                symbol_hash: hash.clone(),
                site_hash: mutant.site_hash.to_string(),
                replacement: mutant.to.clone(),
            });
            let survived = site.and_then(|site| self.last.get(&site).copied())
                == Some(MutantOutcome::Survived);
            let unseen = hash.is_none_or(|hash| !symbols.contains(hash.as_str()));
            match (survived, unseen) {
                (true, _) => 0,
                (false, true) => 1,
                (false, false) => 2,
            }
        };
        let mut ordered = to_run.to_vec();
        ordered.sort_by_key(|index| priority(index));
        let mut files: Vec<(String, Vec<usize>)> = Vec::new();
        for index in ordered {
            let file = planned[index].file();
            match files.iter_mut().find(|(seen, _)| seen == file) {
                Some((_, indexes)) => indexes.push(index),
                None => files.push((file.to_owned(), vec![index])),
            }
        }
        files
    }
}
