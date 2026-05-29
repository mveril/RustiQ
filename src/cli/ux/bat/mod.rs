mod syntax;

use std::io::{self, IsTerminal, Write};

use ::bat::PrettyPrinter;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

pub fn print_toml(content: &str) {
    if PrettyPrinter::new()
        .input_from_bytes(content.as_bytes())
        .paging_mode(::bat::PagingMode::Never)
        .language("toml")
        .print()
        .is_err()
    {
        println!("{content}");
    }
}

pub fn print_paged(content: &str) {
    if PrettyPrinter::new()
        .colored_output(false)
        .strip_ansi(::bat::StripAnsiMode::Never)
        .input_from_bytes(content.as_bytes())
        .paging_mode(::bat::PagingMode::QuitIfOneScreen)
        .print()
        .is_err()
    {
        println!("{content}");
    }
}

pub fn print_xyz(content: &str) {
    if !io::stdout().is_terminal() {
        print!("{content}");
        return;
    }

    if print_highlighted_xyz(content).is_err() {
        print!("{content}");
    }
}

fn print_highlighted_xyz(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    let syntax_set = syntax::syntax_set().ok_or("failed to load XYZ syntax")?;
    let syntax = syntax_set
        .find_syntax_by_extension("xyz")
        .ok_or("missing XYZ syntax")?;
    let themes = ThemeSet::load_defaults();
    let theme = &themes.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut stdout = io::stdout().lock();

    for line in LinesWithEndings::from(content) {
        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(line, syntax_set)?;
        write!(stdout, "{}", as_24_bit_terminal_escaped(&ranges[..], false))?;
    }

    Ok(())
}
