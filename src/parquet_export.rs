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

impl ParquetExporter {
    pub fn new() -> Self {
        let writer_properties = WriterProperties::builder()
            .set_compression(parquet::basic::Compression::SNAPPY)
            .build();

        Self { writer_properties }
    }

    pub fn export_results(
        &self,
        results: &[(String, String, JaccardSimilarity)],
        output_path: &str,
    ) -> Result<()> {
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
            Field::new("instruction_similarity", DataType::Float64, false),
            Field::new("function_similarity", DataType::Float64, false),
            Field::new("basic_block_similarity", DataType::Float64, false),
        ];

        Arc::new(Schema::new(fields))
    }

    fn create_record_batch(
        &self,
        results: &[(String, String, JaccardSimilarity)],
        schema: &Arc<Schema>,
    ) -> Result<RecordBatch> {
        let mut binary1_names = Vec::new();
        let mut binary2_names = Vec::new();
        let mut binary_pairs = Vec::new();
        let mut jaccard_indices = Vec::new();
        let mut instruction_similarities = Vec::new();
        let mut function_similarities = Vec::new();
        let mut basic_block_similarities = Vec::new();

        for (pair_name, _path, similarity) in results {
            // Extract binary names from pair name (format: "binary1|binary2")
            let parts: Vec<&str> = pair_name.split('|').collect();
            if parts.len() == 2 {
                binary1_names.push(parts[0]);
                binary2_names.push(parts[1]);
            } else {
                // Fallback for other formats
                binary1_names.push(pair_name.as_str());
                binary2_names.push("");
            }

            binary_pairs.push(pair_name.as_str());
            jaccard_indices.push(similarity.overall_similarity);
            instruction_similarities.push(similarity.instruction_similarity);
            function_similarities.push(similarity.function_similarity);
            basic_block_similarities.push(similarity.basic_block_similarity);
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
        results: &[(String, String, JaccardSimilarity)],
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
            Field::new("instruction_similarity", DataType::Float64, false),
            Field::new("function_similarity", DataType::Float64, false),
            Field::new("basic_block_similarity", DataType::Float64, false),
            Field::new("overall_similarity", DataType::Float64, false),
            Field::new("analysis_timestamp", DataType::Utf8, false),
            Field::new("analyzer_version", DataType::Utf8, false),
        ];

        Arc::new(Schema::new(fields))
    }

    fn create_detailed_record_batch(
        &self,
        results: &[(String, String, JaccardSimilarity)],
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

        for (name, path, similarity) in results {
            binary_names.push(name.as_str());
            binary_paths.push(path.as_str());
            instruction_similarities.push(similarity.instruction_similarity);
            function_similarities.push(similarity.function_similarity);
            basic_block_similarities.push(similarity.basic_block_similarity);
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
            instruction_similarity: 0.5,
            function_similarity: 0.6,
            basic_block_similarity: 0.7,
            overall_similarity: 0.6,
        };
        let results = vec![(
            "test.exe".to_string(),
            "/path/test.exe".to_string(),
            similarity,
        )];
        let temp_file = NamedTempFile::new().unwrap();
        let output_path = temp_file.path().to_str().unwrap();

        let result = exporter.export_results(&results, output_path);
        assert!(result.is_ok());
    }
}
