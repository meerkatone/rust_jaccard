use crate::jaccard::JaccardSimilarity;
use anyhow::{Context, Result};
use arrow::array::{Array, Float64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use std::fs::File;
use std::sync::Arc;

pub struct ParquetExporter {
    writer_properties: WriterProperties,
}

#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub binary1: String,
    pub binary2: String,
    pub pair_path: String,
    pub similarity: JaccardSimilarity,
}

impl ParquetExporter {
    pub fn new() -> Self {
        let writer_properties = WriterProperties::builder()
            .set_compression(parquet::basic::Compression::SNAPPY)
            .build();

        Self { writer_properties }
    }

    pub fn export_results(&self, results: &[ComparisonResult], output_path: &str) -> Result<()> {
        let schema = self.create_schema();
        let record_batch = self.create_record_batch(results, &schema)?;

        let file = File::create(output_path).context("Failed to create output file")?;

        let mut writer = ArrowWriter::try_new(file, schema, Some(self.writer_properties.clone()))
            .context("Failed to create Arrow writer")?;

        writer
            .write(&record_batch)
            .context("Failed to write record batch")?;

        writer.close().context("Failed to close writer")?;

        Ok(())
    }

    fn create_schema(&self) -> Arc<Schema> {
        let fields = vec![
            Field::new("binary1", DataType::Utf8, false),
            Field::new("binary2", DataType::Utf8, false),
            Field::new("binary_pair", DataType::Utf8, false),
            Field::new("jaccard_index", DataType::Float64, false),
            Field::new("chunk_4_similarity", DataType::Float64, false),
            Field::new("chunk_16_similarity", DataType::Float64, false),
            Field::new("chunk_8_similarity", DataType::Float64, false),
        ];

        Arc::new(Schema::new(fields))
    }

    fn create_record_batch(
        &self,
        results: &[ComparisonResult],
        schema: &Arc<Schema>,
    ) -> Result<RecordBatch> {
        let mut binary1_names = Vec::new();
        let mut binary2_names = Vec::new();
        let mut binary_pairs = Vec::new();
        let mut jaccard_indices = Vec::new();
        let mut instruction_similarities = Vec::new();
        let mut function_similarities = Vec::new();
        let mut basic_block_similarities = Vec::new();

        for result in results {
            let similarity = &result.similarity;
            binary1_names.push(result.binary1.as_str());
            binary2_names.push(result.binary2.as_str());
            binary_pairs.push(format!("{}|{}", result.binary1, result.binary2));
            jaccard_indices.push(similarity.overall_similarity);
            instruction_similarities.push(similarity.chunk_4_similarity);
            function_similarities.push(similarity.chunk_16_similarity);
            basic_block_similarities.push(similarity.chunk_8_similarity);
        }

        let columns: Vec<Arc<dyn Array>> = vec![
            Arc::new(StringArray::from(binary1_names)),
            Arc::new(StringArray::from(binary2_names)),
            Arc::new(StringArray::from(binary_pairs)),
            Arc::new(Float64Array::from(jaccard_indices)),
            Arc::new(Float64Array::from(instruction_similarities)),
            Arc::new(Float64Array::from(function_similarities)),
            Arc::new(Float64Array::from(basic_block_similarities)),
        ];

        RecordBatch::try_new(schema.clone(), columns).context("Failed to create record batch")
    }

    pub fn export_detailed_results(
        &self,
        results: &[ComparisonResult],
        _metadata: &[(&str, &str)],
        output_path: &str,
    ) -> Result<()> {
        let schema = self.create_detailed_schema();
        let record_batch = self.create_detailed_record_batch(results, _metadata, &schema)?;

        let file = File::create(output_path).context("Failed to create output file")?;

        let mut writer = ArrowWriter::try_new(file, schema, Some(self.writer_properties.clone()))
            .context("Failed to create Arrow writer")?;

        writer
            .write(&record_batch)
            .context("Failed to write record batch")?;

        writer.close().context("Failed to close writer")?;

        Ok(())
    }

    fn create_detailed_schema(&self) -> Arc<Schema> {
        let fields = vec![
            Field::new("binary_name", DataType::Utf8, false),
            Field::new("binary_path", DataType::Utf8, false),
            Field::new("chunk_4_similarity", DataType::Float64, false),
            Field::new("chunk_16_similarity", DataType::Float64, false),
            Field::new("chunk_8_similarity", DataType::Float64, false),
            Field::new("overall_similarity", DataType::Float64, false),
            Field::new("analysis_timestamp", DataType::Utf8, false),
            Field::new("analyzer_version", DataType::Utf8, false),
        ];

        Arc::new(Schema::new(fields))
    }

    fn create_detailed_record_batch(
        &self,
        results: &[ComparisonResult],
        _metadata: &[(&str, &str)],
        schema: &Arc<Schema>,
    ) -> Result<RecordBatch> {
        let mut binary_names = Vec::new();
        let mut binary_paths = Vec::new();
        let mut instruction_similarities = Vec::new();
        let mut function_similarities = Vec::new();
        let mut basic_block_similarities = Vec::new();
        let mut overall_similarities = Vec::new();
        let mut timestamps = Vec::new();
        let mut versions = Vec::new();

        let timestamp = chrono::Utc::now().to_rfc3339();
        let version = env!("CARGO_PKG_VERSION");

        for result in results {
            let similarity = &result.similarity;
            binary_names.push(result.binary2.as_str());
            binary_paths.push(result.pair_path.as_str());
            instruction_similarities.push(similarity.chunk_4_similarity);
            function_similarities.push(similarity.chunk_16_similarity);
            basic_block_similarities.push(similarity.chunk_8_similarity);
            overall_similarities.push(similarity.overall_similarity);
            timestamps.push(timestamp.as_str());
            versions.push(version);
        }

        let columns: Vec<Arc<dyn Array>> = vec![
            Arc::new(StringArray::from(binary_names)),
            Arc::new(StringArray::from(binary_paths)),
            Arc::new(Float64Array::from(instruction_similarities)),
            Arc::new(Float64Array::from(function_similarities)),
            Arc::new(Float64Array::from(basic_block_similarities)),
            Arc::new(Float64Array::from(overall_similarities)),
            Arc::new(StringArray::from(timestamps)),
            Arc::new(StringArray::from(versions)),
        ];

        RecordBatch::try_new(schema.clone(), columns)
            .context("Failed to create detailed record batch")
    }
}

impl Default for ParquetExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_export_empty_results() {
        let exporter = ParquetExporter::new();
        let results = vec![];
        let temp_file = NamedTempFile::new().unwrap();
        let output_path = temp_file.path().to_str().unwrap();

        let result = exporter.export_results(&results, output_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_export_single_result() {
        let exporter = ParquetExporter::new();
        let similarity = JaccardSimilarity {
            chunk_4_similarity: 0.5,
            chunk_16_similarity: 0.6,
            chunk_8_similarity: 0.7,
            overall_similarity: 0.6,
        };
        let results = vec![ComparisonResult {
            binary1: "reference|name.exe".to_string(),
            binary2: "test|name.exe".to_string(),
            pair_path: "/path/reference <-> /path/test".to_string(),
            similarity,
        }];
        let temp_file = NamedTempFile::new().unwrap();
        let output_path = temp_file.path().to_str().unwrap();

        let result = exporter.export_results(&results, output_path);
        assert!(result.is_ok());

        let schema = exporter.create_schema();
        let batch = exporter.create_record_batch(&results, &schema).unwrap();
        let binary1 = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let binary2 = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(binary1.value(0), "reference|name.exe");
        assert_eq!(binary2.value(0), "test|name.exe");
    }
}
