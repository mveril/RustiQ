//! Embedded schema verification during migration. Normal CLI runs use TOML only.
use super::resolved::ResolvedInput;

fn evaluate(expression: &str) -> Result<ResolvedInput, String> {
    // Inline the maintained schema so tests do not depend on an installed CLI
    // or runtime paths to package sources.
    let schema = include_str!("nickel/calculation.ncl");
    let rebuild = include_str!("nickel/rebuild-data.ncl");
    let resolve = include_str!("nickel/resolve.ncl")
        .replace("import \"calculation.ncl\"", &format!("({schema})"));
    let source = format!("let ResolveInput = ({resolve}) in let Rebuild = ({rebuild}) in ResolveInput (Rebuild ({expression}))");
    let mut context = nickel_lang::Context::new();
    let expr = context
        .eval_deep_for_export(&source)
        .map_err(|error| format_error(&error))?;
    let json = context
        .expr_to_json(&expr)
        .map_err(|error| format_error(&error))?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

fn format_error(error: &nickel_lang::Error) -> String {
    let mut output = Vec::new();
    match error.format(&mut output, nickel_lang::ErrorFormat::Text) {
        Ok(()) => String::from_utf8_lossy(&output).into_owned(),
        Err(error) => error.to_string(),
    }
}

fn collect_samples(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_samples(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            files.push(path);
        }
    }
}

#[test]
fn nickel_matches_every_valid_sample() {
    let mut files = Vec::new();
    collect_samples(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples"),
        &mut files,
    );
    let mut count = 0;
    for path in files {
        let source = std::fs::read_to_string(&path).unwrap();
        if path
            .file_name()
            .is_some_and(|name| name == "invalid_diagnostics.toml")
        {
            continue;
        }
        let parsed = super::parser::parse_runfile(path.display().to_string(), &source).unwrap();
        let expression = format!(
            "std.deserialize 'Toml {}",
            serde_json::to_string(&source).unwrap()
        );
        let nickel = evaluate(&expression).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(nickel, parsed.resolved, "{}", path.display());
        let calculation = &nickel.calculations()[0];
        assert_eq!(
            format!("{:?}", calculation.hf_config().unwrap()),
            format!("{:?}", {
                let mut hf = parsed.hf_config.unwrap();
                hf.method.span = None;
                hf.guess.span = None;
                hf.diis.max_history.span = None;
                hf.orthogonalization.linear_dependency_threshold.span = None;
                hf
            })
        );
        calculation.integral_config().unwrap();
        count += 1;
    }
    assert_eq!(count, 19, "all valid baseline samples must be compared");
}

#[test]
fn nickel_rejects_invalid_contracts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/nickel/invalid");
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let source = std::fs::read_to_string(&path).unwrap();
        let error = evaluate(&source).expect_err(&format!("{} must fail", path.display()));
        assert!(
            error.contains("contract") || error.contains("missing definition"),
            "{}: {error}",
            path.display()
        );
    }
}

#[test]
fn nickel_defaults_and_batches_are_resolved() {
    let result = evaluate(r#"[{ basis.name = "sto-3g", method.hf.guess = { type = "OneElectron", perturbation = {seed = 42} } }, { basis.name = "6-31g", method.mp2 = {} }]"#).unwrap();
    assert_eq!(result.calculations().len(), 2);
    assert_eq!(result.calculations()[0].basis.name, "sto-3g");
    assert_eq!(result.calculations()[1].basis.name, "6-31g");
    assert!(result.calculations()[0].method.mp2.is_none());
    assert!(result.calculations()[1].method.mp2.is_some());
    for calculation in result.calculations() {
        calculation.hf_config().unwrap();
    }
    assert!(evaluate("[]").is_err());
}

#[test]
fn rebuilt_nested_json_preserves_values_and_validation() {
    let valid = r#"[{"basis":{"name":"sto-3g"},"method":{"hf":{"diis":{"enabled":true,"max_history":8},"guess":{"type":"Random","distribution":"Normal","mean":0,"std_dev":1,"seed":42}}}}]"#;
    let input = evaluate(&format!(
        "std.deserialize 'Json {}",
        serde_json::to_string(valid).unwrap()
    ))
    .unwrap();
    assert!(input.calculations()[0].method.hf.diis.enabled);
    assert_eq!(input.calculations()[0].method.hf.diis.max_history, 8);
    input.calculations()[0].hf_config().unwrap();
    for invalid in [
        valid.replace("\"max_history\":8", "\"max_history\":\"wrong\""),
        valid.replace("\"enabled\":true", "\"enabled\":true,\"unknown\":1"),
    ] {
        assert!(evaluate(&format!(
            "std.deserialize 'Json {}",
            serde_json::to_string(&invalid).unwrap()
        ))
        .is_err());
    }
}

#[test]
fn nickel_matches_tagged_and_optional_toml_settings() {
    for settings in [
        "",
        "[method.mp2]\nmemory_limit = '513 B'\nfrozen_orbitals = 2\n",
        "[method.hf.guess]\ntype = 'Zero'\n",

        "[method.hf.guess]\ntype = 'CoreHamiltonian'\n[method.hf.guess.perturbation]\nseed = 42\n",
        "[method.hf.guess]\ntype = 'OneElectron'\n[method.hf.guess.perturbation]\ndistribution = 'Uniform'\nmin = -0.01\nmax = 0.02\n",
        "[method.hf.guess]\ntype = 'Random'\ndistribution = 'Normal'\nmean = 0.5\nstd_dev = 0.01\nseed = 42\n",
        "[integrals]\nschwarz_threshold = 0\n[cache]\nenabled = true\n[output]\nscf = 'Quiet'\n",
        "[molecule]\nunits = 'Bohr'\ncharge = -1\nmultiplicity = 2\n",
    ] {
        let source = format!("[basis]\nname = 'sto-3g'\n{settings}");
        let legacy = super::parser::parse_runfile("parity.toml", &source).unwrap();
        let nickel = evaluate(&format!("std.deserialize 'Toml {}", serde_json::to_string(&source).unwrap())).unwrap();
        assert_eq!(nickel, legacy.resolved, "{settings}");
        nickel.calculations()[0].hf_config().unwrap();
        nickel.calculations()[0].integral_config().unwrap();
    }
}

#[test]
fn nickel_rejects_nested_types_ranges_and_tags() {
    for field in [
        "molecule.geometry = 1",
        "molecule.charge = 2147483648",
        "molecule.multiplicity = 256",
        "molecule.units = \"Unknown\"",
        "basis.name = true",
        "method.hf.max_iterations = 0",
        "method.hf.max_iterations = 1.5",
        "method.hf.convergence_threshold = 0",
        "method.hf.orthogonalization.linear_dependency_threshold = -1",
        "method.mp2.frozen_orbitals = -1",
        "method.mp2.memory_limit = 512",
        "method.hf.diis.enabled = 1",
        "output.scf = \"unknown\"",
        "cache.extra = true",
        "method.hf.guess = {type = \"Random\", distribution = \"Unknown\"}",
    ] {
        assert!(
            evaluate(&format!("{{ basis.name = \"sto-3g\", {field} }}")).is_err(),
            "{field}"
        );
    }
}
