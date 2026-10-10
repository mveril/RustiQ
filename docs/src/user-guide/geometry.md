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

`geometry info` reports atom and center information, including a nuclear
repulsion value.

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

From the repository root, inspect a geometry with:

```sh
cargo run --locked -- geometry info samples/h2/molecule.xyz
```

The geometry tools can rotate, translate, center, or orient
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
