mod tests {
    use rustiq_core::runfile::RunFile;
    use std::{fs, path::Path};

    fn collect_toml_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_toml_files(&path, files);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                files.push(path);
            }
        }
    }

    #[test]
    fn test_sample_runfiles_parse_with_toml_spanner() {
        let mut files = Vec::new();
        collect_toml_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("samples"),
            &mut files,
        );
        files.retain(|path| {
            path.file_name()
                .is_none_or(|name| name != "invalid_diagnostics.toml")
        });
        assert!(!files.is_empty());

        for path in files {
            let content = fs::read_to_string(&path).unwrap();
            toml_spanner::from_str::<RunFile>(&content)
                .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
        }
    }
}
