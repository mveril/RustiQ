# Geometry files

RustiQ reads molecular coordinates from XYZ files. Each file starts with the
number of atoms, then a comment line, then one line per atom containing an
element symbol or atomic number and three Cartesian coordinates. See the
checked-in H₂ example:

```text
{{#include ../../../samples/h2/molecule.xyz}}
```

RustiQ treats the coordinates as Angstrom by default; TOML can specify Bohr.
The coordinates in XYZ do not specify molecular charge, spin multiplicity, or
units. Those are calculation settings. A charge changes the electron count;
multiplicity specifies the intended spin state. RustiQ checks that the electron
count and multiplicity have compatible parity, but this check does not identify
the physical ground state. See the [configuration guide](configuration.md).

From the repository root, inspect a geometry with:

```sh
cargo run --locked -- geometry info samples/h2/molecule.xyz
```

`geometry info` reports atom and center information, including a nuclear
repulsion value. The `info` command uses raw coordinate values without
conversion; its nuclear-repulsion number is in Hartree only when those
coordinates are in Bohr. The geometry tools can rotate, translate, center, or orient
XYZ coordinates. For example, transformations read and write XYZ and can use
standard input/output:

```sh
cargo run --locked -- geometry translate samples/h2/molecule.xyz --dz 1
```

These are coordinate transformations. They do not change coordinates to
minimize molecular energy and are not geometry optimization. Standalone XYZ
tools do not convert coordinate units; they have no unit metadata.
Transformations change coordinate values, and written XYZ coordinates are
rounded to six decimal places. Use the runfile's `units` setting to tell a
calculation how to interpret its coordinates.

For a TOML calculation, the geometry path is relative to the TOML file's
directory. `init` can create a TOML file that refers to an XYZ file without
copying it:

```sh
cargo run --locked -- init samples/h2/molecule.xyz -o calculation.toml
```

Run commands from the repository root. The relevant executable checks are in
[`tests/cli_samples.rs`](https://github.com/mveril/RustiQ/blob/main/tests/cli_samples.rs)
and [`tests/init_cli.rs`](https://github.com/mveril/RustiQ/blob/main/tests/init_cli.rs).
Geometry parsing and the transformations themselves are implemented in
[`crates/rustiq-core/src/molecules/geometry.rs`](https://github.com/mveril/RustiQ/blob/main/crates/rustiq-core/src/molecules/geometry.rs)
and `src/cli/commands/geometry_command/`.

Continue with [basis sets](basis-sets.md), then [configuration](configuration.md).
