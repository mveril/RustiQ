use crate::config::ResolvedHfMethod;

use super::uhf::Spin;

/// Shared restricted data or separate unrestricted alpha and beta components.
#[derive(Debug, Clone, Copy)]
pub(crate) enum HfComponent<T> {
    Restricted(T),
    Unrestricted(Spin<T>),
}

impl<T> HfComponent<T> {
    pub(crate) fn method(&self) -> ResolvedHfMethod {
        match self {
            Self::Restricted(_) => ResolvedHfMethod::Rhf,
            Self::Unrestricted(_) => ResolvedHfMethod::Uhf,
        }
    }
}
