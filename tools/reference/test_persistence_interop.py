import io
from pathlib import Path

import numpy as np


FIXTURE_DIR = (
    Path(__file__).parents[2] / "crates/rustiq-core/tests/data/persistence"
)


def _load_hex_fixture(name: str) -> np.ndarray:
    payload = bytes.fromhex((FIXTURE_DIR / name).read_text(encoding="ascii"))
    return np.load(io.BytesIO(payload), allow_pickle=False)


def _assert_ao_eri_fixture(values: np.ndarray) -> None:
    assert values.dtype == np.dtype("<f8")
    assert values.shape == (6,)
    np.testing.assert_array_equal(values, [0.5, 1.5, 2.5, 3.5, 4.5, 5.5])


def test_python_numpy_ao_eri_fixture() -> None:
    _assert_ao_eri_fixture(_load_hex_fixture("ao-eri-python-v1.npy.hex"))


def test_numpy_reads_rust_ao_eri_fixture() -> None:
    _assert_ao_eri_fixture(_load_hex_fixture("ao-eri-rust-v1.npy.hex"))
