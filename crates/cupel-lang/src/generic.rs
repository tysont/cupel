// ABOUTME: Attaches the generic producers to a language: each is registered exactly when the
// ABOUTME: language's spec has what drives it; each fact module submits its own (ADR-074).
use std::sync::Arc;

use cupel_core::{FactKind, ProducerRegistry};

use crate::language::Language;
use crate::parse::ParseCache;
use crate::spec::LanguageSpec;

/// Whether a language's spec can drive a generic producer.
pub(crate) type Drives = fn(&LanguageSpec) -> bool;

/// Registers one generic producer for a language.
pub(crate) type Attach = fn(Arc<dyn Language>, Arc<ParseCache>, &mut ProducerRegistry);

/// A generic producer: the fact it makes, what in a language's spec drives it, and how it
/// attaches to a language. The module that extracts the fact submits it with
/// `inventory::submit!`, so adding a generic producer edits no shared list.
pub(crate) struct GenericProducer {
    pub kind: FactKind,
    pub drives: Drives,
    pub attach: Attach,
}

inventory::collect!(GenericProducer);

/// Registers every generic producer `language`'s spec drives.
pub fn register(language: &Arc<dyn Language>, cache: &Arc<ParseCache>, reg: &mut ProducerRegistry) {
    register_except(language, cache, reg, &[]);
}

/// Registers every generic producer `language`'s spec drives except those making `replaced`,
/// for a language that registers its own producer of those facts. Producers attach in
/// fact-catalog order.
pub fn register_except(
    language: &Arc<dyn Language>,
    cache: &Arc<ParseCache>,
    reg: &mut ProducerRegistry,
    replaced: &[FactKind],
) {
    let mut generic: Vec<&GenericProducer> =
        inventory::iter::<GenericProducer>.into_iter().collect();
    generic.sort_by_key(|producer| producer.kind);
    for producer in generic {
        if !replaced.contains(&producer.kind) && (producer.drives)(language.spec()) {
            (producer.attach)(Arc::clone(language), Arc::clone(cache), reg);
        }
    }
}
