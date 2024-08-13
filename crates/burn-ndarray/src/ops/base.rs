use alloc::vec::Vec;
use burn_tensor::ElementConversion;
use burn_tensor::TensorData;
use ndarray::Array;
use ndarray::ArrayBase;
use ndarray::ArrayView1;
use ndarray::Dimension;
use ndarray::Ix;
use ndarray::ViewRepr;
use core::fmt::Debug;
use core::{marker::PhantomData, ops::Range};
use ndarray::s;
use ndarray::Array2;
use ndarray::IntoDimension;
use ndarray::SliceInfo;
use ndarray::Zip;
use num_traits::Signed;

use burn_tensor::Shape;
use ndarray::Axis;
use ndarray::Dim;
use ndarray::IxDyn;
use ndarray::SliceInfoElem;

use crate::element::NdArrayElement;
use crate::ops::macros::{keepdim, mean_dim, prod_dim, sum_dim};
use crate::{reshape, tensor::NdArrayTensor};

pub struct NdArrayOps<E> {
    e: PhantomData<E>,
}

pub(crate) struct NdArrayMathOps<E> {
    e: PhantomData<E>,
}

impl<E> NdArrayOps<E>
where
    E: Copy + Debug,
{
    pub fn slice<const D1: usize, const D2: usize>(
        tensor: NdArrayTensor<E, D1>,
        ranges: [Range<usize>; D2],
    ) -> NdArrayTensor<E, D1> {
        let slices = Self::to_slice_args::<D1, D2>(ranges);
        let array = tensor.array.slice_move(slices.as_slice()).into_shared();

        NdArrayTensor { array }
    }

    pub fn slice_assign<const D1: usize, const D2: usize>(
        tensor: NdArrayTensor<E, D1>,
        ranges: [Range<usize>; D2],
        value: NdArrayTensor<E, D1>,
    ) -> NdArrayTensor<E, D1> {
        let slices = Self::to_slice_args::<D1, D2>(ranges);
        let mut array = tensor.array.into_owned();
        array.slice_mut(slices.as_slice()).assign(&value.array);
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn reshape<const D1: usize, const D2: usize>(
        tensor: NdArrayTensor<E, D1>,
        shape: Shape<D2>,
    ) -> NdArrayTensor<E, D2> {
        reshape!(
            ty E,
            shape shape,
            array tensor.array,
            d D2
        )
    }

    pub fn cat<const D: usize>(
        tensors: Vec<NdArrayTensor<E, D>>,
        dim: usize,
    ) -> NdArrayTensor<E, D> {
        let arrays: Vec<ndarray::ArrayView<E, IxDyn>> =
            tensors.iter().map(|t| t.array.view()).collect();
        let array = ndarray::concatenate(Axis(dim), &arrays)
            .unwrap()
            .into_shared();

        // Transform column-major layout into row-major (standard) layout. (fix #1053)
        let array = NdArrayTensor { array };
        Self::reshape(array.clone(), array.shape())
    }

    fn to_slice_args<const D1: usize, const D2: usize>(
        ranges: [Range<usize>; D2],
    ) -> [SliceInfoElem; D1] {
        let mut slices = [SliceInfoElem::NewAxis; D1];
        for i in 0..D1 {
            if i >= D2 {
                slices[i] = SliceInfoElem::Slice {
                    start: 0,
                    end: None,
                    step: 1,
                }
            } else {
                slices[i] = SliceInfoElem::Slice {
                    start: ranges[i].start as isize,
                    end: Some(ranges[i].end as isize),
                    step: 1,
                }
            }
        }
        slices
    }

    pub fn swap_dims<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        dim1: usize,
        dim2: usize,
    ) -> NdArrayTensor<E, D> {
        let mut array = tensor.array;
        array.swap_axes(dim1, dim2);

        NdArrayTensor::new(array)
    }

    /// Broadcasts the tensor to the given shape
    pub(crate) fn expand<const D1: usize, const D2: usize>(
        tensor: NdArrayTensor<E, D1>,
        shape: Shape<D2>,
    ) -> NdArrayTensor<E, D2> {
        let array = tensor
            .array
            .broadcast(shape.dims.into_dimension())
            .expect("The shapes should be broadcastable")
            // need to convert view to owned array because NdArrayTensor expects owned array
            // and try_into_owned_nocopy() panics for broadcasted arrays (zero strides)
            .into_owned()
            .into_shared();
        NdArrayTensor { array }
    }

    pub fn flip<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        axes: &[usize],
    ) -> NdArrayTensor<E, D> {
        let slice_items: Vec<_> = (0..D)
            .map(|i| {
                if axes.contains(&i) {
                    SliceInfoElem::Slice {
                        start: 0,
                        end: None,
                        step: -1,
                    }
                } else {
                    SliceInfoElem::Slice {
                        start: 0,
                        end: None,
                        step: 1,
                    }
                }
            })
            .collect();
        let slice_info =
            SliceInfo::<Vec<SliceInfoElem>, IxDyn, IxDyn>::try_from(slice_items).unwrap();
        let array = tensor.array.slice(slice_info).into_owned().into_shared();

        NdArrayTensor::new(array)
    }
}

impl<E> NdArrayMathOps<E>
where
    E: Copy + NdArrayElement,
{
    pub fn add<const D: usize>(
        lhs: NdArrayTensor<E, D>,
        rhs: NdArrayTensor<E, D>,
    ) -> NdArrayTensor<E, D> {
        let array = &lhs.array + &rhs.array;
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn add_scalar<const D: usize>(lhs: NdArrayTensor<E, D>, rhs: E) -> NdArrayTensor<E, D> {
        let array = lhs.array + rhs;
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn sub<const D: usize>(
        lhs: NdArrayTensor<E, D>,
        rhs: NdArrayTensor<E, D>,
    ) -> NdArrayTensor<E, D> {
        let array = lhs.array - rhs.array;
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn sub_scalar<const D: usize>(lhs: NdArrayTensor<E, D>, rhs: E) -> NdArrayTensor<E, D> {
        let array = lhs.array - rhs;
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn mul<const D: usize>(
        lhs: NdArrayTensor<E, D>,
        rhs: NdArrayTensor<E, D>,
    ) -> NdArrayTensor<E, D> {
        let array = lhs.array * rhs.array;
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn mul_scalar<const D: usize>(lhs: NdArrayTensor<E, D>, rhs: E) -> NdArrayTensor<E, D> {
        let array = lhs.array * rhs;
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn div<const D: usize>(
        lhs: NdArrayTensor<E, D>,
        rhs: NdArrayTensor<E, D>,
    ) -> NdArrayTensor<E, D> {
        let array = lhs.array / rhs.array;
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn div_scalar<const D: usize>(lhs: NdArrayTensor<E, D>, rhs: E) -> NdArrayTensor<E, D> {
        let array = lhs.array / rhs;
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn remainder_scalar<const D: usize>(lhs: NdArrayTensor<E, D>, rhs: E) -> NdArrayTensor<E, D>
    where
        E: core::ops::Rem<Output = E>,
    {
        let array = lhs.array.mapv(|x| ((x % rhs) + rhs) % rhs);
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn recip<const D: usize>(tensor: NdArrayTensor<E, D>) -> NdArrayTensor<E, D> {
        let array = tensor.array.map(|x| 1.elem::<E>() / *x);
        let array = array.into_shared();

        NdArrayTensor { array }
    }

    pub fn mean<const D: usize>(tensor: NdArrayTensor<E, D>) -> NdArrayTensor<E, 1> {
        let data = TensorData::from([tensor.array.mean().unwrap()]);
        NdArrayTensor::from_data(data)
    }

    pub fn sum<const D: usize>(tensor: NdArrayTensor<E, D>) -> NdArrayTensor<E, 1> {
        let data = TensorData::from([tensor.array.sum()]);
        NdArrayTensor::from_data(data)
    }

    pub fn prod<const D: usize>(tensor: NdArrayTensor<E, D>) -> NdArrayTensor<E, 1> {
        let data = TensorData::from([tensor.array.product()]);
        NdArrayTensor::from_data(data)
    }

    pub fn mean_dim<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        dim: usize,
    ) -> NdArrayTensor<E, D> {
        match D {
            1 => keepdim!(0, dim, tensor, mean),
            2 => keepdim!(1, dim, tensor, mean),
            3 => keepdim!(2, dim, tensor, mean),
            4 => keepdim!(3, dim, tensor, mean),
            5 => keepdim!(4, dim, tensor, mean),
            6 => keepdim!(5, dim, tensor, mean),
            _ => panic!("Dim not supported {D}"),
        }
    }

    pub fn sum_dim<const D: usize>(tensor: NdArrayTensor<E, D>, dim: usize) -> NdArrayTensor<E, D> {
        match D {
            1 => keepdim!(0, dim, tensor, sum),
            2 => keepdim!(1, dim, tensor, sum),
            3 => keepdim!(2, dim, tensor, sum),
            4 => keepdim!(3, dim, tensor, sum),
            5 => keepdim!(4, dim, tensor, sum),
            6 => keepdim!(5, dim, tensor, sum),
            _ => panic!("Dim not supported {D}"),
        }
    }

    pub fn prod_dim<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        dim: usize,
    ) -> NdArrayTensor<E, D> {
        match D {
            1 => keepdim!(0, dim, tensor, prod),
            2 => keepdim!(1, dim, tensor, prod),
            3 => keepdim!(2, dim, tensor, prod),
            4 => keepdim!(3, dim, tensor, prod),
            5 => keepdim!(4, dim, tensor, prod),
            6 => keepdim!(5, dim, tensor, prod),
            _ => panic!("Dim not supported {D}"),
        }
    }

    fn validate_shapes<D: Dimension>(
        axis: usize,
        input: &ArrayBase<ViewRepr<&E>, D>,
        index: &ArrayBase<ViewRepr<&i64>, D>
    ) {
        // Enforce data and index having same number of dimensions
        assert_eq!(
            input.ndim(), index.ndim(),
            "input and index must have the same number of dimensions."
        );
        // Enforce all axes of index except the specified axis have the same length as the
        // corresponding axis in data.
        let mut i: usize = 0;
        for dim in input.shape() {
            if i == axis { continue };
            assert!(
                &index.shape()[i] <= dim,
                "index.size(d) <= input.size(d) for all dimensions d != axis"
            );
            i += 1;
        }
    }

    pub fn gather<const D: usize>(
        axis: usize,
        tensor: NdArrayTensor<E, D>,
        index: NdArrayTensor<i64, D>,
    ) -> NdArrayTensor<E, D> {
        assert!(axis < tensor.array.ndim(), "axis must be in range [0, input.ndim()-1]");
        Self::validate_shapes(axis, &tensor.array.view(), &index.array.view());

        let input_d = tensor.array.into_dyn();
        let raw_dim = index.array.raw_dim();
        let mut out = Array::zeros(raw_dim);

        for (coord, idx) in index.array.indexed_iter() {
            let dim = coord.into_dimension();
            let arr_view: ArrayView1<'_, Ix> = dim.as_array_view();
            let mut indices: Vec<usize> = arr_view.iter().copied().collect();
            let idx = match idx.is_negative() {
                true => (*idx + input_d.shape()[axis] as i64) as usize,
                false => *idx as usize,
            };
            indices[axis] = idx;
            let coords = IxDyn(&indices);
            out[dim] = input_d[coords];
        }
        NdArrayTensor::new(out.into())
    }

    pub fn scatter<const D: usize>(
        dim: usize,
        mut tensor: NdArrayTensor<E, D>,
        mut indices: NdArrayTensor<i64, D>,
        mut value: NdArrayTensor<E, D>,
    ) -> NdArrayTensor<E, D> {
        if dim != D - 1 {
            tensor.array.swap_axes(D - 1, dim);
            indices.array.swap_axes(D - 1, dim);
            value.array.swap_axes(D - 1, dim);
        }

        let (shape_tensor, shape_indices, shape_value) =
            (tensor.shape(), indices.shape(), value.shape());
        let (size_tensor, size_index, size_value) = (
            shape_tensor.dims[D - 1],
            shape_indices.dims[D - 1],
            shape_value.dims[D - 1],
        );
        let batch_size = Self::gather_batch_size(&shape_tensor, &shape_indices);

        if shape_value != shape_indices {
            panic!(
                "Invalid dimension: the shape of the index tensor should be the same as the value \
                 tensor: Index {:?} value {:?}",
                shape_indices.dims, shape_value.dims
            );
        }

        let indices = NdArrayOps::reshape(indices, Shape::new([batch_size, size_index])).array;
        let value = NdArrayOps::reshape(value, Shape::new([batch_size, size_value])).array;
        let mut tensor = NdArrayOps::reshape(tensor, Shape::new([batch_size, size_tensor])).array;

        for b in 0..batch_size {
            let indices = indices.slice(s!(b, ..));

            for (i, index) in indices.iter().enumerate() {
                let index = *index as usize;
                tensor[[b, index]] += value[[b, i]];
            }
        }

        let mut output = NdArrayOps::reshape(
            NdArrayTensor::<E, 2>::new(tensor.into_shared().into_dyn()),
            shape_tensor,
        );
        if dim != D - 1 {
            output.array.swap_axes(D - 1, dim);
        }
        output
    }

    pub fn mask_where<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        mask: NdArrayTensor<bool, D>,
        source: NdArrayTensor<E, D>,
    ) -> NdArrayTensor<E, D> {
        let mask_mul_4tensor = mask.array.mapv(|x| match x {
            true => 0.elem(),
            false => 1.elem(),
        });
        let mask_mul_4source = mask.array.mapv(|x| match x {
            true => 1.elem(),
            false => 0.elem(),
        });
        let array = (tensor.array * mask_mul_4tensor) + (source.array * mask_mul_4source);

        NdArrayTensor::new(array)
    }

    pub fn mask_fill<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        mask: NdArrayTensor<bool, D>,
        value: E,
    ) -> NdArrayTensor<E, D> {
        let mut output = tensor.array.clone();
        let broadcast_mask = mask.array.broadcast(output.dim()).unwrap();
        Zip::from(&mut output)
            .and(&broadcast_mask)
            .for_each(|out, &mask_val| {
                if mask_val {
                    *out = value;
                }
            });
        NdArrayTensor::new(output.into_shared())
    }

    fn gather_batch_size<const D: usize>(
        shape_tensor: &Shape<D>,
        shape_indices: &Shape<D>,
    ) -> usize {
        let mut batch_size = 1;

        for i in 0..D - 1 {
            if shape_tensor.dims[i] != shape_indices.dims[i] {
                panic!(
                    "Unsupported dimension, only the last dimension can differ: Tensor {:?} Index \
                     {:?}",
                    shape_tensor.dims, shape_indices.dims
                );
            }
            batch_size *= shape_indices.dims[i];
        }

        batch_size
    }

    pub fn select<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        dim: usize,
        indices: NdArrayTensor<i64, 1>,
    ) -> NdArrayTensor<E, D> {
        let array = tensor.array.select(
            Axis(dim),
            &indices
                .array
                .into_iter()
                .map(|i| i as usize)
                .collect::<Vec<_>>(),
        );

        NdArrayTensor::new(array.into_shared())
    }

    pub fn select_assign<const D1: usize, const D2: usize>(
        tensor: NdArrayTensor<E, D1>,
        dim: usize,
        indices: NdArrayTensor<i64, 1>,
        value: NdArrayTensor<E, D2>,
    ) -> NdArrayTensor<E, D1> {
        let mut output_array = tensor.array.into_owned();

        for (index_value, index) in indices.array.into_iter().enumerate() {
            let mut view = output_array.index_axis_mut(Axis(dim), index as usize);
            let value = value.array.index_axis(Axis(dim), index_value);

            view.zip_mut_with(&value, |a, b| *a += *b);
        }

        NdArrayTensor::new(output_array.into_shared())
    }
    pub fn argmax<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        dim: usize,
    ) -> NdArrayTensor<i64, D> {
        arg(tensor, dim, CmpType::Max)
    }

    pub fn argmin<const D: usize>(
        tensor: NdArrayTensor<E, D>,
        dim: usize,
    ) -> NdArrayTensor<i64, D> {
        arg(tensor, dim, CmpType::Min)
    }

    pub fn clamp_min<const D: usize>(
        mut tensor: NdArrayTensor<E, D>,
        min: E,
    ) -> NdArrayTensor<E, D> {
        tensor.array.mapv_inplace(|x| match x < min {
            true => min,
            false => x,
        });

        tensor
    }

    pub fn clamp_max<const D: usize>(
        mut tensor: NdArrayTensor<E, D>,
        max: E,
    ) -> NdArrayTensor<E, D> {
        tensor.array.mapv_inplace(|x| match x > max {
            true => max,
            false => x,
        });

        tensor
    }

    pub fn clamp<const D: usize>(
        mut tensor: NdArrayTensor<E, D>,
        min: E,
        max: E,
    ) -> NdArrayTensor<E, D> {
        tensor.array.mapv_inplace(|x| match x < min {
            true => min,
            false => match x > max {
                true => max,
                false => x,
            },
        });

        tensor
    }

    pub(crate) fn elementwise_op<const D: usize, OtherE>(
        lhs: NdArrayTensor<E, D>,
        rhs: NdArrayTensor<OtherE, D>,
        var_name: impl FnMut(&E, &OtherE) -> E,
    ) -> NdArrayTensor<E, D> {
        NdArrayTensor::new(
            Zip::from(lhs.array.view())
                .and(rhs.array.view())
                .map_collect(var_name)
                .into_shared(),
        )
    }

    pub(crate) fn elementwise_op_scalar<const D: usize>(
        lhs: NdArrayTensor<E, D>,
        var_name: impl FnMut(E) -> E,
    ) -> NdArrayTensor<E, D> {
        NdArrayTensor::new(lhs.array.mapv(var_name).into_shared())
    }

    pub(crate) fn sign_op<const D: usize>(tensor: NdArrayTensor<E, D>) -> NdArrayTensor<E, D>
    where
        E: Signed,
    {
        let zero = 0.elem();
        let one = 1.elem::<E>();
        NdArrayTensor::new(
            tensor
                .array
                .mapv(|x| {
                    if x > zero {
                        one
                    } else if x < zero {
                        -one
                    } else {
                        zero
                    }
                })
                .into_shared(),
        )
    }
}

enum CmpType {
    Min,
    Max,
}

fn arg<E: NdArrayElement, const D: usize>(
    tensor: NdArrayTensor<E, D>,
    dim: usize,
    cmp: CmpType,
) -> NdArrayTensor<i64, D> {
    let mut reshape = tensor.array.shape().to_vec();
    reshape[dim] = 1;

    let output = tensor.array.map_axis(Axis(dim), |arr| {
        // Find the min/max value in the array, and return its index.
        let (_e, idx) = arr.indexed_iter().fold((arr[0], 0usize), |acc, (idx, e)| {
            let cmp = match cmp {
                CmpType::Min => e < &acc.0,
                CmpType::Max => e > &acc.0,
            };

            if cmp {
                (*e, idx)
            } else {
                acc
            }
        });

        idx as i64
    });

    let output = output.into_shape(Dim(reshape.as_slice())).unwrap();

    NdArrayTensor {
        array: output.into_shared(),
    }
}

#[cfg(test)]
mod tests {
    use ndarray::array;

    use super::*;

    #[test]
    fn should_generate_row_major_layout_for_cat() {
        let expected_shape: &[usize] = &[4, 6, 2];
        let expected_strides: &[isize] = &[12, 2, 1];
        let expected_array: NdArrayTensor<i32, 3> = NdArrayTensor::from_data(TensorData::from([
            [[1, 0], [2, 0], [3, 0], [4, 0], [5, 0], [6, 0]],
            [[7, 0], [8, 0], [9, 0], [10, 0], [11, 0], [12, 0]],
            [[13, 0], [14, 0], [15, 0], [16, 0], [17, 0], [18, 0]],
            [[19, 0], [20, 0], [21, 0], [22, 0], [23, 0], [24, 0]],
        ]));

        // unsqueeze dim on the outermost axis
        let array = NdArrayOps::reshape(
            NdArrayTensor::<i32, 3>::from_data(TensorData::from([
                [1, 2, 3, 4, 5, 6],
                [7, 8, 9, 10, 11, 12],
                [13, 14, 15, 16, 17, 18],
                [19, 20, 21, 22, 23, 24],
            ])),
            Shape::from([4, 6, 1]),
        );
        let zeros = NdArrayTensor::<i32, 3>::from_data(TensorData::zeros::<i32, _>([4, 6, 1]));
        // make `ndarray` concatenates array on the outermost axis
        let array = NdArrayOps::cat([array, zeros].to_vec(), 2);

        assert!(array.array.is_standard_layout());
        assert_eq!(array.array.shape(), expected_shape);
        assert_eq!(array.array.strides(), expected_strides);
        assert_eq!(
            array.array.into_iter().collect::<Vec<_>>(),
            expected_array.array.into_iter().collect::<Vec<_>>(),
        );
    }

    #[test]
    fn gather() {
        let arr = Array2::<u8>::from_shape_vec((4, 3), (1..13).collect())
            .unwrap()
            .into_shared()
            .into_dyn();
        let tensor = NdArrayTensor::<u8, 2>::new(arr);
        let test_cases = vec![
            (
                0,
                array![ [0i64, 1, 3], [3, 1, 2] ],
                array![ [1u8, 5, 12], [10, 5, 9] ]
            ),
            (
                1,
                array![ [0i64, 2], [1, 1], [2, 1] ],
                array![ [1u8, 3], [5, 5], [9, 8] ]
            ),
            // test negative indices
            (
                0,
                array![ [0i64, 1, -1], [3, 1, -2] ],
                array![ [1u8, 5, 12], [10, 5, 9] ]
            ),
            (
                1,
                array![ [0i64, -1], [1, 1], [2, -2] ],
                array![ [1u8, 3], [5, 5], [9, 8] ]
            ),
        ];
        let mut i = 0;
        for (axis, index, expected) in test_cases {
            let index = NdArrayTensor::new(index.into_shared().into_dyn());
            let expected = NdArrayTensor::<u8, 2>::new(expected.into_shared().into_dyn());
            let out = NdArrayMathOps::gather(axis, tensor.clone(), index);
            assert!(out.array.eq(&expected.array));
            i += 1;
        }
        assert_eq!(i, 4);
    }

}
