use crate::cli::commands::{
    geometry_command::{rotation::rotation_args::RotationArgs, TransformArgs},
    Runnable,
};
use clap::Args;
use nalgebra::Rotation3;
#[derive(Args, Debug, Clone)]
pub struct RotationCommand {
    #[clap(flatten)]
    pub rotation: RotationArgs,
    #[clap(flatten)]
    pub transform_args: TransformArgs,
}

impl Runnable for RotationCommand {
    fn run(&self) -> crate::cli::commands::CommandResult {
        let rotation = Rotation3::<f64>::from(self.rotation);
        self.transform_args.apply_transform(|geometry| {
            geometry.rotate(rotation);
            Ok(())
        })?;
        Ok(())
    }
}
