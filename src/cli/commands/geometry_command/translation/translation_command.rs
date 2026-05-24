use clap::Args;
use nalgebra::Translation3;

use crate::cli::commands::{
    geometry_command::{translation::TranslationArgs, TransformArgs},
    Runnable,
};
#[derive(Debug, Args)]
pub struct TranslationCommand {
    #[command(flatten)]
    pub translation: TranslationArgs,
    #[command(flatten)]
    pub transform_args: TransformArgs,
}

impl Runnable for TranslationCommand {
    fn run(&self) -> crate::cli::commands::CommandResult {
        let translation = Translation3::<f64>::from(self.translation);
        self.transform_args.apply_transform(|geometry| {
            geometry.translate(translation);
            Ok(())
        })?;
        Ok(())
    }
}
