// ABOUTME: The race detectors (ADR-105): one `RaceReports` producer per tool (Go's -race, Miri, ThreadSanitizer), each
// ABOUTME: running a language's tests in every project root and reading its reports into one normalized fact.
//! The race detector producers.
//!
//! Each detector is a [`Spec`]: its language, how to tell it is installed, the command it runs
//! in a project root, and the reader of what that command printed ([`report::Read`]). One
//! [`Detector`] producer runs a spec in every root `ProjectInfo` names (the repository root
//! when it names none), records each root's run, normalizes each report
//! ([`report::Raw::normalized`]), and keeps those touching the scope, without duplicates.

mod cargo;
mod go;
mod miri;
mod report;
mod tsan;

use std::path::{Path, PathBuf};

use cupel_core::{
    Availability, Captured, CostClass, Ctx, EventSink, FactKind, LangId, Langs, Process,
    ProduceCtx, Producer, ProducerId, ProducerRegistry, RaceReports, Result, RootRun, RunStatus,
    Scope, ToolId, ToolProbe,
};

use crate::runner::suite_roots;
use crate::tree::canonical;
use report::{Paths, Read, in_scope};

/// One race detector.
pub struct Spec {
    pub id: &'static str,
    pub lang: &'static str,
    pub cost: CostClass,
    /// The program `doctor` reports, and the command that prints its version.
    pub program: &'static str,
    pub version: &'static [&'static str],
    pub availability: fn(&Ctx) -> Availability,
    pub command: fn(&Invocation<'_>) -> Result<Process>,
    pub read: fn(&Captured) -> Read,
}

/// Where a detector runs: a project root's directory and packages, and the cache directory it
/// builds into.
pub struct Invocation<'a> {
    pub dir: &'a Path,
    pub packages: &'a [String],
    pub cache: PathBuf,
}

/// The `RaceReports` producer a spec describes.
pub struct Detector(&'static Spec);

/// Registers the detector `spec` describes.
fn register(registry: &mut ProducerRegistry, spec: &'static Spec) {
    registry.register::<RaceReports, _>(Detector(spec));
}

impl Producer<RaceReports> for Detector {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from(self.0.lang)])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        self.0.cost
    }

    fn availability(&self, ctx: &Ctx) -> Availability {
        (self.0.availability)(ctx)
    }

    fn tool(&self, _: &Ctx) -> Option<ToolProbe> {
        Some(ToolProbe {
            name: ToolId::from(self.0.program),
            version: self
                .0
                .version
                .iter()
                .map(|word| (*word).to_owned())
                .collect(),
        })
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<RaceReports> {
        let root = canonical(&ctx.ctx.root);
        let cache = ctx.writer.cache_dir().join("race").join(self.0.id);
        let mut fact = RaceReports {
            tool: self.id(),
            runs: Vec::new(),
            reports: Vec::new(),
        };
        for (dir, packages) in suite_roots(ctx) {
            ctx.cancel.checkpoint()?;
            let absolute = canonical(&root.join(&dir));
            let invocation = Invocation {
                dir: &absolute,
                packages: &packages,
                cache: cache.clone(),
            };
            let captured = (self.0.command)(&invocation)?.run(&ctx.cancel)?;
            let read = (self.0.read)(&captured);
            fact.runs.push(RootRun {
                dir,
                status: if read.failed.is_some() {
                    RunStatus::Failed
                } else {
                    RunStatus::Ran
                },
                detail: read.failed,
            });
            let paths = Paths {
                root: &root,
                dir: &absolute,
            };
            for raw in read.reports {
                let report = raw.normalized(&paths);
                if in_scope(&report, scope) && !fact.reports.contains(&report) {
                    fact.reports.push(report);
                }
            }
        }
        Ok(fact)
    }
}
