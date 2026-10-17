// ABOUTME: Minimization candidates of the metamorphic engine (ADR-097 §4): simpler values of the same input type,
// ABOUTME: simplest first, which the engine tries until none still violates the relation.
use cupel_core::InputType;
use serde_json::Value;

/// The values simpler than `value` of type `ty`, simplest first, each once and none equal to
/// `value`: integers toward zero, `false`, shorter strings and lists, then simpler elements.
pub fn candidates(ty: &InputType, value: &Value) -> Vec<Value> {
    let all = match (ty, value) {
        (InputType::Bool, Value::Bool(true)) => vec![Value::Bool(false)],
        (InputType::Int, Value::Number(number)) => number.as_i64().map(ints).unwrap_or_default(),
        (InputType::Str, Value::String(text)) => strings(text),
        (InputType::List(element), Value::Array(items)) => lists(element, items),
        (InputType::Tuple(elements), Value::Array(items)) => tuples(elements, items),
        _ => Vec::new(),
    };
    let mut distinct: Vec<Value> = Vec::with_capacity(all.len());
    for candidate in all {
        if candidate != *value && !distinct.contains(&candidate) {
            distinct.push(candidate);
        }
    }
    distinct
}

/// Zero, the positive of a negative, half, and one step toward zero.
fn ints(n: i64) -> Vec<Value> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![0];
    if n < 0 {
        out.push(n.saturating_neg());
    }
    out.extend([n / 2, n - n.signum()]);
    out.into_iter().map(Value::from).collect()
}

/// Empty, either half, each character dropped, then each character made `a`.
fn strings(text: &str) -> Vec<Value> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let collect = |chars: &[char]| Value::String(chars.iter().collect());
    let mut out: Vec<Value> = shorter(&chars).iter().map(|part| collect(part)).collect();
    for index in 0..chars.len() {
        let mut simpler = chars.clone();
        simpler[index] = 'a';
        out.push(collect(&simpler));
    }
    out
}

/// Empty, either half, each element dropped, then each element simpler.
fn lists(element: &InputType, items: &[Value]) -> Vec<Value> {
    if items.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Value> = shorter(items).into_iter().map(Value::Array).collect();
    out.extend(each_simpler(items, |_| element));
    out
}

/// Each element simpler, the others as they are.
fn tuples(elements: &[InputType], items: &[Value]) -> Vec<Value> {
    if elements.len() != items.len() {
        return Vec::new();
    }
    each_simpler(items, |index| &elements[index])
}

/// `items` with one element replaced by each of its candidates, element by element.
fn each_simpler<'t>(items: &[Value], ty: impl Fn(usize) -> &'t InputType) -> Vec<Value> {
    let mut out = Vec::new();
    for (index, item) in items.iter().enumerate() {
        for simpler in candidates(ty(index), item) {
            let mut replaced = items.to_vec();
            replaced[index] = simpler;
            out.push(Value::Array(replaced));
        }
    }
    out
}

/// The empty sequence, the second half, the first half, and `items` without each one element.
fn shorter<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    let half = items.len() / 2;
    let mut out = vec![Vec::new(), items[half..].to_vec(), items[..half].to_vec()];
    for index in 0..items.len() {
        let mut dropped = items.to_vec();
        dropped.remove(index);
        out.push(dropped);
    }
    out
}
