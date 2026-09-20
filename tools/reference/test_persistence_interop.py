import io
import os
from pathlib import Path
import subprocess

import numpy as np

ROOT = Path(__file__).parents[2]
FIXTURE_DIR = ROOT / "crates/rustiq-core/tests/data/persistence"


def _load_hex_fixture(name: str) -> np.ndarray:
    payload = bytes.fromhex((FIXTURE_DIR / name).read_text(encoding="ascii"))
    return np.load(io.BytesIO(payload), allow_pickle=False)


def _assert_ao_eri_values(values: np.ndarray) -> None:
    assert values.shape == (6,)
    np.testing.assert_array_equal(values, [0.5, 1.5, 2.5, 3.5, 4.5, 5.5])


def test_python_numpy_ao_eri_fixture() -> None:
    values = _load_hex_fixture("ao-eri-python-v1.npy.hex")
    assert values.dtype == np.dtype("<f8")
    _assert_ao_eri_values(values)


def test_numpy_reads_rust_produced_ao_eri(tmp_path: Path) -> None:
    output = tmp_path / "ao-eri-rust.npy"
    environment = os.environ.copy()
    environment["RUSTIQ_NPY_TEST_OUTPUT"] = str(output)
    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "rustiq-core",
            "--no-default-features",
            "persistence::npy::tests::writes_npy_for_numpy_interoperability",
            "--",
            "--ignored",
            "--exact",
        ],
        check=True,
        cwd=ROOT,
        env=environment,
    )

    values = np.load(output, allow_pickle=False)
    assert values.dtype == np.dtype(np.float64)
    _assert_ao_eri_values(values)


def test_python_numpy_big_endian_ao_eri_fixture() -> None:
    values = _load_hex_fixture("ao-eri-python-big-endian-v1.npy.hex")
    assert values.dtype == np.dtype(">f8")
    assert values.shape == (6,)
    np.testing.assert_array_equal(values, [0.5, 1.5, 2.5, 3.5, 4.5, 5.5])
