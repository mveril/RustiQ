use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::cli::color::{self, OutputStream};
use ::bat::{
    assets::HighlightingAssets, config::Config, controller::Controller, Input, PrettyPrinter,
};
use miette::IntoDiagnostic;

const BAT_SYNTAXES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bat-assets/syntaxes.bin"));
const BAT_THEMES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bat-assets/themes.bin"));

static BAT_ASSET_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn print_toml(content: &str) {
    if PrettyPrinter::new()
        .colored_output(color::enabled_for(OutputStream::Stdout))
        .input_from_bytes(content.as_bytes())
        .paging_mode(::bat::PagingMode::Never)
        .language("toml")
        .print()
        .is_err()
    {
        println!("{content}");
    }
}

pub(crate) fn print_json(content: &[u8]) -> miette::Result<()> {
    PrettyPrinter::new()
        .colored_output(color::enabled_for(OutputStream::Stdout))
        .input_from_bytes(content)
        .paging_mode(::bat::PagingMode::Never)
        .language("json")
        .print()
        .into_diagnostic()
        .map(|_| ())
}

pub(crate) fn print_xyz(content: &str) {
    if !color::enabled_for(OutputStream::Stdout) {
        print!("{content}");
        return;
    }

    if print_highlighted_xyz(content).is_err() {
        print!("{content}");
    }
}

fn print_highlighted_xyz(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    let assets = HighlightingAssets::from_cache(bat_asset_cache_dir()?)?;
    let config = Config {
        language: Some("xyz"),
        colored_output: color::enabled_for(OutputStream::Stdout),
        true_color: color::enabled_for(OutputStream::Stdout),
        paging_mode: ::bat::PagingMode::Never,
        ..Default::default()
    };
    let controller = Controller::new(&config, &assets);
    controller.run(vec![Input::from_reader(content.as_bytes()).into()], None)?;
    Ok(())
}

fn bat_asset_cache_dir() -> io::Result<&'static Path> {
    let dir = BAT_ASSET_CACHE_DIR.get_or_init(|| {
        env::temp_dir()
            .join("rustiq")
            .join("bat-assets")
            .join(env!("CARGO_PKG_VERSION"))
    });

    fs::create_dir_all(dir)?;
    write_asset_if_needed(&dir.join("syntaxes.bin"), BAT_SYNTAXES)?;
    write_asset_if_needed(&dir.join("themes.bin"), BAT_THEMES)?;
    Ok(dir.as_path())
}

fn write_asset_if_needed(path: &Path, contents: &[u8]) -> io::Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    fs::write(path, contents)
}
