//! Paired alpha and beta data independent of the orbital layout.

/// A value for each electron spin component.
#[derive(Debug, Clone, Copy)]
pub struct Spin<T> {
    pub(crate) alpha: T,
    pub(crate) beta: T,
}

impl<T> Spin<T> {
    /// Creates a pair of alpha and beta values.
    pub fn new(alpha: T, beta: T) -> Self {
        Self { alpha, beta }
    }

    /// Borrows the alpha component.
    #[must_use]
    pub fn alpha(&self) -> &T {
        &self.alpha
    }

    /// Borrows the beta component.
    #[must_use]
    pub fn beta(&self) -> &T {
        &self.beta
    }

    #[cfg(test)]
    pub(crate) fn as_ref(&self) -> Spin<&T> {
        Spin::new(&self.alpha, &self.beta)
    }

    pub(crate) fn zip_map<U, V>(self, other: Spin<U>, mut f: impl FnMut(T, U) -> V) -> Spin<V> {
        Spin::new(f(self.alpha, other.alpha), f(self.beta, other.beta))
    }
}

impl<T: Clone> Spin<T> {
    pub(crate) fn duplicate(value: T) -> Self {
        Self {
            alpha: value.clone(),
            beta: value,
        }
    }
}
