use crate::{config::ProjectPaths, utils::sha256_hex};
use anyhow::{Context, Result};
use arrow_array::{FixedSizeListArray, Int64Array, RecordBatch, StringArray, types::Float32Type};
use arrow_schema::{DataType, Field, Schema};
use lancedb::{Table, connect, index::Index};
use std::{path::PathBuf, sync::Arc};

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

pub fn sidecar_root(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join("sidecars").join("semantic-index")
}

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
    use crate::config::ProjectPaths;
    use tempfile::tempdir;

    fn project_paths(root: &std::path::Path) -> ProjectPaths {
        ProjectPaths {
            app_root: root.to_path_buf(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
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
}
