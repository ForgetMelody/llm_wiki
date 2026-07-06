use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use walkdir::{DirEntry, WalkDir};

use crate::{
    config::AppConfig,
    model::{ChunkDraft, MarkdownDocument},
};

/// 递归发现知识库中的 Markdown 文件。
pub fn discover_markdown_files(config: &AppConfig) -> Result<Vec<PathBuf>> {
    let exclude_hidden = config.exclude_hidden;
    let exclude_obsidian_dir = config.exclude_obsidian_dir;
    let root = config.knowledge_root.clone();

    let mut files = WalkDir::new(&root)
        .into_iter()
        .filter_entry(move |entry| should_descend(entry, exclude_hidden, exclude_obsidian_dir))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| is_markdown_file(path))
        .collect::<Vec<_>>();

    files.sort();
    Ok(files)
}

/// 读取单个 Markdown 文档，并生成相对路径。
pub fn load_document(root: &Path, path: &Path) -> Result<MarkdownDocument> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read markdown {}", path.display()))?;
    let relative_path = path
        .strip_prefix(root)
        .with_context(|| format!("path {} not under {}", path.display(), root.display()))?
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");

    Ok(MarkdownDocument {
        relative_path,
        absolute_path: path.to_path_buf(),
        content,
    })
}

/// 按标题与段落把 Markdown 切成适合检索的块。
pub fn chunk_document(doc: &MarkdownDocument, chunk_char_limit: usize) -> Vec<ChunkDraft> {
    let sections = split_sections(&doc.content);
    let mut chunks = Vec::new();

    for (heading_path, body) in sections {
        let paragraphs = split_paragraphs(&body);
        let mut current = String::new();

        for paragraph in paragraphs {
            let paragraph = paragraph.trim();
            if paragraph.is_empty() {
                continue;
            }

            let candidate_len = current.len() + paragraph.len() + 2;
            if !current.is_empty() && candidate_len > chunk_char_limit {
                chunks.push(build_chunk(doc, &heading_path, &current));
                current.clear();
            }

            if paragraph.len() > chunk_char_limit {
                if !current.is_empty() {
                    chunks.push(build_chunk(doc, &heading_path, &current));
                    current.clear();
                }
                for piece in split_long_text(paragraph, chunk_char_limit) {
                    chunks.push(build_chunk(doc, &heading_path, &piece));
                }
                continue;
            }

            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(paragraph);
        }

        if !current.trim().is_empty() {
            chunks.push(build_chunk(doc, &heading_path, &current));
        }
    }

    if chunks.is_empty() {
        chunks.push(build_chunk(doc, "", doc.content.trim()));
    }

    chunks
}

fn build_chunk(doc: &MarkdownDocument, heading_path: &str, body: &str) -> ChunkDraft {
    let mut text = String::new();
    text.push_str("Path: ");
    text.push_str(&doc.relative_path);
    text.push('\n');
    if !heading_path.is_empty() {
        text.push_str("Heading: ");
        text.push_str(heading_path);
        text.push_str("\n\n");
    } else {
        text.push('\n');
    }
    text.push_str(body.trim());

    ChunkDraft {
        heading_path: heading_path.to_string(),
        text,
    }
}

fn split_sections(content: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut headings: Vec<String> = Vec::new();
    let mut current_body = Vec::new();
    let mut current_heading = String::new();

    for line in content.lines() {
        if let Some((level, heading)) = parse_heading(line) {
            flush_section(&mut sections, &current_heading, &current_body);
            current_body.clear();
            if level == 0 {
                continue;
            }
            headings.truncate(level.saturating_sub(1));
            headings.push(heading.to_string());
            current_heading = headings.join(" > ");
        } else {
            current_body.push(line);
        }
    }

    flush_section(&mut sections, &current_heading, &current_body);
    sections
}

fn flush_section(sections: &mut Vec<(String, String)>, heading_path: &str, lines: &[&str]) {
    let body = lines.join("\n").trim().to_string();
    if !body.is_empty() {
        sections.push((heading_path.to_string(), body));
    }
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|&ch| ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = trimmed.get(hashes..)?.trim();
    if rest.is_empty() {
        return None;
    }
    Some((hashes, rest))
}

fn split_paragraphs(body: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = Vec::new();

    for line in body.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line.to_string());
        }
    }

    if !current.is_empty() {
        paragraphs.push(current.join("\n"));
    }

    paragraphs
}

fn split_long_text(text: &str, limit: usize) -> Vec<String> {
    if text.chars().count() <= limit {
        return vec![text.to_string()];
    }

    let chars = text.chars().collect::<Vec<_>>();
    chars
        .chunks(limit)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

fn should_descend(entry: &DirEntry, exclude_hidden: bool, exclude_obsidian_dir: bool) -> bool {
    let name = entry.file_name().to_string_lossy();
    if exclude_obsidian_dir && entry.file_type().is_dir() && name == ".obsidian" {
        return false;
    }
    if exclude_hidden && entry.depth() > 0 && name.starts_with('.') {
        return false;
    }
    true
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn chunks_follow_heading_boundaries() {
        let doc = MarkdownDocument {
            relative_path: "demo.md".to_string(),
            absolute_path: PathBuf::from("demo.md"),
            content: "# A\nline1\n\nline2\n## B\nline3".to_string(),
        };

        let chunks = chunk_document(&doc, 32);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text.contains("Heading: A"));
        assert!(chunks[1].text.contains("Heading: A > B"));
    }

    #[test]
    fn discovery_skips_hidden_and_obsidian() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("visible")).unwrap();
        fs::create_dir_all(temp.path().join(".obsidian")).unwrap();
        fs::create_dir_all(temp.path().join(".hidden")).unwrap();
        fs::write(temp.path().join("visible/note.md"), "hello").unwrap();
        fs::write(temp.path().join(".obsidian/app.md"), "ignored").unwrap();
        fs::write(temp.path().join(".hidden/secret.md"), "ignored").unwrap();

        let config = AppConfig {
            knowledge_root: temp.path().to_path_buf(),
            state_dir: temp.path().join("state"),
            database_path: temp.path().join("state/index.sqlite3"),
            embedding_backend: "hashing".to_string(),
            fastembed_model: "MultilingualE5Small".to_string(),
            embedding_cache_dir: temp.path().join("state/fastembed"),
            hashing_dimensions: 128,
            chunk_char_limit: 120,
            search_limit: 8,
            exclude_hidden: true,
            exclude_obsidian_dir: true,
        };

        let files = discover_markdown_files(&config).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("visible/note.md"));
    }
}
