use clap::Args;
use nalgebra::Translation3;

#[derive(Args, Debug, Clone, Copy, Default)]
pub struct TranslationArgs {
    /// Translation along x, in the geometry internal unit.
    #[clap(long, short = 'x', default_value = "0.0")]
    pub dx: f64,
    /// Translation along y, in the geometry internal unit.
    #[clap(long, short = 'y', default_value = "0.0")]
    pub dy: f64,
    /// Translation along z, in the geometry internal unit.
    #[clap(long, short = 'z', default_value = "0.0")]
    pub dz: f64,
}

impl From<TranslationArgs> for Translation3<f64> {
    fn from(translation: TranslationArgs) -> Self {
        Self::new(translation.dx, translation.dy, translation.dz)
    }
}
