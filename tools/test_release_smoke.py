import hashlib
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

from release_smoke import validate, verify_checksum


class ReleaseSmokeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.fixture = self.root / "sto-3g.json"
        self.fixture.write_text("{}")
        self.geometry = self.root / "h2.xyz"
        self.geometry.write_text("2\nH2\nH 0 0 -0.37\nH 0 0 0.37\n")
        self.reference = self.root / "reference.rs"
        self.reference.write_text("const PYSCF_TOTAL_ENERGY: f64 = -1.116_759_307_396_425;")
        self.archive = self.root / "bundle.tar.gz"
        content = self.root / "archive-content"
        content.mkdir()
        (content / "rustiq").write_text("controlled executable placeholder")
        with tarfile.open(self.archive, "w:gz") as bundle:
            bundle.add(content / "rustiq", arcname="rustiq")
        self.checksum = self.root / "bundle.tar.gz.sha256"
        self.checksum.write_text(hashlib.sha256(self.archive.read_bytes()).hexdigest() + "  bundle.tar.gz\n")

    def tearDown(self):
        self.temp.cleanup()

    def runner(self, *, json_output=None):
        def execute(binary, args, cwd, env, log):
            log.write_text("controlled\n")
            if args == ["--help"]:
                return "usage: rustiq"
            if args[0] == "run":
                if isinstance(json_output, Exception):
                    raise json_output
                if json_output is not None:
                    return json_output
                return json.dumps({"schema_version": 1, "calculation": {"hf": {
                    "converged": True, "method": "RHF", "total_energy": -1.116759307396425
                }}})
            return "ok"
        return execute

    def validate(self, runner=None):
        return validate(self.archive, self.fixture, self.geometry, self.reference,
                        self.root / "logs", self.checksum, command_runner=runner or self.runner())

    def test_missing_executable(self):
        empty = self.root / "empty.tar.gz"
        with tarfile.open(empty, "w:gz"):
            pass
        checksum = self.root / "empty.sha256"
        checksum.write_text(hashlib.sha256(empty.read_bytes()).hexdigest())
        with self.assertRaisesRegex(ValueError, "expected one rustiq"):
            validate(empty, self.fixture, self.geometry, self.reference, self.root / "empty-logs", checksum, command_runner=self.runner())

    def test_invalid_archive(self):
        bad = self.root / "bad.zip"
        bad.write_text("not an archive")
        checksum = self.root / "bad.sha256"
        checksum.write_text(hashlib.sha256(bad.read_bytes()).hexdigest())
        with self.assertRaisesRegex(ValueError, "invalid archive"):
            validate(bad, self.fixture, self.geometry, self.reference, self.root / "bad-logs", checksum, command_runner=self.runner())

    def test_corrupted_checksum(self):
        self.checksum.write_text("0" * 64)
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            self.validate()

    def test_invalid_json(self):
        with self.assertRaises(json.JSONDecodeError):
            self.validate(self.runner(json_output="{broken"))

    def test_non_converged(self):
        payload = {"schema_version": 1, "calculation": {"hf": {"converged": False, "method": "RHF", "total_energy": -1.1167}}}
        with self.assertRaisesRegex(ValueError, "did not converge"):
            self.validate(self.runner(json_output=json.dumps(payload)))

    def test_incorrect_energy(self):
        payload = {"schema_version": 1, "calculation": {"hf": {"converged": True, "method": "RHF", "total_energy": -1.0}}}
        with self.assertRaisesRegex(ValueError, "differs from regression reference"):
            self.validate(self.runner(json_output=json.dumps(payload)))

    def test_success(self):
        result = self.validate()
        self.assertEqual(result["checksum"], "verified")
        self.assertEqual(result["startup"], "passed")


if __name__ == "__main__":
    unittest.main()
