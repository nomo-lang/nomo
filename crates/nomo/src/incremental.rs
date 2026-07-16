//! Compiler-owned primitives for content-addressed semantic query caches.
//!
//! Cached values are always derivable from their declared inputs. The cache is
//! an optimization, never a build input, and callers must attach every file or
//! upstream query that can affect a result.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Program;
use crate::diagnostic::Diagnostic;
use crate::project::{Project, project_module_context};
use crate::semantic::{self, SemanticSymbol};
use nomo_target::TargetTriple;

pub const QUERY_SCHEMA_VERSION: u32 = 1;
pub const PERSISTENT_CACHE_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_PERSISTENT_CACHE_MAX_BYTES: u64 = 512 * 1024 * 1024;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentFingerprint(String);

impl ContentFingerprint {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut builder = FingerprintBuilder::new();
        builder.add_bytes(bytes);
        builder.finish()
    }

    pub fn of_text(text: &str) -> Self {
        Self::of_bytes(text.as_bytes())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Default)]
pub struct FingerprintBuilder {
    hasher: Sha256,
}

impl FingerprintBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_bytes(&mut self, bytes: &[u8]) {
        self.hasher.update((bytes.len() as u64).to_le_bytes());
        self.hasher.update(bytes);
    }

    pub fn add_text(&mut self, text: &str) {
        self.add_bytes(text.as_bytes());
    }

    pub fn add_path(&mut self, path: &Path) {
        self.add_text(&path.to_string_lossy());
    }

    pub fn finish(self) -> ContentFingerprint {
        ContentFingerprint(format!("sha256:{:x}", self.hasher.finalize()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputId(String);

impl InputId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn path(path: &Path) -> Self {
        Self::new(format!("file:{}", path.to_string_lossy()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryKey {
    pub schema: u32,
    pub toolchain: String,
    pub target: String,
    pub namespace: String,
    pub identity: String,
    pub fingerprint: ContentFingerprint,
}

impl QueryKey {
    pub fn new(
        target: impl Into<String>,
        namespace: impl Into<String>,
        identity: impl Into<String>,
        fingerprint: ContentFingerprint,
    ) -> Self {
        Self {
            schema: QUERY_SCHEMA_VERSION,
            toolchain: env!("CARGO_PKG_VERSION").to_string(),
            target: target.into(),
            namespace: namespace.into(),
            identity: identity.into(),
            fingerprint,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum QueryDependency {
    Input(InputId),
    Query(QueryKey),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub insertions: u64,
    pub invalidations: u64,
    pub entries: usize,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheSnapshot {
    pub generation: u64,
    pub keys: Vec<QueryKey>,
}

#[derive(Debug)]
struct CachedQuery<V> {
    value: V,
    dependencies: BTreeSet<QueryDependency>,
}

#[derive(Debug)]
struct CacheState<V> {
    entries: BTreeMap<QueryKey, CachedQuery<V>>,
    input_dependents: BTreeMap<InputId, BTreeSet<QueryKey>>,
    query_dependents: BTreeMap<QueryKey, BTreeSet<QueryKey>>,
    hits: u64,
    misses: u64,
    insertions: u64,
    invalidations: u64,
    generation: u64,
}

impl<V> Default for CacheState<V> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            input_dependents: BTreeMap::new(),
            query_dependents: BTreeMap::new(),
            hits: 0,
            misses: 0,
            insertions: 0,
            invalidations: 0,
            generation: 0,
        }
    }
}

#[derive(Debug)]
pub struct QueryCache<V> {
    state: Mutex<CacheState<V>>,
}

impl<V> Default for QueryCache<V> {
    fn default() -> Self {
        Self {
            state: Mutex::new(CacheState::default()),
        }
    }
}

impl<V: Clone> QueryCache<V> {
    pub fn get(&self, key: &QueryKey) -> Option<V> {
        let mut state = self.state.lock().expect("query cache lock poisoned");
        let value = state.entries.get(key).map(|entry| entry.value.clone());
        if value.is_some() {
            state.hits += 1;
        } else {
            state.misses += 1;
        }
        value
    }

    pub fn get_or_compute(
        &self,
        key: QueryKey,
        dependencies: impl IntoIterator<Item = QueryDependency>,
        compute: impl FnOnce() -> V,
    ) -> V {
        if let Some(value) = self.get(&key) {
            return value;
        }
        let computed = compute();
        let dependencies = dependencies.into_iter().collect::<BTreeSet<_>>();
        let mut state = self.state.lock().expect("query cache lock poisoned");
        if let Some(existing) = state.entries.get(&key).map(|entry| entry.value.clone()) {
            state.hits += 1;
            return existing;
        }
        for dependency in &dependencies {
            match dependency {
                QueryDependency::Input(input) => {
                    state
                        .input_dependents
                        .entry(input.clone())
                        .or_default()
                        .insert(key.clone());
                }
                QueryDependency::Query(query) => {
                    state
                        .query_dependents
                        .entry(query.clone())
                        .or_default()
                        .insert(key.clone());
                }
            }
        }
        state.entries.insert(
            key,
            CachedQuery {
                value: computed.clone(),
                dependencies,
            },
        );
        state.insertions += 1;
        state.generation += 1;
        computed
    }

    pub fn invalidate_input(&self, input: &InputId) -> usize {
        let mut state = self.state.lock().expect("query cache lock poisoned");
        let roots = state.input_dependents.remove(input).unwrap_or_default();
        invalidate_queries(&mut state, roots)
    }

    pub fn clear(&self) -> usize {
        let mut state = self.state.lock().expect("query cache lock poisoned");
        let removed = state.entries.len();
        state.entries.clear();
        state.input_dependents.clear();
        state.query_dependents.clear();
        state.invalidations += removed as u64;
        if removed > 0 {
            state.generation += 1;
        }
        removed
    }

    pub fn stats(&self) -> CacheStats {
        let state = self.state.lock().expect("query cache lock poisoned");
        CacheStats {
            hits: state.hits,
            misses: state.misses,
            insertions: state.insertions,
            invalidations: state.invalidations,
            entries: state.entries.len(),
            generation: state.generation,
        }
    }

    pub fn snapshot(&self) -> CacheSnapshot {
        let state = self.state.lock().expect("query cache lock poisoned");
        CacheSnapshot {
            generation: state.generation,
            keys: state.entries.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PersistentCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub writes: u64,
    pub corruptions: u64,
    pub evictions: u64,
    pub entries: usize,
    pub bytes: u64,
    pub capacity_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistentCacheEntry {
    schema: u32,
    key: QueryKey,
    value_sha256: String,
    value_hex: String,
    written_at_unix_seconds: u64,
}

#[derive(Debug)]
struct PersistentCacheCounters {
    hits: AtomicU64,
    misses: AtomicU64,
    writes: AtomicU64,
    corruptions: AtomicU64,
    evictions: AtomicU64,
}

impl Default for PersistentCacheCounters {
    fn default() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            writes: AtomicU64::new(0),
            corruptions: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }
}

/// A rebuildable, content-addressed disk cache for compiler query values.
///
/// Entries live below a schema-versioned directory. Writers create and sync a
/// temporary file before atomically replacing the final entry, so concurrent
/// readers never observe a partial value. Any malformed entry, mismatched key,
/// or value checksum failure is treated as a miss and removed.
#[derive(Debug)]
pub struct PersistentQueryCache {
    base: PathBuf,
    root: PathBuf,
    max_bytes: u64,
    counters: PersistentCacheCounters,
}

impl PersistentQueryCache {
    pub fn at_root(project_or_workspace_root: &Path) -> Self {
        Self::with_max_bytes(
            project_or_workspace_root,
            persistent_cache_capacity_from_env(),
        )
    }

    pub fn with_max_bytes(project_or_workspace_root: &Path, max_bytes: u64) -> Self {
        let base = incremental_cache_base(project_or_workspace_root);
        let root = base.join(format!("v{PERSISTENT_CACHE_SCHEMA_VERSION}"));
        Self {
            base,
            root,
            max_bytes,
            counters: PersistentCacheCounters::default(),
        }
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn get<V: DeserializeOwned>(&self, key: &QueryKey) -> Option<V> {
        let path = self.entry_path(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                self.counters.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };
        let decoded = serde_json::from_slice::<PersistentCacheEntry>(&bytes)
            .map_err(|_| ())
            .and_then(|entry| {
                if entry.schema != PERSISTENT_CACHE_SCHEMA_VERSION || entry.key != *key {
                    return Err(());
                }
                let value_bytes = decode_hex(&entry.value_hex).ok_or(())?;
                if sha256_hex(&value_bytes) != entry.value_sha256 {
                    return Err(());
                }
                serde_json::from_slice::<V>(&value_bytes).map_err(|_| ())
            });
        match decoded {
            Ok(value) => {
                self.counters.hits.fetch_add(1, Ordering::Relaxed);
                trace_persistent_cache("hit", key, &path);
                Some(value)
            }
            Err(()) => {
                self.counters.misses.fetch_add(1, Ordering::Relaxed);
                self.counters.corruptions.fetch_add(1, Ordering::Relaxed);
                let _ = fs::remove_file(&path);
                trace_persistent_cache("corrupt", key, &path);
                None
            }
        }
    }

    pub fn insert<V: Serialize>(&self, key: &QueryKey, value: &V) -> Result<(), String> {
        let value_bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
        let entry = PersistentCacheEntry {
            schema: PERSISTENT_CACHE_SCHEMA_VERSION,
            key: key.clone(),
            value_sha256: sha256_hex(&value_bytes),
            value_hex: encode_hex(&value_bytes),
            written_at_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        let bytes = serde_json::to_vec(&entry).map_err(|error| error.to_string())?;
        let path = self.entry_path(key);
        let parent = path
            .parent()
            .ok_or_else(|| format!("cache entry has no parent: {}", path.display()))?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp = parent.join(format!(
            ".{}.{}.{}.{}.tmp",
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("entry"),
            std::process::id(),
            timestamp,
            sequence
        ));
        let write_result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|error| error.to_string())?;
            file.write_all(&bytes).map_err(|error| error.to_string())?;
            file.sync_all().map_err(|error| error.to_string())?;
            drop(file);
            atomic_replace(&temp, &path)?;
            sync_directory(parent);
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        write_result?;
        self.counters.writes.fetch_add(1, Ordering::Relaxed);
        trace_persistent_cache("write", key, &path);
        self.prune()?;
        Ok(())
    }

    pub fn prune(&self) -> Result<usize, String> {
        if !self.root.exists() {
            return Ok(0);
        }
        let mut files = Vec::new();
        collect_cache_files(&self.root, &mut files)?;
        let mut entries = Vec::new();
        let mut total = 0_u64;
        for path in files {
            if path.extension().and_then(|extension| extension.to_str()) == Some("tmp") {
                let stale = fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .ok()
                    .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                    .is_some_and(|age| age.as_secs() >= 60 * 60);
                if stale {
                    let _ = fs::remove_file(path);
                }
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            let length = metadata.len();
            total = total.saturating_add(length);
            entries.push((metadata.modified().unwrap_or(UNIX_EPOCH), path, length));
        }
        entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        let mut removed = 0;
        for (_, path, length) in entries {
            if total <= self.max_bytes {
                break;
            }
            if fs::remove_file(path).is_ok() {
                total = total.saturating_sub(length);
                removed += 1;
            }
        }
        self.counters
            .evictions
            .fetch_add(removed as u64, Ordering::Relaxed);
        Ok(removed)
    }

    pub fn stats(&self) -> Result<PersistentCacheStats, String> {
        let mut files = Vec::new();
        if self.root.exists() {
            collect_cache_files(&self.root, &mut files)?;
        }
        let mut entries = 0;
        let mut bytes = 0_u64;
        for path in files {
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            if let Ok(metadata) = fs::metadata(path) {
                entries += 1;
                bytes = bytes.saturating_add(metadata.len());
            }
        }
        Ok(PersistentCacheStats {
            hits: self.counters.hits.load(Ordering::Relaxed),
            misses: self.counters.misses.load(Ordering::Relaxed),
            writes: self.counters.writes.load(Ordering::Relaxed),
            corruptions: self.counters.corruptions.load(Ordering::Relaxed),
            evictions: self.counters.evictions.load(Ordering::Relaxed),
            entries,
            bytes,
            capacity_bytes: self.max_bytes,
        })
    }

    fn entry_path(&self, key: &QueryKey) -> PathBuf {
        let encoded = serde_json::to_vec(key).expect("query keys must serialize");
        let digest = sha256_hex(&encoded);
        self.root.join(&digest[..2]).join(format!("{digest}.json"))
    }
}

pub fn incremental_cache_base(project_or_workspace_root: &Path) -> PathBuf {
    project_or_workspace_root.join(".nomo/cache/incremental")
}

pub fn clean_incremental_cache(project_or_workspace_root: &Path) -> Result<PathBuf, String> {
    let base = incremental_cache_base(project_or_workspace_root);
    if base.exists() {
        fs::remove_dir_all(&base).map_err(|error| error.to_string())?;
    }
    Ok(base)
}

fn persistent_cache_capacity_from_env() -> u64 {
    std::env::var("NOMO_INCREMENTAL_CACHE_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_PERSISTENT_CACHE_MAX_BYTES)
}

fn collect_cache_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            collect_cache_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn atomic_replace(temp: &Path, final_path: &Path) -> Result<(), String> {
    match fs::rename(temp, final_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(final_path).map_err(|remove_error| remove_error.to_string())?;
            fs::rename(temp, final_path).map_err(|rename_error| rename_error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn sync_directory(path: &Path) {
    if let Ok(directory) = fs::File::open(path) {
        let _ = directory.sync_all();
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn decode_hex(encoded: &str) -> Option<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        return None;
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = decode_hex_digit(pair[0])?;
            let low = decode_hex_digit(pair[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn decode_hex_digit(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

fn trace_persistent_cache(event: &str, key: &QueryKey, path: &Path) {
    if std::env::var_os("NOMO_INCREMENTAL_TRACE").is_some() {
        eprintln!(
            "incremental-cache {event} {}:{} {}",
            key.namespace,
            key.identity,
            path.display()
        );
    }
}

fn invalidate_queries<V>(state: &mut CacheState<V>, roots: BTreeSet<QueryKey>) -> usize {
    let mut queue = roots.into_iter().collect::<VecDeque<_>>();
    let mut removed = 0;
    let mut visited = BTreeSet::new();
    while let Some(key) = queue.pop_front() {
        if !visited.insert(key.clone()) {
            continue;
        }
        if let Some(dependents) = state.query_dependents.remove(&key) {
            queue.extend(dependents);
        }
        let Some(entry) = state.entries.remove(&key) else {
            continue;
        };
        for dependency in entry.dependencies {
            let dependents = match dependency {
                QueryDependency::Input(input) => state.input_dependents.get_mut(&input),
                QueryDependency::Query(query) => state.query_dependents.get_mut(&query),
            };
            if let Some(dependents) = dependents {
                dependents.remove(&key);
            }
        }
        removed += 1;
    }
    state.input_dependents.retain(|_, keys| !keys.is_empty());
    state.query_dependents.retain(|_, keys| !keys.is_empty());
    state.invalidations += removed as u64;
    if removed > 0 {
        state.generation += 1;
    }
    removed
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SemanticSessionStats {
    pub check: CacheStats,
    pub symbols: CacheStats,
}

/// Content-addressed semantic queries shared by compiler and editor clients.
///
/// The first implementation caches complete project check and symbol results.
/// Its input fingerprint is deliberately conservative: every project/external
/// module source and every in-memory overlay is included. The lower-level
/// [`QueryCache`] dependency graph supports finer-grained parser/type queries
/// without changing this public session contract.
#[derive(Debug, Default)]
pub struct IncrementalSemanticSession {
    checks: QueryCache<Result<Program, Diagnostic>>,
    symbols: QueryCache<Result<Vec<SemanticSymbol>, Diagnostic>>,
}

impl IncrementalSemanticSession {
    pub fn check_project_text(
        &self,
        project: &Project,
        path: &Path,
        source: &str,
        source_overrides: &[(std::path::PathBuf, String)],
        target: &TargetTriple,
    ) -> Result<Program, Diagnostic> {
        let context = project_module_context(project).map_err(|message| {
            Diagnostic::new(
                "E0901",
                message,
                &project.root.join("nomo.toml"),
                1,
                1,
                1,
                "",
            )
        })?;
        let overrides = overrides_with_current(path, source, source_overrides);
        let inputs = semantic_inputs(project, &context.external_modules, &overrides);
        let key = QueryKey::new(
            target.to_string(),
            "semantic-check",
            format!("{}:{}", project.name, path.display()),
            inputs.fingerprint,
        );
        self.checks.get_or_compute(key, inputs.dependencies, || {
            crate::compiler::check_source_text_with_project_modules_and_overrides(
                path,
                source,
                Some(&context.local_source_root),
                &context.external_import_roots,
                &context.external_modules,
                &overrides,
            )
        })
    }

    pub fn symbols_for_project(
        &self,
        project: &Project,
        source_overrides: &[(std::path::PathBuf, String)],
        target: &TargetTriple,
    ) -> Result<Vec<SemanticSymbol>, Diagnostic> {
        let context = project_module_context(project).map_err(|message| {
            Diagnostic::new(
                "E0901",
                message,
                &project.root.join("nomo.toml"),
                1,
                1,
                1,
                "",
            )
        })?;
        let inputs = semantic_inputs(project, &context.external_modules, source_overrides);
        let key = QueryKey::new(
            target.to_string(),
            "semantic-symbols",
            project.name.clone(),
            inputs.fingerprint,
        );
        self.symbols.get_or_compute(key, inputs.dependencies, || {
            semantic::symbols_for_project_with_overrides(project, source_overrides)
        })
    }

    pub fn invalidate_path(&self, path: &Path) -> usize {
        let input = InputId::path(path);
        self.checks.invalidate_input(&input) + self.symbols.invalidate_input(&input)
    }

    pub fn clear(&self) -> usize {
        self.checks.clear() + self.symbols.clear()
    }

    pub fn stats(&self) -> SemanticSessionStats {
        SemanticSessionStats {
            check: self.checks.stats(),
            symbols: self.symbols.stats(),
        }
    }
}

struct SemanticInputs {
    fingerprint: ContentFingerprint,
    dependencies: Vec<QueryDependency>,
}

pub(crate) fn project_query_key(
    project: &Project,
    external_modules: &[crate::compiler::ExternalModule],
    source_overrides: &[(std::path::PathBuf, String)],
    target: &TargetTriple,
    namespace: &str,
    identity: impl Into<String>,
) -> QueryKey {
    let inputs = semantic_inputs(project, external_modules, source_overrides);
    QueryKey::new(target.to_string(), namespace, identity, inputs.fingerprint)
}

fn semantic_inputs(
    project: &Project,
    external_modules: &[crate::compiler::ExternalModule],
    source_overrides: &[(std::path::PathBuf, String)],
) -> SemanticInputs {
    let mut files = vec![project.root.join("nomo.toml")];
    collect_nomo_sources(&project.root.join("src"), &mut files);
    for external in external_modules {
        collect_nomo_sources(&external.source_root, &mut files);
        if let Some(root) = external.source_root.parent() {
            files.push(root.join("nomo.toml"));
        }
    }
    files.sort();
    files.dedup();
    let mut overrides = source_overrides.iter().collect::<Vec<_>>();
    overrides.sort_by(|left, right| left.0.cmp(&right.0));
    let mut builder = FingerprintBuilder::new();
    builder.add_text("nomo-semantic-session-v1");
    let mut dependencies = Vec::new();
    for path in files {
        builder.add_path(&path);
        if let Some((_, source)) = overrides.iter().find(|(candidate, _)| *candidate == path) {
            builder.add_text(source);
        } else {
            match std::fs::read(&path) {
                Ok(contents) => builder.add_bytes(&contents),
                Err(error) => builder.add_text(&format!("missing:{error}")),
            }
        }
        dependencies.push(QueryDependency::Input(InputId::path(&path)));
    }
    for (path, source) in overrides {
        builder.add_path(path);
        builder.add_text(source);
        dependencies.push(QueryDependency::Input(InputId::path(path)));
    }
    SemanticInputs {
        fingerprint: builder.finish(),
        dependencies,
    }
}

fn collect_nomo_sources(root: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_sources(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
}

fn overrides_with_current(
    path: &Path,
    source: &str,
    source_overrides: &[(std::path::PathBuf, String)],
) -> Vec<(std::path::PathBuf, String)> {
    let mut overrides = source_overrides.to_vec();
    if let Some((_, current)) = overrides
        .iter_mut()
        .find(|(candidate, _)| candidate == path)
    {
        *current = source.to_string();
    } else {
        overrides.push((path.to_path_buf(), source.to_string()));
    }
    overrides.sort_by(|left, right| left.0.cmp(&right.0));
    overrides
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn key(name: &str, text: &str) -> QueryKey {
        QueryKey::new(
            "aarch64-apple-darwin-none",
            "semantic",
            name,
            ContentFingerprint::of_text(text),
        )
    }

    #[test]
    fn content_fingerprints_are_framed_and_deterministic() {
        let mut left = FingerprintBuilder::new();
        left.add_text("ab");
        left.add_text("c");
        let mut right = FingerprintBuilder::new();
        right.add_text("a");
        right.add_text("bc");
        assert_ne!(left.finish(), right.finish());
        assert_eq!(
            ContentFingerprint::of_text("same"),
            ContentFingerprint::of_text("same")
        );
    }

    #[test]
    fn repeated_query_hits_without_recomputing() {
        let cache = QueryCache::default();
        let calls = AtomicUsize::new(0);
        let query = key("symbols:app.main", "source");
        let input = QueryDependency::Input(InputId::new("file:src/main.nomo"));
        let first = cache.get_or_compute(query.clone(), [input.clone()], || {
            calls.fetch_add(1, Ordering::SeqCst);
            vec!["main".to_string()]
        });
        let second = cache.get_or_compute(query, [input], || {
            calls.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        });
        assert_eq!(first, second);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(cache.stats().hits, 1);
    }

    #[test]
    fn input_invalidation_propagates_to_dependent_queries() {
        let cache = QueryCache::default();
        let parse = key("parse:app.math", "v1");
        let types = key("types:app.main", "v1");
        cache.get_or_compute(
            parse.clone(),
            [QueryDependency::Input(InputId::new("file:src/math.nomo"))],
            || "ast".to_string(),
        );
        cache.get_or_compute(
            types.clone(),
            [QueryDependency::Query(parse.clone())],
            || "typed".to_string(),
        );

        assert_eq!(
            cache.invalidate_input(&InputId::new("file:src/math.nomo")),
            2
        );
        assert!(cache.get(&parse).is_none());
        assert!(cache.get(&types).is_none());
        assert_eq!(cache.stats().invalidations, 2);
    }

    #[test]
    fn snapshots_are_stable_generation_views() {
        let cache = QueryCache::default();
        let first = key("parse:a", "a");
        cache.get_or_compute(first.clone(), [], || 1_u8);
        let snapshot = cache.snapshot();
        cache.get_or_compute(key("parse:b", "b"), [], || 2_u8);
        assert_eq!(snapshot.keys, vec![first]);
        assert!(cache.snapshot().generation > snapshot.generation);
    }

    #[test]
    fn semantic_session_matches_clean_results_across_edits() {
        let root =
            std::env::temp_dir().join(format!("nomo-incremental-semantic-{}", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let project = crate::project::create_project(&root, "incremental-demo").unwrap();
        let target = TargetTriple::host().unwrap();
        let session = IncrementalSemanticSession::default();
        let valid = std::fs::read_to_string(&project.main).unwrap();

        let first = session
            .check_project_text(&project, &project.main, &valid, &[], &target)
            .unwrap();
        let second = session
            .check_project_text(&project, &project.main, &valid, &[], &target)
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(session.stats().check.hits, 1);

        let invalid = valid.replace("let message: string", "let message: i64");
        let incremental = session
            .check_project_text(&project, &project.main, &invalid, &[], &target)
            .unwrap_err();
        let context = crate::project::project_module_context(&project).unwrap();
        let clean = crate::check_source_text_with_project_modules_and_overrides(
            &project.main,
            &invalid,
            Some(&context.local_source_root),
            &context.external_import_roots,
            &context.external_modules,
            &[(project.main.clone(), invalid.clone())],
        )
        .unwrap_err();
        assert_eq!(incremental, clean);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn persistent_cache_round_trips_and_recovers_from_corruption() {
        let root = temporary_root("persistent-round-trip");
        let cache = PersistentQueryCache::with_max_bytes(&root, 1024 * 1024);
        let query = key("codegen:app.main", "source");
        assert_eq!(cache.get::<String>(&query), None);

        cache.insert(&query, &"generated-c".to_string()).unwrap();
        assert_eq!(cache.get(&query), Some("generated-c".to_string()));
        let stats = cache.stats().unwrap();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);

        let mut files = Vec::new();
        collect_cache_files(cache.root(), &mut files).unwrap();
        let entry = files
            .into_iter()
            .find(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
            .unwrap();
        std::fs::write(&entry, b"{truncated").unwrap();
        assert_eq!(cache.get::<String>(&query), None);
        assert!(!entry.exists());
        assert_eq!(cache.stats().unwrap().corruptions, 1);

        cache.insert(&query, &"recovered".to_string()).unwrap();
        assert_eq!(cache.get(&query), Some("recovered".to_string()));
        clean_incremental_cache(&root).unwrap();
        assert!(!cache.base().exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_cache_prunes_oldest_entries_to_capacity() {
        let root = temporary_root("persistent-prune");
        let cache = PersistentQueryCache::with_max_bytes(&root, 900);
        for index in 0..4 {
            cache
                .insert(
                    &key(&format!("codegen:{index}"), &format!("source-{index}")),
                    &"x".repeat(400),
                )
                .unwrap();
        }
        let stats = cache.stats().unwrap();
        assert!(stats.bytes <= stats.capacity_bytes);
        assert!(stats.entries < 4);
        assert!(stats.evictions > 0);
        clean_incremental_cache(&root).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_checks_match_clean_checks_under_randomized_edits() {
        let root = temporary_root("persistent-equivalence");
        std::fs::create_dir_all(&root).unwrap();
        let project = crate::project::create_project(&root, "persistent-demo").unwrap();
        let original = std::fs::read_to_string(&project.main).unwrap();
        let mut state = 0x4d595df4d0f33173_u64;
        for iteration in 0..32 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let source = match state % 4 {
                0 => original.replace("Hello, Nomo", &format!("edit-{iteration}")),
                1 => original.replace("let message: string", "let message: i64"),
                2 => original.replace("return \"Hello, Nomo\"", "return 42"),
                _ => original.clone(),
            };
            std::fs::write(&project.main, source).unwrap();
            let incremental = crate::project::check_project_with_persistent_cache(&project)
                .map_err(|diagnostic| diagnostic.json());
            let clean =
                crate::project::check_project(&project).map_err(|diagnostic| diagnostic.json());
            assert_eq!(incremental, clean, "edit {iteration}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    fn temporary_root(label: &str) -> PathBuf {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "nomo-incremental-{label}-{}-{sequence}",
            std::process::id()
        ))
    }
}
