"""Regenerate small Python ZIP64 fixtures using the documented V1 wire format.

Uses only the standard library and the checked-in NumPy payload fixtures.
"""

import hashlib
import io
import json
import struct
import zipfile
from pathlib import Path

FIXTURES = (
    Path(__file__).resolve().parents[2] / "crates/rustiq-core/tests/data/persistence"
)


def scientific_identity(snapshot: dict) -> str:
    canonical = bytearray()

    def length(value: int) -> None:
        canonical.extend(struct.pack(">Q", value))

    def text(value: str) -> None:
        encoded = value.encode("utf-8")
        length(len(encoded))
        canonical.extend(encoded)

    def number(value: float) -> None:
        canonical.extend(struct.pack(">d", 0.0 if value == 0 else value))

    text("scientific-identity-v1")
    text("ao-eri-computation-version")
    canonical.extend(struct.pack(">I", snapshot["ao_eri_computation_version"]))
    text("rustiq-compact-eri-v1")
    length(len(snapshot["atoms"]))
    for atom in snapshot["atoms"]:
        canonical.extend(struct.pack(">I", atom["atomic_number"]))
        for coordinate in atom["position"]:
            number(coordinate)
    length(len(snapshot["basis"]))
    for ao in snapshot["basis"]:
        for coordinate in ao["center"]:
            number(coordinate)
        length(len(ao["components"]))
        for component in ao["components"]:
            canonical.extend(bytes(component["angular_momentum"]))
            length(len(component["primitives"]))
            for primitive in component["primitives"]:
                number(primitive["exponent"])
                number(primitive["coefficient"])
    threshold = snapshot["hf"]["eri_schwarz_threshold"]
    canonical.append(int(threshold is not None))
    if threshold is not None:
        number(threshold)
    return digest(canonical)


def digest(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def generate() -> None:
    snapshot = (FIXTURES / "calculation-h2-v1.json").read_bytes()
    request = (FIXTURES / "request-h2-v1.json").read_bytes()
    source = bytes.fromhex((FIXTURES / "source-original-v1.toml.hex").read_text())
    for suffix, npy_fixture in [
        ("little", "ao-eri-python-v1.npy.hex"),
        ("big", "ao-eri-python-big-endian-v1.npy.hex"),
    ]:
        payload = bytes.fromhex((FIXTURES / npy_fixture).read_text())
        manifest = {
            "format": "rustiq-persistence",
            "format_version": 1,
            "kind": "portable",
            "producer": {"name": "Python interoperability fixture", "version": "1"},
            "scientific_identity": {
                "version": 1,
                "digest": scientific_identity(json.loads(snapshot)),
            },
            "request": {
                "path": "request.json",
                "version": 1,
                "size": len(request),
                "digest": digest(request),
            },
            "calculation": {
                "path": "calculation.json",
                "version": 1,
                "size": len(snapshot),
                "digest": digest(snapshot),
            },
            "sources": [
                {
                    "original_name": "../../calculation.toml",
                    "path": "sources/0",
                    "version": 1,
                    "size": len(source),
                    "digest": digest(source),
                }
            ],
            "artifacts": {
                "ao_eri": {
                    "path": "arrays/integrals/ao-eri.npy",
                    "size": len(payload),
                    "digest": digest(payload),
                    "representation": "rustiq-compact-eri-v1",
                    "attributes": {"basis_functions": 2, "computation_version": 1},
                }
            },
        }
        output = io.BytesIO()
        # Force full ZIP64 EOCD/locator/central extras on small fixtures.
        previous_limit = zipfile.ZIP64_LIMIT
        try:
            zipfile.ZIP64_LIMIT = 0
            with zipfile.ZipFile(output, "w", allowZip64=True) as archive:
                for name, content in [
                    ("manifest.json", json.dumps(manifest, indent=2).encode() + b"\n"),
                    ("calculation.json", snapshot),
                    ("request.json", request),
                    ("sources/0", source),
                    ("arrays/integrals/ao-eri.npy", payload),
                ]:
                    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                    info.create_system = 3
                    info.external_attr = 0o100644 << 16
                    info.compress_type = (
                        zipfile.ZIP_STORED
                        if name.endswith(".npy")
                        else zipfile.ZIP_DEFLATED
                    )
                    with archive.open(info, "w", force_zip64=True) as member:
                        member.write(content)
        finally:
            zipfile.ZIP64_LIMIT = previous_limit
        (FIXTURES / f"portable-python-{suffix}-v1.rustiq.hex").write_text(
            output.getvalue().hex() + "\n", encoding="ascii"
        )


if __name__ == "__main__":
    generate()
