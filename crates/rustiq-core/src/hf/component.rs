use super::uhf::Spin;

/// A shared restricted component or separate unrestricted spin components.
#[derive(Debug, Clone, Copy)]
pub(crate) enum HfComponent<T> {
    Rhf(T),
    Uhf(Spin<T>),
}
