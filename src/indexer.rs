use std::collections::HashSet;

use anyhow::Result;

use crate::{
    config::AppConfig,
    db::IndexDatabase,
    embed::EmbeddingEngine,
    hash::sha256_hex,
    markdown,
    model::{ChunkRecord, IndexStats},
};

pub struct Indexer<'a> {
    config: &'a AppConfig,
    db: &'a IndexDatabase,
    embedder: &'a EmbeddingEngine,
}

impl<'a> Indexer<'a> {
    pub fn new(
        config: &'a AppConfig,
        db: &'a IndexDatabase,
        embedder: &'a EmbeddingEngine,
    ) -> Self {
        Self {
            config,
            db,
            embedder,
        }
    }

    /// 扫描全部 Markdown，并只更新发生变化的文档。
    pub fn reindex_all(&self) -> Result<IndexStats> {
        self.db.init()?;
        let files = markdown::discover_markdown_files(self.config)?;
        let known_manifests = self.db.document_manifests()?;
        let embedding_fingerprint = self.config.embedding_fingerprint();
        let mut seen_paths = HashSet::new();

        let mut stats = IndexStats {
            scanned_docs: files.len(),
            updated_docs: 0,
            skipped_docs: 0,
            deleted_docs: 0,
            total_chunks: 0,
        };

        for path in files {
            let doc = markdown::load_document(&self.config.knowledge_root, &path)?;
            let file_hash = sha256_hex(doc.content.as_bytes());
            let previous_manifest = known_manifests.get(&doc.relative_path);
            seen_paths.insert(doc.relative_path.clone());

            if previous_manifest.is_some_and(|manifest| {
                manifest.file_hash == file_hash
                    && manifest.embedding_fingerprint == embedding_fingerprint
            }) {
                stats.skipped_docs += 1;
                continue;
            }

            let chunk_drafts = markdown::chunk_document(&doc, self.config.chunk_char_limit);
            let embeddings = self.embedder.embed_passages(
                &chunk_drafts
                    .iter()
                    .map(|chunk| chunk.text.clone())
                    .collect::<Vec<_>>(),
            )?;

            let chunks = chunk_drafts
                .into_iter()
                .zip(embeddings.into_iter())
                .enumerate()
                .map(|(ordinal, (chunk, embedding))| {
                    let chunk_hash = sha256_hex(chunk.text.as_bytes());
                    ChunkRecord {
                        chunk_id: sha256_hex(format!(
                            "{}:{}:{}:{}",
                            doc.relative_path, ordinal, chunk.heading_path, chunk_hash
                        )),
                        doc_path: doc.relative_path.clone(),
                        ordinal: ordinal as i64,
                        heading_path: chunk.heading_path,
                        chunk_hash,
                        text: chunk.text,
                        embedding,
                    }
                })
                .collect::<Vec<_>>();

            self.db.replace_document(
                &doc.relative_path,
                &file_hash,
                &embedding_fingerprint,
                &chunks,
            )?;
            stats.updated_docs += 1;
        }

        let deleted_paths = self
            .db
            .document_paths()?
            .difference(&seen_paths)
            .cloned()
            .collect::<Vec<_>>();
        stats.deleted_docs = self.db.delete_documents(&deleted_paths)?;
        stats.total_chunks = self.db.total_chunks()?;

        Ok(stats)
    }
}
