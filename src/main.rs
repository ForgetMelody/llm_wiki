use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use llm_wiki::{config::AppConfig, mcp, service::KnowledgeService};

#[derive(Debug, Parser)]
#[command(name = "llm-wiki")]
#[command(about = "Markdown 知识库索引与 MCP 服务")]
struct Cli {
    /// 配置文件路径
    #[arg(long, default_value = "config/llm_wiki.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 执行一次全量扫描；仅重建变更文件
    Index,
    /// 搜索知识库
    Search {
        #[arg(long)]
        query: String,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// 读取知识库原始 Markdown
    Read {
        #[arg(long)]
        path: String,
    },
    /// 通过 stdio 启动 MCP 服务
    ServeMcp,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load(&cli.config)?;
    let service = KnowledgeService::new(config)?;

    match cli.command {
        Command::Index => {
            let stats = service.reindex_all()?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        Command::Search { query, limit } => {
            let result = service.search(&query, limit)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::Read { path } => {
            let result = service.read_document(&path)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::ServeMcp => {
            mcp::serve_stdio(service).await?;
        }
    }

    Ok(())
}
