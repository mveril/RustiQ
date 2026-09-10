//! TOML conversion for the scientific unit enum, owned by the frontend.
use rustiq_core::molecules::units::Units;
use toml_spanner::{Arena, Context, Failed, FromToml, Item, ToToml, ToTomlError, Toml};

#[derive(Toml)]
#[toml(Toml)]
enum UnitsRepr {
    Bohr,
    Angstrom,
}

pub(crate) fn from_toml<'de>(ctx: &mut Context<'de>, item: &Item<'de>) -> Result<Units, Failed> {
    Ok(match UnitsRepr::from_toml(ctx, item)? {
        UnitsRepr::Bohr => Units::Bohr,
        UnitsRepr::Angstrom => Units::Angstrom,
    })
}

pub(crate) fn to_toml<'a>(value: &'a Units, arena: &'a Arena) -> Result<Item<'a>, ToTomlError> {
    match value {
        Units::Bohr => UnitsRepr::Bohr.to_toml(arena),
        Units::Angstrom => UnitsRepr::Angstrom.to_toml(arena),
    }
}
