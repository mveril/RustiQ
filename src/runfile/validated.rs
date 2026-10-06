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
    use super::{
        from_toml_via_try_from, Arena, Context, Failed, Item, PositiveFiniteF64, ToTomlError,
    };
    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<PositiveFiniteF64, Failed> {
        from_toml_via_try_from::<PositiveFiniteF64, f64>(ctx, item)
    }
    #[allow(
        clippy::unnecessary_wraps,
        reason = "The TOML adapter must match the fallible serializer callback signature"
    )]
    pub(crate) fn to_toml<'a>(
        value: &'a PositiveFiniteF64,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.into_inner()))
    }
}

pub(crate) mod non_negative_finite_f64 {
    use super::{
        from_toml_via_try_from, Arena, Context, Failed, Item, NonNegativeFiniteF64, ToTomlError,
    };
    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<NonNegativeFiniteF64, Failed> {
        from_toml_via_try_from::<NonNegativeFiniteF64, f64>(ctx, item)
    }
    #[allow(
        clippy::unnecessary_wraps,
        reason = "The TOML adapter must match the fallible serializer callback signature"
    )]
    pub(crate) fn to_toml<'a>(
        value: &'a NonNegativeFiniteF64,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.into_inner()))
    }
}

pub(crate) mod optional_positive_finite_f64 {
    use super::{Arena, Context, Failed, FromToml, Item, PositiveFiniteF64, ToTomlError};

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

    #[allow(
        clippy::unnecessary_wraps,
        reason = "The TOML adapter must match the fallible serializer callback signature"
    )]
    pub(crate) fn to_toml<'a, T: ToTomlThreshold>(
        value: &'a T,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.to_item()))
    }
}

pub(crate) mod diis_size {
    use super::{from_toml_via_try_from, Arena, Context, DiisSize, Failed, Item, ToTomlError};
    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<DiisSize, Failed> {
        from_toml_via_try_from::<DiisSize, usize>(ctx, item)
    }
    #[allow(
        clippy::unnecessary_wraps,
        reason = "The TOML adapter must match the fallible serializer callback signature"
    )]
    pub(crate) fn to_toml<'a>(
        value: &'a DiisSize,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.into_inner() as i128))
    }
}

pub(crate) mod non_zero_usize {
    use super::{from_toml_via_try_from, Arena, Context, Failed, Item, NonZeroUsize, ToTomlError};

    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<NonZeroUsize, Failed> {
        from_toml_via_try_from::<NonZeroUsize, usize>(ctx, item)
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "The TOML adapter must match the fallible serializer callback signature"
    )]
    pub(crate) fn to_toml<'a>(
        value: &'a NonZeroUsize,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(value.get() as i128))
    }
}

pub(crate) mod usize_as_integer {
    use super::{Arena, Context, Failed, FromToml, Item, ToTomlError};

    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<usize, Failed> {
        usize::from_toml(ctx, item)
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "The TOML adapter must match the fallible serializer callback signature"
    )]
    pub(crate) fn to_toml<'a>(
        value: &'a usize,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(*value as i128))
    }
}

pub(crate) mod non_zero_u8 {
    use super::{from_toml_via_try_from, Arena, Context, Failed, Item, NonZeroU8, ToTomlError};

    pub(crate) fn from_toml<'de>(
        ctx: &mut Context<'de>,
        item: &Item<'de>,
    ) -> Result<NonZeroU8, Failed> {
        from_toml_via_try_from::<NonZeroU8, u8>(ctx, item)
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "The TOML adapter must match the fallible serializer callback signature"
    )]
    pub(crate) fn to_toml<'a>(
        value: &'a NonZeroU8,
        _arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        Ok(Item::from(i128::from(value.get())))
    }
}

pub(crate) mod non_empty_path_buf {
    use super::{Arena, Context, Failed, FromToml, Item, PathBuf, ToToml, ToTomlError};

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

    #[allow(
        clippy::unnecessary_wraps,
        reason = "The TOML adapter must match the fallible serializer callback signature"
    )]
    pub(crate) fn to_toml<'a>(
        value: &'a PathBuf,
        arena: &'a Arena,
    ) -> Result<Item<'a>, ToTomlError> {
        value.to_toml(arena)
    }
}
