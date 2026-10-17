// ABOUTME: Generated inputs for the differential engine (ADR-093 §5): boundary cases, then random cases growing
// ABOUTME: in size, from a SplitMix64 stream seeded by the run seed and the entry, so `--seed` repeats them.
use serde_json::{Number, Value, json};

use super::types::Ty;

/// One call's arguments, as JSON values.
pub type Input = Vec<Value>;

/// Random cases reach this size by the last case.
const MAX_SIZE: u64 = 30;

/// The largest integer a JavaScript number holds exactly.
const SAFE_INTEGER: i64 = (1 << 53) - 1;

/// `SplitMix64`: small, fast, and the same everywhere.
pub struct Rng(u64);

impl Rng {
    /// The stream for `entry` in a run seeded `seed`.
    pub fn for_entry(seed: u64, entry: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&seed.to_le_bytes());
        hasher.update(entry.as_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(&digest.as_bytes()[..8]);
        Self(u64::from_le_bytes(bytes))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `0..n`; 0 when `n` is 0.
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }

    /// A value in `min..=max`.
    fn within(&mut self, min: i64, max: i64) -> i64 {
        let span = max.abs_diff(min);
        let offset = if span == u64::MAX {
            self.next()
        } else {
            self.below(span + 1)
        };
        min.wrapping_add_unsigned(offset)
    }
}

/// `count` inputs for parameters `params`: the boundary cases first (each parameter's
/// boundaries by index), then random cases whose size grows with the case index.
pub fn inputs(params: &[Ty], rng: &mut Rng, count: u64) -> Vec<Input> {
    let boundaries: Vec<Vec<Value>> = params.iter().map(boundaries).collect();
    let rounds = boundaries.iter().map(Vec::len).max().unwrap_or(1) as u64;
    (0..count)
        .map(|case| {
            if case < rounds {
                boundaries
                    .iter()
                    .map(|values| values[index(case) % values.len()].clone())
                    .collect()
            } else {
                let size = 1 + (case - rounds) * MAX_SIZE / count.max(1);
                params.iter().map(|ty| random(ty, rng, size)).collect()
            }
        })
        .collect()
}

/// The values a type's boundary cases take, in order.
fn boundaries(ty: &Ty) -> Vec<Value> {
    match ty {
        Ty::Int { min, max } => {
            let mut values: Vec<i64> = Vec::new();
            for value in [0, 1, -1, *min, *max] {
                if value >= *min && value <= *max && !values.contains(&value) {
                    values.push(value);
                }
            }
            values.into_iter().map(Value::from).collect()
        }
        Ty::Number => vec![
            json!(0),
            json!(1),
            json!(-1),
            json!(0.5),
            json!(SAFE_INTEGER),
        ],
        Ty::Float => vec![json!(0.0), json!(1.0), json!(-1.0), json!(0.5)],
        Ty::Bool => vec![json!(false), json!(true)],
        Ty::Str => vec![json!(""), json!("a"), json!(" "), json!("é")],
        Ty::List(element) => {
            let first = boundaries(element)
                .into_iter()
                .next()
                .unwrap_or(Value::Null);
            vec![json!([]), json!([first])]
        }
        Ty::Optional(inner) => std::iter::once(Value::Null)
            .chain(boundaries(inner))
            .collect(),
        Ty::Any => vec![
            json!(0),
            json!(1),
            json!(-1),
            json!(""),
            json!("a"),
            json!(true),
            json!(false),
            Value::Null,
            json!([]),
        ],
    }
}

/// Characters random strings are made of: letters, digits, space, punctuation, and one
/// non-ASCII letter.
const ALPHABET: [char; 18] = [
    'a', 'b', 'c', 'x', 'y', 'z', 'A', 'B', 'C', '0', '1', '9', ' ', '-', '_', '.', ',', 'é',
];

/// A random value of `ty` no bigger than `size`.
fn random(ty: &Ty, rng: &mut Rng, size: u64) -> Value {
    match ty {
        Ty::Int { min, max } => Value::from(integer(rng, size, *min, *max)),
        Ty::Number => number(rng, size),
        Ty::Float => float(rng, size),
        Ty::Bool => Value::Bool(rng.below(2) == 1),
        Ty::Str => Value::String(text(rng, size)),
        Ty::List(element) => {
            let length = rng.below(size.min(8) + 1);
            Value::Array((0..length).map(|_| random(element, rng, size)).collect())
        }
        Ty::Optional(inner) => {
            if rng.below(4) == 0 {
                Value::Null
            } else {
                random(inner, rng, size)
            }
        }
        Ty::Any => any(rng, size),
    }
}

/// An integer in `min..=max`: small, medium, or anywhere in the range, a third of the time each.
fn integer(rng: &mut Rng, size: u64, min: i64, max: i64) -> i64 {
    let reach = i64::try_from(size * size * 10).unwrap_or(i64::MAX);
    match rng.below(3) {
        0 => rng.within((-10_i64).max(min), 10_i64.min(max)),
        1 => rng.within((-reach).max(min), reach.min(max)),
        _ => rng.within(min, max),
    }
}

fn number(rng: &mut Rng, size: u64) -> Value {
    let whole = integer(rng, size, -SAFE_INTEGER, SAFE_INTEGER);
    if rng.below(4) == 0 {
        half(whole)
    } else {
        Value::from(whole)
    }
}

fn float(rng: &mut Rng, size: u64) -> Value {
    let whole = integer(rng, size, -1_000_000, 1_000_000);
    if rng.below(2) == 0 {
        half(whole)
    } else {
        number_value(i32::try_from(whole).map_or(0.0, f64::from))
    }
}

/// `whole + 0.5`, exactly representable.
#[expect(
    clippy::cast_precision_loss,
    reason = "the integers halved here stay within ±2^53, where f64 is exact"
)]
fn half(whole: i64) -> Value {
    number_value(whole as f64 + 0.5)
}

/// `value` as an index; within a slice's length, so it fits.
fn index(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn number_value(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn text(rng: &mut Rng, size: u64) -> String {
    let length = rng.below(size + 1);
    (0..length)
        .map(|_| ALPHABET[index(rng.below(ALPHABET.len() as u64))])
        .collect()
}

fn any(rng: &mut Rng, size: u64) -> Value {
    match rng.below(5) {
        0 => Value::from(integer(rng, size, -SAFE_INTEGER, SAFE_INTEGER)),
        1 => Value::String(text(rng, size)),
        2 => Value::Bool(rng.below(2) == 1),
        3 => Value::Null,
        _ => {
            let length = rng.below(size.min(8) + 1);
            Value::Array(
                (0..length)
                    .map(|_| Value::from(integer(rng, size, -1000, 1000)))
                    .collect(),
            )
        }
    }
}

/// What a command oracle's `{input}` stands for (ADR-093 §7): for one parameter the argument
/// itself (a string's text, any other value's JSON), else the JSON array of the arguments.
pub fn command_input(input: &[Value]) -> String {
    match input {
        [Value::String(text)] => text.clone(),
        [value] => value.to_string(),
        values => Value::Array(values.to_vec()).to_string(),
    }
}

/// A command oracle's known input as arguments for parameters `params`, the reverse of
/// [`command_input`]; `None` when it does not decode.
pub fn known_input(known: &str, params: &[Ty]) -> Option<Input> {
    let parsed = serde_json::from_str::<Value>(known);
    match params {
        [Ty::Str] => Some(vec![Value::String(known.to_owned())]),
        [Ty::Any] => Some(vec![
            parsed.unwrap_or_else(|_| Value::String(known.to_owned())),
        ]),
        [_] => Some(vec![parsed.ok()?]),
        _ => match parsed.ok()? {
            Value::Array(values) if values.len() == params.len() => Some(values),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BYTE: Ty = Ty::Int { min: 0, max: 255 };

    #[test]
    fn the_same_seed_and_entry_give_the_same_inputs_and_another_entry_others() {
        let params = [Ty::Str, Ty::List(Box::new(BYTE))];
        let first = inputs(&params, &mut Rng::for_entry(7, "a.py:f"), 40);
        let again = inputs(&params, &mut Rng::for_entry(7, "a.py:f"), 40);
        let other = inputs(&params, &mut Rng::for_entry(7, "a.py:g"), 40);
        assert_eq!(first, again);
        assert_ne!(first, other);
        assert_eq!(first.len(), 40);
    }

    #[test]
    fn boundary_cases_come_first_by_index() {
        let cases = inputs(&[BYTE, Ty::Bool], &mut Rng::for_entry(1, "e"), 4);
        assert_eq!(
            cases,
            vec![
                vec![json!(0), json!(false)],
                vec![json!(1), json!(true)],
                vec![json!(255), json!(false)],
                cases[3].clone(),
            ]
        );
    }

    #[test]
    fn random_values_stay_in_their_type() {
        let small = Ty::Int { min: -3, max: 3 };
        let cases = inputs(&[small, Ty::Float], &mut Rng::for_entry(9, "e"), 500);
        let strays = cases
            .iter()
            .filter(|case| {
                !case[0].as_i64().is_some_and(|n| (-3..=3).contains(&n)) || !case[1].is_f64()
            })
            .count();
        assert_eq!((cases.len(), strays), (500, 0));
    }

    #[test]
    fn a_command_sees_one_argument_bare_and_several_as_an_array() {
        assert_eq!(command_input(&[json!("a b")]), "a b");
        assert_eq!(command_input(&[json!(-1)]), "-1");
        assert_eq!(command_input(&[json!(1), json!("x")]), r#"[1,"x"]"#);
    }

    #[test]
    fn known_inputs_decode_as_a_command_would_see_them() {
        assert_eq!(known_input("a b", &[Ty::Str]), Some(vec![json!("a b")]));
        assert_eq!(known_input("-8", &[BYTE]), Some(vec![json!(-8)]));
        assert_eq!(known_input("word", &[Ty::Any]), Some(vec![json!("word")]));
        assert_eq!(
            known_input("[1, true]", &[BYTE, Ty::Bool]),
            Some(vec![json!(1), json!(true)])
        );
        assert_eq!(known_input("[1]", &[BYTE, Ty::Bool]), None);
        assert_eq!(known_input("x", &[BYTE]), None);
    }
}
