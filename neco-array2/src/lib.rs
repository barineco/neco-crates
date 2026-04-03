use std::ops::{Index, IndexMut};

/// Lightweight row-major 2D array used by grid-oriented helper crates.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Array2<T> {
    nrows: usize,
    ncols: usize,
    data: Vec<T>,
}

impl<T> Array2<T> {
    pub fn from_shape_vec(shape: (usize, usize), data: Vec<T>) -> Result<Self, String> {
        let (nrows, ncols) = shape;
        if data.len() != nrows * ncols {
            return Err(format!(
                "Array2 data length mismatch: got {}, expected {} for shape ({nrows}, {ncols})",
                data.len(),
                nrows * ncols
            ));
        }
        Ok(Self { nrows, ncols, data })
    }

    #[inline]
    pub fn dim(&self) -> (usize, usize) {
        (self.nrows, self.ncols)
    }

    #[inline]
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    #[inline]
    pub fn ncols(&self) -> usize {
        self.ncols
    }

    #[inline]
    pub fn shape(&self) -> [usize; 2] {
        [self.nrows, self.ncols]
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.data.iter_mut()
    }

    #[inline]
    fn offset(&self, row: usize, col: usize) -> usize {
        row * self.ncols + col
    }
}

impl<T: Clone> Array2<T> {
    pub fn from_elem(shape: (usize, usize), value: T) -> Self {
        let (nrows, ncols) = shape;
        Self {
            nrows,
            ncols,
            data: vec![value; nrows * ncols],
        }
    }

    pub fn fill(&mut self, value: T) {
        self.data.fill(value);
    }
}

impl<T: Clone + Default> Array2<T> {
    pub fn zeros(shape: (usize, usize)) -> Self {
        Self::from_elem(shape, T::default())
    }
}

impl<T> Index<[usize; 2]> for Array2<T> {
    type Output = T;

    fn index(&self, index: [usize; 2]) -> &Self::Output {
        &self.data[self.offset(index[0], index[1])]
    }
}

impl<T> IndexMut<[usize; 2]> for Array2<T> {
    fn index_mut(&mut self, index: [usize; 2]) -> &mut Self::Output {
        let offset = self.offset(index[0], index[1]);
        &mut self.data[offset]
    }
}

#[cfg(test)]
mod tests {
    use super::Array2;

    #[test]
    fn shape_vec_roundtrip_preserves_row_major_order() {
        let array =
            Array2::from_shape_vec((2, 3), vec![1, 2, 3, 4, 5, 6]).expect("test shape is valid");
        assert_eq!(array.shape(), [2, 3]);
        assert_eq!(array[[0, 0]], 1);
        assert_eq!(array[[0, 2]], 3);
        assert_eq!(array[[1, 0]], 4);
        assert_eq!(array[[1, 2]], 6);
        assert_eq!(array.as_slice(), &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn from_elem_and_fill_cover_whole_buffer() {
        let mut array = Array2::from_elem((2, 2), false);
        array[[1, 1]] = true;
        array.fill(true);
        assert_eq!(array.as_slice(), &[true, true, true, true]);
    }

    #[test]
    fn zeros_uses_default_values() {
        let array = Array2::<f64>::zeros((2, 2));
        assert_eq!(array.as_slice(), &[0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn shape_mismatch_is_rejected() {
        let err = Array2::from_shape_vec((2, 2), vec![1, 2, 3]).expect_err("shape mismatch");
        assert!(err.contains("mismatch"), "{err}");
    }
}
