//! Typed post-SCF methods and a session for archived HF references.

use std::{any::Any, sync::Arc};

use thiserror::Error;

use super::{PortableError, RustiQData};
use crate::{
    calculation::{AnyHfSolution, CalculationError, Mp2Result},
    config::{Mp2Config, ResolvedHfMethod},
};

/// A selected execution function for a configured method and its typed reference.
pub type PostScfExecutor<M> =
    fn(
        &M,
        &<M as PostScfMethod>::Reference,
    ) -> Result<<M as PostScfMethod>::Output, <M as PostScfMethod>::Error>;

/// A configured method that runs against a converged SCF reference.
pub trait PostScfMethod {
    /// Reference description available before loading the solution.
    type ReferenceKind;
    /// Converged solution consumed by the method through a shared reference.
    type Reference: ?Sized;
    type Output;
    type Error;

    /// Selects an executor for this reference kind before loading a solution.
    /// Returns `None` when the method cannot use the reference.
    fn executor(&self, reference: Self::ReferenceKind) -> Option<PostScfExecutor<Self>>;
}

/// A post-SCF method whose reference is a converged Hartree–Fock solution.
pub trait PostHfMethod:
    PostScfMethod<ReferenceKind = ResolvedHfMethod, Reference = AnyHfSolution>
{
}

impl<M> PostHfMethod for M where
    M: PostScfMethod<ReferenceKind = ResolvedHfMethod, Reference = AnyHfSolution>
{
}

/// A prerequisite or execution failure for one post-SCF method.
#[derive(Debug, Error)]
pub enum PostScfError<E> {
    #[error("unsupported archived HF reference: {0:?}")]
    UnsupportedReference(Option<ResolvedHfMethod>),
    #[error("could not restore the saved HF reference: {0}")]
    Restore(#[source] Arc<PortableError>),
    #[error(transparent)]
    Execution(E),
    #[error("the restored SCF solution has an unexpected type")]
    UnexpectedReferenceType,
}

type RestoredReference = Result<Box<dyn Any + Send + Sync>, Arc<PortableError>>;

/// Reuses a single lazy restoration for methods executed on an archived entry.
pub struct PostScfSession<'a> {
    data: &'a RustiQData,
    reference: Option<RestoredReference>,
}

impl<'a> PostScfSession<'a> {
    #[must_use]
    pub fn new(data: &'a RustiQData) -> Self {
        Self {
            data,
            reference: None,
        }
    }

    /// Checks compatibility, restores the reference at most once, and executes the method.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an unsupported reference, failed restoration, or
    /// failed method execution. Restoration failures are shared across eligible methods.
    pub fn run<M: PostHfMethod>(
        &mut self,
        method: &M,
    ) -> Result<M::Output, PostScfError<M::Error>> {
        let archived_method = self.data.resolved_hf_method();
        let Some(execute) = archived_method.and_then(|kind| method.executor(kind)) else {
            return Err(PostScfError::UnsupportedReference(archived_method));
        };
        let reference = self
            .reference
            .get_or_insert_with(|| {
                self.data
                    .restore::<AnyHfSolution>()
                    .map(|solution| Box::new(solution) as Box<dyn Any + Send + Sync>)
                    .map_err(Arc::new)
            })
            .as_ref()
            .map_err(|error| PostScfError::Restore(Arc::clone(error)))?;
        let reference = reference
            .downcast_ref::<M::Reference>()
            .ok_or(PostScfError::UnexpectedReferenceType)?;
        execute(method, reference).map_err(PostScfError::Execution)
    }
}

/// MP2 output obtained from a restored HF reference.
#[derive(Debug)]
pub struct PostScfMp2Result {
    config: Mp2Config,
    pub reference: ResolvedHfMethod,
    pub result: Mp2Result,
    pub nuclear_repulsion_energy: f64,
}

impl PostScfMp2Result {
    /// Records the applied MP2 settings in the requested and resolved snapshots.
    /// Keeps the HF reference and AO integrals needed to rerun MP2.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshots are missing or the settings cannot be represented.
    pub fn update_snapshot(&self, data: RustiQData) -> Result<RustiQData, PortableError> {
        let mut data = data.with_mp2(self.config)?;
        data.retain_mp2_reference();
        Ok(data)
    }
}

/// Scientific failures when running MP2 on a restored reference.
#[derive(Debug, Error, miette::Diagnostic)]
pub enum PostScfMp2Error {
    #[error("MP2 produced a non-finite energy")]
    NonFiniteEnergy,
    #[error(transparent)]
    #[diagnostic(transparent)]
    Calculation(#[from] CalculationError),
}

impl PostScfMethod for Mp2Config {
    type ReferenceKind = ResolvedHfMethod;
    type Reference = AnyHfSolution;
    type Output = PostScfMp2Result;
    type Error = PostScfMp2Error;

    fn executor(&self, reference: ResolvedHfMethod) -> Option<PostScfExecutor<Self>> {
        match reference {
            ResolvedHfMethod::Rhf | ResolvedHfMethod::Uhf => Some(execute_mp2),
        }
    }
}

fn execute_mp2(
    config: &Mp2Config,
    reference: &AnyHfSolution,
) -> Result<PostScfMp2Result, PostScfMp2Error> {
    let result = match reference {
        AnyHfSolution::Restricted(hf) => hf.mp2(*config),
        AnyHfSolution::Unrestricted(hf) => hf.mp2(*config),
    }
    .map_err(crate::calculation::CalculationExecutionError::into_cause)?;
    let nuclear_repulsion_energy = reference.summary().scf.nuclear_repulsion_energy;
    if ![
        result.correlation_energy,
        result.electronic_energy,
        nuclear_repulsion_energy,
        result.electronic_energy + nuclear_repulsion_energy,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        return Err(PostScfMp2Error::NonFiniteEnergy);
    }
    Ok(PostScfMp2Result {
        config: *config,
        reference: reference.method(),
        result,
        nuclear_repulsion_energy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        calculation::CalculationBuilder, molecules::Geometry, test_utils::load_minimal_basis_file,
    };
    use crate::{
        eri::CompactEri,
        persistence::{ConvergedHf, ConvergedHfArtifact, RustiQBundle},
    };

    struct Unsupported;

    impl PostScfMethod for Unsupported {
        type ReferenceKind = ResolvedHfMethod;
        type Reference = AnyHfSolution;
        type Output = ();
        type Error = std::convert::Infallible;

        fn executor(&self, _reference: ResolvedHfMethod) -> Option<PostScfExecutor<Self>> {
            None
        }
    }

    struct IntegralProbe;

    impl PostScfMethod for IntegralProbe {
        type ReferenceKind = ResolvedHfMethod;
        type Reference = AnyHfSolution;
        type Output = CompactEri;
        type Error = std::convert::Infallible;

        fn executor(&self, _reference: ResolvedHfMethod) -> Option<PostScfExecutor<Self>> {
            Some(|_, reference| Ok(reference.ao_eri().clone()))
        }
    }

    #[test]
    fn eligible_methods_share_the_restored_reference() {
        let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
        let basis = load_minimal_basis_file();
        let prepared = CalculationBuilder::new(&geometry, &basis)
            .prepare()
            .unwrap();
        let mut data = RustiQData::from_calculation(&prepared).unwrap();
        let solution = prepared.run_hf().unwrap().into_converged().unwrap();
        data.set::<ConvergedHfArtifact>(&ConvergedHf::from_solution(&solution).unwrap())
            .unwrap();
        data.set_eri(solution.ao_eri()).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hf.rustiq");
        RustiQBundle::new(vec![data]).unwrap().write(&path).unwrap();
        let bundle = RustiQBundle::open_for_restore(path).unwrap();
        let mut session = PostScfSession::new(&bundle.calculations()[0]);

        let first = session.run(&IntegralProbe).unwrap();
        assert!(matches!(
            session.run(&Unsupported),
            Err(PostScfError::UnsupportedReference(_))
        ));
        session.run(&Mp2Config::default()).unwrap();
        let second = session.run(&IntegralProbe).unwrap();

        // The probe keeps the shared allocation alive so a second restore
        // cannot pass by reusing its address.
        assert!(std::ptr::eq(
            first.ordered_values(),
            second.ordered_values()
        ));
    }

    #[test]
    fn skips_ineligible_methods_and_caches_failed_restoration() {
        let geometry = Geometry::from_source("h2.xyz", "2\nH2\nH 0 0 0\nH 1.4 0 0\n").unwrap();
        let basis = load_minimal_basis_file();
        let prepared = CalculationBuilder::new(&geometry, &basis)
            .prepare()
            .unwrap();
        let data = RustiQData::from_calculation(&prepared).unwrap();
        let mut session = PostScfSession::new(&data);
        assert!(matches!(
            session.run(&Unsupported),
            Err(PostScfError::UnsupportedReference(Some(_)))
        ));
        assert!(session.reference.is_none());

        let config = Mp2Config::default();
        let Err(PostScfError::Restore(first)) = session.run(&config) else {
            panic!("expected missing HF data");
        };
        let Err(PostScfError::Restore(second)) = session.run(&config) else {
            panic!("expected cached restoration failure");
        };
        assert!(Arc::ptr_eq(&first, &second));

        session.reference = Some(Ok(Box::new(())));
        assert!(matches!(
            session.run(&config),
            Err(PostScfError::UnexpectedReferenceType)
        ));
    }
}
