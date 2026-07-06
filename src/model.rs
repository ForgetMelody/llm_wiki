use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChunkDraft {
    pub heading_path: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub chunk_id: String,
    pub doc_path: String,
    pub ordinal: i64,
    pub heading_path: String,
    pub chunk_hash: String,
    pub text: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct DocumentManifest {
    pub file_hash: String,
    pub embedding_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchHit {
    pub doc_path: String,
    pub heading_path: String,
    pub score: f32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResponse {
    pub query: String,
    pub total_chunks: usize,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocumentResponse {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexStats {
    pub scanned_docs: usize,
    pub updated_docs: usize,
    pub skipped_docs: usize,
    pub deleted_docs: usize,
    pub total_chunks: usize,
}
