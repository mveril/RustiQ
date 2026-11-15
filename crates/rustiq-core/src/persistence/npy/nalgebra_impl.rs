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

    fn try_from_npy_with_shape<R: Read>(
        npy: NpyFile<R>,
        shape: Self::Shape,
    ) -> Result<Self, NpyError> {
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
        Self::decode_with_shape(npy, shape)
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
