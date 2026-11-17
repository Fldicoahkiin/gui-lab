//! The crate and the npm package for the page side carry the same version (CHANGELOG.md, "Versions").
#[test]
fn the_npm_package_has_the_crates_version() {
    let pkg: serde_json::Value = serde_json::from_str(include_str!("../web/package.json")).unwrap();
    assert_eq!(pkg["version"].as_str(), Some(env!("CARGO_PKG_VERSION")));
}
