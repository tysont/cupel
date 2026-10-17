// ABOUTME: `tool.analyzers` (ADR-102): one producer per language that runs every type checker and linter its
// ABOUTME: toolchain row lists in each project root, normalizing their results to an `AnalyzerReport`.
//! The analyzers producer.
//!
//! `tool.analyzers` is one [`Producer`] of [`AnalyzerReport`] per language whose toolchain row
//! lists `[[rows.analyzers]]`. It runs each listed analyzer that applies to the language (its
//! [`Spec`] in [`tools`]) in every project root `ProjectInfo` names (the repository root when it
//! names none), reads what the tool reported, and keeps the results in files of its scope,
//! relative to the repository root, without exact duplicates. An analyzer that is not
//! installed, whose detection rules do not hold in a root, or whose output cannot be read is
//! recorded with that status; the others still run.

mod tools;

use std::collections::BTreeSet;
use std::path::Path;

use cupel_core::{
    AnalyzerReport, AnalyzerResult, AnalyzerRun, AnalyzerStatus, Availability, CostClass, Ctx,
    EventSink, FactKind, Install, LangId, Langs, Location, Process, ProduceCtx, Producer,
    ProducerId, Result, Scope, ToolId, ToolProbe, output_tail,
};

use crate::runner::{installed, roots};
use crate::toolchain;
use crate::tree::{canonical, relative};
pub use tools::{Raw, SPECS, Spec, Target};

const ID: &str = "tool.analyzers";

/// Runs a language's analyzers (ADR-102).
pub struct Analyzers(LangId);

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| {
        for lang in languages() {
            registry.register::<AnalyzerReport, _>(Analyzers(lang));
        }
    },
});

/// Every language with an analyzer that applies to it, in table order.
fn languages() -> Vec<LangId> {
    toolchain::rows()
        .iter()
        .flat_map(|row| row.langs.iter())
        .filter(|lang| !specs(lang).is_empty())
        .cloned()
        .collect()
}

/// The file extensions of `lang`'s sources, which its analyzers are handed: a scope holds every
/// language's files.
fn extensions(lang: &LangId) -> &'static [&'static str] {
    match lang.as_str() {
        "rust" => &["rs"],
        "python" => &["py", "pyi"],
        "javascript" => &["js", "mjs", "cjs", "jsx"],
        "typescript" => &["ts", "tsx", "mts", "cts"],
        "go" => &["go"],
        _ => &[],
    }
}

/// The analyzers `lang`'s row lists that apply to it, in the row's order.
fn specs(lang: &LangId) -> Vec<&'static Spec> {
    toolchain::for_lang(lang)
        .into_iter()
        .flat_map(|row| &row.analyzers)
        .filter_map(|tool| SPECS.iter().find(|spec| spec.id == tool.id.as_str()))
        .filter(|spec| spec.langs.is_empty() || spec.langs.contains(&lang.as_str()))
        .collect()
}

impl Producer<AnalyzerReport> for Analyzers {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.0.clone()])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    /// Ready when any of the language's analyzers is installed; else the first one is missing.
    fn availability(&self, ctx: &Ctx) -> Availability {
        let specs = specs(&self.0);
        if specs
            .iter()
            .any(|spec| installed(spec.program, spec.node_bin, &ctx.root).is_some())
        {
            return Availability::Ready;
        }
        let first = specs[0];
        Availability::MissingTool {
            name: ToolId::from(first.program),
            install: toolchain::install(&ProducerId::from(first.id), first.program, &ctx.root),
        }
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        let spec = specs(&self.0)
            .into_iter()
            .find(|spec| installed(spec.program, spec.node_bin, &ctx.root).is_some())?;
        Some(crate::runner::probe(
            spec.program,
            spec.node_bin,
            spec.version,
            &ctx.root,
        ))
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<AnalyzerReport> {
        let root = canonical(&ctx.ctx.root);
        let extensions = extensions(&self.0);
        let in_scope: BTreeSet<&str> = scope
            .files
            .iter()
            .map(String::as_str)
            .filter(|file| {
                Path::new(file)
                    .extension()
                    .is_some_and(|ext| extensions.iter().any(|known| ext == *known))
            })
            .collect();
        let mut analyzers = Vec::new();
        for spec in specs(&self.0) {
            ctx.cancel.checkpoint()?;
            analyzers.push(
                Run {
                    ctx,
                    spec,
                    root: &root,
                    in_scope: &in_scope,
                }
                .analyze()?,
            );
        }
        Ok(AnalyzerReport { analyzers })
    }
}

/// One analyzer over every project root.
struct Run<'a> {
    ctx: &'a ProduceCtx,
    spec: &'static Spec,
    /// The repository root, canonical.
    root: &'a Path,
    in_scope: &'a BTreeSet<&'a str>,
}

impl Run<'_> {
    fn analyze(&self) -> Result<AnalyzerRun> {
        let mut run = AnalyzerRun {
            tool: ToolId::from(self.spec.tool),
            status: AnalyzerStatus::Ran,
            detail: None,
            results: Vec::new(),
        };
        let Some(program) = installed(self.spec.program, self.spec.node_bin, self.root) else {
            run.status = AnalyzerStatus::Missing;
            run.detail = Some(self.install());
            return Ok(run);
        };
        let mut configured = false;
        for (dir, packages) in roots(self.ctx) {
            let absolute = canonical(&self.root.join(&dir));
            if !toolchain::detects(&ProducerId::from(self.spec.id), &absolute) {
                continue;
            }
            configured = true;
            let target = Target {
                program: program.clone(),
                dir: &absolute,
                files: self.files_under(&dir),
                packages: &packages,
                cache: self
                    .ctx
                    .writer
                    .cache_dir()
                    .join("analyze")
                    .join(self.spec.tool),
            };
            let Some(command) = (self.spec.command)(&target) else {
                continue;
            };
            match self.run_root(&target, &command)? {
                Ok(raws) => self.keep(&mut run.results, &absolute, raws),
                Err(problem) => {
                    run.status = AnalyzerStatus::Failed;
                    run.detail = Some(problem);
                }
            }
        }
        if !configured {
            run.status = AnalyzerStatus::Unconfigured;
            run.detail = Some(self.spec.unconfigured.to_owned());
        }
        Ok(run)
    }

    /// How to install the analyzer: the table's command, else what to do by hand.
    fn install(&self) -> String {
        match toolchain::install(
            &ProducerId::from(self.spec.id),
            self.spec.program,
            self.root,
        ) {
            Some(Install::Command(command)) => command,
            Some(Install::Hint(hint)) => hint,
            None => format!("install {} and put it on PATH", self.spec.program),
        }
    }

    /// The files in scope under the project root `dir`, relative to it.
    fn files_under(&self, dir: &str) -> Vec<String> {
        self.in_scope
            .iter()
            .filter_map(|file| {
                if dir == "." {
                    Some((*file).to_owned())
                } else {
                    file.strip_prefix(&format!("{dir}/")).map(str::to_owned)
                }
            })
            .collect()
    }

    /// The analyzer's results in one root, or why its output could not be read.
    fn run_root(
        &self,
        target: &Target<'_>,
        command: &[String],
    ) -> Result<std::result::Result<Vec<Raw>, String>> {
        self.ctx.writer.create_dir(&target.cache)?;
        let (program, args) = command.split_first().expect("a command has its program");
        let mut process = Process::new(program, target.dir).args(args.iter().cloned());
        for (key, value) in self.spec.env {
            process = process.env(key, value);
        }
        let captured = process.run(&self.ctx.cancel)?;
        let tail = || {
            output_tail(&format!("{}\n{}", captured.stdout, captured.stderr))
                .unwrap_or_else(|| "no output".to_owned())
        };
        let completed = captured
            .exit_code
            .is_some_and(|code| self.spec.completed_codes.contains(&code));
        Ok(match (self.spec.read)(&captured) {
            Ok(raws) if completed || !raws.is_empty() => Ok(raws),
            Ok(_) => Err(format!(
                "{} exited with {}: {}",
                self.spec.tool,
                captured
                    .exit_code
                    .map_or_else(|| "a signal".to_owned(), |code| format!("code {code}")),
                tail()
            )),
            Err(problem) => Err(format!("{problem}: {}", tail())),
        })
    }

    /// Adds the results in files in scope to `results`, relative to the repository root, once
    /// each. A relative path is the tool's, from the project root `absolute`.
    fn keep(&self, results: &mut Vec<AnalyzerResult>, absolute: &Path, raws: Vec<Raw>) {
        for raw in raws {
            let Some(file) = self.relative(absolute, &raw.file) else {
                continue;
            };
            if !self.in_scope.contains(file.as_str()) {
                continue;
            }
            let result = AnalyzerResult {
                rule: raw.rule,
                level: raw.level,
                message: raw.message,
                location: Location {
                    file,
                    line: raw.line,
                    col: raw.col,
                    end_line: raw.end_line.unwrap_or(raw.line),
                    end_col: raw.end_col,
                },
            };
            if !results.contains(&result) {
                results.push(result);
            }
        }
    }

    /// `file` as the tool named it, relative to the repository root; `None` outside it.
    fn relative(&self, absolute: &Path, file: &str) -> Option<String> {
        relative(self.root, absolute, file)
    }
}

#[cfg(test)]
mod tests {
    use cupel_core::{Cancellation, Writer};

    use super::*;

    static ABSENT: Spec = Spec {
        id: "tool.absent",
        tool: "absent",
        program: "cupel-test-no-such-analyzer",
        node_bin: false,
        version: &[],
        langs: &[],
        command: |_| None,
        env: &[],
        completed_codes: &[0],
        read: |_| Ok(Vec::new()),
        unconfigured: "",
    };

    #[test]
    fn a_missing_analyzer_is_recorded_with_how_to_install_it() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ProduceCtx::new(
            Ctx::new(dir.path().to_path_buf()),
            Writer::new(dir.path(), false).unwrap().confined(),
            Cancellation::new(),
        );
        let in_scope = BTreeSet::new();
        let run = Run {
            ctx: &ctx,
            spec: &ABSENT,
            root: dir.path(),
            in_scope: &in_scope,
        }
        .analyze()
        .unwrap();
        assert_eq!(
            run,
            AnalyzerRun {
                tool: ToolId::from("absent"),
                status: AnalyzerStatus::Missing,
                detail: Some("install cupel-test-no-such-analyzer and put it on PATH".to_owned()),
                results: Vec::new(),
            }
        );
    }

    #[test]
    fn every_analyzer_the_table_lists_has_a_spec_and_every_spec_is_listed() {
        let listed: BTreeSet<&str> = toolchain::rows()
            .iter()
            .flat_map(|row| &row.analyzers)
            .map(|tool| tool.id.as_str())
            .collect();
        let specified: BTreeSet<&str> = SPECS.iter().map(|spec| spec.id).collect();
        assert_eq!(listed, specified);
        assert_eq!(
            languages(),
            ["javascript", "typescript", "python", "rust", "go"].map(LangId::from)
        );
    }
}
