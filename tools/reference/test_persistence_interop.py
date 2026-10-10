import io
import os
import subprocess
from pathlib import Path

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
            "rustiq-chem-core",
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


def test_numpy_reads_rust_portable_archive(tmp_path: Path) -> None:
    import json
    import zipfile

    from generate_portable_fixtures import digest, scientific_identity

    output = tmp_path / "h2.rustiq"
    environment = os.environ.copy()
    environment["RUSTIQ_ARCHIVE_TEST_OUTPUT"] = str(output)
    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "rustiq-chem-core",
            "--no-default-features",
            "persistence::data::portable::tests::writes_archive_for_python_interoperability",
            "--",
            "--ignored",
            "--exact",
        ],
        check=True,
        cwd=ROOT,
        env=environment,
    )
    with zipfile.ZipFile(output) as archive:
        assert archive.testzip() is None
        assert set(archive.namelist()) == {
            "manifest.json",
            "calculations/calculation-0/calculation.json",
            "calculations/calculation-0/request.json",
            "sources/0",
            "calculations/calculation-0/arrays/integrals/ao-eri.npy",
        }
        manifest = json.loads(archive.read("manifest.json"))
        request_bytes = archive.read("calculations/calculation-0/request.json")
        request = json.loads(request_bytes)
        assert manifest["calculations"][0]["request"]["digest"] == digest(request_bytes)
        assert manifest["calculations"][0]["request"]["size"] == len(request_bytes)
        assert request["hf"]["method"] == "auto"
        assert request["units"] == "bohr"
        source_bytes = archive.read("sources/0")
        assert source_bytes == bytes.fromhex(
            (FIXTURE_DIR / "source-original-v1.toml.hex").read_text()
        )
        assert manifest["sources"][0]["digest"] == digest(source_bytes)
        assert manifest["sources"][0]["original_name"] == "../../calculation.toml"
        snapshot_bytes = archive.read("calculations/calculation-0/calculation.json")
        snapshot = json.loads(snapshot_bytes)
        assert manifest["calculations"][0]["scientific_identity"][
            "digest"
        ] == scientific_identity(snapshot)
        assert manifest["calculations"][0]["calculation"]["digest"] == digest(
            snapshot_bytes
        )
        assert snapshot["hf"]["method"] == "rhf"
        assert snapshot["units"] == "bohr"
        payload = archive.read("calculations/calculation-0/arrays/integrals/ao-eri.npy")
        assert manifest["calculations"][0]["artifacts"]["ao_eri"]["digest"] == digest(
            payload
        )
        _assert_ao_eri_values(np.load(io.BytesIO(payload), allow_pickle=False))
        assert (
            archive.getinfo(
                "calculations/calculation-0/arrays/integrals/ao-eri.npy"
            ).compress_type
            == zipfile.ZIP_STORED
        )
        assert all(
            info.date_time == (1980, 1, 1, 0, 0, 0) for info in archive.infolist()
        )


def test_python_portable_golden_fixtures() -> None:
    import json
    import zipfile

    from generate_portable_fixtures import digest, scientific_identity

    for byte_order in ("little", "big"):
        content = bytes.fromhex(
            (FIXTURE_DIR / f"portable-python-{byte_order}-v1.rustiq.hex").read_text()
        )
        assert b"PK\x06\x06" in content
        with zipfile.ZipFile(io.BytesIO(content)) as archive:
            manifest = json.loads(archive.read("manifest.json"))
            source_bytes = archive.read("sources/0")
            assert source_bytes == bytes.fromhex(
                (FIXTURE_DIR / "source-original-v1.toml.hex").read_text()
            )
            assert digest(source_bytes) == manifest["sources"][0]["digest"]
            request_bytes = archive.read("calculations/calculation-0/request.json")
            assert (
                digest(request_bytes)
                == manifest["calculations"][0]["request"]["digest"]
            )
            assert json.loads(request_bytes)["hf"]["method"] == "auto"
            snapshot = json.loads(
                archive.read("calculations/calculation-0/calculation.json")
            )
            assert (
                scientific_identity(snapshot)
                == manifest["calculations"][0]["scientific_identity"]["digest"]
            )
            payload = archive.read(
                "calculations/calculation-0/arrays/integrals/ao-eri.npy"
            )
            assert (
                digest(payload)
                == manifest["calculations"][0]["artifacts"]["ao_eri"]["digest"]
            )
            _assert_ao_eri_values(np.load(io.BytesIO(payload), allow_pickle=False))


def test_python_angstrom_conversion_fixture() -> None:
    import json
    import zipfile
    from decimal import Decimal, localcontext

    from generate_portable_fixtures import digest, scientific_identity

    content = bytes.fromhex(
        (FIXTURE_DIR / "portable-python-angstrom-v1.rustiq.hex").read_text()
    )
    with zipfile.ZipFile(io.BytesIO(content)) as archive:
        manifest = json.loads(archive.read("manifest.json"))
        request_bytes = archive.read("calculations/calculation-0/request.json")
        snapshot_bytes = archive.read("calculations/calculation-0/calculation.json")
        request = json.loads(request_bytes)
        snapshot = json.loads(snapshot_bytes)
        assert request["units"] == "angstrom"
        assert snapshot["units"] == "bohr"
        assert digest(request_bytes) == manifest["calculations"][0]["request"]["digest"]
        assert (
            digest(snapshot_bytes)
            == manifest["calculations"][0]["calculation"]["digest"]
        )
        assert (
            scientific_identity(snapshot)
            == manifest["calculations"][0]["scientific_identity"]["digest"]
        )
        with localcontext() as context:
            context.prec = 100
            for requested, resolved in zip(
                request["atoms"][1]["position"],
                snapshot["atoms"][1]["position"],
                strict=True,
            ):
                assert resolved == float(
                    Decimal.from_float(requested) / Decimal("0.529177210903")
                )
        assert snapshot["atoms"][1]["position"][0] != 0.74 / 0.529177210903


def _assert_multi_archive(content: bytes) -> None:
    import json
    import zipfile

    from generate_portable_fixtures import digest, scientific_identity

    with zipfile.ZipFile(io.BytesIO(content)) as archive:
        assert archive.testzip() is None
        manifest = json.loads(archive.read("manifest.json"))
        assert "scientific_identity" not in manifest
        assert len(manifest["calculations"]) == 2
        assert len(manifest["sources"]) == 1
        source = manifest["sources"][0]
        assert digest(archive.read(source["path"])) == source["digest"]
        for index, entry in enumerate(manifest["calculations"]):
            assert entry["id"] == f"calculation-{index}"
            for field in ("request", "calculation"):
                reference = entry[field]
                payload = archive.read(reference["path"])
                assert len(payload) == reference["size"]
                assert digest(payload) == reference["digest"]
            snapshot = json.loads(archive.read(entry["calculation"]["path"]))
            assert (
                scientific_identity(snapshot) == entry["scientific_identity"]["digest"]
            )
            reference = entry["artifacts"]["ao_eri"]
            payload = archive.read(reference["path"])
            assert digest(payload) == reference["digest"]
            _assert_ao_eri_values(np.load(io.BytesIO(payload), allow_pickle=False))


def test_python_multi_calculation_fixture() -> None:
    content = bytes.fromhex(
        (FIXTURE_DIR / "portable-python-multi-v1.rustiq.hex").read_text()
    )
    _assert_multi_archive(content)


def test_numpy_reads_rust_multi_calculation_archive(tmp_path: Path) -> None:
    output = tmp_path / "batch.rustiq"
    environment = os.environ.copy()
    environment["RUSTIQ_ARCHIVE_TEST_OUTPUT"] = str(output)
    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "rustiq-chem-core",
            "--no-default-features",
            "persistence::data::portable::tests::writes_bundle_for_python_interoperability",
            "--",
            "--ignored",
            "--exact",
        ],
        check=True,
        cwd=ROOT,
        env=environment,
    )
    _assert_multi_archive(output.read_bytes())
