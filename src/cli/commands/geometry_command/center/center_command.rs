use clap::Args;

use crate::cli::commands::{
    geometry_command::{center::CenterType, TransformArgs},
    Runnable,
};

#[derive(Args, Debug, Clone)]
pub struct CenterCommand {
    /// Center to move to the origin.
    #[arg(long = "center")]
    pub center_type: CenterType,
    #[clap(flatten)]
    pub transform_args: TransformArgs,
}

impl Runnable for CenterCommand {
    fn run(&self) -> crate::cli::commands::CommandResult {
        self.transform_args.apply_transform(|geometry| {
            match self.center_type {
                CenterType::Geometry => geometry.centering(),
                CenterType::Mass => geometry.mass_centering()?,
                CenterType::Charge => geometry.charge_centering(),
            };
            Ok(())
        })?;
        Ok(())
    }
}
