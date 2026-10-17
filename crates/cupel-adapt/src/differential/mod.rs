// ABOUTME: The differential engine, `engine.differential` (ADR-093): calls each configured entry point on generated
// ABOUTME: inputs in the working tree and against the run's oracle, and minimizes the first divergence, into `DivergenceResults`.
//! The differential engine.
//!
//! `engine.differential` is one [`Producer`] of [`DivergenceResults`] per language with a
//! harness (`python`, `javascript`, `typescript`, `go`, `rust`). It needs `Symbols` and
//! `ApiSurface` to find each entry point of `[lang.<id>] differential_entries` and its
//! parameter types, generates inputs from the run's seed (`--seed`) and case budget
//! (`--cases`), and runs them through a generated harness in the working tree and through the
//! run's oracle (`ProduceCtx::oracle`): the same harness at a ref's commit, or a command once
//! per input. The first input whose answers differ is shrunk greedily to a smaller one that
//! still does. Nothing is written in the tree; a ref's commit is exported into a temporary
//! directory that is removed when the engine finishes.

mod compare;
mod entry;
mod generate;
mod harness;
mod oracle;
mod shrink;
mod types;

use std::path::{Path, PathBuf};

use cupel_core::syntax::{ApiSurface, Symbols};
use cupel_core::{
    Availability, CostClass, Ctx, Divergence, DivergenceResults, EntryComparison, Error, EventSink,
    FactKind, LangId, Langs, ProduceCtx, Producer, ProducerId, Result, Scope, ToolId, ToolProbe,
    find_program,
};

pub use entry::ENTRIES_KEY;
use entry::Entry;
use generate::{Input, Rng};
use harness::{Call, Language, Prepared, Problem, Site};
use oracle::{Opened, Side};

const ID: &str = "engine.differential";

/// Generated inputs per entry without `--cases`.
pub const DEFAULT_CASES: u64 = 100;

/// Minimization stops after this many steps.
const MAX_ROUNDS: u32 = 100;

/// One minimization round tries at most this many candidates.
const MAX_CANDIDATES: usize = 200;

/// Compares a language's entry points with the run's oracle (ADR-093).
pub struct DifferentialEngine(Language);

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| {
        for language in Language::ALL {
            registry.register::<DivergenceResults, _>(DifferentialEngine(language));
        }
    },
});

impl Producer<DivergenceResults> for DifferentialEngine {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from(self.0.id())])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::Symbols, FactKind::ApiSurface]
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    /// Unconfigured until the language lists entry points; then whether its program is on
    /// `PATH`.
    fn availability(&self, ctx: &Ctx) -> Availability {
        let id = LangId::from(self.0.id());
        if !ctx
            .lang
            .get(&id)
            .is_some_and(|table| table.contains_key(ENTRIES_KEY))
        {
            return Availability::Unconfigured {
                key: format!("lang.{id}.{ENTRIES_KEY}"),
            };
        }
        let program = self.0.program();
        if find_program(program).is_some() {
            Availability::Ready
        } else {
            Availability::MissingTool {
                name: ToolId::from(program),
                install: crate::toolchain::install(&self.id(), program, &ctx.root),
            }
        }
    }

    fn tool(&self, _: &Ctx) -> Option<ToolProbe> {
        Some(ToolProbe {
            name: ToolId::from(self.0.program()),
            version: self
                .0
                .version()
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
    ) -> Result<DivergenceResults> {
        let entries = entry::configured(&ctx.config)?;
        let cases = ctx.focus.cases.unwrap_or(DEFAULT_CASES);
        let mut results = DivergenceResults {
            engine: self.id(),
            oracle: None,
            seed: ctx.seed,
            cases,
            entries: Vec::new(),
        };
        let Some(run_oracle) = &ctx.oracle else {
            return Ok(results);
        };
        let scratch = tempfile::tempdir().map_err(|error| Error::Process {
            program: ID.to_owned(),
            message: format!("cannot make a temporary directory: {error}"),
        })?;
        let root = ctx
            .ctx
            .root
            .canonicalize()
            .unwrap_or_else(|_| ctx.ctx.root.clone());
        // Canonical, as Go resolves a package's directory before it reads an overlay.
        let scratch_dir = scratch
            .path()
            .canonicalize()
            .unwrap_or_else(|_| scratch.path().to_path_buf());
        let opened = Opened::open(&root, run_oracle, &scratch_dir, &ctx.cancel)?;
        results.oracle = Some(opened.compared(run_oracle));
        if matches!(opened, Opened::Spec) {
            return Ok(results);
        }
        let comparer = Comparer {
            language: self.0,
            ctx,
            root: &root,
            scratch: &scratch_dir,
            opened: &opened,
            kind: run_oracle.definition.kind,
            cases,
        };
        let empty = (Symbols::default(), ApiSurface::default());
        let symbols = ctx.upstream.get::<Symbols>().unwrap_or(&empty.0);
        let api = ctx.upstream.get::<ApiSurface>().unwrap_or(&empty.1);
        for (index, entry) in entries.iter().enumerate() {
            if scope.files.contains(&entry.file)
                && let Some(compared) = comparer.entry(index, entry, symbols, api)?
            {
                results.entries.push(compared);
            }
        }
        Ok(results)
    }
}

/// One run's comparisons of a language's entries with an opened oracle.
struct Comparer<'a> {
    language: Language,
    ctx: &'a ProduceCtx,
    root: &'a Path,
    scratch: &'a Path,
    opened: &'a Opened,
    kind: cupel_core::config::OracleKind,
    cases: u64,
}

impl Comparer<'_> {
    /// The comparison of `entry`, the `index`th configured; `None` when `--symbols` left its
    /// function out. A problem with the entry alone skips it; a cancellation or an unreachable
    /// oracle ends the run.
    fn entry(
        &self,
        index: usize,
        entry: &Entry,
        symbols: &Symbols,
        api: &ApiSurface,
    ) -> Result<Option<EntryComparison>> {
        let mut comparison = EntryComparison {
            entry: entry.text.clone(),
            symbol: None,
            compared: 0,
            skipped: None,
            divergence: None,
        };
        let resolved = entry::resolve(entry, symbols, api);
        if let (Ok(resolved), Some(selected)) = (&resolved, &self.ctx.focus.symbols)
            && !selected.contains(&resolved.symbol)
        {
            return Ok(None);
        }
        let outcome = resolved
            .map_err(Problem::Skip)
            .and_then(|resolved| self.compare(index, entry, &resolved, &mut comparison));
        match outcome {
            Ok(()) => {}
            Err(Problem::Skip(reason)) => comparison.skipped = Some(reason),
            Err(Problem::Fatal(error)) => return Err(error),
        }
        Ok(Some(comparison))
    }

    fn compare(
        &self,
        index: usize,
        entry: &Entry,
        resolved: &entry::Resolved,
        comparison: &mut EntryComparison,
    ) -> std::result::Result<(), Problem> {
        comparison.symbol = Some(resolved.symbol.clone());
        let signature = self
            .language
            .signature(&resolved.signature)
            .map_err(|ty| Problem::Skip(format!("no generator for the parameter type `{ty}`")))?;
        let inputs = self.inputs(entry, &signature)?;
        let call = Call {
            entry,
            resolved,
            signature: &signature,
        };
        let cancel = &self.ctx.cancel;
        let head_target = self
            .ctx
            .writer
            .cache_dir()
            .join("differential")
            .join("target");
        let ours = self
            .language
            .prepare(&call, &self.site(self.root, "head", index, head_target))?;
        let theirs = self.side(&call, index)?;
        let our_answers = ours.answers(&inputs, cancel)?;
        let found = theirs.first_disagreement(&inputs, &our_answers, self.kind, cancel)?;
        comparison.compared = found.compared;
        if let Some((case, their_answer)) = found.first {
            let first = Divergence {
                input: inputs[case].clone(),
                found: inputs[case].clone(),
                case: case as u64,
                shrinks: 0,
                implementation: our_answers[case].clone(),
                oracle: their_answer,
            };
            comparison.divergence = Some(self.minimize(&ours, &theirs, first)?);
        }
        Ok(())
    }

    /// The oracle's known inputs decoded for the entry's parameters, then the generated ones.
    fn inputs(
        &self,
        entry: &Entry,
        signature: &types::Signature,
    ) -> std::result::Result<Vec<Input>, Problem> {
        let tys: Vec<types::Ty> = signature
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect();
        let mut inputs = self
            .opened
            .known()
            .iter()
            .map(|known| {
                generate::known_input(known, &tys).ok_or_else(|| {
                    Problem::Skip(format!(
                        "the known input `{known}` does not decode as the arguments of `{}`",
                        entry.function
                    ))
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut rng = Rng::for_entry(self.ctx.seed, &entry.text);
        inputs.extend(generate::inputs(&tys, &mut rng, self.cases));
        Ok(inputs)
    }

    /// The oracle's side for `call`: its harness in a ref's tree, or its command.
    fn side(&self, call: &Call<'_>, index: usize) -> std::result::Result<Side<'_>, Problem> {
        match self.opened {
            Opened::Ref { commit, tree } => {
                let site = self.site(tree, "ref", index, self.scratch.join("target"));
                self.language
                    .prepare(call, &site)
                    .map(|prepared| Side::Harness { prepared, commit })
                    .map_err(|problem| problem.at(oracle::short(commit)))
            }
            Opened::Command { cmd, .. } => Ok(Side::Command {
                cmd,
                root: self.root,
            }),
            Opened::Spec => Err(Problem::Skip("a spec oracle compares nothing".to_owned())),
        }
    }

    fn site<'s>(&'s self, tree: &'s Path, side: &str, index: usize, target: PathBuf) -> Site<'s> {
        Site {
            tree,
            work: self.scratch.join("work").join(side).join(index.to_string()),
            target,
            cancel: &self.ctx.cancel,
        }
    }

    /// `first` shrunk greedily: each round keeps the first one-step-smaller input that still
    /// diverges, until none does or the rounds run out (ADR-093 §9).
    fn minimize(
        &self,
        ours: &Prepared,
        theirs: &Side<'_>,
        mut divergence: Divergence,
    ) -> std::result::Result<Divergence, Problem> {
        let cancel = &self.ctx.cancel;
        while divergence.shrinks < MAX_ROUNDS {
            let mut candidates = shrink::candidates(&divergence.input);
            candidates.truncate(MAX_CANDIDATES);
            if candidates.is_empty() {
                break;
            }
            let answers = ours.answers(&candidates, cancel)?;
            let Some((at, theirs)) = theirs
                .first_disagreement(&candidates, &answers, self.kind, cancel)?
                .first
            else {
                break;
            };
            divergence.input = candidates.swap_remove(at);
            divergence.implementation = answers[at].clone();
            divergence.oracle = theirs;
            divergence.shrinks += 1;
        }
        Ok(divergence)
    }
}
