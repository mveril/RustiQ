#[path = "../build/docs_release.rs"]
mod docs_release;

#[test]
fn documentation_channel_follows_version() {
    for version in ["1.2.3-dev", "1.2.3-dev.1", "1.2.3-alpha.1.dev"] {
        assert!(!docs_release::is_release(version));
    }
    for version in [
        "1.2.3",
        "1.2.3-alpha.1",
        "1.2.3-beta.1",
        "1.2.3+dev",
        "1.2.3-device",
    ] {
        assert!(docs_release::is_release(version));
    }
}
