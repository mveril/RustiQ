pub fn auto_download_value() -> bool {
    std::env::var_os("RUSTIQ_AUTO_DOWNLOAD")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
