//! Rendering adapter for the scientific unit enum, owned by the frontend.
#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde serializer callbacks receive references"
)]
pub(crate) fn serialize<S: serde::Serializer>(
    value: &rustiq_core::molecules::Units,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(match value {
        rustiq_core::molecules::Units::Bohr => "Bohr",
        rustiq_core::molecules::Units::Angstrom => "Angstrom",
        _ => {
            return Err(serde::ser::Error::custom(
                "unsupported coordinate unit in runfile",
            ))
        }
    })
}
