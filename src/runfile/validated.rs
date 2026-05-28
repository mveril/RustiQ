use nutype::nutype;
use std::{
    num::{NonZeroU8, NonZeroUsize},
    path::PathBuf,
};
use toml_spanner::{Arena, Context, Failed, FromToml, Item, ToToml, ToTomlError};

#[nutype(
    validate(finite, greater = 0.0),
    derive(Debug, Clone, Copy, PartialEq, PartialOrd, TryFrom, Into)
)]
pub(crate) struct PositiveFiniteF64(f64);

#[nutype(
    validate(greater = 1),
    derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFrom, Into)
)]
pub(crate) struct DiisSize(usize);

fn from_toml_via_try_from<'de, T, Raw>(
    ctx: &mut Context<'de>,
    item: &Item<'de>,
) -> Result<T, Failed>
where
    Raw: FromToml<'de>,
    T: TryFrom<Raw>,
    T::Error: ToString,
{
    let value = Raw::from_toml(ctx, item)?;
    T::try_from(value).map_err(|error| ctx.report_custom_error(error, item))
}

impl<'de> FromToml<'de> for PositiveFiniteF64 {
    fn from_toml(ctx: &mut Context<'de>, item: &Item<'de>) -> Result<Self, Failed> {
        from_toml_via_try_from::<Self, f64>(ctx, item)
    }
}

impl ToToml for PositiveFiniteF64 {
    fn to_toml<'a>(&'a self, _arena: &'a Arena) -> Result<Item<'a>, ToTomlError> {
        let value: f64 = (*self).into();
        Ok(Item::from(value))
    }
}

impl<'de> FromToml<'de> for DiisSize {
    fn from_toml(ctx: &mut Context<'de>, item: &Item<'de>) -> Result<Self, Failed> {
        from_toml_via_try_from::<Self, usize>(ctx, item)
    }
}

impl ToToml for DiisSize {
    fn to_toml<'a>(&'a self, _arena: &'a Arena) -> Result<Item<'a>, ToTomlError> {
        let value: usize = (*self).into();
        Ok(Item::from(value as i128))
    }
}

pub(crate) mod non_zero_usize {
    use super::*;

    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<NonZeroUsize, Failed> {
        from_toml_via_try_from::<NonZeroUsize, usize>(ctx, item)
    }

    pub(crate) fn to_toml<'a>(
        value: &'a NonZeroUsize,
        arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        let _ = arena;
        Ok(Item::from(value.get() as i128))
    }
}

pub(crate) mod non_zero_u8 {
    use super::*;

    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<NonZeroU8, Failed> {
        from_toml_via_try_from::<NonZeroU8, u8>(ctx, item)
    }

    pub(crate) fn to_toml<'a>(
        value: &'a NonZeroU8,
        arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        let _ = arena;
        Ok(Item::from(value.get() as i128))
    }
}

pub(crate) mod non_empty_path_buf {
    use super::*;

    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<PathBuf, Failed> {
        let value = PathBuf::from_toml(ctx, item)?;
        if value.as_os_str().is_empty() {
            return Err(ctx.report_error_at("molecule geometry path cannot be empty", item.span()));
        }

        Ok(value)
    }

    pub(crate) fn to_toml<'a>(
        value: &'a PathBuf,
        arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        value.to_toml(arena)
    }
}
