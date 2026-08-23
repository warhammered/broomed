use crate::error::CoreError;

// ponytail: LIKE first; add FTS5/embeddings when LIKE measurably slow

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub raw: String,
    pub tags: Vec<String>,
    pub mime: Option<String>,
    pub path_prefix: Option<String>,
}

pub fn parse_query(raw: &str) -> SearchQuery {
    let mut tags = Vec::new();
    let mut mime = None;
    let mut path_prefix = None;
    let mut free: Vec<String> = Vec::new();
    for token in raw.split_whitespace() {
        if let Some(v) = token.strip_prefix("tag:") {
            if !v.is_empty() {
                tags.push(v.to_string());
            }
        } else if let Some(v) = token.strip_prefix("type:") {
            if !v.is_empty() {
                mime = Some(v.to_string());
            }
        } else if let Some(v) = token.strip_prefix("mime:") {
            if !v.is_empty() {
                mime = Some(v.to_string());
            }
        } else if let Some(v) = token.strip_prefix("path:") {
            if !v.is_empty() {
                path_prefix = Some(v.to_string());
            }
        } else {
            free.push(token.to_string());
        }
    }
    SearchQuery {
        raw: free.join(" "),
        tags,
        mime,
        path_prefix,
    }
}

pub fn search_files(
    conn: &rusqlite::Connection,
    q: &SearchQuery,
    limit: usize,
) -> Result<Vec<String>, CoreError> {
    let mut sql = String::from("SELECT path FROM files WHERE 1=1");
    let mut params: Vec<String> = Vec::new();

    if !q.raw.is_empty() {
        sql.push_str(" AND (path LIKE ? OR filename LIKE ?)");
        let like = format!("%{}%", q.raw);
        params.push(like.clone());
        params.push(like);
    }
    if let Some(m) = &q.mime {
        sql.push_str(" AND mime_type LIKE ?");
        params.push(format!("%{}%", m));
    }
    if let Some(p) = &q.path_prefix {
        sql.push_str(" AND path LIKE ?");
        params.push(format!("{}%", p));
    }
    if !q.tags.is_empty() {
        let placeholders = std::iter::repeat_n("?", q.tags.len())
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM file_categories fc WHERE fc.file_id = files.id AND fc.category IN ({}))",
            placeholders
        ));
        for t in &q.tags {
            params.push(t.clone());
        }
    }
    sql.push_str(&format!(" ORDER BY path LIMIT {}", limit));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| row.get(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_schema;
    use rusqlite::{params, Connection};

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("open :memory:");
        create_schema(&conn).expect("create_schema");
        // 3 files
        conn.execute(
            "INSERT INTO files (id, path, filename, mime_type, parent_directory) VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["id1", "/a/photo.jpg", "photo.jpg", "image/jpeg", "/a"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files (id, path, filename, mime_type, parent_directory) VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["id2", "/b/report.pdf", "report.pdf", "application/pdf", "/b"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files (id, path, filename, mime_type, parent_directory) VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["id3", "/a/music.mp3", "music.mp3", "audio/mpeg", "/a"],
        )
        .unwrap();
        // tags via file_categories (category = tag)
        conn.execute(
            "INSERT INTO file_categories (file_id, category) VALUES (?1, ?2)",
            params!["id1", "vacation"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO file_categories (file_id, category) VALUES (?1, ?2)",
            params!["id2", "work"],
        )
        .unwrap();
        conn
    }

    #[test]
    fn parse_query_splits_prefixes() {
        let q = parse_query("hello tag:work type:image path:/a");
        assert_eq!(q.raw, "hello");
        assert_eq!(q.tags, vec!["work"]);
        assert_eq!(q.mime.as_deref(), Some("image"));
        assert_eq!(q.path_prefix.as_deref(), Some("/a"));
    }

    #[test]
    fn parse_query_only_free_text() {
        let q = parse_query("my photo");
        assert_eq!(q.raw, "my photo");
        assert!(q.tags.is_empty());
        assert!(q.mime.is_none());
    }

    #[test]
    fn parse_query_empty() {
        let q = parse_query("");
        assert_eq!(q.raw, "");
        assert!(q.tags.is_empty());
    }

    #[test]
    fn search_by_name() {
        let conn = setup();
        let q = parse_query("photo");
        let res = search_files(&conn, &q, 10).unwrap();
        assert_eq!(res, vec!["/a/photo.jpg"]);
    }

    #[test]
    fn search_by_mime() {
        let conn = setup();
        let q = parse_query("type:image");
        // parse_query puts "image" into mime, raw empty -> mime LIKE %image%
        let res = search_files(&conn, &q, 10).unwrap();
        assert_eq!(res, vec!["/a/photo.jpg"]);
        // also direct mime pdf
        let q2 = parse_query("type:pdf");
        let res2 = search_files(&conn, &q2, 10).unwrap();
        assert_eq!(res2, vec!["/b/report.pdf"]);
    }

    #[test]
    fn search_by_tag() {
        let conn = setup();
        let q = parse_query("tag:work");
        let res = search_files(&conn, &q, 10).unwrap();
        assert_eq!(res, vec!["/b/report.pdf"]);
        let q2 = parse_query("tag:vacation");
        let res2 = search_files(&conn, &q2, 10).unwrap();
        assert_eq!(res2, vec!["/a/photo.jpg"]);
    }

    #[test]
    fn search_path_prefix() {
        let conn = setup();
        let q = parse_query("path:/a");
        let res = search_files(&conn, &q, 10).unwrap();
        assert_eq!(res, vec!["/a/music.mp3", "/a/photo.jpg"]);
    }

    #[test]
    fn search_combined_tag_and_text() {
        let conn = setup();
        // raw "report" + tag work should return report.pdf
        let q = parse_query("report tag:work");
        let res = search_files(&conn, &q, 10).unwrap();
        assert_eq!(res, vec!["/b/report.pdf"]);
        // mismatched tag should return empty
        let q2 = parse_query("report tag:vacation");
        let res2 = search_files(&conn, &q2, 10).unwrap();
        assert!(res2.is_empty());
    }

    #[test]
    fn search_limit() {
        let conn = setup();
        let q = parse_query("");
        let res = search_files(&conn, &q, 2).unwrap();
        assert_eq!(res.len(), 2);
    }
}
