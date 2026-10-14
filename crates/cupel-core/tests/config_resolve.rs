// ABOUTME: Tests for layered resolution: the precedence matrix, profile selection and merge, and the
// ABOUTME: `run.config` echo (effective config plus the layers that contributed to it).
use std::path::{Path, PathBuf};

use cupel_core::config::{
    Config, ConfigError, ConfigFile, ConfigRequest, Effective, Format, Settings, TimeLimit,
};
use cupel_core::{ConfigSource, CostClass, Error};
use serde_json::json;

fn limit(text: &str) -> TimeLimit {
    text.parse().unwrap()
}

fn request(flags: Settings, env: &[(&str, &str)], profile: Option<&str>) -> ConfigRequest {
    ConfigRequest {
        root: PathBuf::from("/repo"),
        config_path: None,
        profile: profile.map(str::to_owned),
        flags,
        env: env
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

fn file(text: &str) -> ConfigFile {
    ConfigFile::parse(text, Path::new("/repo/cupel.toml")).unwrap()
}

fn resolve(file_text: Option<&str>, request: &ConfigRequest) -> Result<Config, Error> {
    match file_text {
        Some(text) => Config::resolve(Some(PathBuf::from("/repo/cupel.toml")), file(text), request),
        None => Config::resolve(None, ConfigFile::default(), request),
    }
}

/// `text` when the layer is `present`, else nothing.
fn when(present: bool, text: &str) -> &str {
    if present { text } else { "" }
}

/// Every combination of layers setting `budget`, each to a distinct value. The highest-precedence
/// layer that sets it wins: flags > env > profile > cupel.toml > defaults (design §6.7).
#[test]
fn precedence_matrix() {
    use ConfigSource::{CupelToml, Defaults, Env, Flags, Profile};
    // (flags, env, profile, cupel.toml) → (effective budget, sources)
    #[rustfmt::skip]
    let rows: [([bool; 4], Option<&str>, &[ConfigSource]); 16] = [
        ([false, false, false, false], None,       &[CupelToml, Defaults]),
        ([false, false, false, true ], Some("4s"), &[CupelToml, Defaults]),
        ([false, false, true,  false], Some("3s"), &[Profile, CupelToml, Defaults]),
        ([false, false, true,  true ], Some("3s"), &[Profile, CupelToml, Defaults]),
        ([false, true,  false, false], Some("2s"), &[Env, CupelToml, Defaults]),
        ([false, true,  false, true ], Some("2s"), &[Env, CupelToml, Defaults]),
        ([false, true,  true,  false], Some("2s"), &[Env, Profile, CupelToml, Defaults]),
        ([false, true,  true,  true ], Some("2s"), &[Env, Profile, CupelToml, Defaults]),
        ([true,  false, false, false], Some("1s"), &[Flags, CupelToml, Defaults]),
        ([true,  false, false, true ], Some("1s"), &[Flags, CupelToml, Defaults]),
        ([true,  false, true,  false], Some("1s"), &[Flags, Profile, CupelToml, Defaults]),
        ([true,  false, true,  true ], Some("1s"), &[Flags, Profile, CupelToml, Defaults]),
        ([true,  true,  false, false], Some("1s"), &[Flags, Env, CupelToml, Defaults]),
        ([true,  true,  false, true ], Some("1s"), &[Flags, Env, CupelToml, Defaults]),
        ([true,  true,  true,  false], Some("1s"), &[Flags, Env, Profile, CupelToml, Defaults]),
        ([true,  true,  true,  true ], Some("1s"), &[Flags, Env, Profile, CupelToml, Defaults]),
    ];
    for ([in_flags, in_env, in_profile, in_file], expected, sources) in rows {
        let flags = Settings {
            budget: in_flags.then(|| limit("1s")),
            ..Settings::default()
        };
        let env: &[(&str, &str)] = &[("CUPEL_BUDGET", "2s")][..usize::from(in_env)];
        let text = format!(
            "{}[profile.p]\n{}",
            when(in_file, "budget = \"4s\"\n"),
            when(in_profile, "budget = \"3s\"\n"),
        );
        // The profile layer is present exactly when profile `p` is selected.
        let profile = in_profile.then_some("p");
        let config = resolve(Some(&text), &request(flags, env, profile)).unwrap();
        let row = [in_flags, in_env, in_profile, in_file];
        assert_eq!(config.effective.budget, expected.map(limit), "{row:?}");
        // depth and format are unset everywhere, so `defaults` always contributes.
        assert_eq!(config.sources, sources, "{row:?}");
    }
}

#[test]
fn defaults_apply_when_nothing_is_set() {
    let config = resolve(None, &request(Settings::default(), &[], None)).unwrap();
    assert_eq!(
        config.effective,
        Effective {
            depth: CostClass::Standard,
            format: None,
            budget: None,
            profile: None,
        }
    );
    assert_eq!(config.sources, [ConfigSource::Defaults]);
    assert_eq!(config.path, None);
}

#[test]
fn each_setting_resolves_independently() {
    let flags = Settings {
        format: Some(Format::Text),
        ..Settings::default()
    };
    let config = resolve(
        Some("depth = \"quick\"\nformat = \"json\"\nbudget = \"9m\"\n[profile.ci]\nbudget = \"1h\"\n"),
        &request(flags, &[("CUPEL_DEPTH", "deep")], Some("ci")),
    )
    .unwrap();
    assert_eq!(
        config.effective,
        Effective {
            depth: CostClass::Deep,
            format: Some(Format::Text),
            budget: Some(limit("1h")),
            profile: Some("ci".to_owned()),
        }
    );
    assert_eq!(
        config.sources,
        [
            ConfigSource::Flags,
            ConfigSource::Env,
            ConfigSource::Profile,
            ConfigSource::CupelToml,
        ],
        "every setting is set by some layer, so no default contributes"
    );
}

#[test]
fn profile_overrides_only_the_settings_it_sets() {
    let text = "depth = \"standard\"\nformat = \"json\"\n\
                [profile.ci]\ndepth = \"deep\"\nformat = \"sarif\"\n\
                [profile.loop]\ndepth = \"quick\"\nbudget = \"20s\"\n";
    let config = resolve(Some(text), &request(Settings::default(), &[], Some("loop"))).unwrap();
    assert_eq!(config.effective.depth, CostClass::Quick);
    assert_eq!(
        config.effective.format,
        Some(Format::Json),
        "from cupel.toml"
    );
    assert_eq!(config.effective.budget, Some(limit("20s")));
    assert_eq!(config.effective.profile.as_deref(), Some("loop"));
}

#[test]
fn cupel_profile_env_selects_a_profile_and_the_flag_wins_over_it() {
    let text = "[profile.ci]\ndepth = \"deep\"\n[profile.loop]\ndepth = \"quick\"\n";
    let by_env = resolve(
        Some(text),
        &request(Settings::default(), &[("CUPEL_PROFILE", "ci")], None),
    )
    .unwrap();
    assert_eq!(by_env.effective.profile.as_deref(), Some("ci"));
    assert_eq!(by_env.effective.depth, CostClass::Deep);

    let by_flag = resolve(
        Some(text),
        &request(
            Settings::default(),
            &[("CUPEL_PROFILE", "ci")],
            Some("loop"),
        ),
    )
    .unwrap();
    assert_eq!(by_flag.effective.profile.as_deref(), Some("loop"));
    assert_eq!(by_flag.effective.depth, CostClass::Quick);
}

#[test]
fn unknown_profile_is_a_usage_error_listing_defined_profiles() {
    let err = resolve(
        Some("[profile.ci]\n[profile.loop]\n"),
        &request(Settings::default(), &[], Some("nightly")),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "profile `nightly` is not defined; defined profiles: `ci`, `loop`"
    );
    assert!(matches!(
        err,
        Error::Config(ConfigError::UnknownProfile { .. })
    ));

    let err = resolve(None, &request(Settings::default(), &[], Some("ci"))).unwrap_err();
    assert_eq!(
        err.to_string(),
        "profile `ci` is not defined; defined profiles: none (no cupel.toml was found)"
    );
}

#[test]
fn invalid_env_values_fail_resolution() {
    let err = resolve(
        None,
        &request(Settings::default(), &[("CUPEL_FORMAT", "xml")], None),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid value `xml` for CUPEL_FORMAT: unknown variant `xml`, expected one of `json`, \
         `jsonl`, `text`, `sarif`"
    );
}

#[test]
fn echo_carries_the_effective_config_and_its_sources() {
    let text = "default_branch = \"main\"\n\
                [lang.rust]\ntest = \"cargo test\"\n\
                [checks.\"static.clones\"]\nthreshold = 0.82\n\
                [checks.\"static.clones\".lang.go]\nthreshold = 0.9\n\
                [gate.changed]\n\"tests.mutation.score\" = \">= 0.80\"\n\"tests.weakening\" = \"none\"\n";
    let flags = Settings {
        depth: Some(CostClass::Quick),
        ..Settings::default()
    };
    let echo = resolve(Some(text), &request(flags, &[], None))
        .unwrap()
        .echo();
    assert_eq!(
        serde_json::to_value(&echo).unwrap(),
        json!({
            "effective": {
                "depth": "quick",
                "format": null,
                "budget": null,
                "profile": null,
                "required_version": null,
                "default_branch": "main",
                "roots": {},
                "lang": {"rust": {"test": "cargo test"}},
                "checks": {"static.clones": {"threshold": 0.82, "lang": {"go": {"threshold": 0.9}}}},
                "oracles": {},
                "gate": {
                    "changed": {"tests.mutation.score": ">= 0.8", "tests.weakening": "none"},
                    "tree": null
                }
            },
            "sources": ["flags", "cupel.toml", "defaults"]
        })
    );
}
