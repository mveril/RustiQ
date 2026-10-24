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

    /// Borrows both components in alpha, beta order.
    #[must_use]
    pub fn as_ref(&self) -> Spin<&T> {
        Spin::new(&self.alpha, &self.beta)
    }

    /// Takes both components in alpha, beta order.
    #[must_use]
    pub fn into_parts(self) -> (T, T) {
        (self.alpha, self.beta)
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

#[cfg(test)]
mod tests {
    use super::Spin;

    #[derive(Debug, PartialEq, Eq)]
    struct NotClone(&'static str);

    #[test]
    fn as_ref_borrows_non_copy_non_clone_components_in_order() {
        let values = Spin::new(NotClone("alpha"), NotClone("beta"));
        let borrowed = values.as_ref();

        assert!(std::ptr::eq(*borrowed.alpha(), &raw const values.alpha));
        assert!(std::ptr::eq(*borrowed.beta(), &raw const values.beta));
        assert_eq!(borrowed.alpha().0, "alpha");
        assert_eq!(borrowed.beta().0, "beta");
    }

    #[test]
    fn into_parts_moves_non_clone_components_in_order() {
        let (alpha, beta) = Spin::new(NotClone("alpha"), NotClone("beta")).into_parts();

        assert_eq!(alpha.0, "alpha");
        assert_eq!(beta.0, "beta");
    }
}
