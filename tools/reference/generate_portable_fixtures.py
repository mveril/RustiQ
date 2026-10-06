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


def portable_manifest(manifest: dict) -> dict:
    entry = {"id": "calculation-0"}
    for field in ("scientific_identity", "request", "calculation", "artifacts"):
        entry[field] = manifest.pop(field)
    for field in ("request", "calculation"):
        entry[field]["path"] = "calculations/calculation-0/" + entry[field]["path"]
    for artifact in entry["artifacts"].values():
        artifact["path"] = "calculations/calculation-0/" + artifact["path"]
    manifest["calculations"] = [entry]
    return manifest


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
        manifest = portable_manifest(manifest)
        output = io.BytesIO()
        # Force full ZIP64 EOCD/locator/central extras on small fixtures.
        previous_limit = zipfile.ZIP64_LIMIT
        try:
            zipfile.ZIP64_LIMIT = 0
            with zipfile.ZipFile(output, "w", allowZip64=True) as archive:
                for name, content in [
                    ("manifest.json", json.dumps(manifest, indent=2).encode() + b"\n"),
                    ("calculations/calculation-0/calculation.json", snapshot),
                    ("calculations/calculation-0/request.json", request),
                    ("sources/0", source),
                    ("calculations/calculation-0/arrays/integrals/ao-eri.npy", payload),
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


def generate_angstrom() -> None:
    """Produce coordinates independently, rounding only after Decimal division."""
    from decimal import Decimal, localcontext

    request = json.loads((FIXTURES / "request-h2-v1.json").read_bytes())
    snapshot = json.loads((FIXTURES / "calculation-h2-v1.json").read_bytes())
    request["units"] = "angstrom"
    request["atoms"][1]["position"] = [0.74, -0.13, 0.0]
    with localcontext() as context:
        context.prec = 100
        coordinates = [
            float(Decimal.from_float(x) / Decimal("0.529177210903"))
            for x in request["atoms"][1]["position"]
        ]
    snapshot["atoms"][1]["position"] = coordinates
    snapshot["basis"][1]["center"] = coordinates
    request_bytes = json.dumps(request, indent=2).encode() + b"\n"
    snapshot_bytes = json.dumps(snapshot, indent=2).encode() + b"\n"
    manifest = {
        "format": "rustiq-persistence",
        "format_version": 1,
        "kind": "portable",
        "producer": {"name": "Python Decimal interoperability fixture", "version": "1"},
        "scientific_identity": {"version": 1, "digest": scientific_identity(snapshot)},
        "request": {
            "path": "request.json",
            "version": 1,
            "size": len(request_bytes),
            "digest": digest(request_bytes),
        },
        "calculation": {
            "path": "calculation.json",
            "version": 1,
            "size": len(snapshot_bytes),
            "digest": digest(snapshot_bytes),
        },
        "artifacts": {},
    }
    manifest = portable_manifest(manifest)
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", allowZip64=True) as archive:
        for name, content in [
            ("manifest.json", json.dumps(manifest, indent=2).encode() + b"\n"),
            ("calculations/calculation-0/request.json", request_bytes),
            ("calculations/calculation-0/calculation.json", snapshot_bytes),
        ]:
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, content)
    (FIXTURES / "portable-python-angstrom-v1.rustiq.hex").write_text(
        output.getvalue().hex() + "\n", encoding="ascii"
    )


def generate_multi() -> None:
    """Two independent entries share source provenance and use opposite NPY byte orders."""

    def write_member(target: zipfile.ZipFile, name: str, content: bytes) -> None:
        info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
        info.create_system = 3
        info.external_attr = 0o100644 << 16
        info.compress_type = (
            zipfile.ZIP_STORED if name.endswith(".npy") else zipfile.ZIP_DEFLATED
        )
        target.writestr(info, content)

    output = io.BytesIO()
    combined = None
    with zipfile.ZipFile(output, "w", allowZip64=True) as target:
        for index, order in enumerate(("little", "big")):
            content = bytes.fromhex(
                (FIXTURES / f"portable-python-{order}-v1.rustiq.hex").read_text()
            )
            with zipfile.ZipFile(io.BytesIO(content)) as source:
                manifest = json.loads(source.read("manifest.json"))
                entry = manifest["calculations"][0]
                old_prefix = "calculations/calculation-0/"
                new_prefix = f"calculations/calculation-{index}/"
                entry["id"] = f"calculation-{index}"
                for reference in (
                    entry["request"],
                    entry["calculation"],
                    *entry["artifacts"].values(),
                ):
                    reference["path"] = reference["path"].replace(
                        old_prefix, new_prefix
                    )
                if combined is None:
                    combined = manifest
                    write_member(target, "sources/0", source.read("sources/0"))
                else:
                    combined["calculations"].append(entry)
                for name in source.namelist():
                    if name.startswith(old_prefix):
                        write_member(
                            target,
                            name.replace(old_prefix, new_prefix),
                            source.read(name),
                        )
        write_member(
            target, "manifest.json", json.dumps(combined, indent=2).encode() + b"\n"
        )
    (FIXTURES / "portable-python-multi-v1.rustiq.hex").write_text(
        output.getvalue().hex() + "\n", encoding="ascii"
    )


if __name__ == "__main__":
    generate()
    generate_angstrom()
    generate_multi()
