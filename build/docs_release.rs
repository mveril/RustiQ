use std::error::Error;

pub fn is_release(
    explicit: Option<&str>,
    manifest: &str,
    marker: Option<&str>,
    version: &str,
) -> Result<bool, Box<dyn Error>> {
    match explicit {
        Some("1") => return Ok(true),
        Some("0") => return Ok(false),
        Some(_) => return Err("RUSTIQ_DOCS_RELEASE must be 0 or 1".into()),
        None => {}
    }
    let document = manifest.parse::<toml_edit::DocumentMut>()?;
    let published_version = document
        .get("package")
        .and_then(|item| item.get("metadata"))
        .and_then(|item| item.get("rustiq"))
        .and_then(|item| item.get("docs-release"))
        .and_then(toml_edit::Item::as_str);
    Ok(published_version == Some(version) || marker.map(str::trim) == Some(version))
}
