// ABOUTME: `tool.tlc` (ADR-116): validates each recorded trace against the run's TLA+ spec with TLC, through a module
// ABOUTME: that extends the spec and constrains each step to the logged state, read back as conforms, diverged, or failed.
//! TLC as a trace validator.
//!
//! For a trace of `n` logged steps, [`module`] writes `CupelTrace.tla`, which extends the spec
//! module, adds the variable `cupel_step`, holds the logged states as a sequence of records,
//! and allows from each state only the spec's step (its `Next`, or the action the step named)
//! to the next logged state, on the variables the step logged. TLC explores that from the
//! spec's `Init` constrained to the first logged state, with the invariant `cupel_step < n`:
//! the invariant fails exactly when the whole trace is a behavior of the spec. Otherwise the
//! search ends, deadlock unchecked, at the deepest step it matched, and the next one is where
//! the trace diverged ([`verdict`]).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, FactKind, Install, Langs, Location, Process,
    ProduceCtx, Producer, ProducerId, Result, Scope, TestResults, ToolProbe, Trace,
    TraceDivergence, TraceSpec, TraceStatus, Traces, output_tail,
};
use serde_json::{Map, Value};

use super::log::{self, Logged};
use crate::runner::{on_path, probe};
use crate::tree::{canonical, relative};

const ID: &str = "tool.tlc";

/// The program run, as tla-bin and most installations name the TLC launcher.
const PROGRAM: &str = "tlc";

/// The generated module, and the configuration TLC reads beside it.
const MODULE: &str = "CupelTrace";

/// The spec language TLC reads.
const LANG: &str = "tla+";

/// Validates recorded traces against a TLA+ spec with TLC.
pub struct Tlc;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<Traces, _>(Tlc),
});

impl Producer<Traces> for Tlc {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    /// Every language whose tests a runner runs: the logging convention is a file format.
    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::TestResults]
    }

    fn cost(&self) -> CostClass {
        CostClass::Exhaustive
    }

    fn availability(&self, _: &Ctx) -> Availability {
        on_path(
            PROGRAM,
            Install::Hint(
                "TLC is in the TLA+ tools (https://github.com/tlaplus/tlaplus/releases): put a \
                 `tlc` script on PATH that runs `java -cp tla2tools.jar tlc2.TLC \"$@\"`, as \
                 tla-bin installs it."
                    .to_owned(),
            ),
        )
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        Some(probe(PROGRAM, false, &[PROGRAM, "-h"], &ctx.root))
    }

    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, _: &mut dyn EventSink) -> Result<Traces> {
        let mut fact = Traces {
            tool: self.id(),
            spec: None,
            problem: None,
            traces: Vec::new(),
        };
        let spec = match super::spec(ctx, LANG, ID) {
            Ok(spec) => spec,
            Err(problem) => {
                fact.problem = Some(problem);
                return Ok(fact);
            }
        };
        fact.spec = Some(spec.clone());
        let Some(baseline) = ctx.upstream.get::<TestResults>() else {
            fact.problem = Some("no test run to record traces from".to_owned());
            return Ok(fact);
        };
        let recorded = super::record(ctx, baseline)?;
        fact.problem = recorded.problem;
        let root = canonical(&ctx.ctx.root);
        for (name, text) in &recorded.traces {
            ctx.cancel.checkpoint()?;
            let steps = match log::read(text) {
                Ok(steps) => steps,
                Err(detail) => {
                    fact.traces.push(failed(name, 0, detail));
                    continue;
                }
            };
            let files: Vec<String> = steps
                .iter()
                .filter_map(|step| relative(&root, &root, step.file.as_deref()?))
                .collect();
            if super::in_scope(&files, scope) {
                fact.traces.push(validate(ctx, &spec, name, &steps)?);
            }
        }
        Ok(fact)
    }
}

/// A trace that could not be validated.
fn failed(name: &str, steps: usize, detail: String) -> Trace {
    Trace {
        name: name.to_owned(),
        steps: steps as u64,
        status: TraceStatus::Failed,
        detail: Some(detail),
        divergence: None,
    }
}

/// Runs TLC on the trace `steps` in a fresh `.cupel/cache/trace/tlc/<name>/` holding the spec's
/// directory's modules, the generated module, and its configuration.
fn validate(ctx: &ProduceCtx, spec: &TraceSpec, name: &str, steps: &[Logged]) -> Result<Trace> {
    let file = ctx.ctx.root.join(&spec.file);
    let Some(spec_module) = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
    else {
        return Ok(failed(
            name,
            steps.len(),
            "the spec oracle names no file".to_owned(),
        ));
    };
    let config = fs::read_to_string(file.with_extension("cfg")).ok();
    let names = SpecConfig::read(config.as_deref());
    let generated = match module(&spec_module, &names, steps) {
        Ok(generated) => generated,
        Err(detail) => return Ok(failed(name, steps.len(), detail)),
    };
    let work = super::fresh(
        ctx,
        &ctx.writer
            .cache_dir()
            .join("trace")
            .join("tlc")
            .join(name.replace(['/', '\\'], "_")),
    )?;
    for module in modules(file.parent().unwrap_or(&ctx.ctx.root)) {
        if let Some(base) = module.file_name() {
            let text = fs::read(&module).map_err(|error| io_error(&module, &error))?;
            ctx.writer.write(&work.join(base), &text)?;
        }
    }
    ctx.writer
        .write(&work.join(format!("{MODULE}.tla")), generated.as_bytes())?;
    ctx.writer.write(
        &work.join(format!("{MODULE}.cfg")),
        names.config().as_bytes(),
    )?;
    let ran = tlc(&work).run(&ctx.cancel)?;
    Ok(verdict(&ran.stdout).trace(&ctx.ctx.root, name, steps))
}

/// TLC on the generated module in `work`, deadlock unchecked, its states under `work/states`.
fn tlc(work: &Path) -> Process {
    let states = work.join("states").to_string_lossy().into_owned();
    Process::new(PROGRAM, work)
        .args([
            "-deadlock",
            "-workers",
            "1",
            "-metadir",
            &states,
            "-config",
            &format!("{MODULE}.cfg"),
            &format!("{MODULE}.tla"),
        ])
        .merged()
}

impl Verdict {
    /// The trace `name` of `steps` as this verdict says it ended; a divergence is placed where
    /// it was logged in the repository at `root`.
    fn trace(self, root: &Path, name: &str, steps: &[Logged]) -> Trace {
        let (status, divergence) = match self {
            Self::Conforms => (TraceStatus::Conforms, None),
            Self::Diverged { matched } => (
                TraceStatus::Diverged,
                Some(divergence(root, steps, matched)),
            ),
            Self::Failed(detail) => return failed(name, steps.len(), detail),
        };
        Trace {
            name: name.to_owned(),
            steps: steps.len() as u64,
            status,
            detail: None,
            divergence,
        }
    }
}

/// The `.tla` files in the spec's directory, which the spec may extend or instantiate.
fn modules(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found: Vec<std::path::PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "tla"))
        .collect();
    found.sort();
    found
}

fn io_error(path: &Path, error: &std::io::Error) -> Error {
    Error::Process {
        program: PROGRAM.to_owned(),
        message: format!("cannot read {}: {error}", path.display()),
    }
}

/// The divergence after `matched` steps the spec allows: the next step, its state and action,
/// and the state before it.
fn divergence(root: &Path, steps: &[Logged], matched: usize) -> TraceDivergence {
    let at = matched.min(steps.len().saturating_sub(1));
    let step = &steps[at];
    let root = canonical(root);
    let location = step
        .file
        .as_deref()
        .and_then(|file| relative(&root, &root, file))
        .map(|file| {
            let line = step.line.unwrap_or(1);
            Location {
                file,
                line,
                col: None,
                end_line: line,
                end_col: None,
            }
        });
    TraceDivergence {
        step: at as u64 + 1,
        action: step.action.clone(),
        state: Value::Object(step.state.clone()),
        previous: at
            .checked_sub(1)
            .map(|before| Value::Object(steps[before].state.clone())),
        location,
    }
}

/// What the spec's own configuration says: its initial predicate, next-state relation, and
/// constant assignments, which the generated configuration repeats.
#[derive(Debug, PartialEq, Eq)]
struct SpecConfig {
    init: String,
    next: String,
    /// The `CONSTANT` and `CONSTANTS` sections, verbatim.
    constants: Vec<String>,
}

/// The keywords that start a section of a TLC configuration.
const KEYWORDS: [&str; 18] = [
    "CONSTANT",
    "CONSTANTS",
    "INIT",
    "NEXT",
    "SPECIFICATION",
    "INVARIANT",
    "INVARIANTS",
    "PROPERTY",
    "PROPERTIES",
    "SYMMETRY",
    "VIEW",
    "CONSTRAINT",
    "CONSTRAINTS",
    "ACTION_CONSTRAINT",
    "ACTION_CONSTRAINTS",
    "CHECK_DEADLOCK",
    "POSTCONDITION",
    "ALIAS",
];

impl SpecConfig {
    /// The spec's `<Spec>.cfg`, read line by line: a section starts at a line whose first word
    /// is a keyword. Without one, or without `INIT` and `NEXT`, the spec's `Init` and `Next`.
    fn read(text: Option<&str>) -> Self {
        let mut config = Self {
            init: "Init".to_owned(),
            next: "Next".to_owned(),
            constants: Vec::new(),
        };
        let mut section = "";
        for line in text.unwrap_or_default().lines() {
            let line = line.split("\\*").next().unwrap_or_default().trim();
            let mut words = line.split_whitespace();
            let Some(first) = words.next() else { continue };
            let rest = if KEYWORDS.contains(&first) {
                section = first;
                words.next()
            } else {
                Some(first)
            };
            match (section, rest) {
                ("INIT", Some(name)) => config.init = name.to_owned(),
                ("NEXT", Some(name)) => config.next = name.to_owned(),
                ("CONSTANT" | "CONSTANTS", _) => config.constants.push(
                    line.strip_prefix(section)
                        .filter(|_| first == section)
                        .unwrap_or(line)
                        .trim()
                        .to_owned(),
                ),
                _ => {}
            }
        }
        config.constants.retain(|line| !line.is_empty());
        config
    }

    /// The generated module's configuration.
    fn config(&self) -> String {
        let mut text =
            format!("INIT {MODULE}Init\nNEXT {MODULE}Next\nINVARIANT {MODULE}Unmatched\n");
        if !self.constants.is_empty() {
            text.push_str("CONSTANTS\n");
            for line in &self.constants {
                text.push_str("    ");
                text.push_str(line);
                text.push('\n');
            }
        }
        text
    }
}

/// The module that checks `steps` against the spec module `spec`; why it cannot be written when
/// a logged name is not a TLA+ identifier or a value has no TLA+ counterpart.
fn module(spec: &str, names: &SpecConfig, steps: &[Logged]) -> std::result::Result<String, String> {
    let (variables, actions) = logged_names(steps)?;
    let states = steps
        .iter()
        .map(|step| record(&step.state))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let logged: Vec<String> = steps
        .iter()
        .map(|step| string(step.action.as_deref().unwrap_or_default()))
        .collect();
    Ok(format!(
        "---- MODULE {MODULE} ----\n\
         EXTENDS {spec}, Integers, Sequences, TLC\n\
         \n\
         VARIABLE cupel_step\n\
         \n\
         {MODULE} == <<\n    {}\n>>\n\
         \n\
         {MODULE}Action == <<{}>>\n\
         \n\
         {MODULE}Init ==\n    /\\ cupel_step = 1\n    /\\ {}\n{}\
         \n\
         {MODULE}Next ==\n    /\\ cupel_step < Len({MODULE})\n    /\\ cupel_step' = cupel_step + 1\n{}{}\
         \n\
         {MODULE}Unmatched == cupel_step < Len({MODULE})\n\
         ====\n",
        states.join(",\n    "),
        logged.join(", "),
        names.init,
        constrained(&variables, "1", ""),
        relation(&actions, &names.next),
        constrained(&variables, "cupel_step + 1", "'"),
    ))
}

/// The variables the steps log and the actions they name after the first, each checked to be
/// a TLA+ identifier.
fn logged_names(steps: &[Logged]) -> std::result::Result<(BTreeSet<&str>, BTreeSet<&str>), String> {
    let variables: BTreeSet<&str> = steps
        .iter()
        .flat_map(|step| step.state.keys())
        .map(String::as_str)
        .collect();
    let actions: BTreeSet<&str> = steps
        .iter()
        .skip(1)
        .filter_map(|step| step.action.as_deref())
        .collect();
    for variable in &variables {
        identifier(variable, "a logged variable")?;
    }
    for action in &actions {
        identifier(action, "a logged action")?;
    }
    Ok((variables, actions))
}

/// One conjunct per variable: when the logged state at `index` has it, the variable (primed by
/// `prime`) equals it.
fn constrained(variables: &BTreeSet<&str>, index: &str, prime: &str) -> String {
    variables
        .iter()
        .map(|variable| {
            format!(
                "    /\\ (\"{variable}\" \\in DOMAIN {MODULE}[{index}] => \
                 {variable}{prime} = {MODULE}[{index}][\"{variable}\"])\n"
            )
        })
        .collect()
}

/// The step each logged state takes: the action it names, else the spec's `next`.
fn relation(actions: &BTreeSet<&str>, next: &str) -> String {
    if actions.is_empty() {
        return format!("    /\\ {next}\n");
    }
    let mut case = String::from("    /\\ CASE ");
    for action in actions {
        case.push_str(&format!(
            "{MODULE}Action[cupel_step + 1] = \"{action}\" -> {action}\n         [] "
        ));
    }
    case.push_str(&format!("OTHER -> {next}\n"));
    case
}

/// `name` when it is a TLA+ identifier (letters, digits, `_`, not only digits).
fn identifier(name: &str, what: &str) -> std::result::Result<(), String> {
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.chars().any(|c| c.is_ascii_alphabetic());
    if valid {
        Ok(())
    } else {
        Err(format!("{what} `{name}` is not a TLA+ identifier"))
    }
}

/// A logged state as a TLA+ record.
fn record(state: &Map<String, Value>) -> std::result::Result<String, String> {
    if state.is_empty() {
        return Ok("[cupel_none \\in {} |-> 0]".to_owned());
    }
    let fields = state
        .iter()
        .map(|(name, value)| {
            identifier(name, "a record field")?;
            Ok(format!("{name} |-> {}", tla(value)?))
        })
        .collect::<std::result::Result<Vec<String>, String>>()?;
    Ok(format!("[{}]", fields.join(", ")))
}

/// A JSON value as a TLA+ value: an integer, a string, a boolean, a sequence, a record, or the
/// TLA+ expression an object `{"$tla": "…"}` gives (a set, a model value, a function).
fn tla(value: &Value) -> std::result::Result<String, String> {
    match value {
        Value::Bool(true) => Ok("TRUE".to_owned()),
        Value::Bool(false) => Ok("FALSE".to_owned()),
        Value::Number(number) if number.is_i64() || number.is_u64() => Ok(number.to_string()),
        Value::Number(number) => Err(format!("TLA+ has no value for the number {number}")),
        Value::String(text) => Ok(string(text)),
        Value::Array(items) => {
            let items = items
                .iter()
                .map(tla)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(format!("<<{}>>", items.join(", ")))
        }
        Value::Object(object) => match (object.len(), object.get("$tla")) {
            (1, Some(Value::String(expression))) => Ok(expression.clone()),
            _ => record(object),
        },
        Value::Null => Err("TLA+ has no value for null".to_owned()),
    }
}

/// `text` as a TLA+ string literal.
fn string(text: &str) -> String {
    let mut literal = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            '\n' => literal.push_str("\\n"),
            '\t' => literal.push_str("\\t"),
            '\r' => literal.push_str("\\r"),
            c => literal.push(c),
        }
    }
    literal.push('"');
    literal
}

/// How TLC's run on a generated module ended.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// The invariant failed: the whole trace is a behavior of the spec.
    Conforms,
    /// The search ended after the spec allowed `matched` steps of the trace.
    Diverged { matched: usize },
    /// TLC could not check it: why.
    Failed(String),
}

/// What TLC's merged `output` says.
fn verdict(output: &str) -> Verdict {
    if output.contains(&format!("Invariant {MODULE}Unmatched is violated")) {
        return Verdict::Conforms;
    }
    if output.contains("Model checking completed. No error has been found.") {
        let distinct = count_before(output, " distinct states found");
        let depth = count_after(output, "The depth of the complete state graph search is ");
        if let (Some(distinct), Some(depth)) = (distinct, depth) {
            return Verdict::Diverged {
                matched: if distinct == 0 { 0 } else { depth },
            };
        }
    }
    Verdict::Failed(problem(output))
}

/// The number ending just before `marker` on the line holding it (`4 distinct states found`).
fn count_before(output: &str, marker: &str) -> Option<usize> {
    let line = output.lines().find(|line| line.contains(marker))?;
    let (before, _) = line.split_once(marker)?;
    number(before.split_whitespace().last()?)
}

/// The number starting just after `marker` (`… search is 2.`).
fn count_after(output: &str, marker: &str) -> Option<usize> {
    let line = output.lines().find(|line| line.contains(marker))?;
    let (_, after) = line.split_once(marker)?;
    number(after.trim_end_matches('.'))
}

/// A count as TLC prints it, with or without thousands separators.
fn number(text: &str) -> Option<usize> {
    text.trim().replace(',', "").parse().ok()
}

/// Why TLC could not check the module: the parser's errors, else TLC's `Error:` lines, else the
/// output's tail.
fn problem(output: &str) -> String {
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    if let Some(start) = lines
        .iter()
        .position(|line| line.starts_with("*** Errors:"))
    {
        let errors: Vec<&str> = lines[start + 1..]
            .iter()
            .take_while(|line| !line.starts_with("Starting..."))
            .filter(|line| !line.is_empty())
            .copied()
            .collect();
        if !errors.is_empty() {
            return errors.join(" ");
        }
    }
    let errors: Vec<&str> = lines
        .iter()
        .filter(|line| line.starts_with("Error:"))
        .copied()
        .collect();
    if errors.is_empty() {
        output_tail(output).unwrap_or_else(|| "TLC printed nothing".to_owned())
    } else {
        errors.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;
    use serde_json::json;

    use super::*;

    fn steps(lines: &[Value]) -> Vec<Logged> {
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        log::read(&text.join("\n")).unwrap()
    }

    fn names() -> SpecConfig {
        SpecConfig::read(None)
    }

    #[test]
    fn the_module_extends_the_spec_and_allows_only_each_logged_step() {
        let logged = steps(&[
            json!({"state": {"count": 0, "done": false}}),
            json!({"action": "Inc", "state": {"count": 1}}),
            json!({"state": {"done": true}}),
        ]);
        assert_eq!(
            module("Counter", &names(), &logged).unwrap(),
            "---- MODULE CupelTrace ----\n\
             EXTENDS Counter, Integers, Sequences, TLC\n\
             \n\
             VARIABLE cupel_step\n\
             \n\
             CupelTrace == <<\n    \
             [count |-> 0, done |-> FALSE],\n    \
             [count |-> 1],\n    \
             [done |-> TRUE]\n\
             >>\n\
             \n\
             CupelTraceAction == <<\"\", \"Inc\", \"\">>\n\
             \n\
             CupelTraceInit ==\n    \
             /\\ cupel_step = 1\n    \
             /\\ Init\n    \
             /\\ (\"count\" \\in DOMAIN CupelTrace[1] => count = CupelTrace[1][\"count\"])\n    \
             /\\ (\"done\" \\in DOMAIN CupelTrace[1] => done = CupelTrace[1][\"done\"])\n\
             \n\
             CupelTraceNext ==\n    \
             /\\ cupel_step < Len(CupelTrace)\n    \
             /\\ cupel_step' = cupel_step + 1\n    \
             /\\ CASE CupelTraceAction[cupel_step + 1] = \"Inc\" -> Inc\n         \
             [] OTHER -> Next\n    \
             /\\ (\"count\" \\in DOMAIN CupelTrace[cupel_step + 1] => count' = \
             CupelTrace[cupel_step + 1][\"count\"])\n    \
             /\\ (\"done\" \\in DOMAIN CupelTrace[cupel_step + 1] => done' = \
             CupelTrace[cupel_step + 1][\"done\"])\n\
             \n\
             CupelTraceUnmatched == cupel_step < Len(CupelTrace)\n\
             ====\n"
        );
    }

    #[test]
    fn without_named_actions_every_step_is_the_specs_next_state_relation() {
        let logged = steps(&[json!({"state": {"x": 0}}), json!({"state": {"x": 1}})]);
        let names = SpecConfig::read(Some("INIT Start\nNEXT\n  Step\n"));
        let text = module("Spec", &names, &logged).unwrap();
        let relations: Vec<&str> = text
            .lines()
            .filter(|line| line.starts_with("    /\\ S") || line.contains("CASE"))
            .collect();
        assert_eq!(relations, ["    /\\ Start", "    /\\ Step"]);
    }

    #[test]
    fn values_map_to_tla_and_what_has_no_counterpart_is_refused() {
        let mapped = [
            json!(-3),
            json!("a \"q\"\n"),
            json!([1, [true]]),
            json!({"to": "b", "seen": {"$tla": "{1, 2}"}}),
        ]
        .map(|value| tla(&value).unwrap());
        assert_eq!(
            mapped,
            [
                "-3",
                "\"a \\\"q\\\"\\n\"",
                "<<1, <<TRUE>>>>",
                "[seen |-> {1, 2}, to |-> \"b\"]",
            ]
        );
        assert_eq!(
            tla(&json!(1.5)).unwrap_err(),
            "TLA+ has no value for the number 1.5"
        );
        assert_eq!(tla(&json!(null)).unwrap_err(), "TLA+ has no value for null");
        let refused = module("Spec", &names(), &steps(&[json!({"state": {"a-b": 1}})]));
        assert_eq!(
            refused.unwrap_err(),
            "a logged variable `a-b` is not a TLA+ identifier"
        );
    }

    #[test]
    fn the_specs_configuration_gives_init_next_and_constants() {
        let config = SpecConfig::read(Some(
            "\\* the model\nCONSTANTS N = 3\n    Procs = {p1, p2}\nINIT Begin \\* start\nNEXT Step\n\
             INVARIANT TypeOK\nCONSTANT Max = 9\n",
        ));
        assert_eq!(
            config,
            SpecConfig {
                init: "Begin".to_owned(),
                next: "Step".to_owned(),
                constants: vec![
                    "N = 3".to_owned(),
                    "Procs = {p1, p2}".to_owned(),
                    "Max = 9".to_owned()
                ],
            }
        );
        assert_eq!(
            config.config(),
            "INIT CupelTraceInit\nNEXT CupelTraceNext\nINVARIANT CupelTraceUnmatched\n\
             CONSTANTS\n    N = 3\n    Procs = {p1, p2}\n    Max = 9\n"
        );
        assert_eq!(SpecConfig::read(None).config().lines().count(), 3);
    }

    #[test]
    fn a_violated_invariant_is_a_trace_the_spec_allows() {
        let capture = Capture::of("trace/tlc", "conforms");
        assert_eq!(verdict(&capture.file("output.txt")), Verdict::Conforms);
    }

    #[test]
    fn a_search_that_ends_names_how_many_steps_the_spec_allowed() {
        let later = Capture::of("trace/tlc", "diverged");
        assert_eq!(
            verdict(&later.file("output.txt")),
            Verdict::Diverged { matched: 2 }
        );
        let initial = Capture::of("trace/tlc", "diverged-at-start");
        assert_eq!(
            verdict(&initial.file("output.txt")),
            Verdict::Diverged { matched: 0 }
        );
    }

    #[test]
    fn a_module_tlc_rejects_fails_with_the_parsers_errors() {
        let capture = Capture::of("trace/tlc", "unknown-action");
        assert_eq!(
            verdict(&capture.file("output.txt")),
            Verdict::Failed(
                "line 23, col 57 to line 23, col 59 of module CupelTrace Unknown operator: \
                 `Inc'."
                    .to_owned()
            )
        );
        assert_eq!(
            verdict("Error: TLC threw an unexpected exception.\nmore\n"),
            Verdict::Failed("Error: TLC threw an unexpected exception.".to_owned())
        );
    }

    #[test]
    fn counts_read_with_or_without_separators() {
        let output = "1,204 states generated, 1,204 distinct states found, 0 states left on queue.\n\
                      The depth of the complete state graph search is 1,203.\n";
        assert_eq!(count_before(output, " distinct states found"), Some(1204));
        assert_eq!(
            count_after(output, "The depth of the complete state graph search is "),
            Some(1203)
        );
    }

    #[test]
    fn a_divergence_holds_the_step_its_state_action_place_and_the_state_before() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir(repo.path().join("src")).unwrap();
        fs::write(repo.path().join("src/lib.rs"), "").unwrap();
        let logged = steps(&[
            json!({"state": {"count": 0}}),
            json!({"action": "Inc", "state": {"count": 2}, "file": "src/lib.rs", "line": 7}),
        ]);
        assert_eq!(
            divergence(repo.path(), &logged, 1),
            TraceDivergence {
                step: 2,
                action: Some("Inc".to_owned()),
                state: json!({"count": 2}),
                previous: Some(json!({"count": 0})),
                location: Some(Location {
                    file: "src/lib.rs".to_owned(),
                    line: 7,
                    col: None,
                    end_line: 7,
                    end_col: None,
                }),
            }
        );
        assert_eq!(divergence(repo.path(), &logged, 0).previous, None);
    }
}
