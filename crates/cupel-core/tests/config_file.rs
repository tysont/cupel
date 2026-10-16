// ABOUTME: Tests for parsing `cupel.toml`: the design §6.7 example, and the usage errors for bad files.
// ABOUTME: Unknown keys at every closed level must be rejected with the valid keys listed.
use std::path::Path;

use cupel_core::config::{
    ConfigError, ConfigFile, Format, GateCondition, GateKey, OracleKind, TimeLimit,
};
use cupel_core::{CheckId, CostClass, Error, LangId};
use serde_json::json;

/// The `cupel.toml` example from design §6.7, verbatim.
const DESIGN_EXAMPLE: &str = r#"
required_version = ">=0.4"
default_branch   = "main"
depth            = "standard"

[roots]                                  # monorepo; omit for single-root
rust   = { path = ".",      lang = "rust" }
web    = { path = "web",    lang = "typescript" }

[lang.rust]
test    = "cargo nextest run --workspace"
coverage = "cargo llvm-cov --lcov --output-path {out}"
bench   = "cargo bench --bench {name} -- --save-baseline {id}"
mutation = { strategy = "schemata", operators = "default" }

[lang.typescript]
test = "pnpm vitest run"
coverage = "pnpm vitest run --coverage --coverage.reporter=lcov"

[checks."static.clones"]
threshold = 0.82
min_lines = 4
min_nodes = 20

[checks."static.deps".layers]            # layering policy
order = ["domain", "app", "infra"]        # infra may depend on app may depend on domain, not reverse
map   = { domain = "src/domain/**", app = "src/app/**", infra = "src/infra/**" }

[checks."tests.mutation"]
timeout_factor = 10
exclude_tags   = ["no-mutate"]

[checks."tests.mutation".lang.python]    # per-language override of any check setting
operators = "extended"

[checks."static.complexity".lang.go]
cyclomatic_max = 15

[oracles.pg]
kind = "command"
cmd  = "psql -X -A -t -c {input}"

[gate.changed]                            # evaluated for --changed scopes
"tests.mutation.score"   = ">= 0.80"
"tests.weakening"        = "none"
"static.crap.max"        = "< 30"
"perf.bench.regression"  = "< 5%"
"static.api-surface"     = "no-breaking"

[gate.tree]                               # evaluated for whole-tree scopes
"tests.mutation.score"   = ">= 0.70"

[profile.ci]
depth = "deep"
format = "sarif"

[profile.loop]
depth = "quick"
budget = "20s"
"#;

fn parse(text: &str) -> Result<ConfigFile, Error> {
    ConfigFile::parse(text, Path::new("/repo/cupel.toml"))
}

/// Asserts that each text is rejected with exactly its problem: the last line of the message,
/// which locates the problem in the lines before it.
fn assert_problems(cases: &[(&str, &str)]) {
    let problems: Vec<String> = cases.iter().map(|(text, _)| problem(text)).collect();
    let expected: Vec<&str> = cases.iter().map(|(_, problem)| *problem).collect();
    assert_eq!(problems, expected);
}

/// The last line of the message `text` is rejected with.
fn problem(text: &str) -> String {
    let message = error_message(text);
    message.lines().last().unwrap_or_default().trim().to_owned()
}

fn error_message(text: &str) -> String {
    let err = parse(text).expect_err("the file should be rejected");
    assert!(
        matches!(err, Error::Config(ConfigError::Invalid { .. })),
        "{err:?}"
    );
    err.to_string()
}

#[test]
fn the_design_examples_settings_roots_languages_checks_and_oracles_parse() {
    let file = parse(DESIGN_EXAMPLE).unwrap();

    assert_eq!(file.required_version.as_deref(), Some(">=0.4"));
    assert_eq!(file.default_branch.as_deref(), Some("main"));
    assert_eq!(file.depth, Some(CostClass::Standard));

    assert_eq!(file.roots["web"].path, "web");
    assert_eq!(file.roots["web"].lang, LangId::from("typescript"));

    let rust = &file.lang[&LangId::from("rust")];
    assert_eq!(rust["test"], json!("cargo nextest run --workspace"));
    assert_eq!(
        rust["mutation"],
        json!({"strategy": "schemata", "operators": "default"})
    );

    let mutation = &file.checks[&CheckId::from("tests.mutation")];
    assert_eq!(mutation.params["timeout_factor"], json!(10));
    assert!(!mutation.params.contains_key("lang"));
    assert_eq!(
        mutation.lang[&LangId::from("python")]["operators"],
        json!("extended")
    );
    let deps = &file.checks[&CheckId::from("static.deps")];
    assert_eq!(
        deps.params["layers"]["order"],
        json!(["domain", "app", "infra"])
    );

    let pg = &file.oracles["pg"];
    assert_eq!(pg.kind, OracleKind::Command);
    assert_eq!(pg.cmd.as_deref(), Some("psql -X -A -t -c {input}"));
}

#[test]
fn the_design_examples_gates_and_profiles_parse() {
    let file = parse(DESIGN_EXAMPLE).unwrap();
    let changed = file.gate.changed.as_ref().unwrap();
    let rules: Vec<String> = changed.rules().iter().map(ToString::to_string).collect();
    assert_eq!(
        rules,
        [
            "tests.mutation.score >= 0.8",
            "tests.weakening none",
            "static.crap.max < 30",
            "perf.bench.regression < 5%",
            "static.api-surface no-breaking",
        ],
        "rules keep their written order"
    );
    assert_eq!(
        changed.rules()[1].key,
        GateKey {
            check: CheckId::from("tests.weakening"),
            metric: None
        }
    );
    assert_eq!(changed.rules()[1].condition, GateCondition::None);
    assert_eq!(file.gate.tree.as_ref().unwrap().rules().len(), 1);

    let ci = &file.profile["ci"];
    assert_eq!(ci.depth, Some(CostClass::Deep));
    assert_eq!(ci.format, Some(Format::Sarif));
    let budget: TimeLimit = "20s".parse().unwrap();
    assert_eq!(file.profile["loop"].budget, Some(budget));
}

#[test]
fn an_empty_file_is_all_defaults() {
    assert_eq!(parse("").unwrap(), ConfigFile::default());
}

#[test]
fn unknown_top_level_key_lists_the_valid_keys() {
    insta::assert_snapshot!(error_message("depth = \"quick\"\ncolour = \"never\"\n"));
}

#[test]
fn unknown_profile_key_lists_the_valid_keys() {
    insta::assert_snapshot!(error_message("[profile.ci]\ndepth = \"deep\"\nseed = 7\n"));
}

#[test]
fn unknown_keys_are_rejected_at_every_closed_level() {
    let cases = [
        (
            "[roots]\nweb = { path = \"web\", language = \"typescript\" }\n",
            "unknown field `language`, expected `path` or `lang`",
        ),
        (
            "[oracles.pg]\nkind = \"command\"\ncommand = \"psql\"\n",
            "unknown field `command`, expected one of `kind`, `ref`, `cmd`, `spec`, `lang`, `inputs`",
        ),
        (
            "[gate.pr]\n\"tests.weakening\" = \"none\"\n",
            "unknown field `pr`, expected `changed` or `tree`",
        ),
        (
            "[plugins]\nfoo = \"bar\"\n",
            "unknown field `plugins`, expected one of `required_version`, `default_branch`, `depth`, \
             `format`, `budget`, `scope`, `roots`, `lang`, `checks`, `oracles`, `gate`, `profile`",
        ),
    ];
    assert_problems(&cases);
}

#[test]
fn bad_values_are_usage_errors() {
    let cases = [
        (
            "depth = \"medium\"\n",
            "unknown variant `medium`, expected one of `quick`, `standard`, `deep`, `exhaustive`",
        ),
        (
            "format = \"xml\"\n",
            "unknown variant `xml`, expected one of `json`, `jsonl`, `text`, `sarif`",
        ),
        (
            "budget = \"soon\"\n",
            "invalid duration `soon`: expected a positive whole number followed by `ms`, `s`, `m`, or `h`, such as `20s`",
        ),
        (
            "[oracles.pg]\nkind = \"sql\"\n",
            "unknown variant `sql`, expected one of `ref`, `command`, `spec`",
        ),
    ];
    assert_problems(&cases);
}

#[test]
fn each_oracle_kind_takes_exactly_its_keys() {
    let cases = [
        (
            "[oracles.PG]\nkind = \"ref\"\nref = \"v1\"\n",
            "invalid oracle name `PG`: must match `[a-z][a-z0-9_-]*`",
        ),
        (
            "[oracles.old]\nkind = \"ref\"\n",
            "invalid oracle `old`: kind `ref` needs `ref`",
        ),
        (
            "[oracles.old]\nkind = \"ref\"\nref = \"v1\"\ncmd = \"echo\"\n",
            "invalid oracle `old`: kind `ref` does not take `cmd`",
        ),
        (
            "[oracles.calc]\nkind = \"command\"\ninputs = [\"1\"]\n",
            "invalid oracle `calc`: kind `command` needs `cmd`",
        ),
        (
            "[oracles.calc]\nkind = \"ref\"\nref = \"v1\"\ninputs = [\"1\"]\n",
            "invalid oracle `calc`: kind `ref` does not take `inputs`",
        ),
        (
            "[oracles.model]\nkind = \"spec\"\nspec = \"model.qnt\"\n",
            "invalid oracle `model`: kind `spec` needs `lang`",
        ),
        (
            "[oracles.model]\nkind = \"spec\"\nspec = \"model.v\"\nlang = \"coq\"\n",
            "invalid oracle `model`: `lang` must be one of `tla+`, `quint`, `p`",
        ),
    ];
    assert_problems(&cases);
}

#[test]
fn a_command_oracle_keeps_its_known_inputs_in_order() {
    let file = parse(
        "[oracles.calc]\nkind = \"command\"\ncmd = \"calc {input}\"\ninputs = [\"2\", \"1\"]\n",
    )
    .unwrap();
    assert_eq!(file.oracles["calc"].inputs, ["2", "1"]);
    let spec =
        parse("[oracles.model]\nkind = \"spec\"\nspec = \"m.qnt\"\nlang = \"quint\"\n").unwrap();
    assert!(spec.oracles["model"].inputs.is_empty());
}

#[test]
fn check_and_language_ids_are_syntax_checked() {
    let cases = [
        (
            "[checks.mutation]\nx = 1\n",
            "invalid check ID `mutation`: check ID must be `<group>.<name>`",
        ),
        (
            "[checks.\"tests.mutation.score\"]\nx = 1\n",
            "invalid check ID `tests.mutation.score`: check ID must be `<group>.<name>`",
        ),
        (
            "[lang.Rust]\ntest = \"cargo test\"\n",
            "invalid language ID `Rust`: must match `[a-z][a-z0-9_-]*`",
        ),
        (
            "[checks.\"tests.mutation\".lang.\"Py thon\"]\nx = 1\n",
            "invalid language ID `Py thon`: must match `[a-z][a-z0-9_-]*`",
        ),
        (
            "[roots]\nweb = { path = \"web\", lang = \"Type Script\" }\n",
            "invalid language ID `Type Script`: must match `[a-z][a-z0-9_-]*`",
        ),
    ];
    assert_problems(&cases);
}

#[test]
fn per_language_overrides_must_be_tables() {
    let overrides = "`lang` in a check table holds per-language override tables, such as \
                     `[checks.\"<id>\".lang.<lang>]`";
    assert_problems(&[
        (
            "[checks.\"tests.mutation\"]\nlang = \"python\"\n",
            overrides,
        ),
        ("[checks.\"tests.mutation\".lang]\npython = 3\n", overrides),
    ]);
}

#[test]
fn malformed_gate_rules_are_rejected_naming_the_rule_and_section() {
    insta::assert_snapshot!(error_message(
        "[gate.changed]\n\"tests.mutation.score\" = \">= 0.8\"\n\"tests.mutation\" = \"> 1\"\n"
    ));
}

#[test]
fn gate_conditions_must_be_strings() {
    assert_problems(&[(
        "[gate.changed]\n\"tests.mutation.score\" = 0.8\n",
        "invalid type: floating point `0.8`, expected a string",
    )]);
}

#[test]
fn toml_syntax_errors_name_the_file() {
    let message = error_message("depth = \n");
    assert_eq!(
        message.lines().next(),
        Some("invalid configuration in `/repo/cupel.toml`: TOML parse error at line 1, column 9")
    );
}
