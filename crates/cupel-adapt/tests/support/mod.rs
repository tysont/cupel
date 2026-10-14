// ABOUTME: Helpers shared by the cupel-adapt integration tests: the registry of every tool producer, and the
// ABOUTME: launch languages as the registry sees them. Included with `mod support;`.
use cupel_core::{Lang, LangFamily, LangId, ProducerRegistry};

/// Every tool producer, registered.
pub fn registry() -> ProducerRegistry {
    let mut registry = ProducerRegistry::default();
    cupel_adapt::register(&mut registry);
    registry
}

/// The launch language `id`, in its family.
pub fn lang(id: &str) -> Lang {
    let family = if matches!(id, "javascript" | "typescript") {
        "ecma"
    } else {
        id
    };
    Lang {
        id: LangId::from(id),
        family: LangFamily::from(family),
    }
}
