use std::io::{Read, Write};

#[cfg(test)]
use nalgebra::DVector;
use nalgebra::{allocator::Allocator, DMatrix, DefaultAllocator, Dim, OMatrix};
use npyz::{NpyFile, WriterBuilder};

use super::{check_npy_shape, map_npy_value_read_error, NpyConvert, NpyError};

impl<Rows: Dim, Cols: Dim> NpyConvert for OMatrix<f64, Rows, Cols>
where
    DefaultAllocator: Allocator<Rows, Cols>,
{
    type Shape = (usize, usize);
    type NpyShape = [u64; 2];

    fn npy_shape((rows, columns): Self::Shape) -> Result<Self::NpyShape, NpyError> {
        Ok([
            u64::try_from(rows).map_err(|_| NpyError::DimensionOverflow)?,
            u64::try_from(columns).map_err(|_| NpyError::DimensionOverflow)?,
        ])
    }

    fn check_shape(npy: &NpyFile<impl Read>, shape: Self::Shape) -> Result<(), NpyError> {
        shape
            .0
            .checked_mul(shape.1)
            .ok_or(NpyError::DimensionOverflow)?;
        let expected = Self::npy_shape(shape)?;
        // Dim::try_to_usize inspects the type: Const<1> returns Some(1), while Dyn
        // returns None even when its runtime size is 1. This allows 1D NPY input
        // for Vector/RowVector types without relaxing shape checks for DMatrix.
        let vector = (Rows::try_to_usize() == Some(1) || Cols::try_to_usize() == Some(1))
            && (shape.0 == 1 || shape.1 == 1)
            && npy.shape() == [expected[0] * expected[1]];
        if !vector {
            check_npy_shape(&expected, npy.shape())?;
        }
        if Rows::try_to_usize().is_some_and(|n| n != shape.0)
            || Cols::try_to_usize().is_some_and(|n| n != shape.1)
        {
            return Err(NpyError::InvalidMatrixShape(expected.into()));
        }
        Ok(())
    }

    fn decode_with_shape<R: Read>(
        npy: NpyFile<R>,
        (rows, columns): Self::Shape,
    ) -> Result<Self, NpyError> {
        rows.checked_mul(columns)
            .ok_or(NpyError::DimensionOverflow)?;
        let order = npy.order();
        let values = npy.into_vec::<f64>().map_err(map_npy_value_read_error)?;
        let r = Rows::from_usize(rows);
        let c = Cols::from_usize(columns);
        if order == npyz::Order::C && rows > 1 && columns > 1 {
            Ok(Self::from_row_slice_generic(r, c, &values))
        } else {
            Ok(Self::from_vec_generic(r, c, values))
        }
    }

    fn write_npy(&self, writer: impl Write) -> Result<(), NpyError> {
        let shape = Self::npy_shape(self.shape())?;
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .order(npyz::Order::Fortran)
            .shape(&shape)
            .writer(writer)
            .begin_nd()
            .map_err(NpyError::Write)?;
        writer
            .extend(self.as_slice().iter().copied())
            .map_err(NpyError::Write)?;
        writer.finish().map_err(NpyError::Write)
    }
}

pub(crate) fn read_dmatrix(reader: impl Read) -> Result<DMatrix<f64>, NpyError> {
    decode_dmatrix(NpyFile::new(reader).map_err(NpyError::Read)?)
}

fn decode_dmatrix<R: Read>(npy: NpyFile<R>) -> Result<DMatrix<f64>, NpyError> {
    if npy.shape().len() != 2 {
        return Err(NpyError::InvalidMatrixShape(
            npy.shape().to_vec().into_boxed_slice(),
        ));
    }
    let rows = usize::try_from(npy.shape()[0])
        .map_err(|_| NpyError::InvalidMatrixShape(npy.shape().to_vec().into_boxed_slice()))?;
    let columns = usize::try_from(npy.shape()[1])
        .map_err(|_| NpyError::InvalidMatrixShape(npy.shape().to_vec().into_boxed_slice()))?;
    rows.checked_mul(columns)
        .ok_or_else(|| NpyError::InvalidMatrixShape(npy.shape().to_vec().into_boxed_slice()))?;
    let order = npy.order();
    let values = npy.into_vec::<f64>().map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            NpyError::InvalidDtype(error.to_string())
        } else {
            NpyError::Read(error)
        }
    })?;
    Ok(matrix_from_npy_values(rows, columns, order, values))
}

fn matrix_from_npy_values(
    rows: usize,
    columns: usize,
    order: npyz::Order,
    values: Vec<f64>,
) -> DMatrix<f64> {
    match order {
        npyz::Order::C => DMatrix::from_row_slice(rows, columns, &values),
        npyz::Order::Fortran => DMatrix::from_vec(rows, columns, values),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn npy(shape: &[u64], order: npyz::Order, values: &[f64]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .shape(shape)
            .order(order)
            .writer(&mut bytes)
            .begin_nd()
            .unwrap();
        writer.extend(values.iter().copied()).unwrap();
        writer.finish().unwrap();
        bytes
    }

    // Exercise the same binary stream against every supported destination family.
    fn check_conversion<M: NpyConvert<Shape = (usize, usize)>>(
        bytes: &[u8],
        actual: &[u64],
        expected: (usize, usize),
        allows_1d: bool,
        static_axes_match: bool,
    ) {
        let axes = [
            u64::try_from(expected.0).unwrap(),
            u64::try_from(expected.1).unwrap(),
        ];
        let shape_matches = actual == axes || (allows_1d && actual == [axes[0] * axes[1]]);
        match M::try_read_with_shape(std::io::Cursor::new(bytes), expected) {
            Ok(value) => {
                assert!(shape_matches && static_axes_match);
                let mut encoded = Vec::new();
                value.write_npy(&mut encoded).unwrap();
                let header = NpyFile::new(encoded.as_slice()).unwrap();
                assert_eq!(header.shape(), axes);
                assert_eq!(header.order(), npyz::Order::Fortran);
                let original = NpyFile::new(bytes).unwrap().into_vec::<f64>().unwrap();
                let decoded = header.into_vec::<f64>().unwrap();
                if expected.0 <= 1 || expected.1 <= 1 {
                    assert_eq!(decoded, original);
                }
            }
            Err(NpyError::InvalidShape {
                expected,
                actual: reported,
            }) => {
                assert!(!shape_matches);
                assert_eq!(&*expected, axes);
                assert_eq!(&*reported, actual);
            }
            Err(NpyError::InvalidMatrixShape(reported)) => {
                assert!(shape_matches && !static_axes_match);
                assert_eq!(&*reported, axes);
            }
            Err(error) => panic!("unexpected conversion error: {error}"),
        }
    }

    #[test]
    fn binary_conversion_contract_acceptance_and_error_matrix() {
        use nalgebra::{RowDVector, RowVector1, RowVector3, SMatrix, SVector, Vector3};
        let shapes: &[&[u64]] = &[
            &[],
            &[0],
            &[1],
            &[3],
            &[4],
            &[0, 1],
            &[1, 0],
            &[1, 1],
            &[1, 3],
            &[3, 1],
            &[2, 3],
            &[3, 2],
            &[2, 2],
            &[1, 1, 1],
            &[1, 3, 1],
        ];
        for actual in shapes {
            for order in [npyz::Order::C, npyz::Order::Fortran] {
                let count = usize::try_from(actual.iter().product::<u64>()).unwrap();
                let values: Vec<_> = (0..count)
                    .map(|i| f64::from(u32::try_from(i).unwrap()) + 0.25)
                    .collect();
                let bytes = npy(actual, order, &values);
                check_conversion::<SVector<f64, 1>>(&bytes, actual, (1, 1), true, true);
                check_conversion::<SMatrix<f64, 1, 1>>(&bytes, actual, (1, 1), true, true);
                check_conversion::<RowVector1<f64>>(&bytes, actual, (1, 1), true, true);
                check_conversion::<Vector3<f64>>(&bytes, actual, (3, 1), true, true);
                check_conversion::<RowVector3<f64>>(&bytes, actual, (1, 3), true, true);
                check_conversion::<SMatrix<f64, 2, 3>>(&bytes, actual, (2, 3), false, true);
                // Explicit dimensions that disagree with static axes must also fail.
                check_conversion::<Vector3<f64>>(&bytes, actual, (1, 3), true, false);
                check_conversion::<RowVector3<f64>>(&bytes, actual, (3, 1), true, false);
                check_conversion::<SMatrix<f64, 2, 3>>(&bytes, actual, (3, 2), false, false);
                for n in [0, 1, 3] {
                    check_conversion::<DVector<f64>>(&bytes, actual, (n, 1), true, true);
                    check_conversion::<RowDVector<f64>>(&bytes, actual, (1, n), true, true);
                    check_conversion::<DMatrix<f64>>(&bytes, actual, (n, 1), false, true);
                    check_conversion::<DMatrix<f64>>(&bytes, actual, (1, n), false, true);
                }
                check_conversion::<DMatrix<f64>>(&bytes, actual, (2, 3), false, true);
            }
        }
    }

    #[test]
    fn singleton_static_vectors_accept_1d_and_exact_2d_but_matrices_do_not() {
        let one_dimensional = npy(&[1], npyz::Order::C, &[7.]);
        let two_dimensional = npy(&[1, 1], npyz::Order::Fortran, &[7.]);
        assert_eq!(
            nalgebra::SVector::<f64, 1>::try_read_with_shape(&one_dimensional[..], (1, 1)).unwrap()
                [0],
            7.
        );
        assert_eq!(
            nalgebra::SMatrix::<f64, 1, 1>::try_read_with_shape(&one_dimensional[..], (1, 1))
                .unwrap()[(0, 0)],
            7.
        );
        assert_eq!(
            nalgebra::RowVector1::<f64>::try_read_with_shape(&one_dimensional[..], (1, 1)).unwrap()
                [0],
            7.
        );
        assert_eq!(
            nalgebra::SMatrix::<f64, 1, 1>::try_read_with_shape(&two_dimensional[..], (1, 1))
                .unwrap()[(0, 0)],
            7.
        );
        assert_eq!(
            DMatrix::<f64>::try_read_with_shape(&two_dimensional[..], (1, 1)).unwrap()[(0, 0)],
            7.
        );
        assert!(DMatrix::<f64>::try_read_with_shape(&one_dimensional[..], (1, 1)).is_err());
        assert_eq!(
            DVector::<f64>::try_read_with_shape(&one_dimensional[..], (1, 1))
                .unwrap()
                .as_slice(),
            &[7.]
        );
        assert_eq!(
            nalgebra::RowDVector::<f64>::try_read_with_shape(&one_dimensional[..], (1, 1))
                .unwrap()
                .as_slice(),
            &[7.]
        );
        let scalar = npy(&[], npyz::Order::C, &[7.]);
        for result in [
            DMatrix::<f64>::try_read_with_shape(&scalar[..], (1, 1)).map(|_| ()),
            nalgebra::SVector::<f64, 1>::try_read_with_shape(&scalar[..], (1, 1)).map(|_| ()),
        ] {
            assert!(matches!(result, Err(NpyError::InvalidShape { .. })));
        }
    }

    #[test]
    fn svector_one_reads_one_dimensional_payload_and_writes_two_dimensions() {
        let one_dimensional = npy(&[1], npyz::Order::C, &[9.]);
        assert_eq!(
            nalgebra::SVector::<f64, 1>::try_read_with_shape(&one_dimensional[..], (1, 1)).unwrap()
                [0],
            9.
        );
        let mut output = Vec::new();
        nalgebra::SVector::<f64, 1>::new(9.)
            .write_npy(&mut output)
            .unwrap();
        assert_eq!(NpyFile::new(output.as_slice()).unwrap().shape(), &[1, 1]);
    }

    #[test]
    fn matrix_one_by_one_requires_2d_and_rejects_numpy_scalar() {
        let matrix = npy(&[1, 1], npyz::Order::Fortran, &[9.]);
        assert_eq!(
            nalgebra::SMatrix::<f64, 1, 1>::try_read_with_shape(&matrix[..], (1, 1)).unwrap()
                [(0, 0)],
            9.
        );
        assert_eq!(
            DMatrix::<f64>::try_read_with_shape(&matrix[..], (1, 1)).unwrap()[(0, 0)],
            9.
        );
        let vector = npy(&[1], npyz::Order::C, &[9.]);
        assert!(DMatrix::<f64>::try_read_with_shape(&vector[..], (1, 1)).is_err());
        let scalar = npy(&[], npyz::Order::C, &[9.]);
        assert!(matches!(
            DMatrix::<f64>::try_read_with_shape(&scalar[..], (1, 1)),
            Err(NpyError::InvalidShape { .. })
        ));
        assert!(matches!(
            nalgebra::SVector::<f64, 1>::try_read_with_shape(&scalar[..], (1, 1)),
            Err(NpyError::InvalidShape { .. })
        ));
    }

    #[test]
    fn singleton_dynamic_and_static_axes_preserve_orientation_and_order() {
        for (shape, values) in [
            (&[1, 3][..], &[1., 2., 3.][..]),
            (&[3, 1][..], &[1., 2., 3.][..]),
        ] {
            for order in [npyz::Order::C, npyz::Order::Fortran] {
                let bytes = npy(shape, order, values);
                assert!(DMatrix::<f64>::try_read_with_shape(
                    &bytes[..],
                    (
                        usize::try_from(shape[0]).unwrap(),
                        usize::try_from(shape[1]).unwrap()
                    )
                )
                .is_ok());
                if shape[0] == 1 {
                    assert!(
                        nalgebra::RowDVector::<f64>::try_read_with_shape(&bytes[..], (1, 3))
                            .is_ok()
                    );
                    assert!(DVector::<f64>::try_read_with_shape(&bytes[..], (3, 1)).is_err());
                } else {
                    assert!(DVector::<f64>::try_read_with_shape(&bytes[..], (3, 1)).is_ok());
                    assert!(
                        nalgebra::RowDVector::<f64>::try_read_with_shape(&bytes[..], (1, 3))
                            .is_err()
                    );
                }
            }
        }
        let one_dimensional = npy(&[3], npyz::Order::C, &[1., 2., 3.]);
        assert_eq!(
            DVector::<f64>::try_read_with_shape(&one_dimensional[..], (3, 1))
                .unwrap()
                .as_slice(),
            &[1., 2., 3.]
        );
        assert_eq!(
            nalgebra::RowDVector::<f64>::try_read_with_shape(&one_dimensional[..], (1, 3))
                .unwrap()
                .as_slice(),
            &[1., 2., 3.]
        );
        assert!(DMatrix::<f64>::try_read_with_shape(&one_dimensional[..], (3, 1)).is_err());
    }

    proptest::proptest! {
        #[test]
        fn matrix_roundtrips_for_arbitrary_shapes_and_storage_orders(
            rows in 1usize..7,
            columns in 1usize..7,
            values in proptest::collection::vec(-1.0e6_f64..1.0e6, 1..=36),
        ) {
            let count = rows * columns;
            proptest::prop_assume!(values.len() >= count);
            let values = &values[..count];
            let expected = DMatrix::from_row_slice(rows, columns, values);

            for order in [npyz::Order::C, npyz::Order::Fortran] {
                let ordered_values = if order == npyz::Order::Fortran { expected.as_slice() } else { values };
                let bytes = npy(&[u64::try_from(rows).unwrap(), u64::try_from(columns).unwrap()], order, ordered_values);
                let restored = DMatrix::<f64>::try_read_with_shape(&bytes[..], (rows, columns)).unwrap();
                proptest::prop_assert_eq!(&restored, &expected);
            }

            let mut bytes = Vec::new();
            expected.write_npy(&mut bytes).unwrap();
            let header = NpyFile::new(bytes.as_slice()).unwrap();
            proptest::prop_assert_eq!(header.shape(), &[u64::try_from(rows).unwrap(), u64::try_from(columns).unwrap()]);
            proptest::prop_assert_eq!(header.order(), npyz::Order::Fortran);
        }
    }

    #[test]
    fn nalgebra_writer_keeps_singleton_matrix_axes() {
        let mut bytes = Vec::new();
        nalgebra::SVector::<f64, 1>::new(7.)
            .write_npy(&mut bytes)
            .unwrap();
        assert_eq!(NpyFile::new(bytes.as_slice()).unwrap().shape(), &[1, 1]);
        bytes.clear();
        nalgebra::SMatrix::<f64, 1, 1>::new(7.)
            .write_npy(&mut bytes)
            .unwrap();
        assert_eq!(NpyFile::new(bytes.as_slice()).unwrap().shape(), &[1, 1]);
        bytes.clear();
        nalgebra::RowVector1::<f64>::new(7.)
            .write_npy(&mut bytes)
            .unwrap();
        assert_eq!(NpyFile::new(bytes.as_slice()).unwrap().shape(), &[1, 1]);
        bytes.clear();
        DVector::<f64>::from_element(1, 7.)
            .write_npy(&mut bytes)
            .unwrap();
        assert_eq!(NpyFile::new(bytes.as_slice()).unwrap().shape(), &[1, 1]);
        bytes.clear();
        DMatrix::<f64>::from_element(1, 1, 7.)
            .write_npy(&mut bytes)
            .unwrap();
        assert_eq!(NpyFile::new(bytes.as_slice()).unwrap().shape(), &[1, 1]);
    }
}
