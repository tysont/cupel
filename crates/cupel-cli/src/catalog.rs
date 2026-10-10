// ABOUTME: The registries `describe` reads: compiled-in languages, the producers they register, and
// ABOUTME: checks. Builds the language, fact, and matrix descriptions from them, never from lists.
use std::path::Path;

use cupel_core::{
    CheckRegistry, CheckSupport, Ctx, FactDescription, FactKind, FactProducers, LangId,
    LanguageDescription, LanguageProducers, LanguageSummary, LanguagesDescription, MatrixCell,
    MatrixDescription, ProducerDescription, ProducerRegistry, RegisteredProducer, SupportMatrix,
    schema,
};
use cupel_lang::{Language, LanguageRegistry};
use serde_json::Value;

/// Every registry `describe` renders.
pub struct Catalog {
    languages: LanguageRegistry,
    producers: ProducerRegistry,
    checks: CheckRegistry,
}

impl Catalog {
    /// The compiled-in languages and their producers. Checks register in P2.1.
    pub fn builtin() -> Self {
        let languages = LanguageRegistry::builtin();
        let mut producers = ProducerRegistry::default();
        languages.register_producers(&mut producers);
        Self {
            languages,
            producers,
            checks: CheckRegistry::default(),
        }
    }

    pub fn language_ids(&self) -> Vec<LangId> {
        self.languages.ids()
    }

    pub fn languages(&self) -> LanguagesDescription {
        LanguagesDescription {
            languages: self
                .languages
                .iter()
                .map(|language| LanguageSummary {
                    id: language.id(),
                    family: language.family(),
                    extensions: extensions(language.as_ref()),
                    stability: language.stability(),
                    facts: self
                        .facts_of(language.as_ref())
                        .into_iter()
                        .map(|facts| facts.kind)
                        .collect(),
                })
                .collect(),
        }
    }

    /// `None` when no language has that ID.
    pub fn language(&self, id: &LangId, root: &Path) -> Option<LanguageDescription> {
        let language = self.languages.get(id)?;
        let matrix = self.matrix_of(root);
        Some(LanguageDescription {
            id: language.id(),
            family: language.family(),
            extensions: extensions(language.as_ref()),
            stability: language.stability(),
            facts: self.facts_of(language.as_ref()),
            checks: matrix
                .cells()
                .filter(|(_, lang, _)| *lang == id)
                .map(|(check, _, support)| CheckSupport {
                    check: check.clone(),
                    support: support.into(),
                })
                .collect(),
        })
    }

    pub fn fact(&self, kind: FactKind) -> FactDescription {
        FactDescription {
            kind,
            version: schema::fact_version(kind),
            syntactic: FactKind::SYNTACTIC.contains(&kind),
            schema: schema::fact(kind).and_then(|schema| match schema.to_value() {
                Value::Object(body) => Some(body),
                _ => None,
            }),
            producers: self
                .languages
                .langs()
                .into_iter()
                .map(|lang| LanguageProducers {
                    producers: self
                        .producers
                        .producers_for(kind, &lang)
                        .map(describe_producer)
                        .collect(),
                    lang: lang.id,
                })
                .collect(),
            consumers: self
                .checks
                .iter()
                .map(cupel_core::Check::manifest)
                .filter(|manifest| manifest.needs.contains(&kind) || manifest.wants.contains(&kind))
                .map(|manifest| manifest.id.clone())
                .collect(),
        }
    }

    pub fn matrix(&self, root: &Path) -> MatrixDescription {
        let matrix = self.matrix_of(root);
        MatrixDescription {
            languages: self.languages.ids(),
            checks: self
                .checks
                .iter()
                .map(|check| check.manifest().id.clone())
                .collect(),
            cells: matrix
                .cells()
                .map(|(check, lang, support)| MatrixCell {
                    check: check.clone(),
                    lang: lang.clone(),
                    support: support.into(),
                })
                .collect(),
        }
    }

    fn matrix_of(&self, root: &Path) -> SupportMatrix {
        SupportMatrix::compute(
            &self.checks,
            &self.producers,
            &self.languages.langs(),
            &Ctx {
                root: root.to_path_buf(),
            },
        )
    }

    /// The fact kinds `language` has producers for, in catalog order, with those producers.
    fn facts_of(&self, language: &dyn Language) -> Vec<FactProducers> {
        let lang = cupel_lang::lang(language);
        FactKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let producers: Vec<ProducerDescription> = self
                    .producers
                    .producers_for(kind, &lang)
                    .map(describe_producer)
                    .collect();
                (!producers.is_empty()).then_some(FactProducers { kind, producers })
            })
            .collect()
    }
}

fn extensions(language: &dyn Language) -> Vec<String> {
    language
        .extensions()
        .iter()
        .map(|ext| (*ext).to_owned())
        .collect()
}

fn describe_producer(producer: &dyn RegisteredProducer) -> ProducerDescription {
    ProducerDescription {
        id: producer.id(),
        cost: producer.cost(),
        needs: producer.needs().to_vec(),
    }
}
