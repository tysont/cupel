// ABOUTME: Reads each property framework's shrunk counterexample back from a failing test's output (ADR-095 §4):
// ABOUTME: proptest's `minimal failing input`, quickcheck's `Arguments`, Hypothesis's example call, fast-check's line.

/// proptest: `minimal failing input: n = 100000`.
pub fn proptest(output: &str) -> Option<String> {
    after_marker(output, "minimal failing input: ")
}

/// quickcheck: `[quickcheck] TEST FAILED. Arguments: (3195450000)`, also after `(runtime error)`.
pub fn quickcheck(output: &str) -> Option<String> {
    output
        .lines()
        .filter(|line| line.contains("[quickcheck] TEST FAILED"))
        .find_map(|line| after_marker(line, "Arguments: "))
}

/// fast-check: `Counterexample: [100000]`.
pub fn fast_check(output: &str) -> Option<String> {
    after_marker(output, "Counterexample: ")
}

/// Hypothesis: `Falsifying example: test_f(` (or `Failing test case: test_f(`), the arguments one
/// per line, then `)`; or the whole call on one line. Each line may carry pytest's `E` prefix.
/// The arguments, joined by `, `.
pub fn hypothesis(output: &str) -> Option<String> {
    let lines: Vec<&str> = output.lines().map(unprefixed).collect();
    let start = lines.iter().position(|line| {
        line.starts_with("Falsifying example: ") || line.starts_with("Failing test case: ")
    })?;
    let (_, call) = lines[start].split_once(": ")?;
    let (_, opened) = call.split_once('(')?;
    if let Some(inline) = opened.strip_suffix(')') {
        return Some(inline.trim().to_owned());
    }
    let arguments: Vec<&str> = lines[start + 1..]
        .iter()
        .take_while(|line| **line != ")")
        .map(|line| line.trim_end_matches(','))
        .collect();
    Some(arguments.join(", "))
}

/// The rest of the first line of `text` containing `marker`, after it, trimmed; `None` when no
/// line has it or the rest is empty.
fn after_marker(text: &str, marker: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.split_once(marker).map(|(_, rest)| rest.trim()))
        .filter(|rest| !rest.is_empty())
        .map(str::to_owned)
}

/// `line` without pytest's `E` failure prefix and surrounding whitespace.
fn unprefixed(line: &str) -> &str {
    let line = line.trim();
    match line.strip_prefix('E') {
        Some(rest) if rest.starts_with(char::is_whitespace) => rest.trim(),
        _ => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `cargo test` failure section, as the libtest parser keeps it.
    const PROPTEST: &str =
        "thread 'digits_counts_decimal_places' (57182020) panicked at tests/props.rs:4:1:
Test failed: assertion failed: `(left == right)`
  left: `0`,
 right: `5` at tests/props.rs:7.
minimal failing input: n = 50000
\tsuccesses: 464
\tlocal rejects: 0
\tglobal rejects: 0

note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace";

    const QUICKCHECK: &str = "thread 'quickcheck_digits' (57374636) panicked at /cargo/quickcheck-1.1.0/src/tester.rs:182:28:
[quickcheck] TEST FAILED. Arguments: (3195450000)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace";

    /// A pytest `JUnit` failure: the message, then the body with its `E` lines.
    const HYPOTHESIS: &str = "AssertionError: assert 0 == 5
 +  where 0 = digits(50000)
Failing test case: test_digits_counts_decimal_places(
    n=50000,
)
Explanation:
    These lines were always and only run by failing test cases:
        src/units/__init__.py:3
@given(st.integers(min_value=0, max_value=99_999))
>       assert digits(n) == len(str(n))
E       AssertionError: assert 0 == 5
E       Failing test case: test_digits_counts_decimal_places(
E           n=50000,
E       )";

    const FAST_CHECK: &str = "Error: Property failed after 8 tests
{ seed: 3, path: \"7:4:1:0:0:1:4:0:0\", endOnFailure: true }
Counterexample: [100000]
Shrunk 8 time(s)

Hint: Enable verbose mode in order to have the list of all failing values encountered during the run
    at buildError (fast-check/lib/fast-check.js:2469:16)";

    #[test]
    fn each_framework_report_gives_its_shrunk_input() {
        assert_eq!(proptest(PROPTEST).as_deref(), Some("n = 50000"));
        assert_eq!(quickcheck(QUICKCHECK).as_deref(), Some("(3195450000)"));
        assert_eq!(hypothesis(HYPOTHESIS).as_deref(), Some("n=50000"));
        assert_eq!(fast_check(FAST_CHECK).as_deref(), Some("[100000]"));
    }

    #[test]
    fn older_hypothesis_reports_and_pytest_prefixes_read_the_same() {
        let older = "E       Falsifying example: test_pair(\nE           a=0,\nE           b='',\nE       )";
        assert_eq!(hypothesis(older).as_deref(), Some("a=0, b=''"));
        let inline = "Falsifying example: test_one(n=3)";
        assert_eq!(hypothesis(inline).as_deref(), Some("n=3"));
    }

    #[test]
    fn output_without_a_report_has_no_counterexample() {
        for read in [proptest, quickcheck, hypothesis, fast_check] {
            assert_eq!(read("thread 'x' panicked at src/lib.rs:3:5:\nboom"), None);
        }
        assert_eq!(proptest("minimal failing input: "), None);
    }
}
