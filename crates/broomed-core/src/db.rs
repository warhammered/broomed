use crate::error::CoreError;

pub const SCHEMA_VERSION: u32 = 1;

const SCHEMA_SQL: &str = include_str!("../migrations/0001_files_index.sql");

pub fn create_schema(conn: &rusqlite::Connection) -> Result<(), CoreError> {
    conn.execute_batch(SCHEMA_SQL)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn schema_creates_in_memory() {
        let conn = Connection::open_in_memory().expect("open :memory:");
        create_schema(&conn).expect("create_schema");
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for t in &[
            "files",
            "file_metadata",
            "file_embeddings",
            "file_categories",
            "operations",
            "operation_items",
            "ai_requests",
            "directories",
            "settings",
        ] {
            assert!(
                tables.contains(&t.to_string()),
                "missing table {t}: {tables:?}"
            );
        }
        // indexes
        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for idx in &[
            "idx_files_hash",
            "idx_files_parent_directory",
            "idx_files_mime_type",
        ] {
            assert!(
                indexes.contains(&idx.to_string()),
                "missing index {idx}: {indexes:?}"
            );
        }
    }
}
