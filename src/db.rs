use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::model::{ChunkRecord, DocumentManifest};

#[derive(Debug, Clone)]
pub struct IndexDatabase {
    db_path: PathBuf,
}

impl IndexDatabase {
    pub fn new(db_path: impl AsRef<Path>) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
        }
    }

    /// 初始化 SQLite schema。
    pub fn init(&self) -> Result<()> {
        let conn = self.open()?;
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS documents (
                doc_path TEXT PRIMARY KEY,
                file_hash TEXT NOT NULL,
                embedding_fingerprint TEXT NOT NULL DEFAULT '',
                indexed_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks (
                chunk_id TEXT PRIMARY KEY,
                doc_path TEXT NOT NULL,
                ordinal INTEGER NOT NULL,
                heading_path TEXT NOT NULL,
                chunk_hash TEXT NOT NULL,
                text TEXT NOT NULL,
                embedding_json TEXT NOT NULL,
                FOREIGN KEY(doc_path) REFERENCES documents(doc_path) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_doc_path ON chunks(doc_path);
            "#,
        )?;
        self.ensure_document_columns(&conn)?;
        Ok(())
    }

    pub fn document_manifests(&self) -> Result<HashMap<String, DocumentManifest>> {
        let conn = self.open()?;
        let mut stmt =
            conn.prepare("SELECT doc_path, file_hash, embedding_fingerprint FROM documents")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                DocumentManifest {
                    file_hash: row.get(1)?,
                    embedding_fingerprint: row.get(2)?,
                },
            ))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (path, manifest) = row?;
            map.insert(path, manifest);
        }
        Ok(map)
    }

    pub fn document_paths(&self) -> Result<HashSet<String>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare("SELECT doc_path FROM documents")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut set = HashSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
    }

    /// 用事务替换单个文档及其全部块。
    pub fn replace_document(
        &self,
        doc_path: &str,
        file_hash: &str,
        embedding_fingerprint: &str,
        chunks: &[ChunkRecord],
    ) -> Result<()> {
        let mut conn = self.open()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO documents(doc_path, file_hash, embedding_fingerprint, indexed_at) VALUES (?1, ?2, ?3, unixepoch()) \
             ON CONFLICT(doc_path) DO UPDATE SET file_hash = excluded.file_hash, embedding_fingerprint = excluded.embedding_fingerprint, indexed_at = excluded.indexed_at",
            params![doc_path, file_hash, embedding_fingerprint],
        )?;
        tx.execute("DELETE FROM chunks WHERE doc_path = ?1", params![doc_path])?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO chunks(chunk_id, doc_path, ordinal, heading_path, chunk_hash, text, embedding_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for chunk in chunks {
                let embedding_json = serde_json::to_string(&chunk.embedding)?;
                stmt.execute(params![
                    chunk.chunk_id,
                    chunk.doc_path,
                    chunk.ordinal,
                    chunk.heading_path,
                    chunk.chunk_hash,
                    chunk.text,
                    embedding_json,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn delete_documents(&self, doc_paths: &[String]) -> Result<usize> {
        if doc_paths.is_empty() {
            return Ok(0);
        }

        let mut conn = self.open()?;
        let tx = conn.transaction()?;
        let mut deleted = 0;
        for doc_path in doc_paths {
            deleted += tx.execute(
                "DELETE FROM documents WHERE doc_path = ?1",
                params![doc_path],
            )?;
        }
        tx.commit()?;
        Ok(deleted)
    }

    pub fn load_all_chunks(&self) -> Result<Vec<ChunkRecord>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT chunk_id, doc_path, ordinal, heading_path, chunk_hash, text, embedding_json \
             FROM chunks ORDER BY doc_path, ordinal",
        )?;
        let rows = stmt.query_map([], |row| {
            let embedding_json: String = row.get(6)?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_json).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;

            Ok(ChunkRecord {
                chunk_id: row.get(0)?,
                doc_path: row.get(1)?,
                ordinal: row.get(2)?,
                heading_path: row.get(3)?,
                chunk_hash: row.get(4)?,
                text: row.get(5)?,
                embedding,
            })
        })?;

        let mut chunks = Vec::new();
        for row in rows {
            chunks.push(row?);
        }
        Ok(chunks)
    }

    pub fn total_chunks(&self) -> Result<usize> {
        let conn = self.open()?;
        let count = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok(count as usize)
    }

    fn open(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)
            .with_context(|| format!("failed to open sqlite {}", self.db_path.display()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(conn)
    }

    fn ensure_document_columns(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(documents)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut has_embedding_fingerprint = false;
        for row in rows {
            if row? == "embedding_fingerprint" {
                has_embedding_fingerprint = true;
                break;
            }
        }
        if !has_embedding_fingerprint {
            conn.execute(
                "ALTER TABLE documents ADD COLUMN embedding_fingerprint TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        Ok(())
    }
}
