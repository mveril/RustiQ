mod info_command;
mod translation;
use clap::Subcommand;
mod center;
mod transform_args;
use info_command::InfoCommand;
pub(super) use transform_args::TransformArgs;
mod isometry_command;
mod rotation;

use super::{CommandResult, Runnable};

#[derive(Subcommand, Debug)]
pub enum GeometryCommands {
    /// Print basic information about a molecular geometry
    Info(InfoCommand),
    /// Rotate a geometry around an axis by an angle in degrees
    Rotate(rotation::RotationCommand),
    /// Translate a geometry by a Cartesian vector
    Translate(translation::TranslationCommand),
    /// Move a selected center of the geometry to the origin
    Center(center::CenterCommand),
    /// Apply a rotation and translation in one pass
    Isometry(isometry_command::IsometryCommand),
}

impl Runnable for GeometryCommands {
    fn run(&self) -> CommandResult {
        match self {
            GeometryCommands::Info(cmd) => cmd.run(),
            GeometryCommands::Rotate(rotation_command) => rotation_command.run(),
            GeometryCommands::Translate(translation_command) => translation_command.run(),
            GeometryCommands::Center(center_command) => center_command.run(),
            GeometryCommands::Isometry(isometry_command) => isometry_command.run(),
        }
    }
}
