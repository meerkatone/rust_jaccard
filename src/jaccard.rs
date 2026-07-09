use crate::BinaryFeatures;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct JaccardSimilarity {
    pub chunk_4_similarity: f64,
    pub chunk_16_similarity: f64,
    pub chunk_8_similarity: f64,
    pub overall_similarity: f64,
}

pub struct JaccardAnalyzer {
    pub chunk_4_weight: f64,
    pub chunk_16_weight: f64,
    pub chunk_8_weight: f64,
}

impl JaccardAnalyzer {
    pub fn new() -> Self {
        Self {
            chunk_4_weight: 0.4,
            chunk_16_weight: 0.4,
            chunk_8_weight: 0.2,
        }
    }

    pub fn with_weights(chunk_4_weight: f64, chunk_16_weight: f64, chunk_8_weight: f64) -> Self {
        Self {
            chunk_4_weight,
            chunk_16_weight,
            chunk_8_weight,
        }
    }

    pub fn calculate_similarity(
        &self,
        binary1: &BinaryFeatures,
        binary2: &BinaryFeatures,
    ) -> JaccardSimilarity {
        let chunk_4_similarity =
            self.jaccard_coefficient(&binary1.chunk_4_hashes, &binary2.chunk_4_hashes);

        let chunk_16_similarity =
            self.jaccard_coefficient(&binary1.chunk_16_hashes, &binary2.chunk_16_hashes);

        let chunk_8_similarity =
            self.jaccard_coefficient(&binary1.chunk_8_hashes, &binary2.chunk_8_hashes);

        let overall_similarity = (chunk_4_similarity * self.chunk_4_weight)
            + (chunk_16_similarity * self.chunk_16_weight)
            + (chunk_8_similarity * self.chunk_8_weight);

        JaccardSimilarity {
            chunk_4_similarity,
            chunk_16_similarity,
            chunk_8_similarity,
            overall_similarity,
        }
    }

    fn jaccard_coefficient(&self, set1: &HashSet<u64>, set2: &HashSet<u64>) -> f64 {
        if set1.is_empty() && set2.is_empty() {
            return 1.0;
        }

        let intersection_size = set1.intersection(set2).count();
        let union_size = set1.union(set2).count();

        if union_size == 0 {
            0.0
        } else {
            intersection_size as f64 / union_size as f64
        }
    }

    pub fn calculate_batch_similarities(
        &self,
        reference: &BinaryFeatures,
        binaries: &[BinaryFeatures],
    ) -> Vec<(String, JaccardSimilarity)> {
        binaries
            .iter()
            .map(|binary| {
                let similarity = self.calculate_similarity(reference, binary);
                (binary.name.clone(), similarity)
            })
            .collect()
    }
}

impl Default for JaccardAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaccard_coefficient_identical() {
        let analyzer = JaccardAnalyzer::new();
        let set1: HashSet<u64> = [1, 2, 3, 4].iter().cloned().collect();
        let set2: HashSet<u64> = [1, 2, 3, 4].iter().cloned().collect();

        let coefficient = analyzer.jaccard_coefficient(&set1, &set2);
        assert_eq!(coefficient, 1.0);
    }

    #[test]
    fn test_jaccard_coefficient_disjoint() {
        let analyzer = JaccardAnalyzer::new();
        let set1: HashSet<u64> = [1, 2, 3].iter().cloned().collect();
        let set2: HashSet<u64> = [4, 5, 6].iter().cloned().collect();

        let coefficient = analyzer.jaccard_coefficient(&set1, &set2);
        assert_eq!(coefficient, 0.0);
    }

    #[test]
    fn test_jaccard_coefficient_partial_overlap() {
        let analyzer = JaccardAnalyzer::new();
        let set1: HashSet<u64> = [1, 2, 3, 4].iter().cloned().collect();
        let set2: HashSet<u64> = [3, 4, 5, 6].iter().cloned().collect();

        let coefficient = analyzer.jaccard_coefficient(&set1, &set2);
        assert_eq!(coefficient, 2.0 / 6.0);
    }

    #[test]
    fn test_jaccard_coefficient_empty_sets() {
        let analyzer = JaccardAnalyzer::new();
        let set1: HashSet<u64> = HashSet::new();
        let set2: HashSet<u64> = HashSet::new();

        let coefficient = analyzer.jaccard_coefficient(&set1, &set2);
        assert_eq!(coefficient, 1.0);
    }
}
