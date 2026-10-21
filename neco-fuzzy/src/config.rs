/// ファジー一致の重みと変換係数です。
#[derive(Clone, Copy, PartialEq)]
pub struct ScoreConfig {
    pub w_pos: f32,
    pub w_bnd: f32,
    pub w_head: f32,
    pub w_gap: f32,
    pub w_span: f32,
    pub w_tail: f32,
    pub w_exact: f32,
    pub w_case: f32,
    pub sigma_base: f32,
    pub n_ref: f32,
    pub w_idf: f32,
    pub confidence_scale: f32,
}

/// 整数スコアへ変換する尺度です。
pub const VALUE_SCALE: f32 = 100.0;

impl Default for ScoreConfig {
    fn default() -> Self {
        Self {
            w_pos: 0.03,
            w_bnd: 0.64,
            w_head: 1.20,
            w_gap: 1.80,
            w_span: 0.02,
            w_tail: 0.08,
            w_exact: 0.60,
            w_case: 0.05,
            sigma_base: 3.0,
            n_ref: 12.0,
            w_idf: 0.0,
            confidence_scale: 0.35,
        }
    }
}

impl ScoreConfig {
    /// 候補長に応じた減衰係数を返します。
    pub fn sigma(&self, candidate_len: usize) -> f32 {
        self.sigma_base * (candidate_len as f32 / self.n_ref).sqrt()
    }
}
