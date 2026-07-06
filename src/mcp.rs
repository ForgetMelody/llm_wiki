use anyhow::Result;
use rmcp::{
    Json, ServiceExt, handler::server::wrapper::Parameters, tool, tool_router, transport::stdio,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    model::{DocumentResponse, IndexStats, SearchResponse},
    service::KnowledgeService,
};

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchParams {
    /// 查询文本
    query: String,
    /// 返回结果数量；未传时使用配置默认值
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadParams {
    /// 相对于 knowledge_root 的 Markdown 路径
    path: String,
}

#[derive(Clone)]
pub struct WikiMcpServer {
    service: KnowledgeService,
}

impl WikiMcpServer {
    pub fn new(service: KnowledgeService) -> Self {
        Self { service }
    }
}

#[tool_router(server_handler)]
impl WikiMcpServer {
    #[tool(description = "Search the indexed Markdown knowledge base")]
    fn search_knowledge(
        &self,
        Parameters(SearchParams { query, limit }): Parameters<SearchParams>,
    ) -> Result<Json<SearchResponse>, String> {
        self.service
            .search(&query, limit)
            .map(Json)
            .map_err(|err| err.to_string())
    }

    #[tool(description = "Read a Markdown document from the knowledge root")]
    fn read_document(
        &self,
        Parameters(ReadParams { path }): Parameters<ReadParams>,
    ) -> Result<Json<DocumentResponse>, String> {
        self.service
            .read_document(&path)
            .map(Json)
            .map_err(|err| err.to_string())
    }

    #[tool(description = "Reindex the full Markdown tree incrementally")]
    fn reindex_all(&self) -> Result<Json<IndexStats>, String> {
        self.service
            .reindex_all()
            .map(Json)
            .map_err(|err| err.to_string())
    }
}

pub async fn serve_stdio(service: KnowledgeService) -> Result<()> {
    let server = WikiMcpServer::new(service).serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
