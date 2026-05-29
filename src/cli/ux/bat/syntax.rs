use std::{path::PathBuf, sync::OnceLock};

use syntect::parsing::SyntaxSet;

static SYNTAX_SET: OnceLock<Option<SyntaxSet>> = OnceLock::new();

pub fn syntax_set() -> Option<&'static SyntaxSet> {
    SYNTAX_SET
        .get_or_init(|| SyntaxSet::load_from_folder(syntax_dir()).ok())
        .as_ref()
}

fn syntax_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/syntaxes")
}
