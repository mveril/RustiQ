//! Embedded Nickel configuration frontend.
use super::resolved::ResolvedInput;

pub(crate) struct ConfigurationError {
    pub(crate) path: Option<String>,
    pub(crate) message: String,
}

fn evaluate_input(expression: &str) -> Result<ResolvedInput, Vec<ConfigurationError>> {
    // Inline the maintained schema so evaluation does not depend on an
    // installed Nickel CLI or runtime paths to package sources.
    let schema = include_str!("nickel/calculation.ncl");
    let rebuild = include_str!("nickel/rebuild-data.ncl");
    let resolve = include_str!("nickel/resolve.ncl")
        .replace("import \"calculation.ncl\"", &format!("({schema})"));
    let source = format!("let ResolveInput = ({resolve}) in let Rebuild = ({rebuild}) in ResolveInput (Rebuild ({expression}))");
    let mut context = nickel_lang::Context::new();
    let expr = context.eval_deep_for_export(&source).map_err(|error| {
        // Contracts on record fields are delayed. Force each sibling separately
        // after a failed export so one invalid field does not hide other errors.
        let original = format_error(&error);
        let mut diagnostics_context = nickel_lang::Context::new();
        let mut errors = Vec::new();
        collect_errors(
            &mut diagnostics_context,
            &source,
            &mut Vec::new(),
            &mut errors,
        );
        if errors.is_empty() {
            errors.push(ConfigurationError {
                path: None,
                message: original,
            });
        }
        errors
    })?;
    let json = context.expr_to_json(&expr).map_err(|error| {
        vec![ConfigurationError {
            path: None,
            message: format_error(&error),
        }]
    })?;
    serde_json::from_str(&json).map_err(|error| {
        let message = error.to_string();
        let path = (message.contains("MP2 memory limit") || message.contains("couldn't parse"))
            .then(|| "method.mp2.memory_limit".to_owned());
        vec![ConfigurationError { path, message }]
    })
}

pub(crate) fn resolve_toml(toml: &str) -> Result<ResolvedInput, Vec<ConfigurationError>> {
    let expression = format!(
        "std.deserialize 'Toml {}",
        serde_json::to_string(toml).expect("serializing a string cannot fail")
    );
    evaluate_input(&expression)
}

fn collect_errors(
    context: &mut nickel_lang::Context,
    expression: &str,
    path: &mut Vec<String>,
    errors: &mut Vec<ConfigurationError>,
) {
    match context.eval_shallow(expression) {
        Ok(value) => {
            if path
                .iter()
                .map(String::as_str)
                .eq(["calculations", "method", "mp2", "memory_limit"])
            {
                if let Some(text) = value.as_str() {
                    if let Err(message) = crate::config::MemoryLimit::parse(text) {
                        errors.push(ConfigurationError {
                            path: Some("method.mp2.memory_limit".to_owned()),
                            message,
                        });
                    }
                }
            }
            if let Some(record) = value.as_record() {
                for (name, _) in record.iter() {
                    let key =
                        serde_json::to_string(name).expect("serializing a string cannot fail");
                    path.push(name.to_owned());
                    collect_errors(context, &format!("({expression}).{key}"), path, errors);
                    path.pop();
                }
            } else if let Some(array) = value.as_array() {
                for index in 0..array.len() {
                    collect_errors(
                        context,
                        &format!("std.array.at {index} ({expression})"),
                        path,
                        errors,
                    );
                }
            }
        }
        Err(error) => {
            let configuration_path = path
                .strip_prefix(&["calculations".to_owned()])
                .unwrap_or(path);
            errors.push(ConfigurationError {
                path: (!configuration_path.is_empty()).then(|| configuration_path.join(".")),
                message: format_error(&error),
            });
        }
    }
}

#[cfg(test)]
fn evaluate(expression: &str) -> Result<ResolvedInput, String> {
    evaluate_input(expression).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn format_error(error: &nickel_lang::Error) -> String {
    let mut output = Vec::new();
    match error.format(&mut output, nickel_lang::ErrorFormat::Text) {
        Ok(()) => String::from_utf8_lossy(&output).into_owned(),
        Err(error) => error.to_string(),
    }
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
fn production_toml_frontend_accepts_tagged_and_optional_settings() {
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
        let parsed = super::parser::parse_runfile("parity.toml", &source).unwrap();
        parsed.hf_config.unwrap();
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
        "method.hf.guess = {type = \"Random\"}",
    ] {
        assert!(
            evaluate(&format!("{{ basis.name = \"sto-3g\", {field} }}")).is_err(),
            "{field}"
        );
    }
}
