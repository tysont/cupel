// ABOUTME: The support matrix: for every (check, language), whether the check can run there and why not.
// ABOUTME: Derived from check needs and registered producers, never declared (design §8.3.5, ADR-015).
use std::collections::BTreeMap;

use crate::check::CheckRegistry;
use crate::fact::FactKind;
use crate::id::{CheckId, LangId, ToolId};
use crate::lang::Lang;
use crate::producer::{Availability, Ctx, ProducerRegistry, RegisteredProducer};

/// Whether a check can run for a language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    Supported,
    /// No producer can make this fact for the language. Permanent until one is written.
    Unsupported(FactKind),
    /// A producer exists but its tool is missing here; `doctor` can fix it.
    Unavailable(ToolId),
    /// A producer exists but needs this configuration key set.
    Unconfigured(String),
}

/// Support for every registered check × every given language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportMatrix {
    cells: BTreeMap<(CheckId, LangId), Support>,
}

impl SupportMatrix {
    pub fn compute(
        checks: &CheckRegistry,
        producers: &ProducerRegistry,
        langs: &[Lang],
        ctx: &Ctx,
    ) -> Self {
        let mut cells = BTreeMap::new();
        for check in checks.iter() {
            for lang in langs {
                let resolver = Resolver {
                    producers,
                    lang,
                    ctx,
                };
                let support = resolver.all(&check.manifest().needs, &mut Vec::new());
                cells.insert((check.manifest().id.clone(), lang.id.clone()), support);
            }
        }
        Self { cells }
    }

    pub fn get(&self, check: &CheckId, lang: &LangId) -> Option<&Support> {
        self.cells.get(&(check.clone(), lang.clone()))
    }

    /// Every cell, ordered by check ID then language ID.
    pub fn cells(&self) -> impl Iterator<Item = (&CheckId, &LangId, &Support)> {
        self.cells
            .iter()
            .map(|((check, lang), support)| (check, lang, support))
    }
}

/// Resolves whether facts can be produced for one language, following producer needs.
struct Resolver<'a> {
    producers: &'a ProducerRegistry,
    lang: &'a Lang,
    ctx: &'a Ctx,
}

impl Resolver<'_> {
    /// The first need that cannot be produced, in order; `Supported` when all can.
    /// `visiting` holds the facts being resolved above this call, to cut dependency cycles.
    fn all(&self, needs: &[FactKind], visiting: &mut Vec<FactKind>) -> Support {
        needs
            .iter()
            .map(|need| self.fact(*need, visiting))
            .find(|support| *support != Support::Supported)
            .unwrap_or(Support::Supported)
    }

    /// Supported when any producer of `kind` for this language can run; otherwise the first
    /// producer's reason, or `Unsupported(kind)` when there is none (ADR-015).
    fn fact(&self, kind: FactKind, visiting: &mut Vec<FactKind>) -> Support {
        if visiting.contains(&kind) {
            return Support::Unsupported(kind);
        }
        visiting.push(kind);
        let support = self.first_runnable(kind, visiting);
        visiting.pop();
        support
    }

    fn first_runnable(&self, kind: FactKind, visiting: &mut Vec<FactKind>) -> Support {
        let candidates = match self.producers.candidates(kind, self.lang, self.ctx) {
            Ok(candidates) => candidates,
            Err(key) => return Support::Unconfigured(key),
        };
        let mut first_failure = None;
        for producer in candidates {
            match self.producer(producer, visiting) {
                Support::Supported => return Support::Supported,
                failure => {
                    first_failure.get_or_insert(failure);
                }
            }
        }
        first_failure.unwrap_or(Support::Unsupported(kind))
    }

    /// Needs are resolved before availability: a permanent gap outranks a missing tool.
    fn producer(&self, producer: &dyn RegisteredProducer, visiting: &mut Vec<FactKind>) -> Support {
        match self.all(producer.needs(), visiting) {
            Support::Supported => match producer.availability(self.ctx) {
                Availability::Ready => Support::Supported,
                Availability::MissingTool { name, .. } => Support::Unavailable(name),
                Availability::Unconfigured { key } => Support::Unconfigured(key),
            },
            upstream => upstream,
        }
    }
}
