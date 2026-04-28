//! Semantic-index sidecar storage.
//!
//! The canonical archive keeps a mirror of semantic indexing state, but the
//! heavier vector search storage lives in sidecar tables/files that can be
//! rebuilt. This module owns that sidecar representation and query path.

use crate::{config::ProjectPaths, utils::sha256_hex};
use anyhow::{Context, Result};
use arrow_array::{
    Array, FixedSizeListArray, Float32Array, Float64Array, Int64Array, RecordBatch, StringArray,
    types::Float32Type,
};
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::{
    DistanceType, Table, connect,
    index::Index,
    query::{ExecutableQuery, QueryBase, Select},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone)]
pub struct SidecarEmbeddingRow {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub provider_id: String,
    pub model: String,
    pub content_hash: String,
    pub indexed_at: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SidecarSearchRow {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub score: f32,
}

pub async fn sync_provider_embeddings(
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
    rows: &[SidecarEmbeddingRow],
    full_rebuild: bool,
    clear_only: bool,
    removed_history_ids: &[i64],
) -> Result<usize> {
    let table_name = provider_table_name(provider_id, model);
    let database = connect_database(paths).await?;
    let table_exists =
        database.table_names().execute().await?.iter().any(|value| value == &table_name);

    if !table_exists {
        if clear_only || rows.is_empty() {
            return Ok(0);
        }
        let table =
            database.create_table(&table_name, vec![build_record_batch(rows)?]).execute().await?;
        maybe_create_vector_index(&table, rows.len()).await?;
        return Ok(rows.len());
    }

    let table = database.open_table(&table_name).execute().await?;
    if clear_only || full_rebuild {
        table.delete("true").await?;
    } else if !removed_history_ids.is_empty() {
        table.delete(&history_predicate(removed_history_ids)).await?;
    }

    if rows.is_empty() {
        return Ok(0);
    }

    let history_ids = rows.iter().map(|row| row.history_id).collect::<Vec<_>>();
    table.delete(&history_predicate(&history_ids)).await?;
    table.add(vec![build_record_batch(rows)?]).execute().await?;
    maybe_create_vector_index(&table, table.count_rows(None).await?).await?;
    Ok(rows.len())
}

pub async fn clear_provider_embeddings(
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
) -> Result<usize> {
    let database = connect_database(paths).await?;
    let table_name = provider_table_name(provider_id, model);
    let table_names = database.table_names().execute().await?;
    if !table_names.iter().any(|value| value == &table_name) {
        return Ok(0);
    }
    let table = database.open_table(&table_name).execute().await?;
    let count = table.count_rows(None).await?;
    table.delete("true").await?;
    Ok(count)
}

pub async fn count_provider_embeddings(
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
) -> Result<usize> {
    let database = connect_database(paths).await?;
    let table_name = provider_table_name(provider_id, model);
    let table_names = database.table_names().execute().await?;
    if !table_names.iter().any(|value| value == &table_name) {
        return Ok(0);
    }
    let table = database.open_table(&table_name).execute().await?;
    table.count_rows(None).await.context("counting sidecar rows")
}

pub async fn search_provider_embeddings(
    paths: &ProjectPaths,
    provider_id: &str,
    model: &str,
    query_vector: &[f32],
    profile_id: Option<&str>,
    domain: Option<&str>,
    limit: usize,
) -> Result<Option<Vec<SidecarSearchRow>>> {
    let database = connect_database(paths).await?;
    let table_name = provider_table_name(provider_id, model);
    let table_names = database.table_names().execute().await?;
    if !table_names.iter().any(|value| value == &table_name) {
        return Ok(None);
    }

    let table = database.open_table(&table_name).execute().await?;
    let mut query = table
        .query()
        .select(Select::columns(&[
            "history_id",
            "profile_id",
            "url",
            "title",
            "domain",
            "visited_at",
            "_distance",
        ]))
        .limit(limit.max(1));
    if let Some(filter) = sidecar_filter(profile_id, domain) {
        query = query.only_if(filter);
    }

    let stream = query
        .nearest_to(query_vector.to_vec())?
        .distance_type(DistanceType::Cosine)
        .refine_factor(5)
        .execute()
        .await
        .context("querying LanceDB semantic sidecar")?;
    let batches: Vec<RecordBatch> = stream.try_collect().await?;

    let mut rows = Vec::new();
    for batch in batches {
        rows.extend(extract_search_rows(&batch)?);
    }
    Ok(Some(rows))
}

/// Returns the total bytes currently consumed by sidecar vector storage.
pub fn sidecar_storage_bytes(paths: &ProjectPaths) -> u64 {
    directory_size(&sidecar_root(paths))
}

/// Returns the root directory where sidecar vector tables live.
pub fn sidecar_root(paths: &ProjectPaths) -> PathBuf {
    paths.semantic_index_dir.clone()
}

/// Derives the stable sidecar table name for one provider/model pair.
pub fn provider_table_name(provider_id: &str, model: &str) -> String {
    let provider = provider_id
        .chars()
        .map(|value| if value.is_ascii_alphanumeric() { value } else { '_' })
        .collect::<String>();
    let digest = sha256_hex(format!("{provider_id}::{model}").as_bytes());
    format!("pathkeep_{provider}_{}", &digest[..12])
}

async fn connect_database(paths: &ProjectPaths) -> Result<lancedb::Connection> {
    let root = sidecar_root(paths);
    std::fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
    connect(&root.display().to_string()).execute().await.context("opening LanceDB sidecar")
}

fn schema(dimensions: usize) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("history_id", DataType::Int64, false),
        Field::new("profile_id", DataType::Utf8, false),
        Field::new("url", DataType::Utf8, false),
        Field::new("title", DataType::Utf8, true),
        Field::new("domain", DataType::Utf8, false),
        Field::new("visited_at", DataType::Utf8, false),
        Field::new("provider_id", DataType::Utf8, false),
        Field::new("model", DataType::Utf8, false),
        Field::new("content_hash", DataType::Utf8, false),
        Field::new("indexed_at", DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dimensions as i32,
            ),
            true,
        ),
    ]))
}

fn build_record_batch(rows: &[SidecarEmbeddingRow]) -> Result<RecordBatch> {
    let dimensions = rows
        .first()
        .map(|row| row.vector.len())
        .context("building a LanceDB record batch requires at least one row")?;
    let schema = schema(dimensions);
    let history_ids =
        Arc::new(Int64Array::from(rows.iter().map(|row| row.history_id).collect::<Vec<_>>()));
    let profile_ids = Arc::new(StringArray::from(
        rows.iter().map(|row| Some(row.profile_id.as_str())).collect::<Vec<_>>(),
    ));
    let urls = Arc::new(StringArray::from(
        rows.iter().map(|row| Some(row.url.as_str())).collect::<Vec<_>>(),
    ));
    let titles = Arc::new(StringArray::from(
        rows.iter().map(|row| row.title.as_deref()).collect::<Vec<_>>(),
    ));
    let domains = Arc::new(StringArray::from(
        rows.iter().map(|row| Some(row.domain.as_str())).collect::<Vec<_>>(),
    ));
    let visited = Arc::new(StringArray::from(
        rows.iter().map(|row| Some(row.visited_at.as_str())).collect::<Vec<_>>(),
    ));
    let providers = Arc::new(StringArray::from(
        rows.iter().map(|row| Some(row.provider_id.as_str())).collect::<Vec<_>>(),
    ));
    let models = Arc::new(StringArray::from(
        rows.iter().map(|row| Some(row.model.as_str())).collect::<Vec<_>>(),
    ));
    let hashes = Arc::new(StringArray::from(
        rows.iter().map(|row| Some(row.content_hash.as_str())).collect::<Vec<_>>(),
    ));
    let indexed = Arc::new(StringArray::from(
        rows.iter().map(|row| Some(row.indexed_at.as_str())).collect::<Vec<_>>(),
    ));
    let embeddings = Arc::new(FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        rows.iter().map(|row| Some(row.vector.iter().copied().map(Some))),
        dimensions as i32,
    ));

    RecordBatch::try_new(
        schema,
        vec![
            history_ids,
            profile_ids,
            urls,
            titles,
            domains,
            visited,
            providers,
            models,
            hashes,
            indexed,
            embeddings,
        ],
    )
    .context("building LanceDB record batch")
}

fn history_predicate(history_ids: &[i64]) -> String {
    let ids = history_ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    format!("history_id IN ({ids})")
}

fn sidecar_filter(profile_id: Option<&str>, domain: Option<&str>) -> Option<String> {
    let mut filters = Vec::new();
    if let Some(profile_id) = profile_id.filter(|value| !value.trim().is_empty()) {
        filters.push(format!("profile_id = '{}'", sql_literal(profile_id)));
    }
    if let Some(domain) = domain.filter(|value| !value.trim().is_empty()) {
        filters.push(format!("domain LIKE '%{}%'", sql_literal(domain)));
    }
    (!filters.is_empty()).then(|| filters.join(" AND "))
}

fn sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn extract_search_rows(batch: &RecordBatch) -> Result<Vec<SidecarSearchRow>> {
    let history_ids = batch
        .column_by_name("history_id")
        .and_then(|column| column.as_any().downcast_ref::<Int64Array>())
        .context("missing history_id in sidecar query result")?;
    let profile_ids = batch
        .column_by_name("profile_id")
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .context("missing profile_id in sidecar query result")?;
    let urls = batch
        .column_by_name("url")
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .context("missing url in sidecar query result")?;
    let titles = batch
        .column_by_name("title")
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .context("missing title in sidecar query result")?;
    let domains = batch
        .column_by_name("domain")
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .context("missing domain in sidecar query result")?;
    let visited_at = batch
        .column_by_name("visited_at")
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .context("missing visited_at in sidecar query result")?;
    let distances =
        batch.column_by_name("_distance").context("missing _distance in sidecar query result")?;

    let mut rows = Vec::with_capacity(batch.num_rows());
    for index in 0..batch.num_rows() {
        let distance = f32_distance(distances.as_ref(), index)?;
        rows.push(SidecarSearchRow {
            history_id: history_ids.value(index),
            profile_id: profile_ids.value(index).to_string(),
            url: urls.value(index).to_string(),
            title: (!titles.is_null(index)).then(|| titles.value(index).to_string()),
            domain: domains.value(index).to_string(),
            visited_at: visited_at.value(index).to_string(),
            score: cosine_score_from_distance(distance),
        });
    }
    Ok(rows)
}

fn f32_distance(column: &dyn arrow_array::Array, index: usize) -> Result<f32> {
    if let Some(distances) = column.as_any().downcast_ref::<Float32Array>() {
        return Ok(distances.value(index));
    }
    if let Some(distances) = column.as_any().downcast_ref::<Float64Array>() {
        return Ok(distances.value(index) as f32);
    }
    anyhow::bail!("unsupported LanceDB _distance column type")
}

fn cosine_score_from_distance(distance: f32) -> f32 {
    (1.0 - distance).clamp(-1.0, 1.0)
}

fn directory_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() {
        return fs::metadata(path).map(|metadata| metadata.len()).unwrap_or_default();
    }
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| directory_size(&entry.path()))
                .sum::<u64>()
        })
        .unwrap_or_default()
}

async fn maybe_create_vector_index(table: &Table, row_count: usize) -> Result<()> {
    if row_count < 256 {
        return Ok(());
    }
    if table.index_stats("embedding").await?.is_none() {
        table.create_index(&["embedding"], Index::Auto).execute().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ProjectPaths, project_paths_with_root};
    use tempfile::tempdir;

    fn project_paths(root: &std::path::Path) -> ProjectPaths {
        project_paths_with_root(root)
    }

    fn row(history_id: i64, vector: &[f32]) -> SidecarEmbeddingRow {
        SidecarEmbeddingRow {
            history_id,
            profile_id: "chrome:Default".to_string(),
            url: format!("https://example.com/{history_id}"),
            title: Some(format!("Row {history_id}")),
            domain: "example.com".to_string(),
            visited_at: "2026-04-07T00:00:00Z".to_string(),
            provider_id: "embed".to_string(),
            model: "text-embedding-3-small".to_string(),
            content_hash: format!("hash-{history_id}"),
            indexed_at: "2026-04-07T00:00:00Z".to_string(),
            vector: vector.to_vec(),
        }
    }

    #[tokio::test]
    async fn sync_count_and_clear_sidecar_rows() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths(dir.path());
        assert_eq!(sidecar_storage_bytes(&paths), 0);
        assert_eq!(
            count_provider_embeddings(&paths, "embed", "text-embedding-3-small")
                .await
                .expect("missing table count"),
            0
        );
        let rows = vec![row(1, &[0.1, 0.2, 0.3]), row(2, &[0.3, 0.2, 0.1])];

        let indexed = sync_provider_embeddings(
            &paths,
            "embed",
            "text-embedding-3-small",
            &rows,
            false,
            false,
            &[],
        )
        .await
        .expect("sync");
        assert_eq!(indexed, 2);
        assert_eq!(
            count_provider_embeddings(&paths, "embed", "text-embedding-3-small")
                .await
                .expect("count"),
            2
        );

        let cleared = clear_provider_embeddings(&paths, "embed", "text-embedding-3-small")
            .await
            .expect("clear");
        assert_eq!(cleared, 2);
        assert_eq!(
            count_provider_embeddings(&paths, "embed", "text-embedding-3-small")
                .await
                .expect("count after clear"),
            0
        );
    }

    #[tokio::test]
    async fn incremental_sync_replaces_changed_rows_and_removes_deleted_history_ids() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths(dir.path());
        sync_provider_embeddings(
            &paths,
            "embed",
            "text-embedding-3-small",
            &[row(1, &[0.1, 0.2, 0.3]), row(2, &[0.3, 0.2, 0.1])],
            false,
            false,
            &[],
        )
        .await
        .expect("initial sync");

        sync_provider_embeddings(
            &paths,
            "embed",
            "text-embedding-3-small",
            &[row(2, &[0.9, 0.8, 0.7]), row(3, &[0.4, 0.5, 0.6])],
            false,
            false,
            &[1],
        )
        .await
        .expect("incremental sync");

        assert_eq!(
            count_provider_embeddings(&paths, "embed", "text-embedding-3-small")
                .await
                .expect("count after incremental"),
            2
        );
    }

    #[tokio::test]
    async fn search_sidecar_rows_honors_filters_and_returns_scores() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths(dir.path());
        sync_provider_embeddings(
            &paths,
            "embed",
            "text-embedding-3-small",
            &[
                row(1, &[1.0, 0.0, 0.0]),
                row(2, &[0.0, 1.0, 0.0]),
                SidecarEmbeddingRow {
                    profile_id: "arc:Profile-2".to_string(),
                    domain: "docs.example.com".to_string(),
                    ..row(3, &[0.9, 0.0, 0.1])
                },
            ],
            false,
            false,
            &[],
        )
        .await
        .expect("seed rows");

        let filtered = search_provider_embeddings(
            &paths,
            "embed",
            "text-embedding-3-small",
            &[1.0, 0.0, 0.0],
            Some("arc:Profile-2"),
            Some("docs.example.com"),
            5,
        )
        .await
        .expect("search")
        .expect("sidecar table");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].history_id, 3);
        assert!(filtered[0].score > 0.0);
        assert!(sidecar_storage_bytes(&paths) > 0);
    }

    #[tokio::test]
    async fn distance_and_large_index_helpers_cover_sidecar_edges() {
        let distances = Float64Array::from(vec![0.25_f64]);
        assert!((f32_distance(&distances, 0).expect("f64 distance") - 0.25).abs() < f32::EPSILON);
        let unsupported = StringArray::from(vec!["bad"]);
        let error = f32_distance(&unsupported, 0).expect_err("unsupported distance column");
        assert!(error.to_string().contains("unsupported LanceDB _distance column type"));

        let dir = tempdir().expect("tempdir");
        let paths = project_paths(dir.path());
        let rows =
            (0..260).map(|index| row(index, &[index as f32 / 260.0, 0.2, 0.3])).collect::<Vec<_>>();
        let indexed = sync_provider_embeddings(
            &paths,
            "embed",
            "text-embedding-3-small",
            &rows,
            true,
            false,
            &[],
        )
        .await
        .expect("large sync");
        assert_eq!(indexed, 260);
        assert_eq!(
            count_provider_embeddings(&paths, "embed", "text-embedding-3-small")
                .await
                .expect("large count"),
            260
        );
    }
}
