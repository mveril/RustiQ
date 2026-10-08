//! Portable orchestration; scientific compatibility stays in rustiq-core.
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
};

use miette::{miette, IntoDiagnostic};
use rustiq_core::{
    calculation::{
        ArtifactReuseDecision, CalculationEvent, CalculationResult, EriCacheAction,
        PreparedCalculation,
    },
    persistence::{AoEriArtifact, RustiQBundle, RustiQData},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
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
            };
        }
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
    pub(crate) fn open(reuse: Option<&Path>, artifact: Option<&Path>) -> miette::Result<Self> {
        let source = reuse
            .map(RustiQBundle::open)
            .transpose()
            .into_diagnostic()?;
        let source_path = reuse.map(fs::canonicalize).transpose().into_diagnostic()?;
        let mut destination = artifact.map(Path::to_path_buf);
        let mut replace = false;
        if let Some(path) = artifact {
            match fs::symlink_metadata(path) {
                Ok(_) => {
                    let canonical = fs::canonicalize(path).into_diagnostic()?;
                    if source_path.as_ref() != Some(&canonical) {
                        return Err(miette!(
                            "artifact destination already exists: {}",
                            path.display()
                        ));
                    }
                    destination = Some(canonical);
                    replace = true;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).into_diagnostic(),
            }
        }
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
        if let Some(bundle) = self.source.borrow_mut().as_mut() {
            let selection = bundle.select_ao_eri(&prepared, true).into_diagnostic()?;
            report.decision = match selection.decision {
                ArtifactReuseDecision::Reused => "reused",
                ArtifactReuseDecision::Missing => "missing",
                ArtifactReuseDecision::Incompatible => "incompatible",
                ArtifactReuseDecision::Ignored => "ignored",
                _ => return Err(miette!("unsupported artifact reuse decision")),
            };
            report.source_index = selection.source_index;
            if let Some(eri) = selection.value {
                data.set::<AoEriArtifact>(eri).into_diagnostic()?;
                report.origin = "archive";
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
        data.set::<AoEriArtifact>(result.hf.ao_eri().clone())
            .into_diagnostic()?;
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
