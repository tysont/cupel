// ABOUTME: The `mutants` index (design §8.8, ADR-080 §10): each run's mutants from its `MutationOutcomes`, found
// ABOUTME: again by cache key (differential reruns) and by site and replacement (the last run's survivors first).
use std::collections::BTreeMap;

use cupel_core::{LangId, Mutant, MutantOutcome, MutationOutcomes, RunId};
use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

/// A mutant's place across runs: its symbol's structural hash, its site, and its replacement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MutantSite {
    pub symbol_hash: String,
    pub site_hash: String,
    pub replacement: String,
}

impl Store {
    /// Indexes every mutant of `outcomes`, which `run` produced for `lang`, in `mutants`.
    pub fn record_mutants(
        &self,
        run: &RunId,
        lang: &LangId,
        outcomes: &MutationOutcomes,
    ) -> Result<()> {
        let run = run.to_string();
        self.write(|tx| {
            for mutant in &outcomes.mutants {
                tx.execute(
                    "INSERT INTO mutants (run_id, lang, engine, cache_key, symbol_hash, site_hash,
                        replacement, outcome, duration_ms, mutant)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        run,
                        lang.as_str(),
                        outcomes.engine.as_str(),
                        mutant.cache_key,
                        mutant.symbol_hash.as_ref().map(ToString::to_string),
                        mutant.site_hash.as_str(),
                        mutant.to,
                        mutant.outcome.as_str(),
                        mutant
                            .duration_ms
                            .map(|ms| i64::try_from(ms).unwrap_or(i64::MAX)),
                        serde_json::to_string(mutant)?,
                    ],
                )?;
            }
            Ok(())
        })
    }

    /// The last mutant stored under each of `keys` that has one.
    pub fn mutants_by_key(&self, keys: &[String]) -> Result<BTreeMap<String, Mutant>> {
        let mut statement = self.connection.prepare(
            "SELECT mutant FROM mutants WHERE cache_key = ?1 ORDER BY rowid DESC LIMIT 1",
        )?;
        let mut found = BTreeMap::new();
        for key in keys {
            let stored: Option<String> = statement.query_row([key], |row| row.get(0)).optional()?;
            if let Some(stored) = stored {
                found.insert(key.clone(), serde_json::from_str(&stored)?);
            }
        }
        Ok(found)
    }

    /// The last stored outcome of every site and replacement whose symbol had a structural hash.
    pub fn last_mutant_outcomes(&self) -> Result<BTreeMap<MutantSite, MutantOutcome>> {
        let mut statement = self.connection.prepare(
            "SELECT symbol_hash, site_hash, replacement, outcome FROM mutants
             WHERE symbol_hash IS NOT NULL ORDER BY rowid",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                MutantSite {
                    symbol_hash: row.get(0)?,
                    site_hash: row.get(1)?,
                    replacement: row.get(2)?,
                },
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut last = BTreeMap::new();
        for row in rows {
            let (site, outcome) = row?;
            let outcome = serde_json::from_value(serde_json::Value::String(outcome))?;
            last.insert(site, outcome);
        }
        Ok(last)
    }
}
