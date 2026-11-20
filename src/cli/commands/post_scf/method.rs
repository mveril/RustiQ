//! Method identifiers, prerequisites and typed dispatch for the campaign.
use miette::miette;
use rustiq_core::{
    calculation::{AnyHfSolution, Mp2Calculation, Mp2Result},
    config::{Mp2Config, ResolvedHfMethod},
    persistence::{PortableError, RustiQData},
};

use super::{Outcome, Skip};
use crate::runfile::resolved::PostScfMethods;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Method {
    Mp2,
    #[cfg(test)]
    RestrictedProbe,
}

impl Method {
    const SUPPORTED: &[Self] = &[Self::Mp2];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Mp2 => "mp2",
            #[cfg(test)]
            Self::RestrictedProbe => "restricted_probe",
        }
    }
}

pub(super) enum Request {
    Mp2(Mp2Config),
    #[cfg(test)]
    RestrictedProbe,
}

pub(super) enum ScientificResult {
    Mp2 {
        config: Mp2Config,
        reference: ResolvedHfMethod,
        result: Mp2Result,
        nuclear_repulsion_energy: f64,
    },
    #[cfg(test)]
    RestrictedProbe,
}

impl ScientificResult {
    pub(super) fn update_snapshot(&self, data: RustiQData) -> Result<RustiQData, PortableError> {
        match self {
            Self::Mp2 { config, .. } => data.with_mp2(*config),
            #[cfg(test)]
            Self::RestrictedProbe => Ok(data),
        }
    }
}

impl Request {
    pub(super) fn select(config: &PostScfMethods, methods: &[String]) -> miette::Result<Vec<Self>> {
        let configured: Vec<_> = config.mp2.into_iter().map(Self::Mp2).collect();
        for name in methods {
            if !Method::SUPPORTED.iter().any(|method| method.name() == name) {
                let supported = Method::SUPPORTED
                    .iter()
                    .map(|method| method.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(miette!(
                    "Post-HF method '{name}' is unavailable. Supported methods: {supported}."
                ));
            }
            if !configured
                .iter()
                .any(|request| request.method().name() == name)
            {
                return Err(miette!("Post-HF method '{name}' is not configured. Add [method.{name}] to the Post-HF TOML file."));
            }
        }
        if configured.is_empty() {
            return Err(miette!(
                "No Post-HF methods are configured. Add [method.mp2] to the Post-HF TOML file."
            ));
        }
        Ok(configured
            .into_iter()
            .filter(|request| {
                methods.is_empty() || methods.iter().any(|name| name == request.method().name())
            })
            .collect())
    }

    pub(super) fn method(&self) -> Method {
        match self {
            Self::Mp2(_) => Method::Mp2,
            #[cfg(test)]
            Self::RestrictedProbe => Method::RestrictedProbe,
        }
    }

    pub(super) fn check_prerequisites(
        &self,
        data: &rustiq_core::persistence::RustiQData,
    ) -> Result<(), Skip> {
        let reference = data.resolved_hf_method();
        let supported = match self {
            Self::Mp2(_) => matches!(
                reference,
                Some(ResolvedHfMethod::Rhf | ResolvedHfMethod::Uhf)
            ),
            #[cfg(test)]
            Self::RestrictedProbe => matches!(reference, Some(ResolvedHfMethod::Rhf)),
        };
        if supported {
            return Ok(());
        }
        Err(unsupported(reference))
    }

    pub(super) fn execute(&self, reference: &AnyHfSolution) -> Outcome {
        match self {
            Self::Mp2(config) => execute_mp2(reference, *config),
            #[cfg(test)]
            Self::RestrictedProbe => Outcome::Succeeded(ScientificResult::RestrictedProbe),
        }
    }
}

fn execute_mp2(reference: &AnyHfSolution, config: Mp2Config) -> Outcome {
    let result = match reference {
        AnyHfSolution::Restricted(hf) => Mp2Calculation::mp2(hf, config),
        AnyHfSolution::Unrestricted(hf) => Mp2Calculation::mp2(hf, config),
        _ => return Outcome::Skipped(unsupported(Some(reference.method()))),
    };
    let nuclear_repulsion_energy = reference.summary().scf.nuclear_repulsion_energy;
    match result {
        Ok(result) => {
            if ![
                result.correlation_energy,
                result.electronic_energy,
                nuclear_repulsion_energy,
                result.electronic_energy + nuclear_repulsion_energy,
            ]
            .into_iter()
            .all(f64::is_finite)
            {
                return Outcome::Failed(miette!("MP2 produced a non-finite energy"));
            }
            Outcome::Succeeded(ScientificResult::Mp2 {
                config,
                reference: reference.method(),
                result,
                nuclear_repulsion_energy,
            })
        }
        Err(error) => Outcome::Failed(error.into_cause().into()),
    }
}

fn unsupported(reference: Option<ResolvedHfMethod>) -> Skip {
    let reason = reference.map_or_else(
        || "archived HF reference type is unavailable".to_owned(),
        |method| format!("the archived {method:?} reference is unsupported by this method"),
    );
    Skip {
        reason,
        help: "Select a calculation with a supported HF reference.",
    }
}
