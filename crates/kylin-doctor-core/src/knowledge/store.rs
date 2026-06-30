use crate::util::epoch_secs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 知识文档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub source: String,
    pub title: String,
    pub chunks: Vec<Chunk>,
    pub added_at: String,
}

/// 文档分块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
}

/// 检索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub doc_id: String,
    pub source: String,
    pub chunk_content: String,
    pub score: f32,
}

/// 知识库存储
pub struct KnowledgeStore {
    base_dir: PathBuf,
    documents: Vec<Document>,
    /// 单调递增的文档 ID 计数器（避免 remove 后 add 产生 ID 碰撞）
    next_id: usize,
}

impl KnowledgeStore {
    /// 创建知识库实例
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            documents: Vec::new(),
            next_id: 0,
        }
    }

    /// 默认路径: ~/.kylin-doctor/knowledge/
    pub fn default_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".kylin-doctor").join("knowledge")
    }

    /// 初始化目录结构
    pub fn init(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.base_dir)?;
        std::fs::create_dir_all(self.base_dir.join("raw_docs"))?;
        std::fs::create_dir_all(self.base_dir.join("chunks"))?;
        Ok(())
    }

    /// 加载已有索引
    pub fn load(&mut self) -> anyhow::Result<()> {
        let index_path = self.base_dir.join("index.json");
        if index_path.exists() {
            let content = std::fs::read_to_string(&index_path)?;
            self.documents = serde_json::from_str(&content)?;
            // 从已有文档 ID 恢复计数器，避免碰撞
            for doc in &self.documents {
                if let Some(num_str) = doc.id.strip_prefix("doc_") {
                    if let Ok(num) = num_str.parse::<usize>() {
                        if num >= self.next_id {
                            self.next_id = num + 1;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 保存索引
    fn save_index(&self) -> anyhow::Result<()> {
        let index_path = self.base_dir.join("index.json");
        if let Some(parent) = index_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.documents)?;
        std::fs::write(&index_path, content)?;
        Ok(())
    }

    /// 添加文档（从文本内容）
    pub fn add_document(&mut self, source: &str, title: &str, content: &str) -> anyhow::Result<String> {
        let doc_id = format!("doc_{}", self.next_id);
        self.next_id += 1;
        let chunks = self.chunk_text(content, 500, 50);

        let doc = Document {
            id: doc_id.clone(),
            source: source.to_string(),
            title: title.to_string(),
            chunks,
            added_at: epoch_secs(),
        };

        // 保存原始文档
        let raw_dir = self.base_dir.join("raw_docs");
        std::fs::create_dir_all(&raw_dir)?;
        let raw_path = raw_dir.join(format!("{}.txt", doc_id));
        std::fs::write(&raw_path, content)?;

        self.documents.push(doc);
        self.save_index()?;

        Ok(doc_id)
    }

    /// 添加文件文档
    pub fn add_file(&mut self, path: &str) -> anyhow::Result<String> {
        let content = std::fs::read_to_string(path)?;
        let title = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        self.add_document(path, &title, &content)
    }

    /// 递归添加目录
    pub fn add_directory(&mut self, dir: &str) -> anyhow::Result<Vec<String>> {
        let mut ids = Vec::new();
        let entries = std::fs::read_dir(dir)?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                ids.extend(self.add_directory(&path.to_string_lossy())?);
            } else if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if matches!(ext.as_str(), "txt" | "md" | "rst" | "conf" | "cfg" | "log") {
                    match self.add_file(&path.to_string_lossy()) {
                        Ok(id) => ids.push(id),
                        Err(e) => eprintln!("⚠️  跳过 {}: {}", path.display(), e),
                    }
                }
            }
        }
        Ok(ids)
    }

    /// 非递归添加目录（仅顶层文件）
    pub fn add_directory_shallow(&mut self, dir: &str) -> anyhow::Result<Vec<String>> {
        let mut ids = Vec::new();
        let entries = std::fs::read_dir(dir)?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_lowercase();
                    if matches!(ext.as_str(), "txt" | "md" | "rst" | "conf" | "cfg" | "log") {
                        match self.add_file(&path.to_string_lossy()) {
                            Ok(id) => ids.push(id),
                            Err(e) => eprintln!("⚠️  跳过 {}: {}", path.display(), e),
                        }
                    }
                }
            }
        }
        Ok(ids)
    }

    /// 为所有文档生成向量嵌入
    pub async fn embed_all(
        &mut self,
        provider: &dyn crate::llm::LlmProvider,
    ) -> anyhow::Result<usize> {
        let mut total_embedded = 0;

        for doc in &mut self.documents {
            for chunk in &mut doc.chunks {
                if chunk.embedding.is_some() {
                    continue;
                }
                match provider.embed(&[chunk.content.clone()]).await {
                    Ok(embeddings) => {
                        if let Some(emb) = embeddings.into_iter().next() {
                            chunk.embedding = Some(emb);
                            total_embedded += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  向量化失败 ({}): {}", chunk.id, e);
                    }
                }
            }
        }

        self.save_index()?;
        Ok(total_embedded)
    }

    /// 语义检索
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();

        for doc in &self.documents {
            for chunk in &doc.chunks {
                if let Some(ref emb) = chunk.embedding {
                    let score = cosine_similarity(query_embedding, emb);
                    results.push(SearchResult {
                        doc_id: doc.id.clone(),
                        source: doc.source.clone(),
                        chunk_content: chunk.content.clone(),
                        score,
                    });
                }
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    /// 关键词检索（后备）
    pub fn search_keyword(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let keywords: Vec<&str> = query_lower.split_whitespace().collect();

        let mut results: Vec<SearchResult> = Vec::new();

        for doc in &self.documents {
            for chunk in &doc.chunks {
                let content_lower = chunk.content.to_lowercase();
                let match_count = keywords.iter().filter(|kw| content_lower.contains(**kw)).count();
                if match_count > 0 {
                    let score = match_count as f32 / keywords.len() as f32;
                    results.push(SearchResult {
                        doc_id: doc.id.clone(),
                        source: doc.source.clone(),
                        chunk_content: chunk.content.clone(),
                        score,
                    });
                }
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    /// 删除文档
    pub fn remove_document(&mut self, doc_id: &str) -> anyhow::Result<()> {
        // 验证 doc_id 格式（防止路径遍历）
        if !doc_id.starts_with("doc_") || doc_id[4..].parse::<usize>().is_err() {
            anyhow::bail!("无效的文档 ID: {}（格式应为 doc_N）", doc_id);
        }

        self.documents.retain(|d| d.id != doc_id);
        let raw_path = self.base_dir.join("raw_docs").join(format!("{}.txt", doc_id));
        if raw_path.exists() {
            std::fs::remove_file(&raw_path)?;
        }
        self.save_index()?;
        Ok(())
    }

    /// 列出所有文档
    pub fn list_documents(&self) -> &[Document] {
        &self.documents
    }

    /// 统计信息
    pub fn stats(&self) -> KnowledgeStats {
        let total_docs = self.documents.len();
        let total_chunks = self.documents.iter().map(|d| d.chunks.len()).sum();
        let embedded_chunks = self
            .documents
            .iter()
            .flat_map(|d| &d.chunks)
            .filter(|c| c.embedding.is_some())
            .count();

        KnowledgeStats {
            total_docs,
            total_chunks,
            embedded_chunks,
        }
    }

    /// 文本分块
    fn chunk_text(&self, text: &str, chunk_size: usize, overlap: usize) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let total = chars.len();

        if total == 0 {
            return chunks;
        }

        let mut start = 0;
        let mut chunk_idx = 0;

        while start < total {
            let end = std::cmp::min(start + chunk_size, total);
            let content: String = chars[start..end].iter().collect();

            chunks.push(Chunk {
                id: format!("chunk_{}_{}", chunks.len(), chunk_idx),
                content,
                embedding: None,
            });

            chunk_idx += 1;
            if end >= total {
                break;
            }
            start = end - overlap;
        }

        chunks
    }
}

/// 知识库统计
#[derive(Debug)]
pub struct KnowledgeStats {
    pub total_docs: usize,
    pub total_chunks: usize,
    pub embedded_chunks: usize,
}

/// 余弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_store() -> KnowledgeStore {
        let dir = std::env::temp_dir().join(format!("kylin-doctor-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        KnowledgeStore::new(dir)
    }

    #[test]
    fn chunk_text_basic() {
        let store = KnowledgeStore::new(PathBuf::new());
        let chunks = store.chunk_text("hello world", 5, 0);
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.content.contains("hello")));
    }

    #[test]
    fn chunk_text_with_overlap() {
        let store = KnowledgeStore::new(PathBuf::new());
        let text = "abcdefghij";
        let chunks = store.chunk_text(text, 5, 2);
        assert!(chunks.len() > 1);
        // Second chunk should overlap with first
        assert!(chunks[1].content.contains("de") || chunks[1].content.contains("cd"));
    }

    #[test]
    fn chunk_text_empty() {
        let store = KnowledgeStore::new(PathBuf::new());
        let chunks = store.chunk_text("", 100, 0);
        assert!(chunks.is_empty());
    }

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn add_and_list_documents() {
        let mut store = temp_store();
        store.init().unwrap();

        store.add_document("test.txt", "Test Doc", "Hello world content").unwrap();
        store.add_document("test2.txt", "Test Doc 2", "More content here").unwrap();

        assert_eq!(store.list_documents().len(), 2);
        assert_eq!(store.list_documents()[0].title, "Test Doc");
        assert_eq!(store.list_documents()[1].title, "Test Doc 2");
    }

    #[test]
    fn stats_empty() {
        let store = temp_store();
        let stats = store.stats();
        assert_eq!(stats.total_docs, 0);
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.embedded_chunks, 0);
    }

    #[test]
    fn stats_after_add() {
        let mut store = temp_store();
        store.init().unwrap();
        store.add_document("test.txt", "Test", "Some content for testing chunks").unwrap();

        let stats = store.stats();
        assert_eq!(stats.total_docs, 1);
        assert!(stats.total_chunks > 0);
        assert_eq!(stats.embedded_chunks, 0); // No embeddings yet
    }

    #[test]
    fn search_keyword_basic() {
        let mut store = temp_store();
        store.init().unwrap();
        store.add_document("linux.txt", "Linux Guide", "Linux is a free operating system kernel").unwrap();
        store.add_document("windows.txt", "Windows Guide", "Windows is a commercial operating system").unwrap();

        let results = store.search_keyword("Linux kernel", 5);
        assert!(!results.is_empty());
        assert!(results[0].chunk_content.to_lowercase().contains("linux"));
    }

    #[test]
    fn search_keyword_no_match() {
        let mut store = temp_store();
        store.init().unwrap();
        store.add_document("test.txt", "Test", "Hello world").unwrap();

        let results = store.search_keyword("nonexistent", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn remove_document() {
        let mut store = temp_store();
        store.init().unwrap();
        let id = store.add_document("test.txt", "Test", "Content").unwrap();
        assert_eq!(store.list_documents().len(), 1);

        store.remove_document(&id).unwrap();
        assert_eq!(store.list_documents().len(), 0);
    }

    #[test]
    fn id_no_collision_after_remove_and_add() {
        let mut store = temp_store();
        store.init().unwrap();

        let id_a = store.add_document("a.txt", "A", "content A").unwrap();
        let id_b = store.add_document("b.txt", "B", "content B").unwrap();
        let id_c = store.add_document("c.txt", "C", "content C").unwrap();
        assert_eq!(id_a, "doc_0");
        assert_eq!(id_b, "doc_1");
        assert_eq!(id_c, "doc_2");

        // Remove middle doc
        store.remove_document(&id_b).unwrap();
        assert_eq!(store.list_documents().len(), 2);

        // Add new doc — must NOT collide with doc_2
        let id_d = store.add_document("d.txt", "D", "content D").unwrap();
        assert_eq!(id_d, "doc_3");
        assert_eq!(store.list_documents().len(), 3);

        // Verify no duplicate IDs
        let ids: Vec<&str> = store.list_documents().iter().map(|d| d.id.as_str()).collect();
        let unique_count = ids.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(unique_count, ids.len(), "Duplicate document IDs detected: {:?}", ids);
    }

    #[test]
    fn remove_document_rejects_invalid_ids() {
        let mut store = temp_store();
        store.init().unwrap();
        store.add_document("test.txt", "Test", "Content").unwrap();

        // 各种非法 ID
        assert!(store.remove_document("evil_id").is_err());
        assert!(store.remove_document("../../etc/passwd").is_err());
        assert!(store.remove_document("doc_../secret").is_err());
        assert!(store.remove_document("doc_-1").is_err());
        assert!(store.remove_document("doc_abc").is_err());
        assert!(store.remove_document("").is_err());

        // 合法 ID
        assert!(store.remove_document("doc_0").is_ok());
    }
}
