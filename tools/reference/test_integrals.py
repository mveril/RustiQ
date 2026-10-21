"""Complete independent AO comparisons; production data are computed once."""
import numpy as np
import pytest
from integrals import validate, metrics, generate_fixture, FIXTURE
import json
import hashlib


def test_ao_integrals():
    report = validate()
    failures = [row for row in report["cases"] if row["status"] != "PASS"]
    assert report["passed"], (failures, report.get("cargo_error"))


def test_metrics_detect_failure_and_nonfinite():
    assert metrics([0., 1.], [0., 1.01], [(0,), (1,)])["exceedances"] == 1
    with pytest.raises(ValueError, match="non-finite"):
        metrics([np.nan], [0.], [(0,)])


def test_checked_in_pyscf_references():
    metadata, generated = generate_fixture()
    stored = json.loads(FIXTURE.read_text())
    assert metadata == {key: value for key, value in stored.items() if key != "array_sha256"}
    assert set(stored["array_sha256"]) == set(generated)
    for name, values in generated.items():
        path = FIXTURE.parent / f"{name}.npy"
        assert hashlib.sha256(path.read_bytes()).hexdigest() == stored["array_sha256"][name]
        np.testing.assert_allclose(np.load(path, allow_pickle=False), values, atol=1e-13, rtol=0, err_msg=name)


def test_real_d_phases_and_normalization():
    from pyscf import gto
    # Rows: xx,xy,xz,yy,yz,zz; columns: xy,yz,z2,xz,x2-y2.
    normalized_monomials = np.array([
        [0, 0, -.5, 0, np.sqrt(3)/2],
        [1, 0, 0, 0, 0], [0, 0, 0, 1, 0],
        [0, 0, -.5, 0, -np.sqrt(3)/2],
        [0, 1, 0, 0, 0], [0, 0, 1, 0, 0]])
    scales = np.sqrt(np.array([5, 15, 15, 5, 15, 5]) / (4*np.pi))
    np.testing.assert_allclose(gto.cart2sph(2), scales[:, None] * normalized_monomials,
                               atol=3e-16, rtol=0)
