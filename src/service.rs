use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::{
    config::AppConfig,
    db::IndexDatabase,
    embed::{EmbeddingEngine, cosine_similarity},
    indexer::Indexer,
    model::{DocumentResponse, SearchHit, SearchResponse},
};

#[derive(Clone)]
pub struct KnowledgeService {
    config: AppConfig,
    db: IndexDatabase,
    embedder: EmbeddingEngine,
}

impl KnowledgeService {
    pub fn new(config: AppConfig) -> Result<Self> {
        let db = IndexDatabase::new(&config.database_path);
        db.init()?;
        let embedder = EmbeddingEngine::new(&config)?;
        Ok(Self {
            config,
            db,
            embedder,
        })
    }

    pub fn reindex_all(&self) -> Result<crate::model::IndexStats> {
        Indexer::new(&self.config, &self.db, &self.embedder).reindex_all()
    }

    /// 向量搜索当前知识库中的全部块。
    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<SearchResponse> {
        let query_embedding = self.embedder.embed_query(query)?;
        let chunks = self.db.load_all_chunks()?;
        let total_chunks = chunks.len();
        let limit = limit.unwrap_or(self.config.search_limit);

        let mut hits = chunks
            .into_iter()
            .map(|chunk| SearchHit {
                doc_path: chunk.doc_path,
                heading_path: chunk.heading_path,
                score: cosine_similarity(&query_embedding, &chunk.embedding),
                text: chunk.text,
            })
            .collect::<Vec<_>>();

        hits.sort_by(|lhs, rhs| rhs.score.total_cmp(&lhs.score));
        hits.truncate(limit);

        Ok(SearchResponse {
            query: query.to_string(),
            total_chunks,
            hits,
        })
    }

    /// 直接返回 Markdown 原文，便于 agent 按路径精读。
    pub fn read_document(&self, relative_path: &str) -> Result<DocumentResponse> {
        let path = self.resolve_relative_path(relative_path)?;
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(DocumentResponse {
            path: relative_path.to_string(),
            content,
        })
    }

    fn resolve_relative_path(&self, relative_path: &str) -> Result<PathBuf> {
        let relative = Path::new(relative_path);
        if relative.is_absolute() {
            bail!("path must be relative to knowledge_root");
        }
        if relative
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            bail!("path must not contain parent traversal");
        }

        let joined = self.config.knowledge_root.join(relative);
        let canonical = joined
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", joined.display()))?;
        let root = self.config.knowledge_root.canonicalize().with_context(|| {
            format!(
                "failed to resolve knowledge root {}",
                self.config.knowledge_root.display()
            )
        })?;
        if !canonical.starts_with(&root) {
            bail!("path escapes knowledge_root");
        }

        Ok(canonical)
    }
}
