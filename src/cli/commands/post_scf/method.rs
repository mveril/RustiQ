//! Method identifiers, prerequisites and typed dispatch for the campaign.
use miette::miette;
use rustiq_core::{
    config::{Mp2Config, ResolvedHfMethod},
    persistence::{PortableError, PostScfError, PostScfMp2Result, PostScfSession, RustiQData},
};

use super::{Outcome, RestoreError, Skip};
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
    Mp2(PostScfMp2Result),
    #[cfg(test)]
    RestrictedProbe,
}

impl ScientificResult {
    pub(super) fn update_snapshot(&self, data: RustiQData) -> Result<RustiQData, PortableError> {
        match self {
            Self::Mp2(result) => result.update_snapshot(data),
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
                    code = "rustiq::post_scf::method_unavailable",
                    "Post-HF method '{name}' is unavailable. Supported methods: {supported}."
                ));
            }
            if !configured
                .iter()
                .any(|request| request.method().name() == name)
            {
                return Err(miette!(code = "rustiq::post_scf::method_not_configured", "Post-HF method '{name}' is not configured. Add [method.{name}] to the Post-HF TOML file."));
            }
        }
        if configured.is_empty() {
            return Err(miette!(
                code = "rustiq::post_scf::no_methods",
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

    pub(super) fn execute(&self, session: &mut PostScfSession<'_>) -> Outcome {
        match self {
            Self::Mp2(config) => outcome(session.run(config).map(ScientificResult::Mp2)),
            #[cfg(test)]
            Self::RestrictedProbe => outcome(session.run(&RestrictedProbe)),
        }
    }
}

fn outcome<E>(result: Result<ScientificResult, PostScfError<E>>) -> Outcome
where
    E: miette::Diagnostic + Send + Sync + 'static,
{
    match result {
        Ok(result) => Outcome::Succeeded(result),
        Err(PostScfError::UnsupportedReference(reference)) => {
            Outcome::Skipped(unsupported(reference))
        }
        Err(PostScfError::Restore(error)) => match RestoreError::from(error.as_ref()) {
            RestoreError::Skipped(skip) => Outcome::Skipped(skip),
            RestoreError::Failed => Outcome::Failed(miette!(
                code = "rustiq::post_scf::restore_failed",
                "could not restore the saved HF reference: {error}"
            )),
        },
        Err(PostScfError::Execution(error)) => Outcome::Failed(miette::Report::new(error)),
    }
}

#[cfg(test)]
struct RestrictedProbe;

#[cfg(test)]
impl rustiq_core::persistence::PostScfMethod for RestrictedProbe {
    type ReferenceKind = ResolvedHfMethod;
    type Reference = rustiq_core::calculation::AnyHfSolution;
    type Output = ScientificResult;
    type Error = std::convert::Infallible;

    fn executor(
        &self,
        reference: ResolvedHfMethod,
    ) -> Option<rustiq_core::persistence::PostScfExecutor<Self>> {
        match reference {
            ResolvedHfMethod::Rhf => Some(|_, _| Ok(ScientificResult::RestrictedProbe)),
            _ => None,
        }
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
