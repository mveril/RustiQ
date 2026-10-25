use std::sync::OnceLock;

use rustiq_core::{
    basis::BasisFile,
    calculation::{
        CalculationBuilder, CalculationEvent, CalculationExecution, CalculationResult, ScfIteration,
    },
    molecules::Geometry,
};

static SAMPLE: OnceLock<(CalculationResult, ScfIteration)> = OnceLock::new();

fn sample() -> &'static (CalculationResult, ScfIteration) {
    SAMPLE.get_or_init(|| {
        let geometry =
            Geometry::from_reader(&include_bytes!("../../../samples/h2/molecule.xyz")[..])
                .expect("bundled H2 geometry is valid");
        let basis = BasisFile::from_reader(
            &include_bytes!("../../../crates/rustiq-core/tests/data/sto-3g.json")[..],
        )
        .expect("bundled STO-3G basis is valid");
        let mut first_iteration = None;
        let result = CalculationBuilder::new(&geometry, &basis)
            .execute_with_events(|event| {
                if let CalculationEvent::ScfIteration(iteration) = event {
                    first_iteration.get_or_insert_with(|| iteration.clone());
                }
            })
            .expect("bundled H2 calculation succeeds");
        (
            result,
            first_iteration.expect("SCF emits at least one iteration"),
        )
    })
}

pub(crate) fn scf_result() -> rustiq_core::calculation::ScfResult {
    sample().0.hf.summary().scf.clone()
}

pub(crate) fn scf_iteration() -> ScfIteration {
    sample().1.clone()
}
