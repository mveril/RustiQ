import io
from pathlib import Path

import numpy as np


def test_python_numpy_ao_eri_fixture() -> None:
    fixture = (
        Path(__file__).parents[2]
        / "crates/rustiq-core/tests/data/persistence/ao-eri-python-v1.npy.hex"
    )
    payload = bytes.fromhex(fixture.read_text(encoding="ascii"))

    values = np.load(io.BytesIO(payload), allow_pickle=False)

    assert values.dtype == np.dtype("<f8")
    assert values.shape == (6,)
    np.testing.assert_array_equal(values, [0.5, 1.5, 2.5, 3.5, 4.5, 5.5])
