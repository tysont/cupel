// ABOUTME: Which languages a producer serves, expressed without core knowing any language by name.
// ABOUTME: `Lang` pairs a language with its family; `Langs` is a producer's coverage (ADR-011).
use crate::id::{LangFamily, LangId};

/// A registered language and the family its symbols compare within.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lang {
    pub id: LangId,
    pub family: LangFamily,
}

/// The languages a producer serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Langs {
    Any,
    Only(Vec<LangId>),
    Family(LangFamily),
}

impl Langs {
    pub fn covers(&self, lang: &Lang) -> bool {
        match self {
            Self::Any => true,
            Self::Only(ids) => ids.contains(&lang.id),
            Self::Family(family) => *family == lang.family,
        }
    }
}
