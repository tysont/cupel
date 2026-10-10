// ABOUTME: Attaches the generic producers to a language: each is registered exactly when the
// ABOUTME: language's spec has what drives it (its query file, and for NormTree a `[normalize]` table).
use std::sync::Arc;

use cupel_core::ProducerRegistry;
use cupel_core::syntax::{Decisions, NormTree, Symbols};

use crate::language::Language;
use crate::parse::ParseCache;
use crate::producer::QueryProducer;
use crate::spec::{LanguageSpec, QueryFile};
use crate::{decisions, normalize, symbols};

/// Whether a language's spec can drive a generic producer.
type Drives = fn(&LanguageSpec) -> bool;

/// Registers one generic producer for a language.
type Attach = fn(Arc<dyn Language>, Arc<ParseCache>, &mut ProducerRegistry);

/// Each generic producer and what drives it, in fact-catalog order.
const GENERIC: &[(Drives, Attach)] = &[
    (
        |spec| spec.has(QueryFile::Symbols),
        |language, cache, reg| {
            reg.register::<Symbols, _>(QueryProducer::new(language, cache, symbols::extract));
        },
    ),
    (
        |spec| spec.has(QueryFile::Symbols) && spec.has(QueryFile::Decisions),
        |language, cache, reg| {
            reg.register::<Decisions, _>(QueryProducer::new(language, cache, decisions::extract));
        },
    ),
    (
        |spec| spec.has(QueryFile::Symbols) && spec.tables().normalize.is_some(),
        |language, cache, reg| {
            reg.register::<NormTree, _>(QueryProducer::new(language, cache, normalize::extract));
        },
    ),
];

/// Registers every generic producer `language`'s spec drives.
pub fn register(language: &Arc<dyn Language>, cache: &Arc<ParseCache>, reg: &mut ProducerRegistry) {
    for (drives, attach) in GENERIC {
        if drives(language.spec()) {
            attach(Arc::clone(language), Arc::clone(cache), reg);
        }
    }
}
