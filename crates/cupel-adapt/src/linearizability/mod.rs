// ABOUTME: The `Histories` producer (ADR-119): runs a language's configured history command, which writes JSON-lines
// ABOUTME: histories into a directory Cupel names, and checks each against the model it declares with Porcupine.
//! The history producers.
//!
//! A project records histories itself: `[lang.<id>] history_command` names a command (a test, a
//! harness) that Cupel runs from the repository root with `CUPEL_HISTORY_DIR` set to an empty
//! directory under `.cupel/cache/linearizability/` and `CUPEL_SEED` set to the run's seed. Every
//! `*.jsonl` file the command writes there is one history ([`history`]); the producer checks each
//! with its checker and never runs the system under test itself.

mod history;
mod porcupine;

use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, FactKind, Histories, LangId, Langs, Process,
    ProduceCtx, Producer, ProducerId, ProducerRegistry, Result, RootRun, RunStatus, Scope, ToolId,
    ToolProbe, output_tail,
};
use serde_json::{Map, Value};

use crate::runner::installed;
use crate::toolchain;

/// The `[lang.<id>]` key naming the command that records histories.
pub const COMMAND_KEY: &str = "history_command";
/// The variable naming the directory the command writes its histories in.
pub const DIR_ENV: &str = "CUPEL_HISTORY_DIR";
/// The variable holding the run's seed.
pub const SEED_ENV: &str = "CUPEL_SEED";

const ID: &str = "tool.porcupine";

inventory::submit!(crate::ToolProducer { id: ID, register });

/// Registers the checker for every language the toolchain table has a row for: any project can
/// record a history, whatever it is written in.
fn register(registry: &mut ProducerRegistry) {
    for row in toolchain::rows() {
        for lang in &row.langs {
            registry.register::<Histories, _>(Porcupine(lang.clone()));
        }
    }
}

/// The command `table` (a `[lang.<id>]` table) names under `history_command`.
fn command(table: Option<&Map<String, Value>>) -> Option<String> {
    table?
        .get(COMMAND_KEY)?
        .as_str()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(str::to_owned)
}

/// Porcupine checking one language's histories.
pub struct Porcupine(LangId);

impl Producer<Histories> for Porcupine {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.0.clone()])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    /// Unconfigured without a history command; else ready when `go` is on `PATH`.
    fn availability(&self, ctx: &Ctx) -> Availability {
        if command(ctx.lang.get(&self.0)).is_none() {
            Availability::Unconfigured {
                key: format!("lang.{}.{COMMAND_KEY}", self.0),
            }
        } else if installed("go", false, &ctx.root).is_some() {
            Availability::Ready
        } else {
            Availability::MissingTool {
                name: ToolId::from("go"),
                install: toolchain::install(&self.id(), "go", &ctx.root),
            }
        }
    }

    fn tool(&self, _: &Ctx) -> Option<ToolProbe> {
        Some(ToolProbe {
            name: ToolId::from("go"),
            version: vec!["go".to_owned(), "version".to_owned()],
        })
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Histories> {
        let command = command(Some(&ctx.config)).ok_or_else(|| Error::Process {
            program: "sh".to_owned(),
            message: format!("no `[lang.{}] {COMMAND_KEY}` is configured", self.0),
        })?;
        let cache = ctx.writer.cache_dir().join("linearizability");
        let dir = emptied(ctx, &cache.join(self.0.as_str()))?;
        let captured = Process::new("sh", &ctx.ctx.root)
            .args(["-c", command.as_str()])
            .env(DIR_ENV, &dir.to_string_lossy())
            .env(SEED_ENV, &ctx.seed.to_string())
            .merged()
            .run(&ctx.cancel)?;
        let run = RootRun {
            dir: ".".to_owned(),
            status: if captured.exit_code == Some(0) {
                RunStatus::Ran
            } else {
                RunStatus::Failed
            },
            detail: (captured.exit_code != Some(0)).then(|| {
                output_tail(&captured.stdout)
                    .unwrap_or_else(|| format!("exited with {:?}", captured.exit_code))
            }),
        };
        let histories = recorded(&dir)
            .into_iter()
            .map(|(name, text)| porcupine::Checkable {
                name,
                recorded: history::read(&text),
            })
            .collect();
        let histories = porcupine::check(
            ctx,
            &cache.join("porcupine"),
            histories,
            porcupine::timeout(ctx.focus.depth),
        )?;
        Ok(Histories {
            tool: self.id(),
            command,
            seed: ctx.seed,
            run,
            histories,
        })
    }
}

/// `dir`, created empty.
fn emptied(ctx: &ProduceCtx, dir: &Path) -> Result<PathBuf> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|error| Error::Process {
            program: "sh".to_owned(),
            message: format!("cannot empty {}: {error}", dir.display()),
        })?;
    }
    ctx.writer.create_dir(dir)
}

/// Every `*.jsonl` file under `dir`, by its path relative to `dir`, with its text.
fn recorded(dir: &Path) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = ignore::WalkBuilder::new(dir)
        .standard_filters(false)
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|entry| {
            let name = entry.path().strip_prefix(dir).ok()?;
            let text = fs::read_to_string(entry.path()).ok()?;
            Some((name.to_string_lossy().replace('\\', "/"), text))
        })
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn table(value: &Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }

    #[test]
    fn a_history_command_must_be_configured() {
        assert_eq!(
            command(Some(&table(&json!({"history_command": " go test ./... "})))).as_deref(),
            Some("go test ./...")
        );
        assert_eq!(command(Some(&table(&json!({"history_command": ""})))), None);
        assert_eq!(
            command(Some(&table(&json!({"history_command": ["a"]})))),
            None
        );
        assert_eq!(command(None), None);
        let mut ctx = Ctx::new(PathBuf::from("/repo"));
        let lang = LangId::from("rust");
        let producer = Porcupine(lang.clone());
        assert_eq!(
            producer.availability(&ctx),
            Availability::Unconfigured {
                key: "lang.rust.history_command".to_owned()
            }
        );
        ctx.lang
            .insert(lang, table(&json!({"history_command": "cargo test"})));
        assert!(!matches!(
            producer.availability(&ctx),
            Availability::Unconfigured { .. }
        ));
    }

    #[test]
    fn every_jsonl_file_under_the_directory_is_a_history_by_its_relative_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("kv")).unwrap();
        fs::write(dir.path().join("kv/b.jsonl"), "b").unwrap();
        fs::write(dir.path().join("a.jsonl"), "a").unwrap();
        fs::write(dir.path().join("notes.txt"), "not a history").unwrap();
        assert_eq!(
            recorded(dir.path()),
            [
                ("a.jsonl".to_owned(), "a".to_owned()),
                ("kv/b.jsonl".to_owned(), "b".to_owned())
            ]
        );
    }
}
