use periodic_table::{periodic_table, Element};

pub fn parse_element(e: &str) -> Result<&'static Element, ParseElementError> {
    match e.parse::<usize>() {
        Ok(z0) => {
            let zel = periodic_table()
                .get(z0 - 1)
                .ok_or(ParseElementError::ZNotFound(z0))
                .copied();
            zel
        }
        Err(_) => {
            let sel = periodic_table().iter().find(|x| x.symbol == e);
            sel.copied()
                .ok_or(ParseElementError::SymbolNotFound(e.to_string()))
        }
    }
}

use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum ParseElementError {
    ZNotFound(usize),
    SymbolNotFound(String),
}

impl Display for ParseElementError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ParseElementError::ZNotFound(z) => {
                write!(f, "Element with atomic number {} not found.", z)
            }
            ParseElementError::SymbolNotFound(symbol) => {
                write!(f, "Element with symbol {} not found", symbol)
            }
        }
    }
}

impl Error for ParseElementError {}
