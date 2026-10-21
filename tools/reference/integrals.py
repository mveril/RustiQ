"""Independent libcint AO references from unnormalized BSE source coefficients."""
from __future__ import annotations

import hashlib
import json
import os
import tempfile
from pathlib import Path

import numpy as np
from pyscf import gto, __version__

from compare_pyscf import REPO_ROOT, load_xyz_body, run_command

BASIS_ROOT = REPO_ROOT / "tests/data/reference/RustiQ/basis_sets"
CASES = [("h2-sto-3g", "samples/h2/molecule.xyz", "sto-3g", False, 2),
         ("h2o-sto-3g", "samples/h2o/h2o.xyz", "sto-3g", False, 7),
         ("h2o-6-31g", "samples/h2o/h2o.xyz", "6-31g", False, 13),
         ("h2o-cc-pvdz", "samples/h2o/h2o.xyz", "cc-pvdz", False, 24),
         ("h2o-cc-pvdz-cart", "samples/h2o/h2o.xyz", "cc-pvdz", True, 25)]
# Absolute acceptance avoids relative error amplification at symmetry zeros.
ATOL = 1e-10
RELATIVE_FLOOR = 1e-8


def reference(xyz, basis_name, cart):
    source = json.loads((BASIS_ROOT / f"{basis_name}.json").read_text())
    basis = {}
    for z in (1, 8):
        shells = []
        for shell in source["elements"][str(z)]["electron_shells"]:
            angular = shell["angular_momentum"]
            coefficients = shell["coefficients"]
            if len(angular) == 1:
                angular = angular * len(coefficients)
            if len(angular) != len(coefficients):
                raise ValueError("Unsupported BSE contraction layout")
            for l, coeff in zip(angular, coefficients, strict=True):
                # Separate contractions preserve BSE order. PySCF independently
                # normalizes primitives and contractions from source coefficients.
                shells.append([l, *[[float(a), float(c)] for a, c in
                                    zip(shell["exponents"], coeff, strict=True)]])
        basis[gto.mole._symbol(z)] = shells
    atoms = []
    for line in load_xyz_body(REPO_ROOT / xyz).splitlines():
        symbol, *coords = line.split()
        atoms.append((symbol, [float(x) / 0.529177210903 for x in coords]))
    mol = gto.M(atom=atoms, unit="Bohr", basis=basis, cart=cart, verbose=0)
    mapping, scales = [], []
    # PySCF stably sorts shells by l; RustiQ retains BSE shell/contraction order.
    shell_order = []
    offset = 0
    for symbol, _ in atoms:
        original = basis[gto.mole._symbol(int(symbol)) if symbol.isdigit() else symbol]
        sorted_ids = sorted(range(len(original)), key=lambda i: original[i][0])
        shell_order.extend(offset + sorted_ids.index(i) for i in range(len(original)))
        offset += len(original)
    for sh in shell_order:
        l = mol.bas_angular(sh)
        start = mol.ao_loc_nr()[sh]
        order = {0: [0], 1: [2, 1, 0], 2: ([5, 4, 3, 2, 1, 0] if cart else list(range(5)))}[l]
        assert mol.bas_nctr(sh) == 1
        mapping.extend(int(start + i) for i in order)
    # libcint Cartesian d functions share a radial normalization; RustiQ
    # normalizes each monomial. This positive diagonal scaling is fixed by
    # analytic Cartesian angular norms, not fitted to integral differences.
    raw_s = mol.intor("int1e_ovlp")
    for sh in range(mol.nbas):
        l = mol.bas_angular(sh)
        if cart and l == 2:
            scales.extend([np.sqrt(5 / (4 * np.pi)), np.sqrt(15 / (4 * np.pi)),
                           np.sqrt(15 / (4 * np.pi)), np.sqrt(5 / (4 * np.pi)),
                           np.sqrt(15 / (4 * np.pi)), np.sqrt(5 / (4 * np.pi))])
        else:
            scales.extend([1.] * (mol.ao_loc_nr()[sh+1] - mol.ao_loc_nr()[sh]))
    scales = np.array(scales)
    assert np.allclose(np.diag(raw_s) * scales**2, 1, atol=1e-13, rtol=0)
    matrices = {}
    factors = scales[mapping]
    for family, operator in [("S", "int1e_ovlp"), ("T", "int1e_kin"), ("V", "int1e_nuc")]:
        matrices[family] = mol.intor(operator)[np.ix_(mapping, mapping)] * np.outer(factors, factors)
    matrices["Hcore"] = matrices["T"] + matrices["V"]
    # s8 avoids a full n^4 allocation. Decode pairs independently of Rust.
    compact = mol.intor("int2e", aosym="s8")
    pairs = [(i, j) for i in range(mol.nao) for j in range(i+1)]
    def pair(i, j):
        a, b = max(i, j), min(i, j)
        return a*(a+1)//2+b
    values, indices = [], []
    for p, (i, j) in enumerate(pairs):
        for k, l in pairs[:p+1]:
            a, b = pair(mapping[i], mapping[j]), pair(mapping[k], mapping[l])
            values.append(compact[pair(a, b)] * factors[i]*factors[j]*factors[k]*factors[l])
            indices.append((i, j, k, l))
    matrices["ERI"] = np.array(values)
    return mol, mapping, matrices, indices


def metrics(actual, expected, indices):
    actual, expected = np.asarray(actual), np.asarray(expected)
    if actual.shape != expected.shape or not (np.isfinite(actual).all() and np.isfinite(expected).all()):
        raise ValueError("Invalid shape or non-finite integral")
    diff = np.abs(actual - expected)
    meaningful = np.abs(expected) >= RELATIVE_FLOOR
    worst = np.argsort(diff)[-5:][::-1]
    return {"count": int(diff.size), "max_absolute_error": float(diff.max()),
            "rms_absolute_error": float(np.sqrt(np.mean(diff**2))),
            "max_meaningful_relative_error": float(np.max(diff[meaningful]/np.abs(expected[meaningful]), initial=0)),
            "relative_reference_floor": RELATIVE_FLOOR, "absolute_tolerance": ATOL,
            "exceedances": int(np.count_nonzero(diff > ATOL)),
            "largest_discrepancies": [{"indices": list(indices[i]), "rustiq": float(actual[i]),
                                       "pyscf": float(expected[i]), "absolute_error": float(diff[i])} for i in worst],
            "status": "PASS" if np.all(diff <= ATOL) else "FAIL"}


def validate():
    with tempfile.TemporaryDirectory(prefix="rustiq-integrals-") as temp:
        output = Path(temp) / "integrals.json"
        env = dict(os.environ, RUSTIQ_INTEGRAL_OUTPUT=str(output))
        cargo_error = None
        try:
            run_command(["cargo", "test", "-p", "rustiq-chem-core", "--lib",
                         "integral_reference_tests::export_integral_validation", "--", "--exact"], env=env)
        except RuntimeError as error:
            if not output.exists():
                raise
            cargo_error = str(error)
        rust = json.loads(output.read_text())
    rows = []
    for actual, (name, xyz, basis, cart, dimension) in zip(rust, CASES, strict=True):
        mol, mapping, expected, eri_indices = reference(xyz, basis, cart)
        assert actual["case_id"] == name
        assert actual["nao"] == mol.nao == dimension
        for family, values in expected.items():
            indices = eri_indices if family == "ERI" else [(i, j) for j in range(dimension) for i in range(dimension)]
            row = {"case_id": name, "molecule": "H2" if name.startswith("h2-") else "H2O",
                   "basis": basis, "representation": "Cartesian" if cart else "spherical",
                   "nao": dimension, "dimensions": [dimension]* (4 if family == "ERI" else 2),
                   "family": family, "ao_mapping": mapping,
                   "basis_sha256": hashlib.sha256((BASIS_ROOT / f"{basis}.json").read_bytes()).hexdigest(),
                   "geometry_sha256": hashlib.sha256((REPO_ROOT / xyz).read_bytes()).hexdigest(),
                   **metrics(actual[family], values.flatten(order="F"), indices)}
            rows.append(row)
    return {"pyscf_version": __version__, "screening": "disabled", "bohr_in_angstrom": 0.529177210903,
            "cases": rows, "cargo_error": cargo_error, "passed": cargo_error is None and all(r["status"] == "PASS" for r in rows)}

FIXTURE = REPO_ROOT / "crates/rustiq-core/tests/data/integrals/pyscf-v1.json"


def generate_fixture():
    cases, arrays = [], {}
    for name, xyz, basis, cart, dimension in CASES:
        mol, mapping, expected, _ = reference(xyz, basis, cart)
        assert mol.nao == dimension
        cases.append({"case_id": name, "geometry": xyz, "basis": basis,
                      "cartesian": cart, "nao": dimension, "ao_mapping": mapping,
                      "basis_sha256": hashlib.sha256((BASIS_ROOT / f"{basis}.json").read_bytes()).hexdigest(),
                      "geometry_sha256": hashlib.sha256((REPO_ROOT / xyz).read_bytes()).hexdigest(),
                      "arrays": {family: f"{name}/{family}" for family in expected}})
        arrays.update({f"{name}/{family}": np.asfortranarray(values) for family, values in expected.items()})
    return {"schema_version": 1, "pyscf_version": __version__,
            "basis_source": "Checked-in Basis Set Exchange JSON; raw exponents and contraction coefficients",
            "basis_source_path": "tests/data/reference/RustiQ/basis_sets/{basis}.json",
            "basis_source_url": "https://www.basissetexchange.org/",
            "ao_conventions": {
                "shells": "atom order, BSE shell and contraction order; mapping undoes PySCF stable l sort",
                "p": "RustiQ z,y,x; PySCF x,y,z; positive phases",
                "spherical_d": "xy,yz,z2,xz,x2-y2; positive phases; normalized real harmonics",
                "cartesian_d": "RustiQ zz,yz,yy,xz,xy,xx; PySCF xx,xy,xz,yy,yz,zz",
                "cartesian_d_scaling": "PySCF xx,yy,zz times sqrt(5/(4*pi)); xy,xz,yz times sqrt(15/(4*pi))",
                "normalization": "Each engine independently normalizes raw BSE primitives and contractions",
                "cartesian_extension": "Only function_type changes in memory; exponents and coefficients unchanged"},
            "bohr_in_angstrom": 0.529177210903, "matrix_order": "column-major",
            "eri_order": "lower triangular AO pairs, lower triangular pair pairs; chemists notation",
            "absolute_tolerance": ATOL, "cases": cases}, arrays


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--regenerate", action="store_true")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if args.regenerate:
        FIXTURE.parent.mkdir(parents=True, exist_ok=True)
        metadata, arrays = generate_fixture()
        metadata["array_sha256"] = {}
        for name, values in arrays.items():
            path = FIXTURE.parent / f"{name}.npy"
            path.parent.mkdir(parents=True, exist_ok=True)
            np.save(path, values, allow_pickle=False)
            metadata["array_sha256"][name] = hashlib.sha256(path.read_bytes()).hexdigest()
        FIXTURE.write_text(json.dumps(metadata, indent=2, allow_nan=False) + "\n")
    if args.report:
        report = validate()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
        raise SystemExit(0 if report["passed"] else 1)
