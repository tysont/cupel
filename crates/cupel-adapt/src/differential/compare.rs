// ABOUTME: When the implementation's answer and the oracle's are the same output (ADR-093 §8): equal texts or
// ABOUTME: equal JSON values; against a command oracle, a JSON string matches its bare text and errors match errors.
use cupel_core::Answer;
use cupel_core::config::OracleKind;
use serde_json::Value;

/// Whether `implementation` and `oracle` agree, for an oracle of `kind`.
pub fn agree(implementation: &Answer, oracle: &Answer, kind: OracleKind) -> bool {
    let command = kind == OracleKind::Command;
    match (implementation, oracle) {
        (Answer::Returned(ours), Answer::Returned(theirs)) => same_value(ours, theirs, command),
        (Answer::Returned(_), _) | (_, Answer::Returned(_)) => false,
        _ if command => true,
        (Answer::Raised(ours), Answer::Raised(theirs))
        | (Answer::Crashed(ours), Answer::Crashed(theirs)) => ours == theirs,
        _ => false,
    }
}

fn same_value(ours: &str, theirs: &str, command: bool) -> bool {
    if ours == theirs {
        return true;
    }
    let parsed = |text: &str| serde_json::from_str::<Value>(text).ok();
    match (parsed(ours), parsed(theirs)) {
        (Some(a), Some(b)) => a == b,
        (Some(Value::String(a)), None) if command => a == theirs,
        (None, Some(Value::String(b))) if command => ours == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn returned(text: &str) -> Answer {
        Answer::Returned(text.to_owned())
    }

    #[test]
    fn returned_values_agree_as_text_or_as_json() {
        assert!(agree(
            &returned("[1, 2]"),
            &returned("[1,2]"),
            OracleKind::Ref
        ));
        assert!(agree(
            &returned(r#"{"a":1,"b":2}"#),
            &returned(r#"{"b":2,"a":1}"#),
            OracleKind::Ref
        ));
        assert!(!agree(&returned("-1"), &returned("0"), OracleKind::Command));
    }

    #[test]
    fn a_json_string_matches_a_commands_bare_text_but_not_a_refs() {
        assert!(agree(
            &returned(r#""low""#),
            &returned("low"),
            OracleKind::Command
        ));
        assert!(!agree(
            &returned(r#""low""#),
            &returned("low"),
            OracleKind::Ref
        ));
    }

    #[test]
    fn errors_agree_with_a_commands_errors_and_with_a_refs_same_error() {
        let raised = |text: &str| Answer::Raised(text.to_owned());
        let crashed = Answer::Crashed("exit 1".to_owned());
        assert!(agree(
            &raised("ValueError: x"),
            &raised("exit 1: no"),
            OracleKind::Command
        ));
        assert!(agree(&crashed, &raised("exit 1: no"), OracleKind::Command));
        assert!(agree(&raised("E: x"), &raised("E: x"), OracleKind::Ref));
        assert!(!agree(&raised("E: x"), &raised("E: y"), OracleKind::Ref));
        assert!(!agree(&crashed, &raised("E: x"), OracleKind::Ref));
        assert!(!agree(&raised("E: x"), &returned("0"), OracleKind::Command));
    }
}
