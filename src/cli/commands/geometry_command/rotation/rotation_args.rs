use clap::Args;
use std::{num::ParseFloatError, ops::Deref, str::FromStr};

use nalgebra::{Rotation3, Unit, Vector3};
use thiserror::Error;

#[derive(Args, Debug, Clone, Copy)]
pub struct RotationArgs {
    /// Rotation angle in degrees.
    #[arg(long)]
    pub angle: f64,
    /// Rotation axis: x, y, z, or a comma-separated vector such as 1,0,0.
    #[arg(long)]
    pub axis: RotationAxis,
}

impl From<RotationArgs> for Rotation3<f64> {
    fn from(value: RotationArgs) -> Self {
        Rotation3::from_axis_angle(&value.axis.into_inner(), value.angle.to_radians())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationAxis {
    axis: Unit<Vector3<f64>>,
}

impl RotationAxis {
    fn into_inner(self) -> Unit<Vector3<f64>> {
        self.axis
    }
}

impl Deref for RotationAxis {
    type Target = Unit<Vector3<f64>>;
    fn deref(&self) -> &Self::Target {
        &self.axis
    }
}

impl AsRef<Unit<Vector3<f64>>> for RotationAxis {
    fn as_ref(&self) -> &Unit<Vector3<f64>> {
        &self.axis
    }
}

impl From<Unit<Vector3<f64>>> for RotationAxis {
    fn from(axis: Unit<Vector3<f64>>) -> Self {
        Self { axis }
    }
}

impl FromStr for RotationAxis {
    type Err = RotationAxisParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let axis = match s.to_lowercase().as_str() {
            "x" => Vector3::x_axis(),
            "y" => Vector3::y_axis(),
            "z" => Vector3::z_axis(),
            _ => {
                let coords: Vec<f64> = s
                    .split(',')
                    .map(|part| {
                        part.trim().parse().map_err(|source| {
                            RotationAxisParseError::InvalidCoordinate {
                                input: s.to_owned(),
                                source,
                            }
                        })
                    })
                    .collect::<Result<_, _>>()?;
                if coords.len() != 3 {
                    return Err(RotationAxisParseError::InvalidCoordinateCount {
                        input: s.to_owned(),
                    });
                }
                Unit::try_new(Vector3::from_row_slice(&coords), f64::EPSILON).ok_or_else(|| {
                    RotationAxisParseError::ZeroVector {
                        input: s.to_owned(),
                    }
                })?
            }
        };
        Ok(axis.into())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RotationAxisParseError {
    #[error("invalid rotation axis '{input}': expected x, y, z, or three comma-separated numbers")]
    InvalidCoordinateCount { input: String },
    #[error("invalid rotation axis coordinate in '{input}'")]
    InvalidCoordinate {
        input: String,
        #[source]
        source: ParseFloatError,
    },
    #[error("invalid zero rotation axis: {input}")]
    ZeroVector { input: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case("x", Vector3::x_axis())]
    #[case("y", Vector3::y_axis())]
    #[case("z", Vector3::z_axis())]
    #[case("X", Vector3::x_axis())]
    #[case("Y", Vector3::y_axis())]
    #[case("Z", Vector3::z_axis())]
    #[case("1,0,0", Vector3::x_axis())]
    #[case("0,1,0", Vector3::y_axis())]
    #[case("0,0,1", Vector3::z_axis())]
    fn test_rotation_axis_from_str(#[case] input: &str, #[case] expected: Unit<Vector3<f64>>) {
        let axis = RotationAxis::from_str(input).unwrap();
        assert_eq!(*axis, expected);
    }

    #[rstest]
    #[case("1,2")]
    #[case("1,2,3,4")]
    fn test_rotation_axis_rejects_invalid_coordinate_count(#[case] input: &str) {
        let result = RotationAxis::from_str(input);
        assert_eq!(
            result,
            Err(RotationAxisParseError::InvalidCoordinateCount {
                input: input.to_owned()
            })
        );
    }

    #[test]
    fn test_rotation_axis_rejects_zero_axis() {
        let input = "0,0,0";
        let result = RotationAxis::from_str(input);
        assert_eq!(
            result,
            Err(RotationAxisParseError::ZeroVector {
                input: input.to_owned()
            })
        );
    }

    #[rstest]
    #[case("1,a,0")]
    #[case("1,a,0,0")]
    fn test_rotation_axis_rejects_invalid_coordinates(#[case] input: &str) {
        let result = RotationAxis::from_str(input);
        assert_eq!(
            result,
            Err(RotationAxisParseError::InvalidCoordinate {
                input: input.to_owned(),
                source: "a".parse::<f64>().unwrap_err(),
            })
        );
    }

    proptest! {
        #[test]
        fn test_rotation_axis_from_coordinates(
            x in -100.0f64..100.0f64,
            y in -100.0f64..100.0f64,
            z in 1.0e-6f64..100.0f64,
        ) {
            let input = format!("{},{},{}", x, y, z);
            let axis = RotationAxis::from_str(&input).unwrap();
            let expected = Unit::new_normalize(Vector3::new(x, y, z));
            prop_assert_eq!(*axis, expected);
        }
    }
}
