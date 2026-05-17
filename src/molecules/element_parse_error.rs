use std::fmt::{self, Display, Formatter};
use std::error::Error;

#[derive(Debug)]
enum ParseElementError {
    ZNotFound(usize),
    SymbolNotFound(&'static [&'static str]),
}

impl Display for ParseElementError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ParseElementError::ZNotFound(z) => {
                write!(f, "Element with atomic number {} not found.", z)
            },
            ParseElementError::SymbolNotFound(symbols) => {
                let symbols_str = symbols.join(", ");
                write!(f, "Element with symbol {} not found", symbols_str)
            },
        }
    }
}

impl Error for ParseElementError {}
