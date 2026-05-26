mod info_command;
mod translation;
use clap::Subcommand;
use delegate::delegate;
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
    delegate! {
        to match self {
            GeometryCommands::Info(command) => command,
            GeometryCommands::Rotate(command) => command,
            GeometryCommands::Translate(command) => command,
            GeometryCommands::Center(command) => command,
            GeometryCommands::Isometry(command) => command,
        } {
            fn run(&self) -> CommandResult;
        }
    }
}
