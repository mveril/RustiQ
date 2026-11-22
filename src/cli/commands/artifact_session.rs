//! Portable orchestration; scientific compatibility stays in rustiq-core.
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
};

use miette::{miette, IntoDiagnostic};
use rustiq_core::{
    calculation::{CalculationEvent, CalculationResult, EriCacheAction, PreparedCalculation},
    persistence::{
        AoEriArtifact, Artifact, ConvergedHf, ConvergedHfArtifact, RustiQBundle, RustiQData,
    },
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ArtifactReport {
    pub name: &'static str,
    pub decision: &'static str,
    pub origin: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_index: Option<usize>,
}

impl ArtifactReport {
    pub(crate) fn on_event(&mut self, event: &CalculationEvent<'_>) {
        if let CalculationEvent::EriCache(event) = event {
            self.origin = match event.action {
                EriCacheAction::Hit => "cache",
                EriCacheAction::Stored => "computation",
                _ => "unknown",
            };
        }
    }
}

/// Resolve an artifact output once so regular runs and Post-SCF share the same
/// no-clobber and symbolic-link behavior.
pub(crate) fn resolve_artifact_destination(
    reuse: Option<&Path>,
    path: &Path,
    force: bool,
) -> miette::Result<(PathBuf, bool)> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            let canonical = fs::canonicalize(path).into_diagnostic()?;
            let source = reuse.map(fs::canonicalize).transpose().into_diagnostic()?;
            if source.as_ref() != Some(&canonical) && !force {
                return Err(miette!(
                    "artifact destination already exists: {}",
                    path.display()
                ));
            }
            Ok((canonical, true))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((path.to_path_buf(), false))
        }
        Err(error) => Err(error).into_diagnostic(),
    }
}

pub(crate) struct ArtifactSession {
    source: RefCell<Option<RustiQBundle>>,
    snapshots: RefCell<Vec<RustiQData>>,
    destination: Option<PathBuf>,
    replace: bool,
    active: bool,
}

impl ArtifactSession {
    pub(crate) fn open(
        reuse: Option<&Path>,
        artifact: Option<&Path>,
        force: bool,
    ) -> miette::Result<Self> {
        let source = reuse
            .map(RustiQBundle::open)
            .transpose()
            .into_diagnostic()?;
        let (destination, replace) = if let Some(path) = artifact {
            let (path, replace) = resolve_artifact_destination(reuse, path, force)?;
            (Some(path), replace)
        } else {
            (None, false)
        };
        Ok(Self {
            source: RefCell::new(source),
            snapshots: RefCell::new(Vec::new()),
            destination,
            replace,
            active: reuse.is_some() || artifact.is_some(),
        })
    }

    pub(crate) fn prepare(
        &self,
        prepared: PreparedCalculation,
    ) -> miette::Result<(PreparedCalculation, Option<ArtifactReport>)> {
        if !self.active {
            return Ok((prepared, None));
        }
        let mut data = RustiQData::from_calculation(&prepared).into_diagnostic()?;
        let mut report = ArtifactReport {
            name: "ao_eri",
            decision: "missing",
            origin: "computation",
            source_index: None,
        };
        if let Some(bundle) = self.source.borrow().as_ref() {
            for (index, candidate) in bundle.calculations().iter().enumerate() {
                if !candidate
                    .artifact_representations()
                    .any(|(name, _)| name == AoEriArtifact::NAME)
                {
                    continue;
                }
                report.decision = "incompatible";
                if !candidate.eri_is_compatible(&prepared) {
                    continue;
                }
                // Share the source handle; execution verifies and decodes the selected payload.
                data = candidate.clone();
                report.decision = "reused";
                report.source_index = Some(index);
                report.origin = "archive";
                break;
            }
        }
        let prepared = prepared.with_reuse_data(data).into_diagnostic()?;
        Ok((prepared, Some(report)))
    }

    pub(crate) fn retain(
        &self,
        prepared: &PreparedCalculation,
        result: &CalculationResult,
        sources: [(&str, &[u8]); 2],
    ) -> miette::Result<()> {
        if self.destination.is_none() {
            return Ok(());
        }
        let mut data = RustiQData::from_calculation(prepared).into_diagnostic()?;
        data.set::<AoEriArtifact>(result.hf.ao_eri())
            .into_diagnostic()?;
        if let Some(solution) = result.hf.as_converged() {
            let hf = ConvergedHf::from_solution(solution).into_diagnostic()?;
            data.set::<ConvergedHfArtifact>(&hf).into_diagnostic()?;
        }
        for (name, bytes) in sources {
            data.add_source(name, bytes).into_diagnostic()?;
        }
        self.snapshots.borrow_mut().push(data);
        Ok(())
    }

    pub(crate) fn publish(&self) -> miette::Result<()> {
        let Some(path) = &self.destination else {
            return Ok(());
        };
        // Close the original archive before replacement, including on Windows.
        self.source.borrow_mut().take();
        let mut bundle = RustiQBundle::new(std::mem::take(&mut *self.snapshots.borrow_mut()))
            .into_diagnostic()?;
        if self.replace {
            bundle.replace(path).into_diagnostic()?;
        } else {
            bundle.write(path).into_diagnostic()?;
        }
        Ok(())
    }
}
