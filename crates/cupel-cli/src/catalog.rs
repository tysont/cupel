// ABOUTME: The registries every verb reads: compiled-in languages, the producers they and the tool layer
// ABOUTME: register, and the native checks. Builds the `describe` payloads from them, never from lists.
use std::path::Path;
use std::sync::Arc;

use cupel_core::{
    CheckDescription, CheckId, CheckRegistry, CheckSupport, Ctx, FactDescription, FactKind,
    FactProducers, FindingDescription, LangId, LanguageDescription, LanguageProducers,
    LanguageSummary, LanguageSupport, LanguagesDescription, MatrixCell, MatrixDescription,
    ProducerDescription, ProducerRegistry, RegisteredProducer, Stability, SupportMatrix,
    VerbSupport, schema,
};
use cupel_lang::{Language, LanguageRegistry, ParseCache};
use cupel_plugin::Plugins;
use serde_json::Value;

use crate::oracle;

/// Every registry `describe` renders.
pub struct Catalog {
    languages: LanguageRegistry,
    producers: ProducerRegistry,
    checks: CheckRegistry,
}

impl Catalog {
    /// The compiled-in languages, their producers and the tool producers, and the native checks.
    pub fn builtin() -> Self {
        Self::with_cache(Arc::default())
    }

    /// [`Catalog::builtin`] with its languages parsing through `cache`, which may be warm (the
    /// daemon's session, ADR-085 §3).
    pub fn with_cache(cache: Arc<ParseCache>) -> Self {
        let languages = LanguageRegistry::with_cache(cache);
        let mut producers = ProducerRegistry::default();
        languages.register_producers(&mut producers);
        cupel_adapt::register(&mut producers);
        cupel_mutate::register(&mut producers);
        Self {
            languages,
            producers,
            checks: checks(),
        }
    }

    /// This catalog with `plugins`' checks and producers registered after the native ones
    /// (ADR-088 §7).
    #[must_use]
    pub fn with_plugins(mut self, plugins: &Plugins) -> Self {
        plugins.register_producers(&mut self.producers);
        plugins.register_checks(&mut self.checks);
        self
    }

    pub fn language_ids(&self) -> Vec<LangId> {
        self.languages.ids()
    }

    pub fn language_registry(&self) -> &LanguageRegistry {
        &self.languages
    }

    pub fn producers(&self) -> &ProducerRegistry {
        &self.producers
    }

    pub fn checks(&self) -> &CheckRegistry {
        &self.checks
    }

    /// `None` when no check has that ID.
    pub fn check(&self, id: &CheckId, root: &Path) -> Option<CheckDescription> {
        let manifest = self.checks.get(id)?.manifest().clone();
        let matrix = self.matrix_of(root);
        Some(CheckDescription {
            languages: matrix
                .cells()
                .filter(|(check, _, _)| *check == id)
                .map(|(_, lang, support)| LanguageSupport {
                    lang: lang.clone(),
                    support: support.into(),
                })
                .collect(),
            manifest,
        })
    }

    /// The finding kind `kind`, from the manifest of the check that emits it, else from the
    /// kinds Cupel attaches to a run outside any check (ADR-091 §5).
    pub fn finding(&self, kind: &str) -> Option<FindingDescription> {
        let emitted = self.checks.iter().find_map(|check| {
            let manifest = check.manifest();
            manifest
                .findings
                .iter()
                .find(|spec| spec.kind == kind)
                .map(|spec| FindingDescription::of(manifest, spec))
        });
        emitted.or_else(|| {
            oracle::cap::finding_kinds()
                .into_iter()
                .find(|spec| spec.kind == kind)
                .map(|spec| FindingDescription {
                    kind: spec.kind,
                    check: CheckId::from("oracle"),
                    action: spec.action,
                    summary: spec.summary,
                    evidence: spec.evidence,
                    stability: Stability::Experimental,
                })
        })
    }

    /// Every finding kind a registered check emits, sorted.
    pub fn finding_kinds(&self) -> Vec<String> {
        let mut kinds: Vec<String> = self
            .checks
            .iter()
            .flat_map(|check| {
                check
                    .manifest()
                    .findings
                    .iter()
                    .map(|spec| spec.kind.clone())
            })
            .collect();
        kinds.sort();
        kinds.extend(
            oracle::cap::finding_kinds()
                .into_iter()
                .map(|spec| spec.kind),
        );
        kinds
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
            toolchain: cupel_adapt::toolchain::for_lang(id)
                .map(cupel_adapt::toolchain::Row::describe),
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
            verbs: vec![self.verb_support(
                crate::tests_verb::IMPACTED,
                crate::tests_verb::IMPACT_FACT,
                root,
            )],
        }
    }

    /// Per language, whether `fact`, which `verb` reads, can be produced.
    fn verb_support(&self, verb: &str, fact: FactKind, root: &Path) -> VerbSupport {
        let ctx = Ctx::new(root.to_path_buf());
        VerbSupport {
            verb: verb.to_owned(),
            fact,
            languages: self
                .languages
                .langs()
                .iter()
                .map(|lang| LanguageSupport {
                    lang: lang.id.clone(),
                    support: (&SupportMatrix::fact(&self.producers, fact, lang, &ctx)).into(),
                })
                .collect(),
        }
    }

    fn matrix_of(&self, root: &Path) -> SupportMatrix {
        SupportMatrix::compute(
            &self.checks,
            &self.producers,
            &self.languages.langs(),
            &Ctx::new(root.to_path_buf()),
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

/// The native checks, registered once through `cupel-checks` (ADR-001, ADR-074).
pub fn checks() -> CheckRegistry {
    let mut registry = CheckRegistry::default();
    cupel_checks::register(&mut registry);
    registry
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
