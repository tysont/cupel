// ABOUTME: Attaches the generic query-driven producers to a language: each is registered exactly
// ABOUTME: when the language's spec has the query file that drives it.
use std::sync::Arc;

use cupel_core::ProducerRegistry;
use cupel_core::syntax::{Decisions, Symbols};

use crate::language::Language;
use crate::parse::ParseCache;
use crate::producer::QueryProducer;
use crate::spec::QueryFile;
use crate::{decisions, symbols};

/// Registers one generic producer for a language.
type Attach = fn(Arc<dyn Language>, Arc<ParseCache>, &mut ProducerRegistry);

/// Each generic producer and the query file that drives it, in fact-catalog order.
const GENERIC: &[(QueryFile, Attach)] = &[
    (QueryFile::Symbols, |language, cache, reg| {
        reg.register::<Symbols, _>(QueryProducer::new(language, cache, symbols::extract));
    }),
    (QueryFile::Decisions, |language, cache, reg| {
        reg.register::<Decisions, _>(QueryProducer::new(language, cache, decisions::extract));
    }),
];

/// Registers every generic producer whose query file `language` has.
pub fn register(language: &Arc<dyn Language>, cache: &Arc<ParseCache>, reg: &mut ProducerRegistry) {
    for (file, attach) in GENERIC {
        if language.spec().has(*file) {
            attach(Arc::clone(language), Arc::clone(cache), reg);
        }
    }
}
