#[cfg(feature = "online")]
pub fn auto_download_value() -> bool {
    std::env::var_os("RUSTIQ_AUTO_DOWNLOAD")
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}
