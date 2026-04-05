/// エネルギーモデルの重みと変換係数をまとめた設定です。
#[derive(Clone, Copy, PartialEq)]
pub struct ScoreConfig {
    /// 先頭から離れるほど増える位置コストの重みです。
    pub w_pos: f32,
    /// 境界度の高い位置へ引き寄せる重みです。
    pub w_bnd: f32,
    /// 文字列先頭および basename 先頭を優遇する重みです。
    pub w_head: f32,
    /// 離れた文字を飛び越えるときのギャップコスト重みです。
    pub w_gap: f32,
    /// スパンを候補長全体へ分散して抑える重みです。
    pub w_span: f32,
    /// 最後の一致より後ろに残る末尾長へのコスト重みです。
    pub w_tail: f32,
    /// 候補全体と query が一致するときのボーナス重みです。
    pub w_exact: f32,
    /// case-insensitive 一致で大小が異なる文字へのコスト重みです。
    pub w_case: f32,
    /// Gaussian 減衰の基準となる sigma です。
    pub sigma_base: f32,
    /// sigma 適応で基準に使う候補長です。
    pub n_ref: f32,
    /// コーパス統計由来の IDF を効かせる重みです。
    pub w_idf: f32,
    /// energy から confidence へ写像するときのスケールです。
    pub confidence_scale: f32,
}

/// energy を互換性のある整数 value に変換するときの尺度です。
pub const VALUE_SCALE: f32 = 100.0;

impl Default for ScoreConfig {
    fn default() -> Self {
        Self {
            // 現行の整数スコア比率
            // +45 boundary, +70 contiguous, -3 position, -2 span,
            // +120 head, +90 basename を初期の相対関係として写します。
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
    /// 候補長に応じて適応させた sigma を返します。
    pub fn sigma(&self, candidate_len: usize) -> f32 {
        self.sigma_base * (candidate_len as f32 / self.n_ref).sqrt()
    }
}
