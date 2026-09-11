use crate::config::ResolvedHfMethod;

use super::uhf::Spin;

/// A shared restricted component or separate unrestricted spin components.
#[derive(Debug, Clone, Copy)]
pub(crate) enum HfComponent<T> {
    Rhf(T),
    Uhf(Spin<T>),
}

impl<T> HfComponent<T> {
    pub(crate) fn method(&self) -> ResolvedHfMethod {
        match self {
            Self::Rhf(_) => ResolvedHfMethod::Rhf,
            Self::Uhf(_) => ResolvedHfMethod::Uhf,
        }
    }
}
