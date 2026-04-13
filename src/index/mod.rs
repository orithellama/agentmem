//! Local code indexing and retrieval primitives.
//!
//! This module provides a cost-first retrieval path for agent workflows:
//!
//! - build a local index once
//! - query small ranked chunks quickly
//! - enforce retrieval budgets before any remote model call

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{AgentMemoryError, Result, StoreError};
use crate::store::Store;
use crate::types::{Key, Namespace, Value};

const INDEX_NAMESPACE: &str = "index";
const INDEX_META_NAMESPACE: &str = "index/meta";
const INDEX_CHUNK_NAMESPACE: &str = "index/chunk";
const INDEX_TOKEN_NAMESPACE: &str = "index/token";

const INDEX_SCHEMA_VERSION: u32 = 1;

const MAX_FILE_BYTES: usize = 256 * 1024;
const CHUNK_LINE_TARGET: usize = 40;
const CHUNK_BYTE_TARGET: usize = 3_500;
const MAX_POSTINGS_PER_TOKEN: usize = 256;

const DEFAULT_TOP_K: usize = 8;
const DEFAULT_TOKEN_BUDGET: usize = 4_000;
const MAX_TOP_K: usize = 64;
const MIN_TOKEN_BUDGET: usize = 128;

const MIN_TOKEN_LEN: usize = 2;
const MAX_TOKEN_LEN: usize = 40;

/// Build-time index report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexBuildReport {
    /// Canonicalized root used for indexing.
    pub root: String,
    /// Number of files indexed.
    pub file_count: usize,
    /// Number of text files skipped.
    pub skipped_files: usize,
    /// Number of chunks persisted.
    pub chunk_count: usize,
    /// Number of token posting lists persisted.
    pub token_count: usize,
}

/// Persisted index summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStats {
    /// Whether an index has been built.
    pub built: bool,
    /// Indexed root, when available.
    pub root: Option<String>,
    /// Number of indexed files.
    pub file_count: usize,
    /// Number of indexed chunks.
    pub chunk_count: usize,
    /// Number of indexed tokens.
    pub token_count: usize,
    /// Build timestamp (unix seconds), when available.
    pub built_unix_seconds: Option<u64>,
}

/// Ranked chunk in query output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryChunk {
    /// Stable chunk identifier.
    pub chunk_id: String,
    /// Relative file path.
    pub path: String,
    /// 1-based start line.
    pub line_start: usize,
    /// 1-based end line.
    pub line_end: usize,
    /// Integer relevance score.
    pub score: u32,
    /// Estimated token count of this chunk.
    pub estimated_tokens: usize,
    /// Chunk content excerpt.
    pub content: String,
}

/// Query result with budget and confidence metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    /// Original user query.
    pub query: String,
    /// Effective top-k cap.
    pub top_k: usize,
    /// Effective token budget cap.
    pub token_budget: usize,
    /// Estimated tokens in selected chunks.
    pub used_tokens: usize,
    /// Matched tokens found in index.
    pub matched_tokens: Vec<String>,
    /// Query tokens absent from index.
    pub missing_tokens: Vec<String>,
    /// Confidence in local retrieval sufficiency.
    pub confidence: f32,
    /// Whether caller should consider remote fallback.
    pub fallback_required: bool,
    /// Selected chunks.
    pub chunks: Vec<QueryChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChunkRecord {
    chunk_id: String,
    path: String,
    line_start: usize,
    line_end: usize,
    content: String,
}

/// Builds a fresh local index under `root` and persists it to the store.
///
/// Existing `index/*` keys are deleted and replaced atomically on final flush.
pub fn build_index(store: &mut Store, root: &Path) -> Result<IndexBuildReport> {
    let root = canonical_or_input(root)?;
    let files = collect_indexable_files(&root)?;

    let mut token_map: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut records = Vec::<ChunkRecord>::new();
    let mut skipped_files = 0;
    let mut chunk_ids = HashSet::<String>::new();

    for path in &files {
        match index_file(path, &root, &mut records, &mut token_map, &mut chunk_ids) {
            Ok(()) => {}
            Err(IndexFileOutcome::Skipped) => {
                skipped_files += 1;
            }
            Err(IndexFileOutcome::Failed(error)) => return Err(error),
        }
    }

    clear_index_namespace(store)?;

    for record in &records {
        let key = key_for(INDEX_CHUNK_NAMESPACE, &record.chunk_id)?;
        let value = json_value(record)?;
        let _ = store.set(key, value)?;
    }

    let mut token_count = 0;
    let mut token_entries: Vec<(String, BTreeSet<String>)> = token_map.into_iter().collect();
    token_entries.sort_by(|left, right| left.0.cmp(&right.0));

    for (token, postings) in token_entries {
        if postings.is_empty() {
            continue;
        }

        let ids: Vec<String> = postings.into_iter().take(MAX_POSTINGS_PER_TOKEN).collect();

        if ids.is_empty() {
            continue;
        }

        let key = key_for(INDEX_TOKEN_NAMESPACE, &token)?;
        let value = json_value(&ids)?;
        let _ = store.set(key, value)?;
        token_count += 1;
    }

    let built_unix_seconds = unix_now();
    let root_display = path_to_unix_string(&root);

    set_meta(store, "schema_version", &INDEX_SCHEMA_VERSION.to_string())?;
    set_meta(store, "root", &root_display)?;
    set_meta(store, "built_unix_seconds", &built_unix_seconds.to_string())?;
    set_meta(store, "file_count", &files.len().to_string())?;
    set_meta(store, "chunk_count", &records.len().to_string())?;
    set_meta(store, "token_count", &token_count.to_string())?;

    store.flush()?;

    Ok(IndexBuildReport {
        root: root_display,
        file_count: files.len(),
        skipped_files,
        chunk_count: records.len(),
        token_count,
    })
}

/// Returns persisted index metadata.
#[must_use]
pub fn read_index_stats(store: &Store) -> IndexStats {
    let root = get_meta(store, "root");
    let built = root.is_some();

    IndexStats {
        built,
        root,
        file_count: get_meta_usize(store, "file_count").unwrap_or(0),
        chunk_count: get_meta_usize(store, "chunk_count").unwrap_or(0),
        token_count: get_meta_usize(store, "token_count").unwrap_or(0),
        built_unix_seconds: get_meta_u64(store, "built_unix_seconds"),
    }
}

/// Queries the local index with hard retrieval controls.
pub fn query_index(
    store: &Store,
    query: &str,
    top_k: usize,
    token_budget: usize,
) -> Result<QueryResult> {
    let top_k = normalize_top_k(top_k);
    let token_budget = normalize_token_budget(token_budget);

    let mut query_tokens = tokenize(query);
    query_tokens.sort();
    query_tokens.dedup();

    if query_tokens.is_empty() {
        return Ok(QueryResult {
            query: query.to_owned(),
            top_k,
            token_budget,
            used_tokens: 0,
            matched_tokens: Vec::new(),
            missing_tokens: Vec::new(),
            confidence: 0.0,
            fallback_required: true,
            chunks: Vec::new(),
        });
    }

    let mut scores = HashMap::<String, u32>::new();
    let mut matched_tokens = Vec::new();
    let mut missing_tokens = Vec::new();

    for token in &query_tokens {
        let token_key = key_for(INDEX_TOKEN_NAMESPACE, token)?;

        let Some(value) = store.get(&token_key) else {
            missing_tokens.push(token.clone());
            continue;
        };

        let ids: Vec<String> = parse_json(value.as_str(), "token posting list")?;

        if ids.is_empty() {
            missing_tokens.push(token.clone());
            continue;
        }

        matched_tokens.push(token.clone());

        for chunk_id in ids {
            let score = scores.entry(chunk_id).or_insert(0);
            *score = score.saturating_add(1);
        }
    }

    if scores.is_empty() {
        return Ok(QueryResult {
            query: query.to_owned(),
            top_k,
            token_budget,
            used_tokens: 0,
            matched_tokens,
            missing_tokens,
            confidence: 0.0,
            fallback_required: true,
            chunks: Vec::new(),
        });
    }

    let mut ranked: Vec<(String, u32)> = scores.into_iter().collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let mut chunks = Vec::new();
    let mut used_tokens: usize = 0;

    for (chunk_id, score) in ranked {
        let chunk_key = key_for(INDEX_CHUNK_NAMESPACE, &chunk_id)?;
        let Some(value) = store.get(&chunk_key) else {
            continue;
        };

        let record: ChunkRecord = parse_json(value.as_str(), "chunk record")?;
        let estimated_tokens = estimate_tokens(&record.content);
        let would_exceed = used_tokens.saturating_add(estimated_tokens) > token_budget;

        if would_exceed && !chunks.is_empty() {
            continue;
        }

        used_tokens = used_tokens.saturating_add(estimated_tokens);

        chunks.push(QueryChunk {
            chunk_id: record.chunk_id,
            path: record.path,
            line_start: record.line_start,
            line_end: record.line_end,
            score,
            estimated_tokens,
            content: record.content,
        });

        if chunks.len() >= top_k {
            break;
        }
    }

    let confidence = if query_tokens.is_empty() {
        0.0
    } else {
        matched_tokens.len() as f32 / query_tokens.len() as f32
    };

    let fallback_required = chunks.is_empty() || confidence < 0.6;

    Ok(QueryResult {
        query: query.to_owned(),
        top_k,
        token_budget,
        used_tokens,
        matched_tokens,
        missing_tokens,
        confidence,
        fallback_required,
        chunks,
    })
}

#[derive(Debug)]
enum IndexFileOutcome {
    Skipped,
    Failed(AgentMemoryError),
}

fn index_file(
    path: &Path,
    root: &Path,
    records: &mut Vec<ChunkRecord>,
    token_map: &mut HashMap<String, BTreeSet<String>>,
    chunk_ids: &mut HashSet<String>,
) -> std::result::Result<(), IndexFileOutcome> {
    let metadata = fs::metadata(path).map_err(map_index_file_error)?;
    let Ok(size) = usize::try_from(metadata.len()) else {
        return Err(IndexFileOutcome::Skipped);
    };

    if size > MAX_FILE_BYTES {
        return Err(IndexFileOutcome::Skipped);
    }

    let Ok(content) = fs::read_to_string(path) else {
        return Err(IndexFileOutcome::Skipped);
    };

    if content.trim().is_empty() {
        return Ok(());
    }

    let rel_path = match path.strip_prefix(root) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => path.to_path_buf(),
    };
    let rel_path_display = path_to_unix_string(&rel_path);

    let chunks = split_into_chunks(&content);

    for (line_start, line_end, chunk_content) in chunks {
        if chunk_content.trim().is_empty() {
            continue;
        }

        let chunk_id = make_chunk_id(
            &rel_path_display,
            line_start,
            line_end,
            &chunk_content,
            chunk_ids,
        );

        let record = ChunkRecord {
            chunk_id: chunk_id.clone(),
            path: rel_path_display.clone(),
            line_start,
            line_end,
            content: chunk_content.clone(),
        };

        let mut combined = String::with_capacity(
            rel_path_display
                .len()
                .saturating_add(chunk_content.len())
                .saturating_add(1),
        );
        combined.push_str(&rel_path_display);
        combined.push(' ');
        combined.push_str(&chunk_content);

        for token in tokenize(&combined) {
            token_map.entry(token).or_default().insert(chunk_id.clone());
        }

        records.push(record);
    }

    Ok(())
}

fn map_index_file_error(error: std::io::Error) -> IndexFileOutcome {
    IndexFileOutcome::Failed(AgentMemoryError::Store(StoreError::malformed(format!(
        "failed while scanning file: {error}"
    ))))
}

fn collect_indexable_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|source| {
            StoreError::read(
                dir.clone(),
                std::io::Error::new(source.kind(), source.to_string()),
            )
        })?;

        for entry in entries {
            let entry = entry.map_err(AgentMemoryError::from)?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(AgentMemoryError::from)?;

            if file_type.is_dir() {
                if should_skip_dir(&path) {
                    continue;
                }

                stack.push(path);
                continue;
            }

            if file_type.is_file() && !should_skip_file(&path) {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|segment| segment.to_str()) else {
        return true;
    };

    if matches!(name, ".git" | ".agentmem" | "target" | "node_modules") {
        return true;
    }

    name.starts_with('.') && name != ".github"
}

fn should_skip_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|segment| segment.to_str()) else {
        return true;
    };

    if name.starts_with('.') {
        return true;
    }

    let lower = name.to_ascii_lowercase();

    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".pdf")
        || lower.ends_with(".zip")
        || lower.ends_with(".tar")
        || lower.ends_with(".gz")
        || lower.ends_with(".lock")
}

fn split_into_chunks(content: &str) -> Vec<(usize, usize, String)> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_start = 1usize;
    let mut current_lines = 0usize;

    for (index, line) in content.lines().enumerate() {
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
        current_lines += 1;

        if current_lines >= CHUNK_LINE_TARGET || current.len() >= CHUNK_BYTE_TARGET {
            let end = index + 1;
            chunks.push((current_start, end, current.clone()));
            current.clear();
            current_lines = 0;
            current_start = end + 1;
        }
    }

    if !current.is_empty() {
        let end = current_start + current_lines - 1;
        chunks.push((current_start, end, current));
    }

    chunks
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in input.chars() {
        let normalized = if ch.is_ascii_alphanumeric() || ch == '_' {
            Some(ch.to_ascii_lowercase())
        } else {
            None
        };

        if let Some(ch) = normalized {
            current.push(ch);
        } else {
            push_token(&mut current, &mut tokens);
        }
    }

    push_token(&mut current, &mut tokens);
    tokens
}

fn push_token(current: &mut String, tokens: &mut Vec<String>) {
    if current.is_empty() {
        return;
    }

    let len = current.len();

    let skip = len < MIN_TOKEN_LEN
        || len > MAX_TOKEN_LEN
        || matches!(
            current.as_str(),
            "fn" | "let"
                | "const"
                | "mod"
                | "pub"
                | "use"
                | "for"
                | "while"
                | "loop"
                | "true"
                | "false"
                | "this"
                | "that"
                | "with"
                | "from"
                | "into"
                | "your"
                | "their"
                | "there"
                | "where"
                | "when"
                | "what"
                | "have"
                | "will"
                | "json"
                | "the"
                | "and"
                | "or"
                | "to"
                | "in"
                | "on"
                | "of"
                | "is"
                | "it"
        );

    if !skip {
        tokens.push(current.clone());
    }

    current.clear();
}

fn make_chunk_id(
    path: &str,
    line_start: usize,
    line_end: usize,
    content: &str,
    used: &mut HashSet<String>,
) -> String {
    let base = format!(
        "c{:016x}",
        fast_hash(&(path, line_start, line_end, content))
    );

    if !used.contains(&base) {
        let _ = used.insert(base.clone());
        return base;
    }

    let mut suffix = 1usize;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !used.contains(&candidate) {
            let _ = used.insert(candidate.clone());
            return candidate;
        }

        suffix = suffix.saturating_add(1);
    }
}

fn fast_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn clear_index_namespace(store: &mut Store) -> Result<()> {
    let namespace = Namespace::new(INDEX_NAMESPACE.to_owned())?;
    let _ = store.delete_namespace(&namespace);
    Ok(())
}

fn key_for(namespace: &str, leaf: &str) -> Result<Key> {
    Key::new(format!("{namespace}/{leaf}"))
}

fn set_meta(store: &mut Store, key_suffix: &str, value: &str) -> Result<()> {
    let key = key_for(INDEX_META_NAMESPACE, key_suffix)?;
    let value = Value::new(value.to_owned())?;
    let _ = store.set(key, value)?;
    Ok(())
}

fn get_meta(store: &Store, key_suffix: &str) -> Option<String> {
    let key = key_for(INDEX_META_NAMESPACE, key_suffix).ok()?;
    let value = store.get(&key)?;
    Some(value.as_str().to_owned())
}

fn get_meta_usize(store: &Store, key_suffix: &str) -> Option<usize> {
    let raw = get_meta(store, key_suffix)?;
    raw.parse::<usize>().ok()
}

fn get_meta_u64(store: &Store, key_suffix: &str) -> Option<u64> {
    let raw = get_meta(store, key_suffix)?;
    raw.parse::<u64>().ok()
}

fn json_value(payload: &impl Serialize) -> Result<Value> {
    let serialized = serde_json::to_string(payload).map_err(|error| {
        AgentMemoryError::Store(StoreError::Serialize {
            message: format!("failed to serialize index payload: {error}"),
        })
    })?;

    Value::new(serialized)
}

fn parse_json<T>(input: &str, label: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_str(input).map_err(|error| {
        AgentMemoryError::Store(StoreError::Deserialize {
            message: format!("failed to deserialize {label}: {error}"),
        })
    })
}

fn canonical_or_input(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path).map_err(AgentMemoryError::from);
    }

    Ok(path.to_path_buf())
}

fn path_to_unix_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn estimate_tokens(content: &str) -> usize {
    let bytes = content.len();
    let estimated = bytes / 4;
    estimated.max(1)
}

fn normalize_top_k(top_k: usize) -> usize {
    if top_k == 0 {
        return DEFAULT_TOP_K;
    }

    top_k.min(MAX_TOP_K)
}

fn normalize_token_budget(token_budget: usize) -> usize {
    if token_budget == 0 {
        return DEFAULT_TOKEN_BUDGET;
    }

    token_budget.max(MIN_TOKEN_BUDGET)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
