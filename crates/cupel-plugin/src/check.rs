// ABOUTME: A check plugin as a `Check` (ADR-089): `plan` asks the plugin, falling back to its cost estimate; `run`
// ABOUTME: writes the declared facts as files, runs it, and emits its findings only once the whole output is valid.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cupel_core::plugin::PluginResult;
use cupel_core::{Check, CheckCtx, CheckResult, EventSink, Facts, Manifest, Plan, Result, Scope};

use crate::command::{PluginCommand, QUICK_LIMIT};
use crate::output::{self, CheckOutput};
use crate::request::{self, Inputs};

/// A check implemented by a plugin, registered beside the native checks (ADR-088 §7).
pub struct PluginCheck {
    manifest: Manifest,
    command: PluginCommand,
}

impl PluginCheck {
    pub(crate) fn new(manifest: Manifest, command: PluginCommand) -> Self {
        Self { manifest, command }
    }

    /// `.cupel/work/<run>/plugins/<check-id>/`.
    fn scratch(&self, ctx: &CheckCtx) -> PathBuf {
        ctx.writer
            .area()
            .join("plugins")
            .join(self.manifest.id.as_str())
    }

    /// Asks the plugin to plan; `None` when it cannot.
    fn planned(&self, ctx: &CheckCtx, scope: &Scope) -> Option<Plan> {
        let scratch = self.scratch(ctx);
        let request = request::request(&inputs(ctx, scope, &scratch), BTreeMap::new());
        let captured = self
            .command
            .call("plan", Some(&request), &ctx.cancel.child(QUICK_LIMIT))
            .ok()?;
        (captured.exit_code == Some(0))
            .then(|| serde_json::from_str(&captured.stdout).ok())
            .flatten()
    }

    /// The check's result from the plugin's, with Cupel's part filled in.
    fn result(&self, reported: PluginResult, findings: u64, note: Option<String>) -> CheckResult {
        let mut notes = reported.notes;
        notes.extend(note);
        CheckResult {
            id: self.manifest.id.clone(),
            status: reported.status,
            tier: self.manifest.tier,
            cost: self.manifest.cost,
            score: reported.score,
            metrics: reported.metrics,
            delta: None,
            findings,
            duration_ms: 0,
            skipped: None,
            oracle: None,
            notes,
        }
    }
}

impl Check for PluginCheck {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The plugin's plan, or the cost class's estimate over the scope's files when the plugin
    /// cannot plan (ADR-089 §8): a broken plan must not stop the other checks.
    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        Ok(self
            .planned(ctx, scope)
            .unwrap_or_else(|| Plan::estimate(self.manifest.cost, scope.files.len() as u64)))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _plan: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let scratch = self.scratch(ctx);
        let files = request::write_facts(
            &ctx.writer,
            &scratch,
            facts
                .declared()
                .filter_map(|kind| facts.json(kind).map(|json| (kind, json))),
        )?;
        let request = request::request(&inputs(ctx, &ctx.scope, &scratch), files);
        let captured = self.command.call("run", Some(&request), &ctx.cancel)?;
        let (CheckOutput { result, events }, note) =
            output::accepted(&self.command, &captured, "result", |stdout| {
                output::check_output(&self.manifest, stdout)
            })?;
        let findings = events
            .iter()
            .filter(|event| matches!(event, cupel_core::Event::Finding(_)))
            .count() as u64;
        for event in events {
            sink.emit(event);
        }
        Ok(self.result(result, findings, note))
    }
}

/// What a check's request is built from: its context over `scope`, with its scratch directory.
fn inputs<'a>(ctx: &'a CheckCtx, scope: &'a Scope, scratch: &'a Path) -> Inputs<'a> {
    Inputs {
        writer: &ctx.writer,
        scope,
        config: &ctx.config,
        cancel: &ctx.cancel,
        scratch,
        run: None,
    }
}
