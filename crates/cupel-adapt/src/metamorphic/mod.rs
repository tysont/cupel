// ABOUTME: The metamorphic engine, `engine.metamorphic` (ADR-097): evaluates each `[lang.python.relations]` relation on
// ABOUTME: inputs generated from the run's seed through the Python harness, minimizing the first violation, into `RelationResults`.
//! The metamorphic engine.
//!
//! `engine.metamorphic` is a [`Producer`] of [`RelationResults`] for Python that needs no other
//! fact. For each relation of `[lang.python.relations]`, by name, it generates the case budget's
//! inputs ([`generate`], from [`relation_seed`]), hands them to the harness (`harness.py`, run as
//! `python3 -c` in the repository root with the inputs on standard input), which stops at the
//! first input the relation does not hold for, and then minimizes that input: the harness is
//! handed every [`candidates`] of the current input and the first that still violates the
//! relation becomes the current one, until none does. Generation and minimization know only
//! the input type; the harness alone knows the language.

mod generate;
mod shrink;

use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, Evaluated, FactKind, LangId, Langs, Location,
    ProduceCtx, Producer, ProducerId, RELATIONS_KEY, Relation, RelationResult, RelationResults,
    Result, Scope, ToolId, ToolProbe, Violation, find_program, relations,
};
use cupel_core::{Process, output_tail};
use serde::Deserialize;
use serde_json::{Value, json};

pub use generate::{generate, relation_seed};
pub use shrink::candidates;

use crate::toolchain;

const ID: &str = "engine.metamorphic";

/// The one language with a harness.
const LANG: &str = "python";

/// The program the harness runs with.
const PROGRAM: &str = "python3";

const HARNESS: &str = include_str!("harness.py");

/// Inputs generated per relation when neither `--cases` nor the relation's `cases` says.
pub const DEFAULT_CASES: u64 = 100;

/// The most minimization steps taken from one violating input.
const MAX_SHRINKS: u32 = 200;

/// Evaluates metamorphic relations (ADR-097).
pub struct MetamorphicEngine;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<RelationResults, _>(MetamorphicEngine),
});

impl Producer<RelationResults> for MetamorphicEngine {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from(LANG)])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    /// `unconfigured` until `[lang.python.relations]` names a relation; then whether `python3`
    /// is installed.
    fn availability(&self, ctx: &Ctx) -> Availability {
        let configured = ctx
            .lang
            .get(&LangId::from(LANG))
            .and_then(|table| table.get(RELATIONS_KEY)?.as_object())
            .is_some_and(|relations| !relations.is_empty());
        if !configured {
            return Availability::Unconfigured {
                key: format!("lang.{LANG}.{RELATIONS_KEY}"),
            };
        }
        if find_program(PROGRAM).is_some() {
            Availability::Ready
        } else {
            Availability::MissingTool {
                name: ToolId::from(PROGRAM),
                install: toolchain::install(&self.id(), PROGRAM, &ctx.root),
            }
        }
    }

    fn tool(&self, _: &Ctx) -> Option<ToolProbe> {
        Some(ToolProbe {
            name: ToolId::from(PROGRAM),
            version: vec![PROGRAM.to_owned(), "--version".to_owned()],
        })
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        _: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<RelationResults> {
        let configured =
            relations(&LangId::from(LANG), &ctx.config).map_err(|problem| Error::Process {
                program: PROGRAM.to_owned(),
                message: problem,
            })?;
        let mut evaluated = Vec::with_capacity(configured.len());
        for (name, relation) in &configured {
            ctx.cancel.checkpoint()?;
            evaluated.push(Harness { ctx, name, relation }.evaluate()?);
        }
        Ok(RelationResults {
            seed: ctx.seed,
            relations: evaluated,
        })
    }
}

/// One relation, evaluated through the harness.
struct Harness<'a> {
    ctx: &'a ProduceCtx,
    name: &'a str,
    relation: &'a Relation,
}

/// What the harness printed for one batch.
#[derive(Deserialize)]
#[serde(untagged)]
enum Response {
    Evaluated(Batch),
    Failed { error: String },
}

#[derive(Deserialize)]
struct Batch {
    location: Option<Location>,
    /// How many inputs were evaluated: up to and including the first violation.
    evaluated: u64,
    violation: Option<Found>,
}

/// The first input of a batch the relation does not hold for.
#[derive(Debug, Clone, Deserialize)]
struct Found {
    index: usize,
    transformed: Evaluated,
    source: Evaluated,
    followup: Option<Evaluated>,
    relation_raised: Option<String>,
}

impl Harness<'_> {
    /// The relation on its case budget's inputs, its first violation minimized.
    fn evaluate(&self) -> Result<RelationResult> {
        let budget = self
            .ctx
            .focus
            .cases
            .or(self.relation.cases)
            .unwrap_or(DEFAULT_CASES);
        let seed = relation_seed(self.ctx.seed, self.name);
        let inputs: Vec<Value> = (0..budget)
            .map(|case| generate(&self.relation.input, seed, case))
            .collect();
        let batch = self.run(&inputs)?;
        let violation = match batch.violation {
            Some(found) => Some(self.minimized(&inputs, found)?),
            None => None,
        };
        Ok(RelationResult {
            name: self.name.to_owned(),
            function: self.relation.function.clone(),
            input: self.relation.input.to_string(),
            cases: batch.evaluated,
            location: batch.location,
            violation,
        })
    }

    /// The violation `found` among `inputs`, after minimization.
    fn minimized(&self, inputs: &[Value], found: Found) -> Result<Violation> {
        let case = found.index as u64;
        let original = inputs[found.index].clone();
        let (mut input, mut found, mut shrinks) = (original.clone(), found, 0);
        while shrinks < MAX_SHRINKS {
            let simpler = candidates(&self.relation.input, &input);
            if simpler.is_empty() {
                break;
            }
            let Some(next) = self.run(&simpler)?.violation else {
                break;
            };
            input = simpler[next.index].clone();
            found = next;
            shrinks += 1;
        }
        Ok(Violation {
            case,
            input,
            original,
            shrinks,
            transformed: found.transformed,
            source: found.source,
            followup: found.followup,
            relation_raised: found.relation_raised,
        })
    }

    /// The harness on `inputs`: their outcome up to the first violation.
    fn run(&self, inputs: &[Value]) -> Result<Batch> {
        let request = json!({
            "function": self.relation.function,
            "transform": self.relation.transform,
            "relation": self.relation.relation,
            "spread": self.relation.input.spreads(),
            "inputs": inputs,
        });
        let captured = Process::new(PROGRAM, &self.ctx.ctx.root)
            .args(["-c", HARNESS])
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .input(request.to_string().into_bytes())
            .run(&self.ctx.cancel)?;
        let response = captured
            .stdout
            .lines()
            .last()
            .and_then(|line| serde_json::from_str::<Response>(line).ok());
        match (captured.exit_code, response) {
            (Some(0), Some(Response::Evaluated(batch))) => Ok(batch),
            (_, Some(Response::Failed { error })) => Err(self.failed(error)),
            (code, _) => Err(self.failed(format!(
                "the harness exited with {} without an outcome: {}",
                code.map_or_else(|| "a signal".to_owned(), |code| format!("code {code}")),
                output_tail(&captured.stderr).unwrap_or_default()
            ))),
        }
    }

    fn failed(&self, problem: String) -> Error {
        Error::Process {
            program: PROGRAM.to_owned(),
            message: format!("relation `{}`: {problem}", self.name),
        }
    }
}
