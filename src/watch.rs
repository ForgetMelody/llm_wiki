use std::{thread, time::Duration};

use anyhow::{Result, bail};

use crate::{model::IndexStats, service::KnowledgeService};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchMode {
    Poll,
}

#[derive(Debug, Clone, Copy)]
pub struct WatchOptions {
    pub mode: WatchMode,
    pub interval_secs: u64,
    pub run_on_startup: bool,
}

#[derive(Debug)]
enum TickOutcome {
    Indexed(IndexStats),
    BusySkipped,
}

/// 运行后台 watch；当前只支持 poll 模式，用固定周期复用现有增量索引逻辑。
pub fn run(service: KnowledgeService, options: WatchOptions) -> Result<()> {
    if options.interval_secs == 0 {
        bail!("watch interval_secs must be greater than zero");
    }

    match options.mode {
        WatchMode::Poll => run_poll_loop(&service, options),
    }
}

/// poll 模式不依赖 tokio time/signal；systemd/前台信号直接终止进程即可。
fn run_poll_loop(service: &KnowledgeService, options: WatchOptions) -> Result<()> {
    if options.run_on_startup {
        report_tick(run_one_tick(service));
    }

    loop {
        thread::sleep(Duration::from_secs(options.interval_secs));
        report_tick(run_one_tick(service));
    }
}

/// 单轮 tick 只尝试获取一次索引锁；若已有 writer 在运行，则跳过而不阻塞。
fn run_one_tick(service: &KnowledgeService) -> Result<TickOutcome> {
    Ok(match service.try_reindex_all()? {
        Some(stats) => TickOutcome::Indexed(stats),
        None => TickOutcome::BusySkipped,
    })
}

fn report_tick(result: Result<TickOutcome>) {
    match result {
        Ok(TickOutcome::Indexed(stats)) => {
            println!(
                "watch tick indexed scanned_docs={} updated_docs={} skipped_docs={} deleted_docs={} total_chunks={}",
                stats.scanned_docs,
                stats.updated_docs,
                stats.skipped_docs,
                stats.deleted_docs,
                stats.total_chunks,
            );
        }
        Ok(TickOutcome::BusySkipped) => {
            println!("watch tick skipped reason=busy");
        }
        Err(err) => {
            eprintln!("watch tick failed error={err:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{TickOutcome, run_one_tick};
    use crate::{config::AppConfig, service::KnowledgeService};
    use tempfile::TempDir;

    fn build_test_config(root: &Path) -> AppConfig {
        let knowledge_root = root.join("wiki");
        let state_dir = root.join("state");
        let embedding_cache_dir = root.join("model-cache");
        fs::create_dir_all(&knowledge_root).unwrap();
        fs::create_dir_all(&state_dir).unwrap();
        fs::create_dir_all(&embedding_cache_dir).unwrap();
        AppConfig {
            knowledge_root,
            state_dir: state_dir.clone(),
            database_path: state_dir.join("index.sqlite3"),
            embedding_backend: "hashing".to_string(),
            fastembed_model: "AllMiniLML6V2".to_string(),
            embedding_cache_dir,
            fastembed_intra_threads: 1,
            fastembed_batch_size: 16,
            hashing_dimensions: 64,
            chunk_char_limit: 256,
            search_limit: 8,
            exclude_hidden: true,
            exclude_obsidian_dir: true,
            metadata_frontmatter_enabled: true,
            graph_enabled: false,
            graph_semantic_neighbors_per_node: 6,
            graph_semantic_min_score: 0.42,
        }
    }

    #[test]
    fn poll_tick_indexes_then_skips_unchanged_documents() {
        let temp = TempDir::new().unwrap();
        let config = build_test_config(temp.path());
        fs::write(
            config.knowledge_root.join("note.md"),
            "# Note\n\nwatch mode should reuse incremental indexing\n",
        )
        .unwrap();
        let service = KnowledgeService::new(config).unwrap();

        let first = run_one_tick(&service).unwrap();
        match first {
            TickOutcome::Indexed(stats) => {
                assert_eq!(stats.scanned_docs, 1);
                assert_eq!(stats.updated_docs, 1);
                assert_eq!(stats.skipped_docs, 0);
            }
            TickOutcome::BusySkipped => panic!("unexpected busy skip on first tick"),
        }

        let second = run_one_tick(&service).unwrap();
        match second {
            TickOutcome::Indexed(stats) => {
                assert_eq!(stats.scanned_docs, 1);
                assert_eq!(stats.updated_docs, 0);
                assert_eq!(stats.skipped_docs, 1);
            }
            TickOutcome::BusySkipped => panic!("unexpected busy skip on second tick"),
        }
    }
}
