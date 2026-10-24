# Geometry files

Examples in this chapter that use `samples/` or `tests/data/` require a cloned
repository and should be run from its root. Release binaries do not include
those files; the [first-calculation guide](../getting-started/first-calculation.md)
shows how to create local inputs and obtain a basis.

RustiQ reads molecular coordinates from XYZ files. Each file starts with the
number of atoms, then a comment line, then one line per atom containing an
element symbol or atomic number and three Cartesian coordinates. See the
checked-in H₂ example:

```text
{{#include ../../../samples/h2/molecule.xyz}}
```

The atom count must not exceed the number of atom lines present in the file.
RustiQ reports a geometry parse error when the count and available lines do
not match, including when the count is too large to represent the supplied
geometry.

RustiQ treats the coordinates as Angstrom by default; TOML can specify Bohr.
The coordinates in XYZ do not specify molecular charge, spin multiplicity, or
units. Those are calculation settings. A charge changes the electron count;
multiplicity specifies the intended spin state. RustiQ checks that the electron
count and multiplicity have compatible parity, but this check does not identify
the physical ground state. See the [configuration guide](configuration.md).

Calculations convert between Bohr and Angstrom using the
[2018 CODATA Bohr radius published by NIST](https://physics.nist.gov/cuu/pdf/all_2018.pdf):
`5.29177210903e-11 m`, multiplied by `1e10` to obtain Angstrom per Bohr.
RustiQ keeps this edition to preserve existing numerical results. The conversion
regression can be run with
`cargo test -p rustiq-chem-core conversion_preserves_the_2018_codata_factor`;
it checks the exact floating-point result for one Bohr.

`geometry info` reports atom and center information, including a nuclear
repulsion value.
The calculation returns an error if any two atoms have exactly identical
coordinates, because their nuclear repulsion is undefined.

> [!WARNING]
> **Nuclear repulsion: beware of XYZ coordinate units.**
>
> `geometry info` **does not convert coordinates from Angstrom to Bohr**.
> XYZ files have no unit metadata, so this command evaluates nuclear
> repulsion directly from the supplied numbers, effectively treating them
> as **Bohr**. For an XYZ file written in **Angstrom** (including the H₂
> sample below), its reported nuclear-repulsion value is **not a physically
> meaningful energy in Hartree**. **Do not interpret or compare it with
> energies from a molecular calculation.**
>
> Unlike `geometry info`, `rustiq run` converts input coordinates using
> `[molecule].units` (default: `"Angstrom"`) before computing energies.
> This setting does not apply to the standalone geometry command.

With only the release binary, save this geometry as `h2.xyz` as shown in the
[first-calculation guide](../getting-started/first-calculation.md). After
reading the units warning above, inspect or transform the local file with:

```sh
rustiq geometry info h2.xyz
rustiq geometry translate h2.xyz --dz 1 -o h2-shifted.xyz
```

From the repository root, inspect a geometry with:

```sh
rustiq geometry info samples/h2/molecule.xyz
```

The geometry tools can rotate, translate, center, or orient
XYZ coordinates. For example, transformations read and write XYZ and can use
standard input/output:

```sh
rustiq geometry translate samples/h2/molecule.xyz --dz 1
```

These are coordinate transformations. They do not change coordinates to
minimize molecular energy and are not geometry optimization. Standalone XYZ
tools do not convert coordinate units; they have no unit metadata.
Transformations change coordinate values, and written XYZ coordinates are
rounded to six decimal places. Use the runfile's `units` setting to tell a
calculation how to interpret its coordinates.

Mass centering and principal-axis orientation use atomic weights from
[`mendeleev`](https://docs.rs/mendeleev/0.8.4/mendeleev/enum.Element.html#method.atomic_weight):
interval-valued weights use the conventional value, uncertain weights use the
central value, and elements without a standard weight use the mass number of
the most stable isotope. Consequently, these transformations can produce
slightly different coordinates than versions that used a different periodic
table dataset. In the Rust core API, `Atom` stores `mendeleev::Element` by
value. The core crate re-exports this type as `rustiq_core::Element`, so a
downstream crate can construct hydrogen atoms with
`rustiq_core::Element::H` without declaring its own direct
`mendeleev` dependency. The same element type supplies atomic numbers for
electronic-structure calculations and atomic weights for geometry operations.
The core methods for mass centering, inertia tensors, and principal-axis
orientation return their values directly without an atomic-weight error.

For a TOML calculation, the geometry path is relative to the TOML file's
directory. `init` can create a TOML file that refers to an XYZ file without
copying it:

```sh
rustiq init samples/h2/molecule.xyz -o calculation.toml
```

Run commands from the repository root. The relevant executable checks are in
[`tests/cli_samples.rs`](https://github.com/mveril/RustiQ/blob/main/tests/cli_samples.rs)
and [`tests/init_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/init_cli.rs).
Geometry parsing and the transformations themselves are implemented in
[`crates/rustiq-core/src/molecules/geometry.rs`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/molecules/geometry.rs)
and `src/cli/commands/geometry_command/`.

The core `Geometry::nucl_repulsion` API returns a dedicated
`NuclearRepulsionError::CoincidentAtoms` for coincident nuclei. It traverses
atom pairs without allocating an intermediate collection.

Continue with [basis sets](basis-sets.md), then [configuration](configuration.md).
