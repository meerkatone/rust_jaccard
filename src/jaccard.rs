use std::collections::HashSet;
use crate::BinaryFeatures;

#[derive(Debug, Clone)]
pub struct JaccardSimilarity {
    pub instruction_similarity: f64,
    pub function_similarity: f64,
    pub basic_block_similarity: f64,
    pub overall_similarity: f64,
}

pub struct JaccardAnalyzer {
    pub instruction_weight: f64,
    pub function_weight: f64,
    pub basic_block_weight: f64,
}

impl JaccardAnalyzer {
    pub fn new() -> Self {
        Self {
            instruction_weight: 0.4,
            function_weight: 0.4,
            basic_block_weight: 0.2,
        }
    }

    pub fn with_weights(instruction_weight: f64, function_weight: f64, basic_block_weight: f64) -> Self {
        Self {
            instruction_weight,
            function_weight,
            basic_block_weight,
        }
    }

    pub fn calculate_similarity(&self, binary1: &BinaryFeatures, binary2: &BinaryFeatures) -> JaccardSimilarity {
        let instruction_similarity = self.jaccard_coefficient(
            &binary1.instruction_hashes,
            &binary2.instruction_hashes,
        );

        let function_similarity = self.jaccard_coefficient(
            &binary1.function_hashes,
            &binary2.function_hashes,
        );

        let basic_block_similarity = self.jaccard_coefficient(
            &binary1.basic_block_hashes,
            &binary2.basic_block_hashes,
        );

        let overall_similarity = (instruction_similarity * self.instruction_weight)
            + (function_similarity * self.function_weight)
            + (basic_block_similarity * self.basic_block_weight);

        JaccardSimilarity {
            instruction_similarity,
            function_similarity,
            basic_block_similarity,
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

    pub fn calculate_batch_similarities(&self, reference: &BinaryFeatures, binaries: &[BinaryFeatures]) -> Vec<(String, JaccardSimilarity)> {
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