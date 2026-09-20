pub use rustiq_core::config::validated::{DiisSize, NonNegativeFiniteF64, PositiveFiniteF64};
use std::{
    num::{NonZeroU8, NonZeroUsize},
    path::PathBuf,
};
use toml_spanner::{Arena, Context, Failed, FromToml, Item, ToToml, ToTomlError};

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

pub(crate) mod positive_finite_f64 {
    use super::*;
    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<PositiveFiniteF64, Failed> {
        from_toml_via_try_from::<PositiveFiniteF64, f64>(ctx, item)
    }
    pub(crate) fn to_toml<'a>(
        value: &'a PositiveFiniteF64,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.into_inner()))
    }
}

pub(crate) mod non_negative_finite_f64 {
    use super::*;
    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<NonNegativeFiniteF64, Failed> {
        from_toml_via_try_from::<NonNegativeFiniteF64, f64>(ctx, item)
    }
    pub(crate) fn to_toml<'a>(
        value: &'a NonNegativeFiniteF64,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.into_inner()))
    }
}

pub(crate) mod optional_positive_finite_f64 {
    use super::*;

    pub(crate) trait ToTomlThreshold {
        fn to_item(&self) -> f64;
    }

    impl ToTomlThreshold for PositiveFiniteF64 {
        fn to_item(&self) -> f64 {
            self.into_inner()
        }
    }

    impl ToTomlThreshold for Option<PositiveFiniteF64> {
        fn to_item(&self) -> f64 {
            self.map_or(0.0, PositiveFiniteF64::into_inner)
        }
    }

    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<Option<PositiveFiniteF64>, Failed> {
        let value = f64::from_toml(ctx, item)?;
        if value == 0.0 {
            Ok(None)
        } else {
            PositiveFiniteF64::try_new(value)
                .map(Some)
                .map_err(|error| ctx.report_custom_error(error, item))
        }
    }

    pub(crate) fn to_toml<'a, T: ToTomlThreshold>(
        value: &'a T,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.to_item()))
    }
}

pub(crate) mod diis_size {
    use super::*;
    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<DiisSize, Failed> {
        from_toml_via_try_from::<DiisSize, usize>(ctx, item)
    }
    pub(crate) fn to_toml<'a>(
        value: &'a DiisSize,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.into_inner() as i128))
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

pub(crate) mod usize_as_integer {
    use super::*;

    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<usize, Failed> {
        usize::from_toml(ctx, item)
    }

    pub(crate) fn to_toml<'a>(value: &'a usize, arena: &'a Arena) -> Result<Item<'a>, ToTomlError> {
        let _ = arena;
        Ok(Item::from(*value as i128))
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
