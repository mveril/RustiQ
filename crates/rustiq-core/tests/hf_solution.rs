#![allow(
    clippy::unwrap_used,
    reason = "Integration tests and their fixture helpers intentionally panic on unexpected failures"
)]

use rustiq_core::{
    basis::BasisFile,
    calculation::{
        AnyHfSolution, CalculationBuilder, CalculationError, CalculationEvent,
        CalculationExecution, CalculationExecutionError, HfOutcome, Mp2Result,
    },
    config::{HfConfig, HfMethod, MoleculeConfig, Mp2Config, ResolvedHfMethod},
    molecules::{Geometry, Units},
};

fn solution(method: HfMethod, iterations: usize) -> HfOutcome {
    let geometry =
        Geometry::from_reader(&include_bytes!("data/samples/h2/molecule.xyz")[..]).unwrap();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &file)
        .with_molecule_config({
            let mut updated_config = MoleculeConfig::default();
            updated_config.units = Units::Angstrom;
            updated_config
        })
        .with_hf({
            let mut updated_config = HfConfig::default();
            updated_config.method = method.into();
            updated_config.max_iterations = iterations.try_into().unwrap();
            updated_config
        })
        // run_hf must ignore MP2 configuration, including invalid configurations.
        .with_mp2({
            let mut updated_config = Mp2Config::default();
            updated_config.frozen_orbitals = 99.into();
            updated_config
        })
        .prepare()
        .unwrap();
    let mut mp2_events = 0;
    let mut calculation_events = 0;
    let hf = prepared
        .run_hf_with_events(|event| {
            if matches!(event, CalculationEvent::Mp2Completed { .. }) {
                mp2_events += 1;
            }
            if matches!(event, CalculationEvent::CalculationCompleted(_)) {
                calculation_events += 1;
            }
        })
        .unwrap();
    assert_eq!(mp2_events, 0);
    assert_eq!(calculation_events, 0);
    hf
}

#[allow(
    clippy::panic,
    clippy::panic_in_result_fn,
    reason = "A new public HF solution variant requires updating this test helper"
)]
fn mp2_for_runtime_reference(
    hf: &AnyHfSolution,
    config: Mp2Config,
) -> Result<Mp2Result, CalculationExecutionError> {
    match hf {
        AnyHfSolution::Restricted(hf) => hf.mp2(config),
        AnyHfSolution::Unrestricted(hf) => hf.mp2(config),
        _ => panic!("unexpected future HF solution variant"),
    }
}

#[test]
fn outcome_reports_resolved_method_for_both_convergence_states() {
    for (method, expected) in [
        (HfMethod::Auto, ResolvedHfMethod::Rhf),
        (HfMethod::Rhf, ResolvedHfMethod::Rhf),
        (HfMethod::Uhf, ResolvedHfMethod::Uhf),
    ] {
        for iterations in [1, 100] {
            let outcome = solution(method, iterations);
            assert_eq!(outcome.method(), expected);
            assert_eq!(outcome.is_converged(), iterations == 100);
            match outcome {
                HfOutcome::Converged(hf) => assert_eq!(hf.method(), expected),
                HfOutcome::Unconverged(hf) => assert_eq!(hf.method(), expected),
            }
        }
    }
}

#[test]
fn hf_outlives_inputs_and_can_retry_mp2_after_error() {
    for (method, resolved_method) in [
        (HfMethod::Rhf, ResolvedHfMethod::Rhf),
        (HfMethod::Uhf, ResolvedHfMethod::Uhf),
    ] {
        let HfOutcome::Converged(hf) = solution(method, 100) else {
            panic!("expected convergence");
        };
        assert_eq!(hf.method(), resolved_method);
        let first = mp2_for_runtime_reference(&hf, Mp2Config::default()).unwrap();
        assert!(std::ptr::eq(hf.summary(), hf.clone().summary()));
        let error = mp2_for_runtime_reference(&hf, {
            let mut updated_config = Mp2Config::default();
            updated_config.frozen_orbitals = 99.into();
            updated_config
        })
        .unwrap_err();
        assert!(matches!(error.cause(), CalculationError::Mp2 { .. }));
        assert!(matches!(error.hf(), Some(HfOutcome::Converged(_))));
        drop(hf);
        let HfOutcome::Converged(recovered) = error.into_hf().unwrap() else {
            panic!("expected convergence");
        };
        assert_eq!(
            first,
            mp2_for_runtime_reference(&recovered, Mp2Config::default()).unwrap()
        );
        assert_eq!(
            first,
            mp2_for_runtime_reference(&recovered, Mp2Config::default()).unwrap()
        );
    }
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "Canonicalization or cloning must preserve the previously computed state exactly"
)]
fn unconverged_hf_is_retained_but_cannot_run_mp2() {
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let hf = solution(method, 1);
        let HfOutcome::Unconverged(hf) = hf else {
            panic!("expected unconverged HF");
        };
        assert_eq!(hf.summary().scf.iterations, 1);
        assert!(hf.summary().scf.total_energy.is_finite());
        assert_eq!(
            hf.clone().summary().scf.total_energy,
            hf.summary().scf.total_energy
        );
    }
}

#[test]
fn mp2_memory_errors_preserve_hf_for_retry() {
    use rustiq_core::calculation::Mp2Error;
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        let HfOutcome::Converged(hf) = solution(method, 100) else {
            panic!("convergence");
        };
        for budget in [0, 1] {
            let error = mp2_for_runtime_reference(&hf, {
                let mut updated_config = Mp2Config::default();
                updated_config.memory_limit =
                    rustiq_core::config::MemoryLimit::Fixed(bytesize::ByteSize::b(budget)).into();
                updated_config
            })
            .unwrap_err();
            assert!(matches!(
                error.cause(),
                CalculationError::Mp2 {
                    error: Mp2Error::InvalidMemoryLimit | Mp2Error::InsufficientMemory { .. },
                    ..
                }
            ));
            let HfOutcome::Converged(recovered) = error.into_hf().unwrap() else {
                panic!("retained HF");
            };
            let result = mp2_for_runtime_reference(&recovered, {
                let mut updated_config = Mp2Config::default();
                updated_config.memory_limit =
                    rustiq_core::config::MemoryLimit::Fixed(bytesize::ByteSize::b(1024)).into();
                updated_config
            })
            .unwrap();
            assert_eq!(
                result,
                mp2_for_runtime_reference(&hf, Mp2Config::default()).unwrap()
            );
        }
    }
}

#[test]
fn preparation_error_has_no_hf() {
    let geometry =
        Geometry::from_reader(&include_bytes!("data/samples/h2/molecule.xyz")[..]).unwrap();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let error = CalculationBuilder::new(&geometry, &file)
        .with_molecule_config({
            let mut updated_config = MoleculeConfig::default();
            updated_config.charge = 99.into();
            updated_config
        })
        .execute()
        .unwrap_err();
    assert!(error.hf().is_none());
    assert!(matches!(error.cause(), CalculationError::Molecule { .. }));
}

#[test]
fn typed_solutions_preserve_reference_and_mp2() {
    use rustiq_core::calculation::{
        AnyHfSolution, Mp2Calculation, RestrictedMethod, RhfSolution, UhfSolution,
        UnrestrictedMethod,
    };

    fn via_capability<S: Mp2Calculation>(solution: &S) -> rustiq_core::calculation::Mp2Result {
        solution.mp2(Mp2Config::default()).unwrap()
    }

    let HfOutcome::Converged(AnyHfSolution::Restricted(hf)) = solution(HfMethod::Rhf, 100) else {
        panic!("expected restricted converged solution");
    };
    let hf: RhfSolution = hf;
    let occupied: &usize = hf.occupied_orbitals();
    assert_eq!(*occupied, 1);
    assert_eq!(hf.mo_coefficients().ncols(), hf.orbital_energies().len());
    assert!(std::ptr::eq(
        hf.mo_coefficients(),
        hf.clone().mo_coefficients()
    ));
    assert_eq!(hf.method(), RestrictedMethod::Rhf);
    assert_eq!(HfMethod::from(hf.method()), HfMethod::Rhf);
    assert_eq!(ResolvedHfMethod::from(hf.method()), ResolvedHfMethod::Rhf);
    let expected = via_capability(&hf);
    assert_eq!(expected, hf.mp2(Mp2Config::default()).unwrap());

    let HfOutcome::Converged(AnyHfSolution::Unrestricted(hf)) = solution(HfMethod::Uhf, 100) else {
        panic!("expected unrestricted converged solution");
    };
    let hf: UhfSolution = hf;
    assert_eq!(*hf.occupied_orbitals().alpha(), 1);
    assert_eq!(*hf.occupied_orbitals().beta(), 1);
    assert_eq!(
        hf.mo_coefficients().alpha().ncols(),
        hf.orbital_energies().alpha().len()
    );
    assert_eq!(
        hf.mo_coefficients().beta().ncols(),
        hf.orbital_energies().beta().len()
    );
    assert!(std::ptr::eq(
        hf.mo_coefficients(),
        hf.clone().mo_coefficients()
    ));
    assert_eq!(hf.method(), UnrestrictedMethod::Uhf);
    assert_eq!(HfMethod::from(hf.method()), HfMethod::Uhf);
    assert_eq!(ResolvedHfMethod::from(hf.method()), ResolvedHfMethod::Uhf);
    let expected = via_capability(&hf);
    assert_eq!(expected, hf.mp2(Mp2Config::default()).unwrap());
}

#[test]
fn method_conversions_accept_only_the_matching_explicit_reference() {
    use rustiq_core::config::{HfMethodConversionError, RestrictedMethod, UnrestrictedMethod};
    for method in [HfMethod::Auto, HfMethod::Rhf, HfMethod::Uhf] {
        let restricted = RestrictedMethod::try_from(method);
        let unrestricted = UnrestrictedMethod::try_from(method);
        if method == HfMethod::Rhf {
            assert_eq!(restricted, Ok(RestrictedMethod::Rhf));
        } else {
            assert_eq!(
                restricted,
                Err(HfMethodConversionError::Restricted { method })
            );
        }
        if method == HfMethod::Uhf {
            assert_eq!(unrestricted, Ok(UnrestrictedMethod::Uhf));
        } else {
            assert_eq!(
                unrestricted,
                Err(HfMethodConversionError::Unrestricted { method })
            );
        }
    }
    assert_eq!(
        RestrictedMethod::try_from(ResolvedHfMethod::Rhf),
        Ok(RestrictedMethod::Rhf)
    );
    assert_eq!(
        UnrestrictedMethod::try_from(ResolvedHfMethod::Uhf),
        Ok(UnrestrictedMethod::Uhf)
    );
    assert_eq!(
        RestrictedMethod::try_from(ResolvedHfMethod::Uhf),
        Err(HfMethodConversionError::Restricted {
            method: HfMethod::Uhf
        })
    );
    assert_eq!(
        UnrestrictedMethod::try_from(ResolvedHfMethod::Rhf),
        Err(HfMethodConversionError::Unrestricted {
            method: HfMethod::Rhf
        })
    );
}

#[test]
fn open_shell_solution_retains_separate_occupations_through_mp2_recovery() {
    use rustiq_core::calculation::AnyHfSolution;
    let geometry = Geometry::from_reader(&include_bytes!("data/samples/oh/oh.xyz")[..]).unwrap();
    let file = BasisFile::from_reader(&include_bytes!("data/sto-3g.json")[..]).unwrap();
    let prepared = CalculationBuilder::new(&geometry, &file)
        .with_molecule_config({
            let mut updated_config = MoleculeConfig::default();
            updated_config.units = Units::Angstrom;
            updated_config.multiplicity = std::num::NonZeroU8::new(2).unwrap().into();
            updated_config
        })
        .prepare()
        .unwrap();
    let HfOutcome::Converged(AnyHfSolution::Unrestricted(hf)) = prepared.run_hf().unwrap() else {
        panic!("expected converged unrestricted OH");
    };
    assert_eq!(*hf.occupied_orbitals().alpha(), 5);
    assert_eq!(*hf.occupied_orbitals().beta(), 4);
    assert_eq!(
        hf.mo_coefficients().alpha().ncols(),
        hf.orbital_energies().alpha().len()
    );
    assert_eq!(
        hf.mo_coefficients().beta().ncols(),
        hf.orbital_energies().beta().len()
    );
    let error = hf
        .mp2({
            let mut updated_config = Mp2Config::default();
            updated_config.frozen_orbitals = 99.into();
            updated_config
        })
        .unwrap_err();
    let HfOutcome::Converged(AnyHfSolution::Unrestricted(recovered)) = error.into_hf().unwrap()
    else {
        panic!("expected retained unrestricted solution");
    };
    assert_eq!(*recovered.occupied_orbitals().alpha(), 5);
    assert_eq!(*recovered.occupied_orbitals().beta(), 4);
    assert!(std::ptr::eq(
        hf.mo_coefficients(),
        recovered.mo_coefficients()
    ));
    assert_eq!(
        hf.mp2(Mp2Config::default()).unwrap(),
        recovered.mp2(Mp2Config::default()).unwrap()
    );
}

#[test]
fn unconverged_solutions_keep_reference_specific_occupation_types() {
    use rustiq_core::calculation::{AnyHfSolution, RhfSolution, Spin, UhfSolution, Unconverged};

    let HfOutcome::Unconverged(AnyHfSolution::Restricted(hf)) = solution(HfMethod::Rhf, 1) else {
        panic!("expected unconverged restricted solution");
    };
    let hf: RhfSolution<Unconverged> = hf;
    let occupied: &usize = hf.occupied_orbitals();
    assert_eq!(*occupied, 1);

    let HfOutcome::Unconverged(AnyHfSolution::Unrestricted(hf)) = solution(HfMethod::Uhf, 1) else {
        panic!("expected unconverged unrestricted solution");
    };
    let hf: UhfSolution<Unconverged> = hf;
    let occupied: &Spin<usize> = hf.occupied_orbitals();
    assert_eq!(*occupied.alpha(), 1);
    assert_eq!(*occupied.beta(), 1);
}

#[test]
fn borrowed_accessors_preserve_reference_state_and_shared_data() {
    use rustiq_core::calculation::AnyHfSolution;

    fn check_reference<State>(hf: &AnyHfSolution<State>, expected: ResolvedHfMethod) {
        assert_eq!(hf.method(), expected);
        match expected {
            ResolvedHfMethod::Rhf => {
                let restricted = hf.as_restricted().unwrap();
                assert!(hf.as_unrestricted().is_none());
                assert!(std::ptr::eq(hf.summary(), restricted.summary()));
                assert!(std::ptr::eq(hf.ao_eri(), restricted.ao_eri()));
                assert_eq!(*restricted.occupied_orbitals(), 1);
            }
            ResolvedHfMethod::Uhf => {
                let unrestricted = hf.as_unrestricted().unwrap();
                assert!(hf.as_restricted().is_none());
                assert!(std::ptr::eq(hf.summary(), unrestricted.summary()));
                assert!(std::ptr::eq(hf.ao_eri(), unrestricted.ao_eri()));
                assert_eq!(*unrestricted.occupied_orbitals().alpha(), 1);
                assert_eq!(*unrestricted.occupied_orbitals().beta(), 1);
            }
            _ => panic!("unexpected future HF method variant"),
        }
    }

    for (method, expected) in [
        (HfMethod::Rhf, ResolvedHfMethod::Rhf),
        (HfMethod::Uhf, ResolvedHfMethod::Uhf),
    ] {
        for iterations in [1, 100] {
            let outcome = solution(method, iterations);
            if iterations == 100 {
                assert!(outcome.as_unconverged().is_none());
                check_reference(outcome.as_converged().unwrap(), expected);
            } else {
                assert!(outcome.as_converged().is_none());
                check_reference(outcome.as_unconverged().unwrap(), expected);
            }
        }
    }
}

#[test]
fn try_from_converged_output_retains_unconverged_output_in_errors() {
    use rustiq_core::calculation::{AnyHfSolution, Converged};
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        for iterations in [1, 100] {
            let outcome = solution(method, iterations);
            let retained = outcome.clone();
            if iterations == 100 {
                let converged: AnyHfSolution<Converged> = outcome.try_into().unwrap();
                assert!(std::ptr::eq(converged.summary(), retained.summary()));
                assert!(std::ptr::eq(converged.ao_eri(), retained.ao_eri()));
                assert_eq!(converged.method(), retained.method());
            } else {
                let error = AnyHfSolution::<Converged>::try_from(outcome).unwrap_err();
                assert!(matches!(
                    error.cause(),
                    CalculationError::HfNotConverged { iterations: 1 }
                ));
                let recovered = error.into_hf().unwrap();
                assert!(recovered.as_converged().is_none());
                assert!(recovered.as_unconverged().is_some());
                assert!(std::ptr::eq(recovered.summary(), retained.summary()));
                assert!(std::ptr::eq(recovered.ao_eri(), retained.ao_eri()));
                assert_eq!(recovered.method(), retained.method());
            }
        }
    }
}

#[test]
fn taking_wrong_reference_returns_a_typed_error_and_retains_the_original_solution() {
    let restricted = solution(HfMethod::Rhf, 100).into_converged().unwrap();
    let retained = restricted.clone();
    let error = restricted.into_unrestricted().unwrap_err();
    assert_eq!(
        error.expected(),
        rustiq_core::calculation::HfReferenceKind::Unrestricted
    );
    assert_eq!(
        error.actual(),
        rustiq_core::calculation::HfReferenceKind::Restricted
    );
    let original = error.into_solution();
    assert!(std::ptr::eq(original.summary(), retained.summary()));
    let restricted = original.into_restricted().unwrap();
    assert!(std::ptr::eq(restricted.summary(), retained.summary()));
    assert!(std::ptr::eq(restricted.ao_eri(), retained.ao_eri()));

    let unrestricted = solution(HfMethod::Uhf, 100).into_converged().unwrap();
    let retained = unrestricted.clone();
    let error = unrestricted.into_restricted().unwrap_err();
    assert_eq!(
        error.expected(),
        rustiq_core::calculation::HfReferenceKind::Restricted
    );
    assert_eq!(
        error.actual(),
        rustiq_core::calculation::HfReferenceKind::Unrestricted
    );
    let original = error.into_solution();
    assert!(std::ptr::eq(original.summary(), retained.summary()));
    let unrestricted = original.into_unrestricted().unwrap();
    assert!(std::ptr::eq(unrestricted.summary(), retained.summary()));
    assert!(std::ptr::eq(unrestricted.ao_eri(), retained.ao_eri()));
}

#[test]
fn standard_reference_conversions_round_trip_both_convergence_states() {
    use rustiq_core::calculation::{AnyHfSolution, RhfSolution, UhfSolution};

    fn round_trip<State: Clone + std::fmt::Debug>(
        hf: AnyHfSolution<State>,
    ) -> AnyHfSolution<State> {
        let retained = hf.clone();
        let wrapped = match hf.method() {
            ResolvedHfMethod::Rhf => {
                let error = UhfSolution::<State>::try_from(hf).unwrap_err();
                assert_eq!(
                    error.expected(),
                    rustiq_core::calculation::HfReferenceKind::Unrestricted
                );
                assert_eq!(
                    error.actual(),
                    rustiq_core::calculation::HfReferenceKind::Restricted
                );
                let original = error.into_solution();
                assert!(std::ptr::eq(original.summary(), retained.summary()));
                let typed: RhfSolution<State> = original.try_into().unwrap();
                AnyHfSolution::from(typed)
            }
            ResolvedHfMethod::Uhf => {
                let error = RhfSolution::<State>::try_from(hf).unwrap_err();
                assert_eq!(
                    error.expected(),
                    rustiq_core::calculation::HfReferenceKind::Restricted
                );
                assert_eq!(
                    error.actual(),
                    rustiq_core::calculation::HfReferenceKind::Unrestricted
                );
                let original = error.into_solution();
                assert!(std::ptr::eq(original.summary(), retained.summary()));
                let typed: UhfSolution<State> = original.try_into().unwrap();
                typed.into()
            }
            _ => panic!("unexpected future HF method variant"),
        };
        assert!(std::ptr::eq(wrapped.summary(), retained.summary()));
        assert!(std::ptr::eq(wrapped.ao_eri(), retained.ao_eri()));
        wrapped
    }

    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        for iterations in [1, 100] {
            let outcome = solution(method, iterations);
            let retained = outcome.clone();
            let restored: HfOutcome = match outcome {
                HfOutcome::Converged(hf) => HfOutcome::from(round_trip(hf)),
                HfOutcome::Unconverged(hf) => round_trip(hf).into(),
            };
            assert_eq!(restored.is_converged(), retained.is_converged());
            assert_eq!(restored.method(), retained.method());
            assert!(std::ptr::eq(restored.summary(), retained.summary()));
            assert!(std::ptr::eq(restored.ao_eri(), retained.ao_eri()));
        }
    }
}

fn assert_shared_scf_output<M: rustiq_core::prelude::ScfMethod, State>(
    actual: &rustiq_core::prelude::ScfSolution<M, State>,
    expected: &rustiq_core::prelude::ScfSolution<M, State>,
) {
    assert!(std::ptr::eq(
        actual.method_descriptor(),
        expected.method_descriptor()
    ));
    assert!(std::ptr::eq(
        actual.mo_coefficients(),
        expected.mo_coefficients()
    ));
    assert!(std::ptr::eq(
        actual.orbital_energies(),
        expected.orbital_energies()
    ));
    assert!(std::ptr::eq(actual.occupations(), expected.occupations()));
    assert!(std::ptr::eq(actual.summary(), expected.summary()));
    assert!(std::ptr::eq(
        actual.retained_data(),
        expected.retained_data()
    ));
}

#[test]
fn generic_owned_output_preserves_hf_storage_state_and_mp2() {
    use rustiq_core::prelude::{
        AnyHfSolution, HfMethodDescriptor, HfReference, HfSolution, ScfMethod, ScfSolution,
    };

    fn check<R: HfReference, State>(hf: HfSolution<R, State>) -> HfSolution<R, State> {
        fn inspect<M: ScfMethod, S>(scf: &ScfSolution<M, S>) {
            assert_shared_scf_output(scf, &scf.clone());
        }
        let retained = hf.clone();
        inspect(hf.as_ref());
        assert!(std::ptr::eq(hf.as_scf(), hf.as_ref()));
        let scf: ScfSolution<HfMethodDescriptor<R>, State> = hf.into();
        assert_shared_scf_output(&scf, retained.as_scf());
        assert!(std::ptr::eq(scf.retained_data(), retained.ao_eri()));
        assert!(std::ptr::eq(
            scf.occupations(),
            retained.occupied_orbitals()
        ));
        assert!(std::ptr::eq(
            scf.scf_result(),
            retained.as_scf().scf_result()
        ));
        let resolved: ResolvedHfMethod = scf.method_descriptor().method().into();
        assert_eq!(resolved, retained.method().into());
        let hf: HfSolution<R, State> = scf.into();
        // The named methods use the same allocation-preserving conversion traits.
        let hf = HfSolution::from_scf(hf.into_scf());
        assert_shared_scf_output(hf.as_scf(), retained.as_scf());
        hf
    }
    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        for iterations in [1, 100] {
            match solution(method, iterations) {
                HfOutcome::Converged(AnyHfSolution::Restricted(hf)) => {
                    let expected = hf.mp2(Mp2Config::default()).unwrap();
                    assert_eq!(check(hf).mp2(Mp2Config::default()).unwrap(), expected);
                }
                HfOutcome::Converged(AnyHfSolution::Unrestricted(hf)) => {
                    let expected = hf.mp2(Mp2Config::default()).unwrap();
                    assert_eq!(check(hf).mp2(Mp2Config::default()).unwrap(), expected);
                }
                HfOutcome::Unconverged(AnyHfSolution::Restricted(hf)) => {
                    check(hf);
                }
                HfOutcome::Unconverged(AnyHfSolution::Unrestricted(hf)) => {
                    check(hf);
                }
                _ => panic!("unexpected future HF outcome variant"),
            }
        }
    }
}

#[test]
fn generic_runtime_conversions_preserve_data_on_success_and_reference_mismatch() {
    use rustiq_core::prelude::{
        AnyHfSolution, HfMethodDescriptor, Restricted, ScfSolution, Unrestricted,
    };

    fn round_trip<State>(hf: AnyHfSolution<State>) -> AnyHfSolution<State> {
        let retained = hf.clone();
        let restored: AnyHfSolution<State> = match hf.method() {
            ResolvedHfMethod::Rhf => {
                let Err(error) =
                    ScfSolution::<HfMethodDescriptor<Unrestricted>, State>::try_from(hf)
                else {
                    panic!("restricted output must not become unrestricted");
                };
                assert_eq!(
                    error.expected(),
                    rustiq_core::calculation::HfReferenceKind::Unrestricted
                );
                assert_eq!(
                    error.actual(),
                    rustiq_core::calculation::HfReferenceKind::Restricted
                );
                let original = error.into_solution();
                assert!(matches!(original, AnyHfSolution::Restricted(_)));
                assert_shared_scf_output(
                    original.as_restricted().unwrap().as_scf(),
                    retained.as_restricted().unwrap().as_scf(),
                );
                let Ok(scf) =
                    ScfSolution::<HfMethodDescriptor<Restricted>, State>::try_from(original)
                else {
                    panic!("restricted output must be extractable");
                };
                assert_shared_scf_output(&scf, retained.as_restricted().unwrap().as_scf());
                scf.into()
            }
            ResolvedHfMethod::Uhf => {
                let Err(error) = ScfSolution::<HfMethodDescriptor<Restricted>, State>::try_from(hf)
                else {
                    panic!("unrestricted output must not become restricted");
                };
                assert_eq!(
                    error.expected(),
                    rustiq_core::calculation::HfReferenceKind::Restricted
                );
                assert_eq!(
                    error.actual(),
                    rustiq_core::calculation::HfReferenceKind::Unrestricted
                );
                let original = error.into_solution();
                assert!(matches!(original, AnyHfSolution::Unrestricted(_)));
                assert_shared_scf_output(
                    original.as_unrestricted().unwrap().as_scf(),
                    retained.as_unrestricted().unwrap().as_scf(),
                );
                let Ok(scf) =
                    ScfSolution::<HfMethodDescriptor<Unrestricted>, State>::try_from(original)
                else {
                    panic!("unrestricted output must be extractable");
                };
                assert_shared_scf_output(&scf, retained.as_unrestricted().unwrap().as_scf());
                scf.into()
            }
            _ => panic!("unexpected future HF method variant"),
        };
        assert_eq!(restored.method(), retained.method());
        assert!(std::ptr::eq(restored.summary(), retained.summary()));
        assert!(std::ptr::eq(restored.ao_eri(), retained.ao_eri()));
        match &restored {
            AnyHfSolution::Restricted(hf) => {
                assert_shared_scf_output(hf.as_scf(), retained.as_restricted().unwrap().as_scf());
            }
            AnyHfSolution::Unrestricted(hf) => {
                assert_shared_scf_output(hf.as_scf(), retained.as_unrestricted().unwrap().as_scf());
            }
            _ => panic!("unexpected future HF solution variant"),
        }
        restored
    }

    for method in [HfMethod::Rhf, HfMethod::Uhf] {
        for iterations in [1, 100] {
            let outcome = solution(method, iterations);
            let retained = outcome.clone();
            let restored: HfOutcome = match outcome {
                HfOutcome::Converged(hf) => round_trip(hf).into(),
                HfOutcome::Unconverged(hf) => round_trip(hf).into(),
            };
            assert_eq!(restored.is_converged(), retained.is_converged());
            assert_eq!(restored.method(), retained.method());
            assert!(std::ptr::eq(restored.summary(), retained.summary()));
            assert!(std::ptr::eq(restored.ao_eri(), retained.ao_eri()));
        }
    }
}
