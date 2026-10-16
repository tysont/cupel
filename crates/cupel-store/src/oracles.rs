// ABOUTME: Oracle verifications in the store: what `oracle verify` recorded for each verified oracle, by
// ABOUTME: name, with the identity hash a later run compares against (ADR-090 §6).
use cupel_core::RunId;
use cupel_core::config::OracleConfig;
use rusqlite::{OptionalExtension, Row, params};

use crate::{Result, Store, parse_run};

/// One verified oracle: the definition verified, when, its identity hash, and the run that
/// recorded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedOracle {
    pub name: String,
    pub definition: OracleConfig,
    /// RFC 3339, the verifying run's start.
    pub verified_at: String,
    /// The oracle's identity hash when it was verified (ADR-090 §4).
    pub hash: String,
    pub run: RunId,
}

const COLUMNS: &str = "name, spec, verified_at, verified_hash, run_id";

impl Store {
    /// Records `oracle` as verified, replacing an earlier verification of the same name.
    pub fn record_oracle(&self, oracle: &VerifiedOracle) -> Result<()> {
        let spec = serde_json::to_string(&oracle.definition)?;
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO oracles (name, kind, spec, verified_at, verified_hash, run_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    oracle.name,
                    oracle.definition.kind.as_str(),
                    spec,
                    oracle.verified_at,
                    oracle.hash,
                    oracle.run.to_string(),
                ],
            )?;
            Ok(())
        })
    }

    /// The verification of the oracle `name`, if it has one.
    pub fn verified_oracle(&self, name: &str) -> Result<Option<VerifiedOracle>> {
        let query = format!("SELECT {COLUMNS} FROM oracles WHERE name = ?1");
        self.connection
            .query_row(&query, [name], read_row)
            .optional()?
            .map(verification)
            .transpose()
    }

    /// Every verification, by name.
    pub fn verified_oracles(&self) -> Result<Vec<VerifiedOracle>> {
        let query = format!("SELECT {COLUMNS} FROM oracles ORDER BY name");
        let mut statement = self.connection.prepare(&query)?;
        statement
            .query_map([], read_row)?
            .map(|row| verification(row?))
            .collect()
    }

    /// Deletes the verification of the oracle `name`; nothing when it has none.
    pub fn forget_oracle(&self, name: &str) -> Result<()> {
        self.write(|tx| {
            tx.execute("DELETE FROM oracles WHERE name = ?1", [name])?;
            Ok(())
        })
    }
}

type OracleRow = (String, String, String, String, String);

fn read_row(row: &Row<'_>) -> rusqlite::Result<OracleRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

fn verification((name, spec, verified_at, hash, run): OracleRow) -> Result<VerifiedOracle> {
    Ok(VerifiedOracle {
        name,
        definition: serde_json::from_str(&spec)?,
        verified_at,
        hash,
        run: parse_run(&run)?,
    })
}
