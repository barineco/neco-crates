use crate::Array2;
use core::fmt;

/// Störmer-Verlet triple-buffer for `w`, `u`, and `v`.
pub struct FieldSet {
    w: [Array2<f64>; 3],
    u: [Array2<f64>; 3],
    v: [Array2<f64>; 3],
    generation: usize,
}

/// Split borrows: current / previous (read) and next (write).
pub struct SplitBufs<'a> {
    pub w_cur: &'a Array2<f64>,
    pub w_prev: &'a Array2<f64>,
    pub w_next: &'a mut Array2<f64>,
    pub u_cur: &'a Array2<f64>,
    pub u_prev: &'a Array2<f64>,
    pub u_next: &'a mut Array2<f64>,
    pub v_cur: &'a Array2<f64>,
    pub v_prev: &'a Array2<f64>,
    pub v_next: &'a mut Array2<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointError {
    InvalidBufferShape { field: &'static str },
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBufferShape { field } => {
                write!(f, "checkpoint {field} buffer must match its declared shape")
            }
        }
    }
}

impl std::error::Error for CheckpointError {}

impl FieldSet {
    pub fn new(nx: usize, ny: usize) -> Self {
        Self {
            w: std::array::from_fn(|_| Array2::zeros((nx, ny))),
            u: std::array::from_fn(|_| Array2::zeros((nx, ny))),
            v: std::array::from_fn(|_| Array2::zeros((nx, ny))),
            generation: 0,
        }
    }

    #[inline]
    pub fn w(&self) -> &Array2<f64> {
        &self.w[self.generation % 3]
    }

    #[inline]
    pub fn w_prev(&self) -> &Array2<f64> {
        &self.w[(self.generation + 2) % 3]
    }

    #[inline]
    pub fn u(&self) -> &Array2<f64> {
        &self.u[self.generation % 3]
    }

    #[inline]
    pub fn u_prev(&self) -> &Array2<f64> {
        &self.u[(self.generation + 2) % 3]
    }

    #[inline]
    pub fn v(&self) -> &Array2<f64> {
        &self.v[self.generation % 3]
    }

    #[inline]
    pub fn v_prev(&self) -> &Array2<f64> {
        &self.v[(self.generation + 2) % 3]
    }

    #[inline]
    pub fn w_mut(&mut self) -> &mut Array2<f64> {
        let index = self.generation % 3;
        &mut self.w[index]
    }

    #[inline]
    pub fn u_mut(&mut self) -> &mut Array2<f64> {
        let index = self.generation % 3;
        &mut self.u[index]
    }

    #[inline]
    pub fn v_mut(&mut self) -> &mut Array2<f64> {
        let index = self.generation % 3;
        &mut self.v[index]
    }

    /// Safety: current, previous, and next are distinct indices.
    #[inline]
    pub fn split_bufs(&mut self) -> SplitBufs<'_> {
        let cur = self.generation % 3;
        let prev = (self.generation + 2) % 3;
        let next = (self.generation + 1) % 3;
        unsafe {
            let w = self.w.as_mut_ptr();
            let u = self.u.as_mut_ptr();
            let v = self.v.as_mut_ptr();
            SplitBufs {
                w_cur: &*w.add(cur),
                w_prev: &*w.add(prev),
                w_next: &mut *w.add(next),
                u_cur: &*u.add(cur),
                u_prev: &*u.add(prev),
                u_next: &mut *u.add(next),
                v_cur: &*v.add(cur),
                v_prev: &*v.add(prev),
                v_next: &mut *v.add(next),
            }
        }
    }

    #[inline]
    pub fn advance(&mut self) {
        self.generation += 1;
    }

    pub fn to_checkpoint(&self) -> FieldSetCheckpoint {
        let flatten = |buffers: &[Array2<f64>; 3]| -> [Vec<f64>; 3] {
            std::array::from_fn(|index| buffers[index].as_slice().to_vec())
        };
        let shape = (self.w[0].nrows(), self.w[0].ncols());
        FieldSetCheckpoint {
            w: flatten(&self.w),
            u: flatten(&self.u),
            v: flatten(&self.v),
            generation: self.generation,
            shape,
        }
    }

    pub fn restore_checkpoint(
        &mut self,
        checkpoint: &FieldSetCheckpoint,
    ) -> Result<(), CheckpointError> {
        let (nx, ny) = checkpoint.shape;
        for index in 0..3 {
            self.w[index] = Array2::from_shape_vec((nx, ny), checkpoint.w[index].clone())
                .map_err(|_| CheckpointError::InvalidBufferShape { field: "w" })?;
            self.u[index] = Array2::from_shape_vec((nx, ny), checkpoint.u[index].clone())
                .map_err(|_| CheckpointError::InvalidBufferShape { field: "u" })?;
            self.v[index] = Array2::from_shape_vec((nx, ny), checkpoint.v[index].clone())
                .map_err(|_| CheckpointError::InvalidBufferShape { field: "v" })?;
        }
        self.generation = checkpoint.generation;
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldSetCheckpoint {
    pub w: [Vec<f64>; 3],
    pub u: [Vec<f64>; 3],
    pub v: [Vec<f64>; 3],
    pub generation: usize,
    pub shape: (usize, usize),
}
