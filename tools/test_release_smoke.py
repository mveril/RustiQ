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
        manual = content / "manual/html"
        (manual / "mathjax").mkdir(parents=True)
        for name in [
            "index.html",
            "mathjax/MathJax.js",
            "mermaid-test.min.js",
            "mermaid-init-test.js",
        ]:
            (manual / name).write_text("controlled manual asset")
        with tarfile.open(self.archive, "w:gz") as bundle:
            bundle.add(content / "rustiq", arcname="rustiq")
            bundle.add(manual, arcname="manual/html")
        self.checksum = self.root / "bundle.tar.gz.sha256"
        self.checksum.write_text(hashlib.sha256(self.archive.read_bytes()).hexdigest() + "  bundle.tar.gz\n")

    def tearDown(self):
        self.temp.cleanup()

    def runner(self, *, json_output=None):
        def execute(binary, args, cwd, env, log):
            log.write_text("controlled\n")
            if args == ["--help"]:
                return "usage: rustiq"
            if args == ["--version", "--verbose"]:
                return "rustiq 0.1.0-alpha.1\n  Git commit                " + "a" * 40 + "\n"
            if args == ["docs", "--print-path"]:
                return str(binary.parent / "manual/html/index.html") + "\n"
            if args == ["docs", "--web", "--print-path"]:
                return "https://mveril.github.io/RustiQ/dev/\n"
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

    def test_missing_git_provenance(self):
        original = self.runner()

        def execute(binary, args, cwd, env, log):
            if args == ["--version", "--verbose"]:
                return "rustiq 0.1.0-alpha.1\n  Git commit                unknown\n"
            return original(binary, args, cwd, env, log)

        with self.assertRaisesRegex(ValueError, "lacks Git provenance"):
            self.validate(runner=execute)

    def test_wrong_relocated_manual_is_rejected(self):
        original = self.runner()

        def execute(binary, args, cwd, env, log):
            if args == ["docs", "--print-path"]:
                return "https://mveril.github.io/RustiQ/\n"
            return original(binary, args, cwd, env, log)

        with self.assertRaisesRegex(ValueError, "DOCUMENTATION_FAILURE"):
            self.validate(runner=execute)

    def test_missing_manual_is_rejected(self):
        binary = self.root / "archive-content/rustiq"
        with tarfile.open(self.archive, "w:gz") as bundle:
            bundle.add(binary, arcname="rustiq")
        self.checksum.write_text(hashlib.sha256(self.archive.read_bytes()).hexdigest())
        with self.assertRaisesRegex(ValueError, "lacks the complete offline manual"):
            self.validate()

    def test_missing_executable(self):
        empty = self.root / "empty.tar.gz"
        with tarfile.open(empty, "w:gz"):
            pass
        checksum = Path(str(empty) + ".sha256")
        checksum.write_text(hashlib.sha256(empty.read_bytes()).hexdigest())
        with self.assertRaisesRegex(ValueError, "expected one rustiq"):
            validate(empty, self.fixture, self.geometry, self.reference, self.root / "empty-logs", checksum, command_runner=self.runner())

    def test_invalid_archive(self):
        bad = self.root / "bad.zip"
        bad.write_text("not an archive")
        checksum = Path(str(bad) + ".sha256")
        checksum.write_text(hashlib.sha256(bad.read_bytes()).hexdigest())
        with self.assertRaisesRegex(ValueError, "invalid archive"):
            validate(bad, self.fixture, self.geometry, self.reference, self.root / "bad-logs", checksum, command_runner=self.runner())

    def test_corrupted_checksum(self):
        self.checksum.write_text("0" * 64)
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            self.validate()

    def test_checksum_exact_filename_and_gnu_format(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.checksum.write_text(f"{digest} *{self.archive.name}\n")
        verify_checksum(self.archive, self.checksum)
        self.checksum.write_text(f"{digest}  prefix-{self.archive.name}\n")
        with self.assertRaisesRegex(ValueError, "exact checksum entry"):
            verify_checksum(self.archive, self.checksum)

    def test_checksum_rejects_ambiguous_and_malformed_entries(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.checksum.write_text(f"{digest}  {self.archive.name}\n{digest} *{self.archive.name}\n")
        with self.assertRaisesRegex(ValueError, "found 2"):
            verify_checksum(self.archive, self.checksum)
        self.checksum.write_text(f"{digest}  {self.archive.name}\nnot a checksum\n")
        with self.assertRaisesRegex(ValueError, "malformed"):
            verify_checksum(self.archive, self.checksum)

    def test_digest_only_checksum_requires_matching_sidecar(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.checksum.write_text(digest)
        verify_checksum(self.archive, self.checksum)
        wrong_name = self.root / "checksums.txt"
        wrong_name.write_text(digest)
        with self.assertRaisesRegex(ValueError, "digest-only checksum"):
            verify_checksum(self.archive, wrong_name)

    def test_checksum_accepts_blank_lines_in_dist_sidecar(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        for text in (
            f"{digest}  {self.archive.name}\n\n",
            f"\n{digest}  {self.archive.name}\n  \n",
            f"{digest}\n\n",
        ):
            with self.subTest(text=text):
                self.checksum.write_text(text, encoding="utf-8")
                verify_checksum(self.archive, self.checksum)

    def test_checksum_accepts_blank_lines_in_sha256_sum(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        summed = self.root / "sha256.sum"
        summed.write_text(f"\n{digest} *{self.archive.name}\n\n", encoding="utf-8")
        verify_checksum(self.archive, summed)

    def test_checksum_rejects_empty_and_malformed_contents(self):
        with self.assertRaisesRegex(ValueError, "contains no entries"):
            self.checksum.write_text("\n   \n", encoding="utf-8")
            verify_checksum(self.archive, self.checksum)
        self.checksum.write_text("not a digest\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "malformed"):
            verify_checksum(self.archive, self.checksum)

    def test_invalid_json(self):
        with self.assertRaisesRegex(ValueError, "JSON_VALIDATION_FAILURE"):
            self.validate(self.runner(json_output="{broken"))

    def test_non_converged(self):
        payload = {"schema_version": 1, "calculation": {"hf": {"converged": False, "method": "RHF", "total_energy": -1.1167}}}
        with self.assertRaisesRegex(ValueError, "did not converge"):
            self.validate(self.runner(json_output=json.dumps(payload)))

    def test_incorrect_energy(self):
        payload = {"schema_version": 1, "calculation": {"hf": {"converged": True, "method": "RHF", "total_energy": -1.0}}}
        with self.assertRaisesRegex(ValueError, "differs from regression reference"):
            self.validate(self.runner(json_output=json.dumps(payload)))

    def test_nonfinite_energy_values_are_rejected(self):
        for value in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(value=value):
                payload = {"schema_version": 1, "calculation": {"hf": {
                    "converged": True, "method": "RHF", "total_energy": value
                }}}
                with self.assertRaisesRegex(ValueError, "must be finite"):
                    self.validate(self.runner(json_output=json.dumps(payload)))

    def test_missing_or_nonnumeric_energy_is_rejected(self):
        for hf in (
            {"converged": True, "method": "RHF"},
            {"converged": True, "method": "RHF", "total_energy": "not an energy"},
        ):
            with self.subTest(hf=hf):
                payload = {"schema_version": 1, "calculation": {"hf": hf}}
                with self.assertRaisesRegex(ValueError, "missing or nonnumeric"):
                    self.validate(self.runner(json_output=json.dumps(payload)))

    def test_nonfinite_reference_energy_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "reference energy must be finite"):
            validate(self.archive, self.fixture, self.geometry, self.reference,
                     self.root / "logs", self.checksum, expected_energy=float("nan"),
                     command_runner=self.runner())

    def test_success(self):
        result = self.validate()
        self.assertEqual(result["checksum"], "verified")
        self.assertEqual(result["startup"], "passed")


if __name__ == "__main__":
    unittest.main()
