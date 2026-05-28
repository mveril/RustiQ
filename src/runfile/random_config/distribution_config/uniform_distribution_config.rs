use rand::distr::uniform::{Error, Uniform};
use toml_spanner::{Context, Failed, FromToml, Item, Toml};

#[derive(Debug, Clone, Copy, Toml)]
#[toml(ToToml)]
pub(crate) struct UniformDistributionConfig {
    pub min: f64,
    pub max: f64,
}

impl UniformDistributionConfig {
    pub(crate) fn new<'de>(
        ctx: &mut Context<'de>,
        min_item: &Item<'de>,
        max_item: &Item<'de>,
        min: f64,
        max: f64,
    ) -> Result<Self, Failed> {
        if !min.is_finite() {
            return Err(
                ctx.report_error_at("uniform distribution min must be finite", min_item.span())
            );
        }
        if !max.is_finite() {
            return Err(
                ctx.report_error_at("uniform distribution max must be finite", max_item.span())
            );
        }
        if Uniform::new(min, max).is_err() {
            return Err(ctx.report_error_at(
                "uniform distribution max must be greater than min",
                max_item.span(),
            ));
        }

        Ok(Self { min, max })
    }
}

impl<'de> FromToml<'de> for UniformDistributionConfig {
    fn from_toml(ctx: &mut Context<'de>, item: &Item<'de>) -> Result<Self, Failed> {
        let mut table = item.table_helper(ctx)?;
        let min_item = table.required_item("min")?;
        let max_item = table.required_item("max")?;
        let min = f64::from_toml(table.ctx, min_item)?;
        let max = f64::from_toml(table.ctx, max_item)?;
        table.require_empty()?;

        Self::new(ctx, min_item, max_item, min, max)
    }
}

impl TryFrom<UniformDistributionConfig> for Uniform<f64> {
    type Error = Error;

    fn try_from(value: UniformDistributionConfig) -> Result<Self, Self::Error> {
        Uniform::new(value.min, value.max)
    }
}
