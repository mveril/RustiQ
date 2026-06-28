use clap::Args;
use miette::IntoDiagnostic;

use crate::cli::commands::{geometry_command::TransformArgs, Runnable};

#[derive(Args, Debug, Clone)]
pub struct OrienteCommand {
    #[clap(flatten)]
    pub transform_args: TransformArgs,
}

impl Runnable for OrienteCommand {
    fn run(&self) -> crate::cli::commands::CommandResult {
        self.transform_args.apply_transform(|geometry| {
            geometry.orient_along_principal_axes().into_diagnostic()?;
            Ok(())
        })?;
        Ok(())
    }
}
