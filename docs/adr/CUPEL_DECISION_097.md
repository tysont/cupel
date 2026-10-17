# CUPEL_DECISION_097: Metamorphic relations: where they are declared, the `RelationResults` fact, and the engine that evaluates them

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.5

## Context

Design §5.3: `gen.metamorphic` evaluates "user-declared metamorphic relations in config (`sort(x) == sort(shuffle(x))`)" on generated inputs and reports `metamorphic.violated`; cost `deep`, tier 4, "build". The catalog (§8.3.2) lists `DivergenceResults` ("per generated input: both outputs, minimized", produced by the differential engine) as consumed by both `gen.differential` and `gen.metamorphic`. Nothing says where a relation is written, what names the function under test, how inputs are generated or minimized, or how a check that never sees a language or a process (§8.3.1, §6.7: "a check never reads `[lang.*]`; a producer never reads `[checks.*]`") gets a relation evaluated in Python or TypeScript.

A relation has three language-specific parts (the function under test, the input transformation, and the relation between the two outputs) and one part that need not be (what the inputs are). Evaluating it means running the project's code, which only a producer may do.

## Options

**Where relations are declared.** (a) `[checks."gen.metamorphic"]`: the check's table, but the check cannot run code, and the parts written in a language would then be read by a check; it also breaks the rule that a check's settings are language-neutral. (b) A top-level `[relations.<name>]` with a `lang` key, like `[oracles.<name>]` (ADR-090): validated at load, but no producer sees it (`ProduceCtx` carries the language's `[lang.<id>]` table only), so a new path from the configuration to producers would be needed. (c) `[lang.<id>.relations.<name>]`: the relation is a fact about how to exercise that language's code, read by the producer that already gets the table, and absent from every other language's.

**The fact.** (a) `DivergenceResults`, as the catalog says. But the registry selects one producer per (kind, language): the metamorphic engine and P5.3's differential engine could not both provide it for one language without the check of one reading the other's results; and a metamorphic violation has a transformed input and a relation the differential shape has no place for. (b) A sibling kind for relations only: one catalog row and this ADR (plan §2.5), an execution fact with no conformance inventory, as `TestReruns` (ADR-078).

**Where inputs are generated and minimized.** (a) In each language's harness (Hypothesis-like strategies in Python, fast-check in TypeScript): idiomatic, but every language reimplements generation, seeding, and shrinking, and the results differ per language for the same seed. (b) In Rust, over a small language-neutral type grammar whose values are JSON: one generator and one shrinker for every language, deterministic from the run's seed; the harness only converts JSON to the language's values and evaluates.

**How the harness runs.** As a file written under `.cupel/` and run by the language's launcher, or as source embedded in the binary and run by the interpreter (`python3 -c`) with the request on standard input. The second writes nothing and needs no launcher; `Process` (ADR-076 §3) already takes input, captures output, and honours cancellation.

## Decision

1. **Relations are declared in `[lang.<id>.relations.<name>]`**, read by the language's metamorphic producer. Names match `[a-z][a-z0-9_-]*`. Keys, exactly:
   - `function` (required): the function under test, in the harness's notation. Python: `module:name` (`name` may be dotted), imported with the repository root on `sys.path`.
   - `input` (required): the type of the generated inputs: `bool`, `int`, `str`, `list[T]`, or `tuple[T, ...]` (any spacing; printed canonically). A tuple is spread into the function's positional arguments.
   - `transform` (required): an expression in the language for a callable from an input to its follow-up input. Python: evaluated in the function's module namespace.
   - `relation` (optional): an expression for a callable from the two outputs (source, follow-up) to whether the relation holds; default equality (`==`).
   - `cases` (optional, a positive integer): how many inputs to generate; default 100.

   The table's shape is checked when `cupel.toml` loads (`cupel_core::relations`, called for every `[lang.<id>]` table): an unknown key, a missing or empty string, an input type outside the grammar, a non-positive `cases`, or a bad name is a usage error (exit 2) naming the relation and `[lang.<id>.relations]`. Whether the expressions are valid is the harness's to say (decision 2).

2. **The engine, `engine.metamorphic`** (`cupel-adapt/src/metamorphic/`), produces `RelationResults` for Python, needs nothing, costs `deep`, and is `unconfigured` (key `lang.python.relations`) until a relation is declared, then `unavailable(python3)` without the interpreter. For each relation, by name, it generates the inputs, hands them in one batch to the harness (`harness.py`, embedded, run as `python3 -c` in the repository root with `PYTHONDONTWRITEBYTECODE=1`, the request on standard input), and the harness prints one JSON line: the function's location (file relative to the root, first and last line of its definition, when `inspect` finds it there), how many inputs it evaluated, and the first input the relation did not hold for. On each input it calls the function, the transform, and the function on the follow-up input, each on a deep copy, with anything printed sent to standard error. **The relation is violated** when any of the three raises, or the relation returns false or raises. A function that cannot be imported, a missing name, or an expression that does not evaluate to a callable is a setup failure: the engine's error, `relation `<name>`: …`, which the scheduler records as the check's `status: error`.

3. **Generation** is language-neutral and deterministic: relation `<name>` draws with `relation_seed(seed, name)` (FNV-1a of the name mixed with the run's seed, so adding a relation changes no other's inputs); case `k`'s value depends on that seed and `k` alone (splitmix64), at size `min(8, 1 + k/4)`: integers in `±16·size²`, strings and lists of length `0..=size` (strings from `abcxyz019 _-`). Small cases come first, so a relation that fails on small inputs fails early.

4. **Minimization** starts from the first violating input and repeatedly hands the harness every simpler candidate of the current input, simplest first, taking the first that still violates, until none does (at most 200 steps): integers toward 0 (`0`, the positive of a negative, half, one step toward zero), `true` to `false`, strings and lists to empty, either half, one element dropped, then each element simpler (characters to `a`); tuples element by element. Each candidate is a value of the input type.

5. **The fact, `relation-results`** (`FactKind::RelationResults`, payload `cupel_core::RelationResults`, version 1), providable by producer plugins like every kind with a payload (ADR-088 §8):

```text
RelationResults {
  seed: u64,                               // the run's seed
  relations: [RelationResult {             // every configured relation, by name
    name, function, input: String,         // as configured; input printed canonically
    cases: u64,                            // inputs evaluated: all, or up to the first violation
    location: Option<Location>,            // the function's definition, when the harness finds it in the repository
    violation: Option<Violation {
      case: u64,                           // the generated case that first violated the relation
      input: Value, original: Value,       // minimized, and as generated
      shrinks: u32,
      transformed: Evaluated,              // the transform on `input`
      source: Evaluated,                   // the function on `input`
      followup: Option<Evaluated>,         // the function on the follow-up input; null when the transform raised
      relation_raised: Option<String>,
    }>,
  }],
}
Evaluated = {"value": JSON} | {"raised": "Type: message"} | {"repr": "…"}   // a value JSON cannot hold
```

## Consequences

- The catalog gains a row the design did not list, and `DivergenceResults` stays the differential engine's alone; a future engine that evaluates both kinds of comparison could produce both facts.
- Only Python has a harness. Another language is one harness (its source and its interpreter) and its relations' notation; generation and minimization are shared. Until then `gen.metamorphic` is `unsupported(relation-results)` for the other four languages, and a relation declared under `[lang.<other>]` is checked for shape and otherwise unused.
- Functions are imported from the repository root. A package under `src/` or in a monorepo root needs its module path from the root; a per-relation or per-language import root is a follow-up.
- `describe matrix` and the README's matrix compute support outside any configuration, so they show `gen.metamorphic` for Python as `unconfigured(lang.python.relations)`; a run in a repository that declares relations runs it.
- Every relation is evaluated whatever the scope; the check reports only those whose function is defined in a file in scope (ADR-098). Narrowing the evaluation itself to the scope is a follow-up.
- The input grammar has no floats, maps, or bounded ranges; relations over them need the grammar extended (and the shrinker with it).
