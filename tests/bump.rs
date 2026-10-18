//! Bumping a dependency: which release each bump reaches, and the constraint it writes.

use uview::domain::{Bumped, VersionChoice, bump_constraint, bump_targets, declared_floor};

fn listed(versions: &[&str]) -> Vec<VersionChoice> {
    versions
        .iter()
        .map(|v| VersionChoice {
            version: v.to_string(),
            pre: v.contains(|c: char| c.is_ascii_alphabetic()),
            needs_python: None,
        })
        .collect()
}

#[test]
fn each_bump_reaches_the_newest_release_it_allows() {
    let versions = listed(&[
        "1.0.0", "0.28.1", "0.28.0", "0.26.0", "0.25.2", "0.25.1", "0.25.0",
    ]);
    assert_eq!(
        bump_targets("0.25.0", &versions),
        [
            Some("0.25.2".into()),
            Some("0.28.1".into()),
            Some("1.0.0".into())
        ]
    );
    // Nothing newer: up to date.
    assert_eq!(bump_targets("1.0.0", &versions), [None, None, None]);
    // A patch with none to go to, while a minor and a major have.
    assert_eq!(
        bump_targets("0.25.2", &versions),
        [None, Some("0.28.1".into()), Some("1.0.0".into())]
    );
}

#[test]
fn pre_releases_and_releases_for_a_newer_python_do_not_count() {
    let mut versions = listed(&["2.0.0b1", "1.18.0", "1.17.1"]);
    versions[1].needs_python = Some(">=3.13".into());
    assert_eq!(
        bump_targets("1.17.0", &versions),
        [
            Some("1.17.1".into()),
            Some("1.17.1".into()),
            Some("1.17.1".into())
        ]
    );
    // From a pre-release, a newer pre-release counts.
    assert_eq!(
        bump_targets("2.0.0a1", &versions)[2],
        Some("2.0.0b1".into())
    );
}

#[test]
fn the_declared_operator_is_kept_with_the_new_version() {
    let to = |declared: &str, new: &str| bump_constraint(declared, new);
    assert_eq!(
        to("==0.25.0", "0.28.1"),
        Some(Bumped::To("==0.28.1".into()))
    );
    assert_eq!(to(">=0.27", "0.28.1"), Some(Bumped::To(">=0.28.1".into())));
    assert_eq!(
        to("~=0.27.0", "0.28.1"),
        Some(Bumped::To("~=0.28.1".into()))
    );
    assert_eq!(
        to("==0.27.*", "0.28.1"),
        Some(Bumped::To("==0.28.*".into()))
    );
    // No constraint: a lower bound, as uv writes one.
    assert_eq!(to("", "0.28.1"), Some(Bumped::To(">=0.28.1".into())));
    // The marker is the pin path's to keep, and an upper bound that admits the version stays.
    assert_eq!(
        to(">=1.15,<2; python_version < '3.11'", "1.17.0"),
        Some(Bumped::To(">=1.17.0,<2".into()))
    );
    // A URL has no version to bump.
    assert_eq!(to("@ git+https://example.com/pkg", "1.0"), None);
}

#[test]
fn a_range_the_new_version_falls_outside_moves_with_its_width_once_confirmed() {
    assert_eq!(
        bump_constraint(">=1.15,<1.16", "1.17.0"),
        Some(Bumped::OutOfRange {
            old: ">=1.15,<1.16".into(),
            new: ">=1.17.0,<1.18".into()
        })
    );
    assert_eq!(
        bump_constraint(">=1,<2", "2.3.0"),
        Some(Bumped::OutOfRange {
            old: ">=1,<2".into(),
            new: ">=2.3.0,<3".into()
        })
    );
    // Exclusions are kept.
    assert_eq!(
        bump_constraint(">=1.15,!=1.15.3,<1.16", "1.17.0"),
        Some(Bumped::OutOfRange {
            old: ">=1.15,!=1.15.3,<1.16".into(),
            new: ">=1.17.0,!=1.15.3,<1.18".into()
        })
    );
    // One that excludes the very version cannot be bumped to it.
    assert_eq!(bump_constraint(">=1.15,!=1.17.0,<1.16", "1.17.0"), None);
}

#[test]
fn a_package_not_installed_is_measured_from_its_lowest_declared_version() {
    assert_eq!(declared_floor(">=0.27,<1"), Some("0.27".into()));
    assert_eq!(
        declared_floor("==1.2.3; python_version < '3.11'"),
        Some("1.2.3".into())
    );
    assert_eq!(declared_floor("<2"), None);
    assert_eq!(declared_floor(""), None);
}
