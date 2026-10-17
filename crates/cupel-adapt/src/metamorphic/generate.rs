// ABOUTME: Generated inputs of the metamorphic engine (ADR-097 §3): JSON values of an input type, each from the
// ABOUTME: relation's seed and the case's index alone, small for the first cases and growing to a fixed size.
use cupel_core::InputType;
use serde_json::Value;

use crate::flake::rerun_seed;

/// The largest size a case grows to: list and string lengths up to it, integers up to
/// `16 * size²` in magnitude.
const MAX_SIZE: u64 = 8;

/// The printable characters generated strings are made of.
const ALPHABET: &[u8] = b"abcxyz019 _-";

/// Case `case` of the inputs drawn with `seed`: the same value for the same three arguments.
pub fn generate(ty: &InputType, seed: u64, case: u64) -> Value {
    let mut rng = Rng {
        seed: rerun_seed(seed ^ case.rotate_left(32), 0),
        drawn: 0,
    };
    value(ty, &mut rng, size(case))
}

/// The seed relation `name` draws its inputs with in a run with `seed`, so adding or removing
/// one relation leaves every other's inputs as they were.
pub fn relation_seed(seed: u64, name: &str) -> u64 {
    // FNV-1a of the name, mixed with the run's seed.
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    rerun_seed(seed ^ hash, 0)
}

/// Case `case`'s size: one for the first four cases, one more every four, then [`MAX_SIZE`].
fn size(case: u64) -> u64 {
    (1 + case / 4).min(MAX_SIZE)
}

fn value(ty: &InputType, rng: &mut Rng, size: u64) -> Value {
    match ty {
        InputType::Bool => Value::Bool(rng.below(2) == 1),
        InputType::Int => {
            let bound = 16 * size * size;
            let drawn = i64::try_from(rng.below(2 * bound + 1)).unwrap_or(0);
            Value::from(drawn - i64::try_from(bound).unwrap_or(0))
        }
        InputType::Str => {
            let length = rng.below(size + 1);
            let text: String = (0..length)
                .map(|_| char::from(ALPHABET[index(rng.below(ALPHABET.len() as u64))]))
                .collect();
            Value::String(text)
        }
        InputType::List(element) => {
            let length = rng.below(size + 1);
            Value::Array((0..length).map(|_| value(element, rng, size)).collect())
        }
        InputType::Tuple(elements) => Value::Array(
            elements
                .iter()
                .map(|element| value(element, rng, size))
                .collect(),
        ),
    }
}

fn index(drawn: u64) -> usize {
    usize::try_from(drawn).unwrap_or(0)
}

/// A splitmix64 sequence: the `drawn`th value from `seed`, as the flake engine derives its
/// reruns' seeds.
struct Rng {
    seed: u64,
    drawn: u32,
}

impl Rng {
    fn next(&mut self) -> u64 {
        self.drawn = self.drawn.wrapping_add(1);
        rerun_seed(self.seed, self.drawn)
    }

    /// A draw in `0..bound`; `bound` is at least 1.
    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }
}
