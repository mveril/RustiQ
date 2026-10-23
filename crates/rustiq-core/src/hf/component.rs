use super::uhf::Spin;

/// Shared restricted data or separate unrestricted alpha and beta components.
#[derive(Debug, Clone, Copy)]
pub(crate) enum HfComponent<T> {
    Restricted(T),
    Unrestricted(Spin<T>),
}
