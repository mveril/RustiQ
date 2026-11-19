use std::{
    fs::OpenOptions,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

use clap::{Args, ValueEnum};
use mendeleev::Element;
use miette::{miette, IntoDiagnostic, WrapErr};
use rustiq_core::persistence::{
    CalculationContext, ConvergedHf, ConvergedHfArtifact, HfOrbitalChannel, HfOrbitals, ResolvedAo,
    RustiQBundle,
};

use crate::cli::commands::{CommandResult, Runnable};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExportFormat {
    Molden,
}

#[derive(Debug, Args)]
pub(crate) struct ExportCommand {
    /// Portable .rustiq bundle.
    path: PathBuf,
    /// Select the export format.
    #[arg(long, value_enum)]
    format: ExportFormat,
    /// Zero-based calculation index. Required for bundles with multiple calculations.
    #[arg(long)]
    calculation: Option<usize>,
    /// File to write. Writes to standard output when omitted.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

impl Runnable for ExportCommand {
    fn run(&self) -> CommandResult {
        let bundle = RustiQBundle::open_for_restore(&self.path).into_diagnostic()?;
        let calculations = bundle.calculations();
        let index = match self.calculation {
            Some(index) => index,
            None if calculations.len() == 1 => 0,
            None => {
                return Err(miette!(
                    "This bundle contains {} calculations. Use --calculation with a zero-based index.",
                    calculations.len()
                ));
            }
        };
        let data = calculations.get(index).ok_or_else(|| {
            miette!(
                "Calculation index {index} is out of range. Use a zero-based index from 0 to {}.",
                calculations.len() - 1
            )
        })?;
        let context = data
            .calculation()
            .ok_or_else(|| miette!("Calculation {index} has no resolved context."))?;
        let hf = data
            .get::<ConvergedHfArtifact>()
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("could not read the converged HF artifact for calculation {index}")
            })?
            .ok_or_else(|| {
                miette!(
                    "Calculation {index} has no saved converged HF orbitals. Save a converged HF calculation with rustiq run --artifact to export its orbitals."
                )
            })?;

        match self.format {
            ExportFormat::Molden => {
                let basis = MoldenBasis::from_context(context)
                    .wrap_err_with(|| format!("could not export calculation {index} to Molden"))?;
                if let Some(path) = &self.output {
                    let file = open_export_file(&self.path, path)?;
                    basis
                        .write(&hf, file.as_file())
                        .into_diagnostic()
                        .wrap_err_with(|| format!("could not write Molden file {}", path.display()))
                } else {
                    basis.write(&hf, io::stdout().lock()).into_diagnostic()
                }
            }
        }
    }
}

fn open_export_file(input: &Path, output: &Path) -> miette::Result<same_file::Handle> {
    let input_file = same_file::Handle::from_path(input).into_diagnostic()?;
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(output)
        .and_then(same_file::Handle::from_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("could not open Molden output {}", output.display()))?;
    // Compare the open handles before truncating, including symlinks and hard links.
    if file == input_file {
        return Err(miette!("Molden output must not replace the input bundle."));
    }
    file.as_file()
        .set_len(0)
        .into_diagnostic()
        .wrap_err_with(|| format!("could not truncate Molden output {}", output.display()))?;
    Ok(file)
}

struct MoldenBasis {
    atoms: Vec<(u32, [f64; 3])>,
    shells: Vec<Vec<Shell>>,
    ao_order: Vec<usize>,
    spherical_d: bool,
}

struct Shell {
    label: char,
    primitives: Vec<(f64, f64)>,
}

impl MoldenBasis {
    fn from_context(context: &CalculationContext) -> miette::Result<Self> {
        let atoms: Vec<_> = context.atoms().collect();
        let mut aos = context.basis().enumerate().peekable();
        let mut shells = Vec::with_capacity(atoms.len());
        let mut ao_order = Vec::with_capacity(aos.len());
        let mut spherical_d = None;
        for (index, &(_, position)) in atoms.iter().enumerate() {
            let mut atom_shells = Vec::new();
            while let Some(&(offset, ao)) = aos.peek() {
                if !same_center(ao.center(), position) {
                    break;
                }
                let angular = ao
                    .components()
                    .next()
                    .ok_or_else(|| miette!("AO {} has no Cartesian components.", offset + 1))?
                    .angular_momentum();
                let l: u32 = angular.into_iter().map(u32::from).sum();
                let cartesian = cartesian_components(l).ok_or_else(|| {
                    miette!("Molden export supports angular momentum only through g.")
                })?;
                // In a real spherical d shell, RustiQ's first AO is xy (m = -2).
                // In a Cartesian d shell, the first AO is zz.
                let spherical = l == 2 && angular == [1, 1, 0];
                let count = if spherical { 5 } else { cartesian.len() };
                let rows = aos.by_ref().take(count).collect::<Vec<_>>();
                let shell =
                    Shell::from_rows(&rows, position, l, spherical, cartesian, &mut ao_order)
                        .ok_or_else(|| {
                            miette!(
                                "AOs starting at row {} do not form a Molden shell.",
                                offset + 1
                            )
                        })?;
                if l >= 2 && *spherical_d.get_or_insert(spherical) != spherical {
                    return Err(miette!("Molden export requires a consistent Cartesian or spherical representation for d and higher shells."));
                }
                atom_shells.push(shell);
            }
            if atom_shells.is_empty() {
                return Err(miette!(
                    "Could not associate basis shells with atom {}.",
                    index + 1
                ));
            }
            shells.push(atom_shells);
        }
        if aos.next().is_some() {
            return Err(miette!(
                "The stored basis contains AOs outside the ordered atomic shells."
            ));
        }
        Ok(Self {
            atoms,
            shells,
            ao_order,
            spherical_d: spherical_d.unwrap_or(false),
        })
    }

    fn write(&self, hf: &ConvergedHf, output: impl Write) -> io::Result<()> {
        let mut output = BufWriter::new(output);
        writeln!(output, "[Molden Format]\n[Atoms] AU")?;
        for (index, &(number, [x, y, z])) in self.atoms.iter().enumerate() {
            let element = Element::list()
                .get(number.saturating_sub(1) as usize)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid atomic number")
                })?;
            writeln!(
                output,
                "{} {} {number} {x:.16e} {y:.16e} {z:.16e}",
                element.symbol(),
                index + 1
            )?;
        }

        writeln!(output, "[GTO]")?;
        for (index, atom_shells) in self.shells.iter().enumerate() {
            writeln!(output, "{} 0", index + 1)?;
            for shell in atom_shells {
                writeln!(output, " {} {} 1.00", shell.label, shell.primitives.len())?;
                for &(exponent, coefficient) in &shell.primitives {
                    writeln!(output, "  {exponent:.16e} {coefficient:.16e}")?;
                }
            }
            writeln!(output)?;
        }
        if self.spherical_d {
            writeln!(output, "[5D]")?;
        }
        writeln!(output, "[MO]")?;
        match hf.orbitals() {
            HfOrbitals::Rhf(rhf) => self.write_channel(&rhf.shared, "Alpha", 2, &mut output)?,
            HfOrbitals::Uhf(uhf) => {
                self.write_channel(uhf.channels.alpha(), "Alpha", 1, &mut output)?;
                self.write_channel(uhf.channels.beta(), "Beta", 1, &mut output)?;
            }
        }
        output.flush()
    }

    fn write_channel(
        &self,
        channel: &HfOrbitalChannel,
        spin: &str,
        occupied_electrons: u8,
        mut output: impl Write,
    ) -> io::Result<()> {
        let columns = channel.coefficients().chunks_exact(self.ao_order.len());
        for (index, (&energy, coefficients)) in channel.energies().iter().zip(columns).enumerate() {
            let occupation = if (index as u64) < channel.occupied() {
                occupied_electrons
            } else {
                0
            };
            writeln!(
                output,
                " Sym= A\n Ene= {energy:.16e}\n Spin= {spin}\n Occup= {occupation}"
            )?;
            for (row, &source) in self.ao_order.iter().enumerate() {
                writeln!(output, " {} {:.16e}", row + 1, coefficients[source])?;
            }
        }
        Ok(())
    }
}

type Angular = [u8; 3];
type Component = (Angular, f64);

impl Shell {
    fn from_rows(
        rows: &[(usize, ResolvedAo<'_>)],
        position: [f64; 3],
        l: u32,
        spherical: bool,
        cartesian: &[Angular],
        ao_order: &mut Vec<usize>,
    ) -> Option<Self> {
        // Molden's real d functions are ordered m = 0, +1, -1, +2, -2.
        // Factors here multiply individually normalized Cartesian primitives.
        let root = 3.0_f64.sqrt() / 2.0;
        let spherical_d: &[&[Component]] = &[
            &[([2, 0, 0], -0.5), ([0, 2, 0], -0.5), ([0, 0, 2], 1.0)],
            &[([1, 0, 1], 1.0)],
            &[([0, 1, 1], 1.0)],
            &[([2, 0, 0], root), ([0, 2, 0], -root)],
            &[([1, 1, 0], 1.0)],
        ];
        let cartesian: Vec<[Component; 1]> = cartesian.iter().map(|&ang| [(ang, 1.0)]).collect();
        let patterns: Vec<&[Component]> = if spherical {
            spherical_d.to_vec()
        } else {
            cartesian.iter().map(<[Component; 1]>::as_slice).collect()
        };
        if rows.len() != patterns.len()
            || rows
                .iter()
                .any(|(_, ao)| !same_center(ao.center(), position))
        {
            return None;
        }

        let mut primitives = None;
        for pattern in patterns {
            let (row, ao) = rows.iter().find(|(_, ao)| {
                ao.components().len() == pattern.len()
                    && ao
                        .components()
                        .zip(pattern)
                        .all(|(component, &(angular, _))| component.angular_momentum() == angular)
            })?;
            // The first Molden AO contains a component with unit factor.
            let radial = match &mut primitives {
                Some(radial) => radial,
                None => primitives.insert(
                    ao.components()
                        .last()?
                        .normalized_primitives()
                        .collect::<Vec<_>>(),
                ),
            };
            if !ao
                .components()
                .zip(pattern)
                .all(|(component, &(_, factor))| {
                    let actual = component.normalized_primitives();
                    actual.len() == radial.len()
                        && actual.zip(radial.iter()).all(
                            |((exponent, coefficient), &(expected_exponent, expected))| {
                                let expected = expected * factor;
                                exponent.to_bits() == expected_exponent.to_bits()
                                    && coefficient.is_finite()
                                    && (coefficient - expected).abs()
                                        <= 1e-12 * coefficient.abs().max(expected.abs())
                            },
                        )
                })
            {
                return None;
            }
            ao_order.push(*row);
        }
        Some(Self {
            label: *['s', 'p', 'd', 'f', 'g'].get(l as usize)?,
            primitives: primitives?,
        })
    }
}

fn same_center(a: [f64; 3], b: [f64; 3]) -> bool {
    // Positions are copied from the atoms when the basis is constructed.
    a.map(f64::to_bits) == b.map(f64::to_bits)
}

fn cartesian_components(l: u32) -> Option<&'static [Angular]> {
    // Molden coefficient order for normalized Cartesian primitives.
    Some(match l {
        0 => &[[0, 0, 0]],
        1 => &[[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        2 => &[
            [2, 0, 0],
            [0, 2, 0],
            [0, 0, 2],
            [1, 1, 0],
            [1, 0, 1],
            [0, 1, 1],
        ],
        3 => &[
            [3, 0, 0],
            [0, 3, 0],
            [0, 0, 3],
            [1, 2, 0],
            [2, 1, 0],
            [2, 0, 1],
            [1, 0, 2],
            [0, 1, 2],
            [0, 2, 1],
            [1, 1, 1],
        ],
        4 => &[
            [4, 0, 0],
            [0, 4, 0],
            [0, 0, 4],
            [3, 1, 0],
            [3, 0, 1],
            [1, 3, 0],
            [0, 3, 1],
            [1, 0, 3],
            [0, 1, 3],
            [2, 2, 0],
            [2, 0, 2],
            [0, 2, 2],
            [2, 1, 1],
            [1, 2, 1],
            [1, 1, 2],
        ],
        _ => return None,
    })
}
