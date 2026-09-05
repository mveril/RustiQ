use std::{
    fs,
    io::Write,
    num::NonZeroU8,
    path::{Path, PathBuf},
};

use clap::{Args, ValueEnum};
use miette::{miette, IntoDiagnostic, WrapErr};

use super::{CommandResult, Runnable};
use crate::{
    molecules::{geometry::Geometry, molecule::Molecule, units::Units},
    runfile::{
        global::{molecule_config::MoleculeConfig, Global},
        hf::{HfConfig, HfMethod},
        mp2::Mp2Config,
        output::Defaults,
        RunFile,
    },
};

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum Method {
    #[default]
    Auto,
    Rhf,
    Uhf,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum GeometryUnits {
    #[default]
    Angstrom,
    Bohr,
}

#[derive(Debug, Args)]
pub struct InitCommand {
    /// XYZ geometry file to reference in the calculation
    input: PathBuf,
    /// Calculation file to create (parent directory must exist)
    #[arg(short, long, default_value = "calculation.toml")]
    output: PathBuf,
    /// Replace an existing calculation file
    #[arg(short, long)]
    force: bool,
    /// Basis set name (availability is checked by run)
    #[arg(long, default_value = "sto-3g")]
    basis: String,
    /// Molecular charge
    #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
    charge: i32,
    /// Spin multiplicity; defaults to singlet for even electron counts, doublet for odd
    #[arg(long)]
    multiplicity: Option<NonZeroU8>,
    /// Units of the XYZ coordinates
    #[arg(long, value_enum, default_value = "angstrom")]
    units: GeometryUnits,
    /// Hartree-Fock method
    #[arg(long, value_enum, default_value = "auto")]
    hf: Method,
    /// Include an MP2 calculation with no frozen orbitals
    #[arg(long)]
    mp2: bool,
}

// Both paths are canonical absolute paths. Different filesystem roots cannot
// be represented by a relative path (for example different Windows drives).
fn relative_geometry(input: &Path, directory: &Path) -> PathBuf {
    let source: Vec<_> = input.components().collect();
    let base: Vec<_> = directory.components().collect();
    let shared = source.iter().zip(&base).take_while(|(a, b)| a == b).count();
    if shared == 0 {
        return input.to_path_buf();
    }
    let mut result = PathBuf::new();
    for _ in &base[shared..] {
        result.push("..");
    }
    for component in &source[shared..] {
        result.push(component.as_os_str());
    }
    result
}

impl Runnable for InitCommand {
    fn run(&self) -> CommandResult {
        if self.basis.trim().is_empty() {
            return Err(miette!("Basis name must not be empty"));
        }
        let input = self
            .input
            .canonicalize()
            .into_diagnostic()
            .wrap_err_with(|| format!("Cannot read geometry {}", self.input.display()))?;
        let geometry = Geometry::from_path(&input).into_diagnostic()?;
        let nuclear_charge: i64 = geometry
            .atoms
            .iter()
            .map(|atom| i64::from(atom.element.atomic_number))
            .sum();
        let electrons = nuclear_charge - i64::from(self.charge);
        if electrons <= 0 || electrons > i64::from(i32::MAX) || nuclear_charge > i64::from(i32::MAX)
        {
            return Err(miette!("Invalid molecular electron count: {electrons}"));
        }
        let multiplicity = self
            .multiplicity
            .unwrap_or_else(|| NonZeroU8::new(if electrons % 2 == 0 { 1 } else { 2 }).unwrap());
        let units = match self.units {
            GeometryUnits::Angstrom => Units::Angstrom,
            GeometryUnits::Bohr => Units::Bohr,
        };
        let molecule =
            Molecule::try_new(geometry, units, self.charge, multiplicity).into_diagnostic()?;
        let method = match self.hf {
            Method::Auto => HfMethod::Auto,
            Method::Rhf => HfMethod::Rhf,
            Method::Uhf => HfMethod::Uhf,
        };
        let resolved = method.resolve(&molecule).into_diagnostic()?;
        let parent = self
            .output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let directory = parent
            .canonicalize()
            .into_diagnostic()
            .wrap_err("Output directory must exist")?;
        let filename = self
            .output
            .file_name()
            .ok_or_else(|| miette!("Output must name a file"))?;
        let output = directory.join(filename);
        match fs::symlink_metadata(&output) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(miette!(
                        "Output must be a regular file, not a directory or symbolic link"
                    ));
                }
                if output.canonicalize().into_diagnostic()? == input {
                    return Err(miette!("Output must not replace the source geometry"));
                }
                if !self.force {
                    return Err(miette!(
                        "{} already exists; use --force to replace it",
                        output.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).into_diagnostic(),
        }
        let run = RunFile {
            global: Global {
                basis: self.basis.clone(),
                molecule: MoleculeConfig {
                    geometry: relative_geometry(&input, &directory),
                    charge: self.charge,
                    multiplicity,
                    molecule_unit: units,
                },
            },
            hf: Some(HfConfig {
                method,
                ..HfConfig::default()
            }),
            mp2: self.mp2.then(Mp2Config::default),
        };
        let content = toml_spanner::to_string(&run.output(Defaults::Omit)).into_diagnostic()?;
        let mut temporary = tempfile::NamedTempFile::new_in(&directory).into_diagnostic()?;
        temporary.write_all(content.as_bytes()).into_diagnostic()?;
        temporary.flush().into_diagnostic()?;
        if self.force {
            temporary.persist(&output).into_diagnostic()?;
        } else {
            temporary
                .persist_noclobber(&output)
                .into_diagnostic()
                .wrap_err("Cannot create calculation file; use --force if it already exists")?;
        }
        println!(
            "Created {} (multiplicity {}, HF {:?} → {}{})",
            self.output.display(),
            multiplicity,
            self.hf,
            resolved,
            if self.mp2 { ", MP2" } else { "" }
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::{commands::Commands, Cli},
        runfile::parser::parse_runfile,
    };
    use clap::Parser;

    fn command(input: &Path, output: &Path, extra: &[&str]) -> InitCommand {
        let mut args = vec![
            std::ffi::OsString::from("rustiq"),
            "init".into(),
            input.into(),
            "-o".into(),
            output.into(),
        ];
        args.extend(extra.iter().map(std::ffi::OsString::from));
        let Commands::Init(command) = Cli::try_parse_from(args).unwrap().command else {
            panic!("expected init")
        };
        command
    }

    #[test]
    fn test_init_runfile_restores_defaults_for_run_display() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("h2.xyz");
        let output = temp.path().join("calculation.toml");
        fs::write(&input, "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();

        command(&input, &output, &["--mp2"]).run().unwrap();
        let content = fs::read_to_string(&output).unwrap();
        // Use the same parser and formatted TOML that RunCommand passes to bat.
        let parsed = parse_runfile("calculation.toml", &content).unwrap();
        for field in [
            "charge",
            "multiplicity",
            "molecule_unit",
            "method",
            "max_iterations",
            "convergence_threshold",
            "frozen_orbitals",
        ] {
            assert!(!content.contains(field), "init must omit {field}");
            assert!(
                parsed.formatted_toml.contains(field),
                "run display must include {field}"
            );
        }
        let molecule = &parsed.runfile.global.molecule;
        assert_eq!(temp.path().join(&molecule.geometry), input);
        assert_eq!(molecule.charge, 0);
        assert_eq!(molecule.multiplicity.get(), 1);
        assert_eq!(molecule.molecule_unit, Units::Angstrom);
        let hf = parsed.runfile.hf.as_ref().unwrap();
        assert_eq!(hf.method, HfMethod::Auto);
        assert_eq!(hf.max_iterations.get(), 100);
        assert_eq!(hf.convergence_threshold.into_inner(), 1e-8);
        assert_eq!(parsed.runfile.mp2.as_ref().unwrap().frozen_orbitals, 0);
        assert_eq!(fs::read_to_string(&output).unwrap(), content);
    }

    #[test]
    fn generated_defaults_are_minimal_even_when_explicit() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("molecule.xyz");
        let output = temp.path().join("calculation.toml");
        fs::write(&input, "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();
        for extra in [
            vec![],
            vec![
                "--force",
                "--hf",
                "auto",
                "--charge",
                "0",
                "--multiplicity",
                "1",
                "--units",
                "angstrom",
            ],
        ] {
            command(&input, &output, &extra).run().unwrap();
            let content = fs::read_to_string(&output).unwrap();
            let lines: Vec<_> = content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect();
            assert_eq!(
                lines,
                [
                    "[global]",
                    "basis = \"sto-3g\"",
                    "[global.molecule]",
                    "geometry = \"molecule.xyz\"",
                    "[hf]"
                ]
            );
        }
        command(&input, &output, &["--force", "--mp2"])
            .run()
            .unwrap();
        let content = fs::read_to_string(&output).unwrap();
        assert!(content.contains("[mp2]"));
        assert!(!content.contains("frozen_orbitals"));
        let run = parse_runfile("generated", &content).unwrap().runfile;
        assert!(run.hf.is_some());
        assert_eq!(run.mp2.unwrap().frozen_orbitals, 0);
    }

    #[test]
    fn generated_files_roundtrip_options_and_relative_paths() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("molecule.xyz");
        fs::write(&input, "2\nH2\nH 0 0 0\nH 0 0 0.74\n").unwrap();
        let directory = temp.path().join("calculations");
        fs::create_dir(&directory).unwrap();
        let output = directory.join("calculation.toml");
        for (extra, expected_method, charge, multiplicity, mp2) in [
            (vec![], HfMethod::Auto, 0, 1, false),
            (
                vec!["--hf", "rhf", "--mp2", "-f"],
                HfMethod::Rhf,
                0,
                1,
                true,
            ),
            (
                vec![
                    "--hf", "uhf", "--charge", "-1", "--units", "bohr", "--mp2", "--force",
                ],
                HfMethod::Uhf,
                -1,
                2,
                true,
            ),
            (
                vec!["--multiplicity", "3", "--force"],
                HfMethod::Auto,
                0,
                3,
                false,
            ),
        ] {
            command(&input, &output, &extra).run().unwrap();
            let content = fs::read_to_string(&output).unwrap();
            let run = parse_runfile("generated", &content).unwrap().runfile;
            assert_eq!(run.global.basis, "sto-3g");
            assert_eq!(run.global.molecule.geometry, Path::new("../molecule.xyz"));
            assert_eq!(run.global.molecule.charge, charge);
            assert_eq!(run.global.molecule.multiplicity.get(), multiplicity);
            assert_eq!(
                run.global.molecule.molecule_unit,
                if charge == -1 {
                    Units::Bohr
                } else {
                    Units::Angstrom
                }
            );
            assert_eq!(run.hf.unwrap().method, expected_method);
            assert_eq!(run.mp2.is_some(), mp2);
            if let Some(config) = run.mp2 {
                assert_eq!(config.frozen_orbitals, 0);
            }
        }
        let special_basis = "a \"quoted\" basis\\name";
        command(&input, &output, &["--force", "--basis", special_basis])
            .run()
            .unwrap();
        let parsed = parse_runfile("generated", &fs::read_to_string(output).unwrap()).unwrap();
        assert_eq!(parsed.runfile.global.basis, special_basis);
    }

    #[test]
    fn errors_preserve_existing_output_and_geometry() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("molecule.xyz");
        let xyz = "1\nH\nH 0 0 0\n";
        fs::write(&input, xyz).unwrap();
        let output = temp.path().join("calculation.toml");
        fs::write(&output, "original").unwrap();
        for extra in [
            vec![],
            vec!["--force", "--hf", "rhf"],
            vec!["--force", "--multiplicity", "1"],
            vec!["--force", "--charge", "1"],
            vec!["--force", "--charge", "-2147483648"],
            vec!["--force", "--basis", " "],
        ] {
            assert!(command(&input, &output, &extra).run().is_err());
            assert_eq!(fs::read_to_string(&output).unwrap(), "original");
        }
        assert!(command(&input, &input, &["--force"]).run().is_err());
        assert_eq!(fs::read_to_string(&input).unwrap(), xyz);
        assert!(
            command(&input, &temp.path().join("missing/calculation.toml"), &[])
                .run()
                .is_err()
        );
        assert!(command(&input, temp.path(), &["--force"]).run().is_err());
        let missing = temp.path().join("missing.xyz");
        assert!(command(&missing, &output, &["--force"]).run().is_err());
        fs::write(&input, "invalid XYZ").unwrap();
        assert!(command(&input, &output, &["--force"]).run().is_err());
        assert_eq!(fs::read_to_string(output).unwrap(), "original");
    }

    #[test]
    fn clap_defaults_and_invalid_values() {
        let Commands::Init(command) = Cli::try_parse_from(["rustiq", "init", "molecule.xyz"])
            .unwrap()
            .command
        else {
            panic!("expected init")
        };
        assert_eq!(command.output, Path::new("calculation.toml"));
        assert!(!command.force);
        for args in [
            vec!["rustiq", "init"],
            vec!["rustiq", "init", "x", "--multiplicity", "0"],
            vec!["rustiq", "init", "x", "--multiplicity", "256"],
            vec!["rustiq", "init", "x", "--hf", "invalid"],
            vec!["rustiq", "init", "x", "--units", "invalid"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn escaped_geometry_paths_and_symlink_output() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("mol \"quoted\"\\name.xyz");
        fs::write(&input, "1\nH\nH 0 0 0\n").unwrap();
        let output = temp.path().join("calculation.toml");
        command(&input, &output, &[]).run().unwrap();
        let run = parse_runfile("generated", &fs::read_to_string(&output).unwrap())
            .unwrap()
            .runfile;
        assert_eq!(temp.path().join(run.global.molecule.geometry), input);
        let link = temp.path().join("link.toml");
        std::os::unix::fs::symlink(&output, &link).unwrap();
        assert!(command(&input, &link, &["--force"]).run().is_err());
    }
}
