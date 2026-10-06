//! Optional opaque provenance, kept outside the scientific request and identity.
pub(crate) const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_SOURCES: usize = 256;
pub(crate) const MAX_SOURCE_NAME_BYTES: usize = 4096;

/// Exact original source bytes with an informational original name.
/// The name is never an archive member name or an extraction instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProvenance {
    pub(crate) original_name: String,
    pub(crate) bytes: Vec<u8>,
}

impl SourceProvenance {
    pub fn original_name(&self) -> &str {
        &self.original_name
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
