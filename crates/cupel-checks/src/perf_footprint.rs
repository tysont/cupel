// ABOUTME: `perf.footprint`: vulnerable and copyleft dependencies and dependency growth from the Footprint fact,
// ABOUTME: reported for what changed against the range's base when there is one, else for every dependency (ADR-103).
use std::collections::{BTreeMap, BTreeSet};

use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Dependency, Event, EventSink,
    FactKind, Facts, Finding, Footprint, Location, Manifest, Plan, Result, Scope, Score, Severity,
    Stability, Tier, Vulnerability,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "perf.footprint";

/// The check has no settings; any key is a configuration error (ADR-103).
const SETTINGS: Limits = Limits::new(&[]);

/// License identifiers that start a strong or network copyleft license (ADR-103 §6).
const COPYLEFT: [&str; 6] = ["GPL", "AGPL", "LGPL", "SSPL", "EUPL", "OSL"];

/// Evidence of `footprint.grew`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Grew {
    /// Dependencies at the base commit.
    base: u64,
    /// Dependencies at head.
    head: u64,
    /// `name@version` of each dependency head has and the base did not.
    added: Vec<String>,
    /// `name@version` of each dependency the base had and head does not.
    removed: Vec<String>,
}

/// Evidence of `dep.vuln`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Vulnerable {
    package: String,
    version: String,
    /// The advisory's ID.
    advisory: String,
    summary: String,
    /// The first version that fixes it, when the advisory names one.
    fixed: Option<String>,
}

/// Evidence of `dep.license`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Licensed {
    package: String,
    version: Option<String>,
    /// The license expression, which offers no alternative free of copyleft.
    license: String,
}

pub struct PerfFootprint {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(PerfFootprint::new())));

impl PerfFootprint {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::Footprint],
                wants: vec![],
                findings: vec![
                    kind::<Grew>(
                        "footprint.grew",
                        ActionKind::Review,
                        "The range added dependencies: there are more than at its base commit.",
                    ),
                    kind::<Vulnerable>(
                        "dep.vuln",
                        ActionKind::FixBug,
                        "A dependency has a known vulnerability in the configured advisories.",
                    ),
                    kind::<Licensed>(
                        "dep.license",
                        ActionKind::Review,
                        "A dependency's license offers no alternative free of copyleft (GPL, \
                         AGPL, LGPL, SSPL, EUPL, OSL).",
                    ),
                ],
                metrics: vec![
                    metric(
                        "dependencies",
                        "Dependencies the lockfiles and manifests list.",
                    ),
                    metric(
                        "vulnerable",
                        "Reported dependencies with a known vulnerability (zero when no \
                         advisories are configured; a note says so).",
                    ),
                    metric(
                        "copyleft",
                        "Reported dependencies with a copyleft license (zero when licenses are \
                         not read; a note says so).",
                    ),
                    metric(
                        "artifact_bytes",
                        "Total size of the configured artifacts, for a size budget.",
                    ),
                ],
                settings: SETTINGS.specs(),
                stability: Stability::Experimental,
                summary: "Dependencies, their licenses and known vulnerabilities, and artifact \
                          sizes, read offline; against the range's base under --changed or \
                          --base."
                    .to_owned(),
            },
        }
    }
}

impl Check for PerfFootprint {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.plan_files(&self.manifest, ctx, scope)
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let footprint = facts.get::<Footprint>()?;
        let reported = Reported::of(footprint);
        let mut findings = Vec::new();
        let mut vulnerable = BTreeSet::new();
        for vulnerability in footprint.vulnerabilities.iter().flatten() {
            if let Some(dependency) = reported.find(&vulnerability.package, &vulnerability.version)
            {
                vulnerable.insert(dependency.spelled());
                findings.push(self.vulnerable(vulnerability, dependency, reported.commit));
            }
        }
        let mut copyleft = 0u64;
        for dependency in &reported.dependencies {
            if let Some(license) = dependency.license.as_deref().filter(|l| is_copyleft(l)) {
                copyleft += 1;
                findings.push(self.licensed(dependency, license, reported.commit));
            }
        }
        if let Some(grew) = self.grew(footprint, &reported) {
            findings.push(grew);
        }
        let count = findings.len() as u64;
        for finding in findings {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        let dependencies = footprint.dependencies.len() as u64;
        let bytes: u64 = footprint.artifacts.iter().flatten().map(|a| a.bytes).sum();
        #[allow(
            clippy::cast_precision_loss,
            reason = "counts and sizes stay far below 2^52, where f64 is exact"
        )]
        let metrics = BTreeMap::from([
            ("dependencies".to_owned(), dependencies as f64),
            ("vulnerable".to_owned(), vulnerable.len() as f64),
            ("copyleft".to_owned(), copyleft as f64),
            ("artifact_bytes".to_owned(), bytes as f64),
        ]);
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(count > 0),
            Some(Score {
                value: metrics["dependencies"],
                unit: "dependencies".to_owned(),
                n: dependencies,
            }),
            metrics,
            count,
        );
        result.notes = unmeasured(footprint);
        Ok(result)
    }

    /// Counts and sizes add up across languages.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }

    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }
}

/// The dependencies findings are about: those not at the base, or all without one.
struct Reported<'a> {
    dependencies: Vec<&'a Dependency>,
    /// The base commit, when there is one.
    commit: Option<&'a str>,
}

impl<'a> Reported<'a> {
    fn of(footprint: &'a Footprint) -> Self {
        let Some(base) = &footprint.base else {
            return Self {
                dependencies: footprint.dependencies.iter().collect(),
                commit: None,
            };
        };
        let before: BTreeSet<String> = base.dependencies.iter().map(Dependency::spelled).collect();
        Self {
            dependencies: footprint
                .dependencies
                .iter()
                .filter(|dependency| !before.contains(&dependency.spelled()))
                .collect(),
            commit: Some(&base.commit),
        }
    }

    fn find(&self, package: &str, version: &str) -> Option<&'a Dependency> {
        self.dependencies
            .iter()
            .copied()
            .find(|d| d.name == package && d.version.as_deref() == Some(version))
    }
}

/// What the fact did not measure, as notes, so a zero metric is never read as a measurement.
fn unmeasured(footprint: &Footprint) -> Vec<String> {
    let mut notes = Vec::new();
    if footprint.manifests.is_empty() {
        notes.push("no lockfile or manifest found".to_owned());
    }
    if !footprint.licenses && !footprint.dependencies.is_empty() {
        notes.push("licenses not read for these dependencies".to_owned());
    }
    if footprint.vulnerabilities.is_none() {
        notes.push(
            "vulnerabilities not measured: set `[lang.<id>] advisories` to a directory of OSV \
             advisories"
                .to_owned(),
        );
    }
    if footprint.artifacts.is_none() {
        notes.push(
            "artifact sizes not measured: set `[lang.<id>] artifacts` to their paths or globs"
                .to_owned(),
        );
    }
    notes
}

impl PerfFootprint {
    fn vulnerable(
        &self,
        vulnerability: &Vulnerability,
        dependency: &Dependency,
        commit: Option<&str>,
    ) -> Finding {
        let package = &vulnerability.package;
        let fixed = vulnerability.fixed.as_ref().map_or_else(
            || "a version without it".to_owned(),
            |fixed| format!("{fixed} or later"),
        );
        self.finding(Said {
            kind: "dep.vuln",
            subject: format!("{}|{}", dependency.spelled(), vulnerability.id),
            severity: Severity::Fail,
            title: format!(
                "{} {}: {}",
                dependency.spelled(),
                vulnerability.id,
                vulnerability.summary
            ),
            evidence: evidence(&Vulnerable {
                package: package.clone(),
                version: vulnerability.version.clone(),
                advisory: vulnerability.id.clone(),
                summary: vulnerability.summary.clone(),
                fixed: vulnerability.fixed.clone(),
            }),
            summary: format!("Upgrade `{package}` to {fixed}, or replace it."),
            at: Some(dependency),
            commit,
        })
    }

    fn licensed(&self, dependency: &Dependency, license: &str, commit: Option<&str>) -> Finding {
        let name = &dependency.name;
        self.finding(Said {
            kind: "dep.license",
            subject: dependency.spelled(),
            severity: Severity::Warn,
            title: format!("{} is licensed {license}", dependency.spelled()),
            evidence: evidence(&Licensed {
                package: name.clone(),
                version: dependency.version.clone(),
                license: license.to_owned(),
            }),
            summary: format!(
                "Confirm the project may ship `{name}` under {license}, or replace it with a \
                 permissively licensed alternative."
            ),
            at: Some(dependency),
            commit,
        })
    }

    /// `footprint.grew` when there is a base and head has more dependencies than it.
    fn grew(&self, footprint: &Footprint, reported: &Reported<'_>) -> Option<Finding> {
        let base = footprint.base.as_ref()?;
        let (before, after) = (base.dependencies.len(), footprint.dependencies.len());
        if after <= before {
            return None;
        }
        let head: BTreeSet<String> = footprint
            .dependencies
            .iter()
            .map(Dependency::spelled)
            .collect();
        let removed: Vec<String> = base
            .dependencies
            .iter()
            .map(Dependency::spelled)
            .filter(|spelled| !head.contains(spelled))
            .collect();
        let added: Vec<String> = reported.dependencies.iter().map(|d| d.spelled()).collect();
        Some(self.finding(Said {
            kind: "footprint.grew",
            subject: "dependencies".to_owned(),
            severity: Severity::Warn,
            title: format!("dependencies grew from {before} to {after}"),
            summary: format!(
                "Check that each added dependency ({}) is needed, or drop it.",
                added.join(", ")
            ),
            evidence: evidence(&Grew {
                base: before as u64,
                head: after as u64,
                added,
                removed,
            }),
            at: reported.dependencies.first().copied(),
            commit: reported.commit,
        }))
    }

    fn finding(&self, said: Said<'_>) -> Finding {
        SubjectFinding {
            kind: said.kind,
            subject: said.subject,
            severity: said.severity,
            confidence: 1.0,
            title: said.title,
            summary: said.summary,
            evidence: said.evidence,
            location: said.at.map(|dependency| Location {
                file: dependency.file.clone(),
                line: dependency.line,
                col: None,
                end_line: dependency.line,
                end_col: None,
            }),
            repro: match said.commit {
                Some(commit) => format!("cupel check --only {} --base {commit}", self.manifest.id),
                None => format!("cupel check --only {}", self.manifest.id),
            },
        }
        .finding(&self.manifest)
    }
}

/// What one finding says.
struct Said<'a> {
    kind: &'static str,
    subject: String,
    severity: Severity,
    title: String,
    evidence: Value,
    summary: String,
    at: Option<&'a Dependency>,
    commit: Option<&'a str>,
}

/// Whether `expression` offers no alternative free of copyleft: SPDX `AND` binds tighter than
/// `OR`, parentheses group, and `WITH` an exception keeps the license it qualifies.
fn is_copyleft(expression: &str) -> bool {
    let spaced = expression.replace('(', " ( ").replace(')', " ) ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut at = 0;
    !free(&tokens, &mut at)
}

/// Whether the `OR` of terms starting at `at` has an alternative free of copyleft.
fn free(tokens: &[&str], at: &mut usize) -> bool {
    let mut any = all_free(tokens, at);
    while tokens
        .get(*at)
        .is_some_and(|token| token.eq_ignore_ascii_case("OR"))
    {
        *at += 1;
        any |= all_free(tokens, at);
    }
    any
}

/// Whether every factor of the `AND` starting at `at` is free of copyleft.
fn all_free(tokens: &[&str], at: &mut usize) -> bool {
    let mut all = factor_free(tokens, at);
    while tokens
        .get(*at)
        .is_some_and(|token| token.eq_ignore_ascii_case("AND"))
    {
        *at += 1;
        all &= factor_free(tokens, at);
    }
    all
}

/// Whether the license or parenthesized expression at `at` is free of copyleft.
fn factor_free(tokens: &[&str], at: &mut usize) -> bool {
    let Some(token) = tokens.get(*at) else {
        return true;
    };
    *at += 1;
    if *token == "(" {
        let inner = free(tokens, at);
        if tokens.get(*at) == Some(&")") {
            *at += 1;
        }
        return inner;
    }
    if tokens
        .get(*at)
        .is_some_and(|next| next.eq_ignore_ascii_case("WITH"))
    {
        *at += 2;
    }
    let upper = token.to_ascii_uppercase();
    !COPYLEFT.iter().any(|prefix| {
        upper.strip_prefix(prefix).is_some_and(|rest| {
            rest.is_empty()
                || rest.starts_with(['-', 'V'])
                || rest.starts_with(|c: char| c.is_ascii_digit())
        })
    })
}
