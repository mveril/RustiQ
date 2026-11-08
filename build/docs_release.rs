/// Development versions reserve a complete `dev` prerelease identifier.
pub fn is_release(version: &str) -> bool {
    let version = version.split('+').next().unwrap_or(version);
    !version
        .split_once('-')
        .is_some_and(|(_, pre)| pre.split('.').any(|part| part == "dev"))
}
