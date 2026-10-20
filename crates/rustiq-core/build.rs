use std::{
    env,
    fs::File,
    io::{self, BufWriter, Write},
    path::PathBuf,
};

use mendeleev::Element;

const SCIENTIFIC_NOUNS: &[&str] = &[
    "accelerator",
    "amplitude",
    "anion",
    "asteroid",
    "atom",
    "baryon",
    "beam",
    "boson",
    "catalyst",
    "cation",
    "comet",
    "condensate",
    "cosmos",
    "crystal",
    "detector",
    "dipole",
    "electron",
    "fermion",
    "field",
    "flux",
    "galaxy",
    "hadron",
    "ion",
    "isotope",
    "lattice",
    "lepton",
    "magneton",
    "molecule",
    "nebula",
    "neutrino",
    "neutron",
    "nucleus",
    "orbital",
    "particle",
    "phonon",
    "photon",
    "plasma",
    "planet",
    "pluto",
    "positron",
    "proton",
    "pulsar",
    "quanta",
    "quark",
    "reagent",
    "resonance",
    "spectrum",
    "spin",
    "vector",
    "wave",
];

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let mut output = BufWriter::new(File::create(output_dir.join("cache_nouns.rs"))?);
    writeln!(output, "const NOUNS: &[&str] = &[")?;
    for noun in SCIENTIFIC_NOUNS {
        writeln!(output, "    {noun:?},")?;
    }
    for element in Element::list() {
        let noun = element.name().to_ascii_lowercase();
        writeln!(output, "    {noun:?},")?;
    }
    writeln!(output, "];")?;
    output.flush()
}
