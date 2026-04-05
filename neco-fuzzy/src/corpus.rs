#[derive(Debug, Clone, PartialEq)]
pub struct CorpusStats {
    ascii_idf: [f32; 128],
    candidate_count: usize,
    mean_length: f32,
}

impl CorpusStats {
    pub fn from_candidates(candidates: &[&str]) -> Self {
        if candidates.is_empty() {
            return Self {
                ascii_idf: [0.0; 128],
                candidate_count: 0,
                mean_length: 0.0,
            };
        }

        let mut document_counts = [0usize; 128];
        let mut total_length = 0usize;

        for candidate in candidates {
            total_length += candidate.len();

            let mut seen = [false; 128];
            for &byte in candidate.as_bytes() {
                if byte < 128 {
                    seen[usize::from(byte)] = true;
                }
            }

            for (index, present) in seen.into_iter().enumerate() {
                if present {
                    document_counts[index] += 1;
                }
            }
        }

        let candidate_count = candidates.len();
        let candidate_count_f32 = candidate_count as f32;
        let mut ascii_idf = [0.0; 128];

        for (index, &count) in document_counts.iter().enumerate() {
            if count > 0 {
                let df = (count as f32) / candidate_count_f32;
                ascii_idf[index] = (1.0 / df).ln();
            }
        }

        Self {
            ascii_idf,
            candidate_count,
            mean_length: (total_length as f32) / candidate_count_f32,
        }
    }

    pub fn idf(&self, byte: u8) -> f32 {
        if byte < 128 {
            self.ascii_idf[usize::from(byte)]
        } else {
            0.0
        }
    }

    pub fn candidate_count(&self) -> usize {
        self.candidate_count
    }

    pub fn mean_length(&self) -> f32 {
        self.mean_length
    }
}
