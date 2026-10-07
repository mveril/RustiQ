//! Embedded Nickel configuration frontend.
use super::resolved::ResolvedInput;
use nickel_lang_core::{
    cache::{CacheHub, InputFormat, SourcePath},
    error::{Error, IntoDiagnostics, NullReporter},
    eval::{
        cache::CacheImpl,
        value::{Container, NickelValue},
        VirtualMachine, VmContext,
    },
    files::FileId,
    identifier::Ident,
    term::Term,
};

struct Context {
    vm: VmContext<CacheHub, CacheImpl>,
    input_id: Option<FileId>,
}

impl Context {
    fn new() -> Self {
        Self {
            vm: VmContext::new(CacheHub::new(), std::io::sink(), NullReporter {}),
            input_id: None,
        }
    }

    fn eval(&mut self, source: &str, deep: bool) -> Result<NickelValue, Error> {
        let id = self.vm.import_resolver.sources.add_string(
            SourcePath::Path("<rustiq-schema>".into(), InputFormat::Nickel),
            source.to_owned(),
        );
        let value = self.vm.prepare_eval(id)?;
        let mut vm = VirtualMachine::new(&mut self.vm);
        if deep {
            Ok(vm.eval_full_for_export(value)?)
        } else {
            Ok(vm.eval(value)?)
        }
    }

    fn configuration_error(&self, path: Option<String>, error: Error) -> ConfigurationError {
        let mut files = self.vm.import_resolver.sources.files().clone();
        let mut details = Vec::new();
        let mut messages = Vec::new();
        let mut span = None;
        for diagnostic in error.into_diagnostics(&mut files) {
            messages.push(diagnostic.message.clone());
            let mut sources = Vec::new();
            for label in &diagnostic.labels {
                if !sources.contains(&label.file_id) {
                    sources.push(label.file_id);
                }
                if Some(label.file_id) == self.input_id && span.is_none() {
                    span = Some((label.range.start, label.range.len()).into());
                }
            }
            if sources.is_empty() {
                details.push(super::diagnostics::NickelDiagnosticDetail {
                    message: diagnostic.message.clone(),
                    source_code: None,
                    labels: Vec::new(),
                    notes: (!diagnostic.notes.is_empty()).then(|| diagnostic.notes.join("\n")),
                });
            }
            for id in sources {
                let labels = diagnostic
                    .labels
                    .iter()
                    .filter(|label| label.file_id == id)
                    .map(|label| {
                        miette::LabeledSpan::new(
                            Some(label.message.clone()),
                            label.range.start,
                            label.range.len(),
                        )
                    })
                    .collect();
                details.push(super::diagnostics::NickelDiagnosticDetail {
                    message: diagnostic.message.clone(),
                    source_code: Some(miette::NamedSource::new(
                        files.name(id).to_string_lossy(),
                        files.source(id).to_owned(),
                    )),
                    labels,
                    notes: (!diagnostic.notes.is_empty()).then(|| diagnostic.notes.join("\n")),
                });
            }
        }
        ConfigurationError {
            kind: ConfigurationErrorKind::Contract,
            path,
            message: messages.join("\n"),
            span,
            details,
        }
    }
}

pub(crate) struct ConfigurationError {
    pub(crate) kind: ConfigurationErrorKind,
    pub(crate) path: Option<String>,
    pub(crate) message: String,
    pub(crate) span: Option<miette::SourceSpan>,
    pub(crate) details: Vec<super::diagnostics::NickelDiagnosticDetail>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigurationErrorKind {
    TomlSyntax,
    Contract,
    Domain,
}

fn evaluate_input_with_context(
    expression: &str,
    mut context: Context,
) -> Result<ResolvedInput, Vec<ConfigurationError>> {
    // Inline the maintained schema so evaluation does not depend on an
    // installed Nickel CLI or runtime paths to package sources.
    let schema = include_str!("nickel/calculation.ncl");
    let rebuild = include_str!("nickel/rebuild-data.ncl");
    let resolve = include_str!("nickel/resolve.ncl")
        .replace("import \"calculation.ncl\"", &format!("({schema})"));
    let source = format!("let ResolveInput = ({resolve}) in let Rebuild = ({rebuild}) in ResolveInput (Rebuild ({expression}))");
    let expr = context.eval(&source, true).map_err(|error| {
        // Contracts on record fields are delayed. Force each sibling separately
        // after a failed export so one invalid field does not hide other errors.
        let original = context.configuration_error(None, error);
        let mut errors = Vec::new();
        collect_errors(&mut context, &source, &mut Vec::new(), &mut errors);
        if errors.is_empty() {
            errors.push(original);
        }
        errors
    })?;
    let json = nickel_lang_core::serialize::to_string(
        nickel_lang_core::serialize::ExportFormat::Json,
        &expr,
    )
    .map_err(|error| {
        vec![context.configuration_error(
            None,
            error.with_pos_table(context.vm.pos_table.clone()).into(),
        )]
    })?;
    serde_json::from_str(&json).map_err(|error| {
        let message = error.to_string();
        let path = (message.contains("MP2 memory limit") || message.contains("couldn't parse"))
            .then(|| "method.mp2.memory_limit".to_owned());
        vec![ConfigurationError {
            kind: ConfigurationErrorKind::Domain,
            path,
            message,
            span: None,
            details: Vec::new(),
        }]
    })
}

/// Nickel is also the TOML exporter, so no separate configuration parser is linked.
pub(crate) fn export_toml(json: &str) -> Result<String, super::output::RenderError> {
    let mut context = Context::new();
    let rebuild = include_str!("nickel/rebuild-data.ncl");
    let source = format!(
        "({rebuild}) (std.deserialize 'Json {})",
        serde_json::to_string(json).expect("serializing a string cannot fail")
    );
    let value = context.eval(&source, true).map_err(|error| {
        super::output::RenderError(context.configuration_error(None, error).message)
    })?;
    let export = |value: &NickelValue| {
        nickel_lang_core::serialize::to_string(
            nickel_lang_core::serialize::ExportFormat::Toml,
            value,
        )
        .map_err(|error| {
            super::output::RenderError(
                context
                    .configuration_error(
                        None,
                        error.with_pos_table(context.vm.pos_table.clone()).into(),
                    )
                    .message,
            )
        })
    };
    // Preserve the CLI's section order using Nickel values, without reparsing output.
    if let Some(record) = value.as_record().and_then(Container::into_opt) {
        let sections = [
            "molecule",
            "basis",
            "method",
            "integrals",
            "cache",
            "output",
        ];
        if record.fields.keys().any(|key| key.label() == "basis")
            && record
                .fields
                .keys()
                .all(|key| sections.contains(&key.label()))
        {
            let mut output = String::new();
            for section_name in sections {
                let mut section = record.clone();
                section.fields.retain(|key, _| key.label() == section_name);
                if section.fields.is_empty() {
                    continue;
                }
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&export(&NickelValue::record_posless(section))?);
            }
            return Ok(output);
        }
    }
    export(&value)
}

pub(crate) fn resolve_toml(toml: &str) -> Result<ResolvedInput, Vec<ConfigurationError>> {
    let (context, _) = toml_context("<input.toml>", toml)?;
    evaluate_input_with_context("Input", context)
}

fn toml_context(
    source_name: &str,
    toml: &str,
) -> Result<(Context, super::source_map::TomlSourceMap), Vec<ConfigurationError>> {
    // Nickel 0.18's TOML importer cannot represent TOML's non-finite numbers.
    // This guard covers every internal import entry point; Nickel remains
    // authoritative for configuration structure and validation.
    if let Ok(document) = toml.parse::<toml_edit::DocumentMut>() {
        if let Some((path, span)) = non_finite_value(&document, &mut Vec::new()) {
            return Err(vec![ConfigurationError {
                kind: ConfigurationErrorKind::Domain,
                path: Some(path.join(".")),
                message: "non-finite TOML numbers are unsupported".to_owned(),
                span,
                details: Vec::new(),
            }]);
        }
    }

    let mut context = Context::new();
    let id = context.vm.import_resolver.sources.add_string(
        SourcePath::Path(source_name.into(), InputFormat::Toml),
        toml.to_owned(),
    );
    context.input_id = Some(id);
    if let Err(error) =
        context
            .vm
            .import_resolver
            .parse_to_term(&mut context.vm.pos_table, id, InputFormat::Toml)
    {
        let mut error = context.configuration_error(None, error.into());
        error.kind = ConfigurationErrorKind::TomlSyntax;
        return Err(vec![error]);
    }
    let value = context
        .vm
        .import_resolver
        .terms
        .get_owned(id)
        .expect("parsed input is cached");
    let locations = super::source_map::TomlSourceMap::from_value(&value, &context.vm.pos_table);
    context.vm = context.vm.with_extend_env(vec![(
        Ident::new("Input"),
        NickelValue::term_posless(Term::ResolvedImport(id)),
    )]);
    Ok((context, locations))
}

pub(crate) fn resolve_toml_with_locations(
    source_name: &str,
    toml: &str,
) -> (
    Result<ResolvedInput, Vec<ConfigurationError>>,
    Option<super::source_map::TomlSourceMap>,
) {
    match toml_context(source_name, toml) {
        Ok((context, locations)) => (
            evaluate_input_with_context("Input", context),
            Some(locations),
        ),
        Err(errors) => (Err(errors), None),
    }
}

fn non_finite_value(
    table: &toml_edit::Table,
    path: &mut Vec<String>,
) -> Option<(Vec<String>, Option<miette::SourceSpan>)> {
    for (key, item) in table {
        path.push(key.to_owned());
        let found = match item {
            toml_edit::Item::Value(value) => non_finite_item_value(value, path).or_else(|| {
                (matches!(value, toml_edit::Value::Float(number) if !number.value().is_finite()))
                    .then(|| {
                        (
                            path.clone(),
                            item.span().map(|span| (span.start, span.len()).into()),
                        )
                    })
            }),
            toml_edit::Item::Table(table) => non_finite_value(table, path),
            toml_edit::Item::ArrayOfTables(tables) => tables
                .iter()
                .find_map(|table| non_finite_value(table, path)),
            toml_edit::Item::None => None,
        };
        path.pop();
        if found.is_some() {
            return found;
        }
    }
    None
}

fn non_finite_item_value(
    value: &toml_edit::Value,
    path: &mut Vec<String>,
) -> Option<(Vec<String>, Option<miette::SourceSpan>)> {
    if matches!(value, toml_edit::Value::Float(number) if !number.value().is_finite()) {
        return Some((
            path.clone(),
            value.span().map(|span| (span.start, span.len()).into()),
        ));
    }
    match value {
        toml_edit::Value::Array(array) => array
            .iter()
            .find_map(|value| non_finite_item_value(value, path)),
        toml_edit::Value::InlineTable(table) => {
            for (key, value) in table {
                path.push(key.to_owned());
                let found = non_finite_item_value(value, path);
                path.pop();
                if found.is_some() {
                    return found;
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn source_locations(
    toml: &str,
) -> Result<super::source_map::TomlSourceMap, Vec<ConfigurationError>> {
    toml_context("<input.toml>", toml).map(|(_, locations)| locations)
}

#[cfg(test)]
fn evaluate_input(expression: &str) -> Result<ResolvedInput, Vec<ConfigurationError>> {
    evaluate_input_with_context(expression, Context::new())
}

fn collect_errors(
    context: &mut Context,
    expression: &str,
    path: &mut Vec<String>,
    errors: &mut Vec<ConfigurationError>,
) {
    match context.eval(expression, false) {
        Ok(value) => {
            if path
                .iter()
                .map(String::as_str)
                .eq(["calculations", "method", "mp2", "memory_limit"])
            {
                if let Some(text) = value.as_string().map(|text| text.as_str()) {
                    if let Err(message) = crate::config::MemoryLimit::parse(text) {
                        errors.push(ConfigurationError {
                            kind: ConfigurationErrorKind::Domain,
                            path: Some("method.mp2.memory_limit".to_owned()),
                            message,
                            span: None,
                            details: Vec::new(),
                        });
                    }
                }
            }
            if let Some(record) = value.as_record().and_then(Container::into_opt) {
                for name in record.fields.keys() {
                    let name = name.label();
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
            errors.push(context.configuration_error(
                (!configuration_path.is_empty()).then(|| configuration_path.join(".")),
                error,
            ));
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

#[test]
fn native_contract_notes_are_preserved() {
    let mut context = Context::new();
    let error = context.eval(r#"0 | std.contract.custom (fun _ _ => 'Error { message = "invalid setting", notes = ["native contract advice"] })"#, true).unwrap_err();
    let error = context.configuration_error(None, error);
    assert!(error.details.iter().any(|detail| detail
        .notes
        .as_deref()
        .is_some_and(|notes| notes.contains("native contract advice"))));
}
