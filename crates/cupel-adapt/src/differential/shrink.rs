// ABOUTME: One-step shrinks of a divergent input for the differential engine's greedy minimization (ADR-093 §9):
// ABOUTME: every argument made a little smaller in turn, by the shape of its JSON value.
use serde_json::{Number, Value};

use super::generate::Input;

/// Every input one step smaller than `input`, argument by argument, in the order minimization
/// tries them. Each shrink keeps the value's shape, so it stays a valid argument.
pub fn candidates(input: &Input) -> Vec<Input> {
    let mut all = Vec::new();
    for (at, value) in input.iter().enumerate() {
        for smaller in smaller(value) {
            let mut candidate = input.clone();
            candidate[at] = smaller;
            all.push(candidate);
        }
    }
    all
}

/// One-step-smaller values of `value`, without repeats or `value` itself.
fn smaller(value: &Value) -> Vec<Value> {
    let mut found: Vec<Value> = Vec::new();
    let shrinks = match value {
        Value::Number(number) => numbers(number),
        Value::String(text) => strings(text),
        Value::Array(items) => lists(items),
        Value::Bool(true) => vec![Value::Bool(false)],
        Value::Bool(false) | Value::Null | Value::Object(_) => Vec::new(),
    };
    for shrink in shrinks {
        if shrink != *value && !found.contains(&shrink) {
            found.push(shrink);
        }
    }
    found
}

/// An integer toward 0: to 0, then ever closer to itself by halves (`n - n/2`, `n - n/4`, …,
/// `n - 1`), so the first that still diverges is the smallest of them; a float to 0 or its
/// integer part.
fn numbers(number: &Number) -> Vec<Value> {
    if let Some(n) = number.as_i64() {
        return closer(n).into_iter().map(Value::from).collect();
    }
    if let Some(n) = number.as_u64() {
        return vec![Value::from(0), Value::from(n / 2), Value::from(n - 1)];
    }
    let float = number.as_f64().unwrap_or_default();
    [0.0, float.trunc()]
        .into_iter()
        .filter_map(|value| Number::from_f64(value).map(Value::Number))
        .collect()
}

/// 0, then `n - n/2`, `n - n/4`, …, `n - signum(n)`.
fn closer(n: i64) -> Vec<i64> {
    let mut values = vec![0];
    let mut step = n / 2;
    while step != 0 {
        values.push(n - step);
        step /= 2;
    }
    if n != 0 {
        values.push(n - n.signum());
    }
    values
}

/// A string to empty, to its first half, and one character shorter at either end.
fn strings(text: &str) -> Vec<Value> {
    let chars: Vec<char> = text.chars().collect();
    let taken = |range: std::ops::Range<usize>| Value::String(chars[range].iter().collect());
    if chars.is_empty() {
        return Vec::new();
    }
    vec![
        Value::String(String::new()),
        taken(0..chars.len() / 2),
        taken(0..chars.len() - 1),
        taken(1..chars.len()),
    ]
}

/// A list to empty, to its first half, one element shorter at either end, then with one
/// element shrunk.
fn lists(items: &[Value]) -> Vec<Value> {
    if items.is_empty() {
        return Vec::new();
    }
    let mut shrinks = vec![
        Value::Array(Vec::new()),
        Value::Array(items[..items.len() / 2].to_vec()),
        Value::Array(items[..items.len() - 1].to_vec()),
        Value::Array(items[1..].to_vec()),
    ];
    for (at, item) in items.iter().enumerate() {
        shrinks.extend(smaller(item).into_iter().map(|smaller| {
            let mut list = items.to_vec();
            list[at] = smaller;
            Value::Array(list)
        }));
    }
    shrinks
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn integers_shrink_toward_zero() {
        assert_eq!(
            candidates(&vec![json!(-73)]),
            [0, -37, -55, -64, -69, -71, -72]
                .map(|n| vec![json!(n)])
                .to_vec()
        );
        assert_eq!(candidates(&vec![json!(-1)]), vec![vec![json!(0)]]);
        assert!(candidates(&vec![json!(0)]).is_empty());
    }

    #[test]
    fn strings_and_lists_get_shorter_and_lists_shrink_their_elements() {
        assert_eq!(
            candidates(&vec![json!("abc")]),
            vec![
                vec![json!("")],
                vec![json!("a")],
                vec![json!("ab")],
                vec![json!("bc")]
            ]
        );
        assert_eq!(
            candidates(&vec![json!([2])]),
            vec![vec![json!([])], vec![json!([0])], vec![json!([1])]]
        );
    }

    #[test]
    fn each_argument_shrinks_in_turn_with_the_others_kept() {
        assert_eq!(
            candidates(&vec![json!(true), json!(1.5)]),
            vec![
                vec![json!(false), json!(1.5)],
                vec![json!(true), json!(0.0)],
                vec![json!(true), json!(1.0)]
            ]
        );
    }
}
