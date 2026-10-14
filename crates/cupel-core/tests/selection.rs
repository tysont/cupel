// ABOUTME: Check selection: depth against cost class, --only/--skip with IDs and groups, --tier N+,
// ABOUTME: and the usage errors for unknown IDs and malformed tiers (ADR-040).
mod support;

use support::Declared;

use cupel_core::{CheckRegistry, CostClass, Error, Manifest, Selection, Tier};

fn registry() -> CheckRegistry {
    let mut registry = CheckRegistry::default();
    for (id, tier, cost) in [
        ("static.complexity", 0, CostClass::Quick),
        ("static.clones", 0, CostClass::Quick),
        ("static.analyze", 1, CostClass::Standard),
        ("tests.quality", 2, CostClass::Quick),
        ("tests.mutation", 3, CostClass::Standard),
        ("gen.fuzz", 4, CostClass::Deep),
    ] {
        registry
            .register(Box::new(Declared(Manifest {
                tier: Tier::try_from(tier).unwrap(),
                cost,
                ..support::manifest(id, &[], &[])
            })))
            .unwrap();
    }
    registry
}

fn selection(depth: CostClass) -> Selection {
    Selection {
        depth,
        only: Vec::new(),
        skip: Vec::new(),
        tier: None,
    }
}

fn ids(selection: &Selection) -> Vec<String> {
    let registry = registry();
    selection
        .select(&registry)
        .unwrap()
        .iter()
        .map(|check| check.manifest().id.to_string())
        .collect()
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn depth_selects_checks_at_or_below_its_cost_class() {
    assert_eq!(
        ids(&selection(CostClass::Quick)),
        ["static.complexity", "static.clones", "tests.quality"]
    );
    assert_eq!(ids(&selection(CostClass::Standard)).len(), 5);
    assert_eq!(ids(&selection(CostClass::Exhaustive)).len(), 6);
}

#[test]
fn only_expands_groups_and_ignores_depth() {
    let only = |names: &[&str]| Selection {
        only: words(names),
        ..selection(CostClass::Quick)
    };
    assert_eq!(
        ids(&only(&["static"])),
        ["static.complexity", "static.clones", "static.analyze"]
    );
    assert_eq!(ids(&only(&["gen.fuzz"])), ["gen.fuzz"]);
    assert_eq!(
        ids(&only(&["tests", "static.clones"])),
        ["static.clones", "tests.quality", "tests.mutation"],
        "registry order, not argument order"
    );
}

#[test]
fn skip_removes_ids_and_groups_after_only() {
    let skipping = Selection {
        only: words(&["static", "tests"]),
        skip: words(&["static.clones", "tests"]),
        ..selection(CostClass::Quick)
    };
    assert_eq!(ids(&skipping), ["static.complexity", "static.analyze"]);
    let default_minus_group = Selection {
        skip: words(&["static"]),
        ..selection(CostClass::Quick)
    };
    assert_eq!(ids(&default_minus_group), ["tests.quality"]);
}

#[test]
fn tier_keeps_checks_at_or_above_it() {
    let tier = |text: &str| Selection {
        tier: Some(text.to_owned()),
        ..selection(CostClass::Exhaustive)
    };
    assert_eq!(ids(&tier("3+")), ["tests.mutation", "gen.fuzz"]);
    assert_eq!(ids(&tier("4")), ["gen.fuzz"]);
    assert_eq!(ids(&tier("0+")).len(), 6);
}

#[test]
fn unknown_ids_and_bad_tiers_are_usage_errors_naming_the_flag() {
    let registry = registry();
    let error = |selection: Selection| match selection.select(&registry) {
        Err(Error::Argument(error)) => error,
        Err(other) => panic!("expected an argument error, got {other:?}"),
        Ok(_) => panic!("expected an argument error"),
    };
    let unknown = error(Selection {
        only: words(&["static.complexty"]),
        ..selection(CostClass::Quick)
    });
    assert_eq!(unknown.argument, "--only");
    assert_eq!(unknown.value.as_deref(), Some("static.complexty"));
    assert!(unknown.valid.contains(&"static".to_owned()));
    assert!(unknown.valid.contains(&"static.complexity".to_owned()));
    let skip = error(Selection {
        skip: words(&["perf"]),
        ..selection(CostClass::Quick)
    });
    assert_eq!(skip.argument, "--skip");
    for bad in ["7+", "x", "+", "-1"] {
        let tier = error(Selection {
            tier: Some(bad.to_owned()),
            ..selection(CostClass::Quick)
        });
        assert_eq!(tier.argument, "--tier", "{bad}");
    }
}
