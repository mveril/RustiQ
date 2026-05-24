use clap::ValueEnum;
#[derive(ValueEnum, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CenterType {
    /// Center of mass.
    Mass,
    /// Geometric center.
    Geometry,
    /// Nuclear-charge weighted center.
    Charge,
}
