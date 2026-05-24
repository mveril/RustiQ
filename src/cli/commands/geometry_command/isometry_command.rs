use crate::cli::commands::{
    geometry_command::{
        rotation::rotation_args::RotationArgs, translation::TranslationArgs, TransformArgs,
    },
    Runnable,
};
use clap::Args;
use nalgebra::{Isometry3, Rotation3, Translation3};

#[derive(Args, Debug, Clone)]
pub struct IsometryCommand {
    #[clap(flatten)]
    pub rotation_args: RotationArgs,
    #[clap(flatten)]
    pub translation_args: TranslationArgs,
    #[clap(flatten)]
    pub transform_args: TransformArgs,
}

impl Runnable for IsometryCommand {
    fn run(&self) -> crate::cli::commands::CommandResult {
        self.transform_args.apply_transform(|geometry| {
            let rotation: Rotation3<f64> = self.rotation_args.into();
            let translation: Translation3<f64> = self.translation_args.into();
            let transformation: Isometry3<f64> =
                Isometry3::from_parts(translation, rotation.into());
            geometry.transform(&transformation);
            Ok(())
        })?;
        Ok(())
    }
}
