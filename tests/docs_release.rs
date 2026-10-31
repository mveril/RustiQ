#[path = "../build/docs_release.rs"]
mod docs_release;

#[test]
fn selects_release_only_with_explicit_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let development = "[package]\nname = \"rustiq-chem\"\n";
    let published = "[package.metadata.rustiq]\ndocs-release = \"0.1.0-beta.1\"\n";
    let version = "0.1.0-beta.1";
    assert!(!docs_release::is_release(None, development, None, version)?);
    assert!(docs_release::is_release(None, published, None, version)?);
    assert!(docs_release::is_release(
        None,
        development,
        Some(version),
        version
    )?);
    assert!(docs_release::is_release(
        Some("1"),
        development,
        None,
        version
    )?);
    assert!(!docs_release::is_release(
        Some("0"),
        published,
        Some(version),
        version
    )?);
    assert!(!docs_release::is_release(
        None,
        published,
        None,
        "0.1.0-beta.2"
    )?);
    assert!(docs_release::is_release(Some("yes"), development, None, version).is_err());
    Ok(())
}
