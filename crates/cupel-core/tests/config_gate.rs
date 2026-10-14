// ABOUTME: Gate-rule parsing matrix: every operator, both key shapes, and the inputs that must be rejected.
// ABOUTME: Parsing only; the rules' evaluation against run metrics is tested with the evaluator (P2.9).
use cupel_core::CheckId;
use cupel_core::config::{
    Comparison, ConfigError, GateCondition, GateKey, GateRuleSpec, Threshold,
};

fn key(check: &str, metric: Option<&str>) -> GateKey {
    GateKey {
        check: CheckId::from(check),
        metric: metric.map(str::to_owned),
    }
}

fn compare(op: Comparison, value: f64, percent: bool) -> GateCondition {
    GateCondition::Compare {
        op,
        threshold: Threshold { value, percent },
    }
}

/// Parses each (key, condition) and asserts its key and condition.
fn assert_parses(cases: &[(&str, &str, GateKey, GateCondition)]) {
    for (raw_key, raw_condition, expected_key, expected_condition) in cases {
        let rule = GateRuleSpec::parse(raw_key, raw_condition)
            .unwrap_or_else(|err| panic!("{raw_key} = {raw_condition}: {err}"));
        assert_eq!(
            (&rule.key, &rule.condition),
            (expected_key, expected_condition),
            "{raw_key} = {raw_condition}"
        );
    }
}

#[test]
fn parses_every_comparison() {
    assert_parses(&[
        (
            "tests.mutation.score",
            ">= 0.80",
            key("tests.mutation", Some("score")),
            compare(Comparison::Ge, 0.80, false),
        ),
        (
            "static.crap.max",
            "> 30",
            key("static.crap", Some("max")),
            compare(Comparison::Gt, 30.0, false),
        ),
        (
            "static.crap.max",
            "<= 30",
            key("static.crap", Some("max")),
            compare(Comparison::Le, 30.0, false),
        ),
        (
            "static.crap.max",
            "< 30",
            key("static.crap", Some("max")),
            compare(Comparison::Lt, 30.0, false),
        ),
        (
            "tests.weakening.count",
            "== 0",
            key("tests.weakening", Some("count")),
            compare(Comparison::Eq, 0.0, false),
        ),
        (
            "perf.bench.regression",
            "< 5%",
            key("perf.bench", Some("regression")),
            compare(Comparison::Lt, 5.0, true),
        ),
    ]);
}

#[test]
fn parses_every_presence_condition() {
    assert_parses(&[
        (
            "tests.weakening",
            "none",
            key("tests.weakening", None),
            GateCondition::None,
        ),
        (
            "static.api-surface",
            "no-breaking",
            key("static.api-surface", None),
            GateCondition::NoBreaking,
        ),
    ]);
}

#[test]
fn whitespace_around_the_operator_and_operand_is_optional() {
    for condition in [">=0.8", ">= 0.8", "  >=   0.8  "] {
        let rule = GateRuleSpec::parse("tests.mutation.score", condition).unwrap();
        assert_eq!(rule.condition, compare(Comparison::Ge, 0.8, false));
    }
    let rule = GateRuleSpec::parse("tests.weakening", " none ").unwrap();
    assert_eq!(rule.condition, GateCondition::None);
}

#[test]
fn displays_as_key_operator_operand() {
    let cases = [
        (
            "tests.mutation.score",
            ">= 0.80",
            "tests.mutation.score >= 0.8",
        ),
        ("perf.bench.regression", "<5%", "perf.bench.regression < 5%"),
        ("static.crap.max", "== 30", "static.crap.max == 30"),
        ("tests.weakening", "none", "tests.weakening none"),
        (
            "static.api-surface",
            "no-breaking",
            "static.api-surface no-breaking",
        ),
    ];
    for (raw_key, condition, expected) in cases {
        let rule = GateRuleSpec::parse(raw_key, condition).unwrap();
        assert_eq!(rule.to_string(), expected);
    }
}

#[test]
fn rejects_malformed_rules_with_a_reason() {
    let cases = [
        // keys
        ("tests", "none", "check ID must be `<group>.<name>`"),
        ("", "none", "check ID must be `<group>.<name>`"),
        ("tests..score", ">= 1", "empty segment"),
        ("Tests.mutation.score", ">= 1", "segment `Tests`"),
        ("tests.mutation.score.max", ">= 1", "at most one metric"),
        ("tests.mut ation", "none", "segment `mut ation`"),
        // operators
        ("tests.mutation.score", "=> 0.8", "unknown operator"),
        ("tests.mutation.score", "!= 0.8", "unknown operator"),
        ("tests.mutation.score", "= 0.8", "unknown operator"),
        ("tests.mutation.score", "", "unknown operator"),
        ("tests.weakening", "nothing", "unknown operator"),
        // operands
        ("tests.mutation.score", ">=", "expects a number"),
        ("tests.mutation.score", ">= high", "expects a number"),
        ("tests.mutation.score", ">= 0.8 0.9", "expects a number"),
        ("tests.mutation.score", ">= NaN", "expects a number"),
        ("tests.mutation.score", ">= inf", "expects a number"),
        ("tests.mutation.score", ">= %", "expects a number"),
        ("tests.weakening", "none 0", "takes no operand"),
        ("static.api-surface", "no-breaking yes", "takes no operand"),
        // operator and key shape must agree
        ("tests.mutation", ">= 0.8", "needs a metric"),
        (
            "tests.weakening.count",
            "none",
            "applies to a check, not a metric",
        ),
        (
            "static.api-surface.count",
            "no-breaking",
            "applies to a check, not a metric",
        ),
    ];
    for (raw_key, condition, reason_part) in cases {
        let err = GateRuleSpec::parse(raw_key, condition)
            .expect_err(&format!("{raw_key:?} = {condition:?} should be rejected"));
        let ConfigError::InvalidGateRule {
            key: err_key,
            condition: err_condition,
            reason,
        } = &err
        else {
            panic!("{raw_key:?} = {condition:?}: unexpected error {err:?}");
        };
        assert_eq!(err_key, raw_key);
        assert_eq!(err_condition, condition);
        assert!(
            reason.contains(reason_part),
            "{raw_key:?} = {condition:?}: reason {reason:?} lacks {reason_part:?}"
        );
    }
}

#[test]
fn error_message_quotes_the_rule_and_lists_the_operators() {
    let err = GateRuleSpec::parse("tests.mutation.score", "=> 0.8").unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid gate rule `\"tests.mutation.score\" = \"=> 0.8\"`: unknown operator; \
         expected one of `>=`, `>`, `<=`, `<`, `==`, `none`, `no-breaking`"
    );
}
