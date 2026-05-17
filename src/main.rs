use basis::basis_store::BasisStore;
use basis::basisfile::BasisFile;
use basis::gaussian::basis::Basis;
use clap::{arg, command, value_parser, Arg, Command};
use cli::ux::BasisTableItem;
use cli::Cli;
use indicatif::{ProgressBar, ProgressStyle};
use std::cell::OnceCell;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::{env, io};
use tabled::Table;
use tokio::runtime::Runtime as TokioRuntime;
mod basis;
mod runfile;
use runfile::RunFile;
mod cli;
mod eri;
mod hf;
mod math_utils;
mod molecules;
use clap::Parser;
use cli::commands::Runable;
use molecules::{geometry::Geometry, molecule::Molecule};
use rayon::prelude::*;
#[cfg(test)]
pub(crate) mod test_utils;

fn main() {
    let app: Cli = Cli::parse();
    app.command.run()
}
