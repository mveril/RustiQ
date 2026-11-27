# Error messages and exit codes

When a command fails, read the message in your terminal. It may name a file,
highlight an input value, or suggest a fix. If the message includes a code
starting with `rustiq::`, search for that exact code on this page.

For example, `rustiq::run::input_read_failed` means RustiQ could not read the
calculation input. If the message says the file was not found, check the file
name and the directory you ran the command from. For
`rustiq::basis::missing_offline`, follow the
[steps for making the basis available locally](basis-sets.md).

## Check the exit code

The exit code is a number your shell receives when RustiQ finishes. It tells
you how the command ended. The `rustiq::…` code identifies an individual
problem and can help you decide what to check.

For example, with `rustiq` installed, run these commands in Bash or another
POSIX shell from a directory where `missing.toml` does not exist:

```sh
status=0
rustiq run missing.toml || status=$?
printf 'Exit code: %s\n' "$status"
```

RustiQ prints `rustiq::run::input_read_failed` in the error message, and the
last line printed by the example is `Exit code: 1`. The shell puts the status
of the failed command in `$?`, so the example saves it immediately.

In PowerShell, save the native process status immediately:

```powershell
rustiq run missing.toml
$status = $LASTEXITCODE
Write-Output "Exit code: $status"
```

| Exit code | What it means | What to check |
| --- | --- | --- |
| `0` | The command completed successfully. Some commands allow skips. | Read the report if you need to know which items were skipped. |
| `1` | The command encountered an error. | Read the message and look up its diagnostic code below. |
| `2` | RustiQ rejected the command-line arguments. | Run the command with `--help` to check its syntax and options. |
| `3` | No requested calculation succeeded because Hartree–Fock did not converge. | Review the [Hartree–Fock convergence guidance](hartree-fock.md). |
| `4` | Some calculations or methods succeeded and others failed or did not converge. | Review the individual outcomes before using the results. |
| `5` | Strict bundle verification skipped some artifacts and found no failures among those it checked. | Review the skipped entries before relying on the verification. |

See the [command-line reference](../reference/cli.md#exit-status-and-diagnostics)
for the detailed rules for batches and bundle verification. Post-SCF campaigns
return `0` when at least one method succeeds and none fails, even with skips;
`4` when successes coexist with failures; and `1` when none succeeds.

## Find codes in JSON reports

When a JSON report is produced, failed calculations in a batch put the code
in `error.code` and the explanation in `error.message`. In artifact verification
reports, invalid entries carry `code` alongside the `errors` list. Older reports
may omit these code fields. Post-SCF skipped and failed outcomes use
`diagnostic.code`, with the explanation in `diagnostic.message` and optional
`diagnostic.help`. Failures preserve specific codes and use
`rustiq::post_scf::failed` only when no code is available.

These JSON additions are a beta.2 compatibility change within schema version 1.
Updated schemas accept older reports; consumers validating new reports against
older strict schemas may need to update their schemas. A command can write a JSON report and still return
a nonzero exit code, so check both when scripting.

## Look up a diagnostic code

Use the message and any highlighted input to narrow down the cause. If you
ask for help, include the code, the full message, and the command you ran.

### CLI, help, and generated files

| Code | Meaning and what to check |
| --- | --- |
| `rustiq::cli::invalid_arguments` | The command line was rejected. Check the command's `--help` for options and valid combinations. |
| `rustiq::cli::diagnostic_hook` | The diagnostic renderer could not be initialized. Report the issue with the RustiQ version and invocation. |
| `rustiq::cli::version_write_failed` | Version output could not be written. Check the destination or downstream pipe. |
| `rustiq::docs::browser_failed` | RustiQ could not launch a browser. Open the manual location shown in the diagnostic directly. |
| `rustiq::docs::failed` | The manual could not be accessed. Check the underlying filesystem error. |
| `rustiq::docs::incomplete_manual` | The installed manual directory lacks its entry page. Repair the installation. |
| `rustiq::docs::installation_directory` | RustiQ could not find the executable's installation directory. Check how the executable was installed or invoked. |
| `rustiq::generate::packaging_value` | `RUSTIQ_PACKAGING` has an invalid value or is not valid Unicode. Check the packaging environment. |
| `rustiq::generate::unknown_shell` | The requested completion shell was not recognized. Use a shell listed in the command help. |
| `rustiq::completions::failed` | Completion generation or output failed. Check the cause and the output destination. |
| `rustiq::manpages::failed` | Manual-page generation failed. Check the output directory and its permissions. |
| `rustiq::unexpected_error` | RustiQ rendered a diagnostic without a more specific code. Use the underlying message when investigating or reporting it. |

### Input generation and geometry

| Code | Meaning and what to check |
| --- | --- |
| `rustiq::init::empty_basis` | The basis name is empty. Supply a basis name. |
| `rustiq::init::failed` | Input generation failed. Check the cause for geometry, configuration, or output errors. |
| `rustiq::init::geometry_path` | The geometry path could not be represented in the generated input. Check the path and filename. |
| `rustiq::init::invalid_electron_count` | The geometry and charge imply an invalid electron count. Check the atoms and charge. |
| `rustiq::init::output_exists` | The destination already exists. Choose another path or use `--force` to replace it. |
| `rustiq::init::output_filename` | RustiQ could not determine a usable output filename. Supply a file path. |
| `rustiq::init::output_is_geometry` | The output resolves to the input geometry. Choose a different destination. |
| `rustiq::init::output_type` | The destination has an unsupported file type. Choose a regular file destination. |
| `rustiq::geometry::atom_count` | The XYZ header has an invalid atom count. Check the first line. |
| `rustiq::geometry::atom_count_mismatch` | The file has fewer atom lines than declared. Correct the count or add the missing lines. |
| `rustiq::geometry::atom_lines` | One or more atom records are invalid. Read the related diagnostics. |
| `rustiq::geometry::invalid_atom_line` | An XYZ atom record is invalid. Check the highlighted element or coordinates. |
| `rustiq::geometry::io` | The XYZ input could not be read. Check the input path, permissions, and encoding. |
| `rustiq::geometry::dangling_output` | The output path is a symbolic link with a missing target. Repair the link or choose another path. |
| `rustiq::geometry::output_exists` | The destination already exists. Choose another path or use `--force` to replace it. |
| `rustiq::geometry::publish_failed` | The temporary geometry output could not be published. Check the destination and directory permissions. |
| `rustiq::geometry::failed` | A geometry command failed. Check the underlying calculation or filesystem error. |

### Basis sets and cache

| Code | Meaning and what to check |
| --- | --- |
| `rustiq::basis::failed` | A basis management operation failed. Read the cause and check the store or download service involved. |
| `rustiq::basis::import_failed` | One or more basis imports failed. Read the per-file diagnostics and retry the failed imports. |
| `rustiq::basis::import_file_failed` | A specific basis file could not be imported. Check its path, format, and store permissions. |
| `rustiq::basis::missing_offline` | The requested basis is absent from the local store and automatic download is disabled or unavailable. Import the basis or use an online build with download enabled. |
| `rustiq::basis::resolve_failed` | RustiQ could not load or download the requested basis. Check the local store and, when downloading, the network and basis name. |
| `rustiq::cache::entry_not_found` | The named cache entry does not exist. Check `cache list` and the selected cache directory. |
| `rustiq::cache::failed` | Cache listing or removal failed. Check the cache directory and the underlying filesystem error. |

### Run inputs and calculations

| Code | Meaning and what to check |
| --- | --- |
| `rustiq::runfile::nickel` | The input could not be parsed, evaluated, or validated. Check the highlighted input and related errors. |
| `rustiq::runfile::nickel::detail` | An individual input error reported with the main runfile diagnostic. Check its message and highlighted field. |
| `rustiq::run::input_read_failed` | RustiQ could not read the calculation file or standard input. Check the path, permissions, and UTF-8 encoding. |
| `rustiq::run::geometry_read_failed` | RustiQ could not read the XYZ file referenced by the calculation. Check the resolved path, permissions, and UTF-8 encoding. |
| `rustiq::run::empty_bundle` | The input bundle contains no calculations. Supply a bundle with a calculation. |
| `rustiq::run::empty_study` | Validated Nickel input unexpectedly contains no calculation. Check that the study produces at least one calculation. |
| `rustiq::run::portable_input_artifact` | `--artifact` was used with a portable input. Omit the option for this invocation. |
| `rustiq::run::portable_input_cache_dir` | `--cache-dir` was used with a portable input. Omit the option for this invocation. |
| `rustiq::run::portable_input_download` | A basis auto-download flag was used with a portable input. Omit the flag for this invocation. |
| `rustiq::run::portable_input_reuse` | `--reuse` was used with a portable input that already supplies its reusable artifacts. Omit the option. |
| `rustiq::run::portable_prepare_failed` | A calculation could not be reconstructed from a bundle. Check the cause and inspect or verify the bundle. |
| `rustiq::run::calculation_failed` | An individual calculation failed during input conversion, rendering, or artifact preparation or retention. Check the cause for the affected operation. |
| `rustiq::run::non_finite_result` | A calculation result contains a non-finite number. Inspect the calculation's inputs and numerical settings before using the result. |
| `rustiq::run::incomplete_batch` | At least one calculation failed or did not converge. Inspect the per-calculation outcomes. |
| `rustiq::run::report_write_failed` | RustiQ could not write calculation progress or a text report. Check the output destination or downstream pipe. |
| `rustiq::run::failed` | Run orchestration or report output failed. Read the cause for the specific operation. |
| `rustiq::calculation::basis` | The basis could not be applied to the geometry. Check element coverage and the reported basis data. |
| `rustiq::calculation::molecule` | The molecular configuration is invalid. Check geometry, charge, and spin multiplicity. |
| `rustiq::calculation::hf_method` | The requested HF reference could not be resolved for the molecule. Check the HF method and spin multiplicity. |
| `rustiq::calculation::hf_setup` | HF preparation failed. Check the cited SCF setting, input geometry, or basis. |
| `rustiq::calculation::numerical` | A numerical operation failed during the calculation. Use the underlying message to identify the affected operation. |
| `rustiq::calculation::diis` | The direct inversion in the iterative subspace (DIIS) acceleration step failed. Inspect the SCF diagnostics and DIIS settings. |
| `rustiq::calculation::artifact` | A calculation could not use an artifact. Check the cause and verify the input bundle if applicable. |
| `rustiq::calculation::mp2` | MP2 preparation or execution failed. Check the cause, including any cited frozen orbitals or workspace settings. |
| `rustiq::scf::non_convergence` | HF reached an unconverged outcome. Inspect the SCF history and convergence settings before using the orbitals for MP2. |

### Portable bundles and Post-HF

| Code | Meaning and what to check |
| --- | --- |
| `rustiq::artifact::open_failed` | RustiQ could not open or read the bundle for the operation. Check the path, permissions, and archive or manifest error. |
| `rustiq::artifact::calculation_out_of_range` | The selected calculation index is absent. Use an index shown by `artifact inspect`. |
| `rustiq::artifact::calculation_required` | The bundle has multiple calculations and the operation needs one. Supply `--calculation`. |
| `rustiq::artifact::destination_exists` | The artifact destination already exists. Choose a different path or explicitly allow replacement. |
| `rustiq::artifact::export_missing_orbitals` | The selected calculation lacks the orbitals needed for export. Use a bundle with a converged HF solution. |
| `rustiq::artifact::export_output_exists` | The export destination already exists. Choose a different path or use `--force`. |
| `rustiq::artifact::export_output_is_input` | The export path resolves to the input bundle. Choose a different destination. |
| `rustiq::artifact::export_publish_failed` | RustiQ could not publish the export file. Check the destination and directory permissions. |
| `rustiq::artifact::failed` | Bundle inspection, verification, or export failed. Read the cause for the affected operation. |
| `rustiq::artifact::missing_request` | The bundle calculation lacks the stored request needed for inspection. Use a bundle containing the request. |
| `rustiq::artifact::verification_batch_failed` | Verification found invalid artifacts. Inspect the individual verification errors. |
| `rustiq::artifact::verification_failed` | An individual artifact failed verification. Check the cause for the affected bundle entry. |
| `rustiq::artifact::verification_incomplete` | Strict verification skipped at least one artifact. Review the skipped entries and their representations. |
| `rustiq::artifact::verification_skipped` | An artifact name or representation is unsupported. Check the entry and use a RustiQ build that supports it if available. |
| `rustiq::post_scf::calculation_out_of_range` | The selected Post-HF calculation index is absent. Inspect the bundle for valid indices. |
| `rustiq::post_scf::config_read_failed` | The Post-HF TOML could not be read. Check its path, permissions, and UTF-8 encoding. |
| `rustiq::post_scf::input_required` | The operation requires a portable bundle as input. Supply the input bundle path. |
| `rustiq::post_scf::method_not_configured` | The selected method has no configuration in the Post-HF TOML. Add the named method table. |
| `rustiq::post_scf::method_unavailable` | The requested Post-HF method is unavailable. Choose a supported method. |
| `rustiq::post_scf::no_methods` | No Post-HF method was selected from the configuration. Add or select a supported method. |
| `rustiq::post_scf::no_successful_calculations` | Every selected Post-HF calculation failed or was skipped. Read the individual outcomes. |
| `rustiq::post_scf::output_is_input` | The Post-HF output resolves to the input bundle. Choose a different path. |
| `rustiq::post_scf::restore_failed` | The HF reference could not be restored. Check the cause and verify the source artifact. |
| `rustiq::post_scf::skipped` | Post-HF execution was skipped for a selected calculation. Read the reason and help in the diagnostic. |
| `rustiq::post_scf::failed` | A Post-HF calculation failed without a more specific identifier. Inspect the underlying error and method configuration. |
| `rustiq::mp2::non_finite_energy` | MP2 returned a non-finite energy on a restored reference. Inspect the reference and MP2 settings before using any reported energy. |

For contributors, see [how diagnostics are implemented](../development/contributing.md#miette-diagnostics).
