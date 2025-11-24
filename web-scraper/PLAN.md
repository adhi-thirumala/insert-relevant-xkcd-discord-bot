# XKCD Discord Bot - Technical Planning Document

## Table of Contents

1. [System Architecture](#system-architecture)
2. [Data Layer Design](#data-layer-design)
3. [Web Scraper Implementation](#web-scraper-implementation)
4. [Backend/RAG Pipeline](#backendrag-pipeline)
5. [Discord Bot Interface](#discord-bot-interface)
6. [Deployment Architecture](#deployment-architecture)
7. [Development Roadmap](#development-roadmap)

---

## 1. System Architecture

### High-Level Design

The system consists of three main Rust crates:

1. **web-scraper**: Data ingestion and maintenance
2. **backend**: RAG logic and retrieval
3. **discord-bot**: User interface

All components share a single libSQL database via Docker volume.

### Communication Flow
```
Discord User
    ↓ (command: !xkcd query)
Discord Bot
    ↓ (search request)
Backend RAG Engine
    ↓ (vector search)
libSQL Database
    ↓ (results)
Backend RAG Engine
    ↓ (formatted response)
Discord Bot
    ↓ (embed message)
Discord User
```

### Technology Stack

| Component | Technology | Rationale |
|-----------|------------|-----------|
| Language | Rust | Memory safety, performance, strong typing |
| Database | libSQL embedded | SQLite-compatible, native vectors, no external service |
| Embeddings | all-MiniLM-L6-v2 | 384-dim, fast, good quality, local |
| Discord | Serenity | Mature, well-maintained Discord library |
| Async | Tokio | Industry standard async runtime |
| Web Scraping | mediawiki crate | Native Rust MediaWiki API client |
| Text Chunking | text-splitter | Semantic boundary detection |
| Scheduling | tokio-cron-scheduler | Built-in cron scheduling |

---

## 2. Data Layer Design

### Database Choice: libSQL Embedded

**Decision**: Use libSQL in embedded file mode with Docker volume sharing.

**Why libSQL over alternatives**:
- ✅ Native vector types (F32_BLOB) with vector search
- ✅ SQLite-compatible (huge ecosystem, battle-tested)
- ✅ Embedded (no separate server process)
- ✅ Excellent Rust support
- ✅ Perfect for ~3000 comics + ~15,000 chunks

**Rejected alternatives**:
- ❌ Dedicated vector DBs (Qdrant, Milvus): Overkill, extra complexity
- ❌ PostgreSQL + pgvector: Too heavy for this scale
- ❌ sqlite-vec extension: Less mature than libSQL native vectors

### Schema Design
```sql
-- Core comic metadata
CREATE TABLE xkcd_comics (
    comic_number INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    url TEXT NOT NULL,              -- explainxkcd.com URL
    xkcd_url TEXT NOT NULL,         -- xkcd.com URL
    hover_text TEXT,
    last_revision_id INTEGER NOT NULL,           -- MediaWiki's revid
    last_revision_timestamp TEXT NOT NULL,       -- Format: "20241115123456"
    scraped_at TEXT NOT NULL,                    -- When you first scraped it
    updated_at TEXT NOT NULL                      -- When you last updated it
);

-- Semantic chunks with embeddings
CREATE TABLE xkcd_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    comic_number INTEGER NOT NULL,
    chunk_text TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    section_type TEXT,              -- 'title_hover', 'explanation', 'transcript', 'trivia'
    embedding F32_BLOB(384) NOT NULL,
    
    FOREIGN KEY (comic_number) REFERENCES xkcd_comics(comic_number)
);

-- Vector search index
CREATE INDEX chunks_vec_idx ON xkcd_chunks(
    libsql_vector_idx(embedding, 'metric=cosine')
);

-- Fetch all chunks for a comic efficiently
CREATE INDEX idx_comic_chunks ON xkcd_chunks(comic_number, chunk_index);
```

### Vector Search Queries

**Basic similarity search**:
```sql
SELECT 
    xc.comic_number,
    xc.chunk_text,
    c.title,
    c.xkcd_url,
    c.hover_text,
    distance
FROM vector_top_k('chunks_vec_idx', vector32(?), 20) v
JOIN xkcd_chunks xc ON xc.rowid = v.id
JOIN xkcd_comics c ON c.comic_number = xc.comic_number
ORDER BY distance;
```

**With metadata filtering**:
```sql
SELECT 
    xc.comic_number,
    xc.chunk_text,
    c.title,
    distance
FROM vector_top_k('chunks_vec_idx', vector32(?), 20) v
JOIN xkcd_chunks xc ON xc.rowid = v.id
JOIN xkcd_comics c ON c.comic_number = xc.comic_number
WHERE c.published_date > '2020-01-01'
ORDER BY distance;
```

### Storage Estimates

- **3,000 comics**: Negligible metadata (<1 MB)
- **15,000 chunks** (avg 5 per comic):
  - Text: ~10 MB
  - Embeddings (384-dim float32): 15,000 × 384 × 4 bytes = 23 MB
- **Total**: ~35 MB (tiny!)

**Conclusion**: No storage optimization needed at this scale.

---

## 3. Web Scraper Implementation

### 3.1 Architecture
```
web-scraper/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Public API exports
│   ├── daemon.rs            # Built-in scheduler
│   ├── config.rs            # Configuration management
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── scrape_all.rs    # Initial bulk scrape
│   │   ├── scrape_new.rs    # Daily new comics check
│   │   ├── check_updates.rs # Weekly update check
│   │   ├── scrape_comic.rs  # Single comic scrape
│   │   └── stats.rs         # Database statistics
│   ├── wiki/
│   │   ├── mod.rs
│   │   ├── client.rs        # MediaWiki API wrapper
│   │   └── parser.rs        # Parse wikitext to clean text
│   ├── chunker.rs           # Semantic text chunking
│   ├── embedder.rs          # Embedding model wrapper
│   ├── storage.rs           # Database operations
│   └── models.rs            # Shared data structures
└── Cargo.toml
```

### 3.2 Dependencies
```toml
[dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Database
libsql = "0.4"

# Web scraping
mediawiki = "0.2"
reqwest = { version = "0.11", features = ["json"] }

# Text processing
text-splitter = "0.14"
parse_wiki_text = "0.1"

# Embeddings
fastembed = "3.0"

# Scheduling
tokio-cron-scheduler = "0.9"

# Utilities
clap = { version = "4.4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"
anyhow = "1.0"
```

### 3.3 Data Source: explainxkcd.com

**API Access**:
- MediaWiki API endpoint: `https://www.explainxkcd.com/w/api.php`
- Page naming: Comic number as string (e.g., "149", "2000")
- Rate limiting: 500ms between requests, 5 concurrent max

**Content Extraction**:
- **Explanation** section: Main explanation text (MUST extract)
- **Transcript** section: Panel descriptions (MUST extract)
- **Trivia** section: Additional notes (MUST extract)
- **Discussion** section: User comments (SKIP)

**Error Handling**:
- Missing page: Log warning, skip comic
- Empty explanation: Store with empty field, log warning
- Parse error: Log error, retry once, then skip

### 3.4 Chunking Strategy

**Approach**: Semantic chunking with `text-splitter` crate

**Configuration**:
```rust
use text_splitter::TextSplitter;

let splitter = TextSplitter::default()
    .with_trim_chunks(true);

// Target 200-500 characters per chunk
let chunks = splitter.chunks(text, 200..500);
```

**Chunk Types**:

1. **Title + Hover Text** (chunk_index=0, section_type='title_hover')
   - Combines comic title and hover text
   - Often contains the punchline
   
2. **Explanation Chunks** (chunk_index=1+, section_type='explanation')
   - Main explanation split semantically
   - Typically 3-8 chunks per comic
   
3. **Transcript Chunks** (section_type='transcript')
   - Panel-by-panel descriptions
   - Only if transcript is substantial (>50 chars)
   
4. **Trivia Chunks** (section_type='trivia')
   - Additional context and references
   - Only if trivia section exists

**Example Output** (Comic #149 "Sandwich"):
```
Chunk 0 (title_hover): "Title: Sandwich\nHover text: Proper User Policy..."
Chunk 1 (explanation): "This comic refers to the technique of arbitrary code execution..."
Chunk 2 (explanation): "The name Robert'); DROP TABLE Students;-- is a reference..."
Chunk 3 (transcript): "Transcript: [A man is talking to a woman...]"
```

### 3.5 Embedding Pipeline

**Model**: all-MiniLM-L6-v2 via fastembed-rs

**Specifications**:
- Dimensions: 384
- Speed: ~5,000 sentences/sec on CPU
- Size: ~80 MB model file
- Context window: 512 tokens

**Batching Strategy**:
```rust
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(InitOptions {
            model_name: EmbeddingModel::AllMiniLML6V2,
            show_download_progress: true,
            ..Default::default()
        })?;
        
        Ok(Self { model })
    }
    
    // Batch embedding for performance
    pub fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let embeddings = self.model.embed(texts.to_vec(), None)?;
        Ok(embeddings)
    }
}
```

**Batch Size**: 32-128 chunks per batch (optimal for CPU)

**Performance**:
- Sequential embedding: ~100ms per chunk
- Batched embedding: ~300ms per 32 chunks
- **Speed improvement**: ~10x faster with batching

**Accuracy**: Zero accuracy difference between batched and sequential

### 3.6 Scraping Operations

#### Initial Scrape: scrape-all

**Purpose**: One-time bulk scrape of all existing comics

**Algorithm**:
```rust
pub async fn scrape_all(db: &Database, concurrency: usize) -> Result<()> {
    // 1. Fetch latest comic number from xkcd.com/info.0.json
    let latest = fetch_latest_comic_number().await?;
    
    // 2. Create comic number list
    let comic_numbers: Vec<u32> = (1..=latest).collect();
    
    // 3. Scrape with controlled concurrency
    stream::iter(comic_numbers)
        .map(|num| async move {
            // Rate limit: 500ms delay
            tokio::time::sleep(Duration::from_millis(500)).await;
            scrape_and_store_comic(db, num).await
        })
        .buffer_unordered(concurrency)  // 5 concurrent
        .for_each(|result| async {
            match result {
                Ok(_) => info!("✓ Scraped comic #{}", num),
                Err(e) => warn!("✗ Failed: {}", e),
            }
        })
        .await;
        
    Ok(())
}
```

**Performance**:
- 3,000 comics / 5 concurrent = 600 batches
- 500ms per request × 600 batches = ~5 minutes total

#### Daily Scrape: scrape-new

**Purpose**: Check for new comics (fast operation)

**Algorithm**:
```rust
pub async fn scrape_new(db: &Database) -> Result<usize> {
    // 1. Get latest from xkcd.com
    let latest_xkcd = fetch_latest_comic_number().await?;
    
    // 2. Get our latest
    let our_latest = db.query_one::<u32>(
        "SELECT MAX(comic_number) FROM xkcd_comics"
    ).await?.unwrap_or(0);
    
    // 3. Scrape gap
    if latest_xkcd <= our_latest {
        return Ok(0);  // No new comics
    }
    
    let mut count = 0;
    for num in (our_latest + 1)..=latest_xkcd {
        scrape_and_store_comic(db, num).await?;
        count += 1;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    Ok(count)
}
```

**Typical runtime**: 1-2 seconds (XKCD publishes 1-2 comics/week)

#### Weekly Update Check: check-updates

**Purpose**: Detect explanation updates on existing comics

**Algorithm**:
```rust
pub async fn check_updates(db: &Database) -> Result<usize> {
    let stored_comics = db.query::<StoredComic>(
        "SELECT comic_number, explanation_hash FROM xkcd_comics"
    ).await?;
    
    let mut updated = 0;
    
    for stored in stored_comics {
        // Fetch current wiki content
        let wiki_page = fetch_wiki_page(stored.comic_number).await?;
        let new_hash = hash_content(&wiki_page.explanation);
        
        // Compare hashes
        if new_hash != stored.explanation_hash {
            info!("Comic {} updated, re-scraping", stored.comic_number);
            scrape_and_store_comic(db, stored.comic_number).await?;
            updated += 1;
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    Ok(updated)
}

fn hash_content(text: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

**Typical runtime**: ~25 minutes (3000 comics × 500ms)

**Frequency**: Weekly (Sunday 3am UTC)

### 3.7 Core Scraping Function
```rust
async fn scrape_and_store_comic(db: &Database, comic_num: u32) -> Result<()> {
    // 1. Fetch from wiki
    let wiki_page = wiki_client.fetch_page(&comic_num.to_string()).await?;
    
    // 2. Parse sections
    let parsed = parser::parse_wiki_content(&wiki_page)?;
    
    // 3. Store comic metadata
    let comic = Comic {
        comic_number: comic_num,
        title: parsed.title,
        url: format!("https://www.explainxkcd.com/wiki/index.php/{}", comic_num),
        xkcd_url: format!("https://xkcd.com/{}/", comic_num),
        hover_text: parsed.hover_text,
        published_date: parsed.date,
        explanation_hash: hash_content(&parsed.explanation),
        last_scraped: Utc::now().to_rfc3339(),
    };
    
    db.insert_comic(&comic).await?;
    
    // 4. Chunk content
    let chunks = chunker.chunk_comic(&parsed)?;
    
    // 5. Generate embeddings (batched)
    let texts: Vec<&str> = chunks.iter().map(|c| c.chunk_text.as_str()).collect();
    let embeddings = embedder.encode_batch(&texts)?;
    
    // 6. Store chunks with embeddings
    for (chunk, embedding) in chunks.into_iter().zip(embeddings) {
        db.insert_chunk(&chunk, &embedding).await?;
    }
    
    Ok(())
}
```

### 3.8 Built-in Scheduler (Daemon Mode)

**Implementation**:
```rust
// src/daemon.rs

use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn run(db: Database) -> Result<()> {
    db.init_schema().await?;
    
    let scheduler = JobScheduler::new().await?;
    
    // Daily: 00:00 UTC - Check for new comics
    let db_daily = db.clone();
    scheduler.add(Job::new_async("0 0 0 * * *", move |_, _| {
        let db = db_daily.clone();
        Box::pin(async move {
            info!("🔄 Running daily new comics check");
            match scrape_new(&db).await {
                Ok(n) => info!("✓ Scraped {} new comics", n),
                Err(e) => error!("✗ Daily scrape failed: {}", e),
            }
        })
    })?).await?;
    
    // Weekly: Sunday 03:00 UTC - Full update check
    let db_weekly = db.clone();
    scheduler.add(Job::new_async("0 0 3 * * 0", move |_, _| {
        let db = db_weekly.clone();
        Box::pin(async move {
            info!("🔄 Running weekly update check");
            match check_updates(&db).await {
                Ok(n) => info!("✓ Updated {} comics", n),
                Err(e) => error!("✗ Update check failed: {}", e),
            }
        })
    })?).await?;
    
    scheduler.start().await?;
    
    // Keep running
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
```

### 3.9 CLI Interface
```rust
// src/main.rs

#[derive(Parser)]
#[command(name = "xkcd-scraper")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run daemon with scheduled tasks
    Daemon,
    
    /// Initialize database schema
    Init,
    
    /// Scrape all comics (initial)
    ScrapeAll {
        #[arg(long, default_value = "5")]
        concurrency: usize,
    },
    
    /// Check for new comics
    ScrapeNew,
    
    /// Check all comics for updates
    CheckUpdates,
    
    /// Scrape specific comic
    ScrapeComic { number: u32 },
    
    /// Display statistics
    Stats,
}
```

**Usage Examples**:
```bash
# Production daemon
cargo run --release -- daemon

# Initial setup
cargo run -- init
cargo run -- scrape-all --concurrency 5

# Manual operations
cargo run -- scrape-new
cargo run -- scrape-comic 149
cargo run -- stats
```

---

## 4. Backend/RAG Pipeline

### 4.1 Architecture
```
backend/
├── src/
│   ├── lib.rs                  # Public API
│   ├── retriever.rs            # Two-stage retrieval
│   ├── query_enhancer.rs       # LLM-powered enhancement
│   ├── llm_client.rs           # Claude API wrapper
│   └── models.rs               # Shared types
└── Cargo.toml
```

### 4.2 RAG Strategy: Two-Stage Retrieval

**Flow**:
```
User Query
    ↓
Stage 1: LLM Theme Extraction
    ↓ (["programming", "SQL injection", "security"])
Stage 2: Enhanced Vector Search
    ↓ (top 20 chunks)
Stage 3: Chunk Deduplication
    ↓ (top 10 unique comics)
Stage 4: LLM Re-ranking
    ↓ (top 3 comics with reasons)
Final Results
```

### 4.3 Query Enhancement

**Purpose**: Extract XKCD-relevant themes from natural language queries

**Implementation**:
```rust
pub struct QueryEnhancer {
    llm: LLMClient,
}

impl QueryEnhancer {
    pub async fn extract_themes(&self, query: &str) -> Result<Vec<String>> {
        let prompt = format!(r#"
Given this user query about XKCD comics: "{}"

Extract 3-5 specific XKCD-related themes. Common themes:
- Programming/coding (Python, JavaScript, regex, etc.)
- Mathematics and physics
- Internet culture and memes
- Science and technology
- Relationships and social interactions
- Philosophy and existentialism
- What if? scenarios
- Meta-humor about webcomics

Output only JSON array: ["theme1", "theme2", "theme3"]
"#, query);

        let response = self.llm.complete(&prompt).await?;
        let themes: Vec<String> = serde_json::from_str(&response)?;
        Ok(themes)
    }
    
    pub fn enhance_query(&self, original: &str, themes: &[String]) -> String {
        format!("{}. Related themes: {}", original, themes.join(", "))
    }
}
```

**Example**:
```
Input:  "machine learning"
Themes: ["AI", "neural networks", "training data", "overfitting"]
Output: "machine learning. Related themes: AI, neural networks, training data, overfitting"
```

### 4.4 Vector Search

**Basic Search**:
```rust
pub async fn search_chunks(
    &self, 
    query_embedding: &[f32], 
    top_k: usize
) -> Result<Vec<ChunkResult>> {
    let results = self.db.query(
        "SELECT 
            xc.comic_number,
            xc.chunk_text,
            xc.section_type,
            c.title,
            c.xkcd_url,
            c.hover_text,
            distance
         FROM vector_top_k('chunks_vec_idx', vector32(?), ?) v
         JOIN xkcd_chunks xc ON xc.rowid = v.id
         JOIN xkcd_comics c ON c.comic_number = xc.comic_number
         ORDER BY distance",
        params![query_embedding, top_k]
    ).await?;
    
    Ok(results)
}
```

### 4.5 Chunk Deduplication

**Purpose**: Aggregate chunks back to unique comics

**Algorithm**:
```rust
pub fn deduplicate_chunks(&self, chunks: Vec<ChunkResult>) -> Vec<ComicWithChunks> {
    let mut comic_map: HashMap<u32, ComicWithChunks> = HashMap::new();
    
    for chunk in chunks {
        let entry = comic_map.entry(chunk.comic_number).or_insert_with(|| {
            ComicWithChunks {
                number: chunk.comic_number,
                title: chunk.title.clone(),
                xkcd_url: chunk.xkcd_url.clone(),
                hover_text: chunk.hover_text.clone(),
                relevant_chunks: Vec::new(),
                best_distance: f32::MAX,
            }
        });
        
        entry.relevant_chunks.push(chunk.chunk_text);
        entry.best_distance = entry.best_distance.min(chunk.distance);
    }
    
    // Sort by best distance
    let mut comics: Vec<_> = comic_map.into_values().collect();
    comics.sort_by(|a, b| a.best_distance.partial_cmp(&b.best_distance).unwrap());
    
    comics.into_iter().take(10).collect()
}
```

### 4.6 LLM Re-ranking

**Purpose**: Use LLM to pick the most relevant comics and explain why

**Implementation**:
```rust
pub async fn llm_rerank(
    &self, 
    query: &str, 
    comics: &[ComicWithChunks]
) -> Result<Vec<RankedComic>> {
    let prompt = format!(r#"
User query: "{}"

Candidate XKCD comics (ranked by embedding similarity):

{}

Select the TOP 3 most relevant comics. Consider:
1. Does it directly address the query topic?
2. Is the humor style appropriate?
3. Is this a canonical XKCD for this topic?

Output JSON only:
[
  {{"comic_number": 123, "reason": "This comic directly addresses SQL injection..."}},
  {{"comic_number": 456, "reason": "Classic XKCD about..."}}
]
"#, 
        query,
        comics.iter().enumerate()
            .map(|(i, c)| format!(
                "{}. #{}: {}\n   Hover: {}\n   Relevant: {}", 
                i+1, c.number, c.title, c.hover_text,
                c.relevant_chunks.join(" ... ")
            ))
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    
    let response = self.llm.complete(&prompt).await?;
    let ranked: Vec<RankedComic> = serde_json::from_str(&response)?;
    Ok(ranked)
}
```

### 4.7 Complete Retrieval Pipeline
```rust
pub struct TwoStageRetriever {
    db: Database,
    embedder: Embedder,
    llm: LLMClient,
    query_enhancer: QueryEnhancer,
}

impl TwoStageRetriever {
    pub async fn retrieve(&self, query: &str) -> Result<Vec<RankedComic>> {
        // Stage 1: Extract themes
        let themes = self.query_enhancer.extract_themes(query).await?;
        info!("Extracted themes: {:?}", themes);
        
        // Stage 2: Enhanced vector search
        let enhanced_query = self.query_enhancer.enhance_query(query, &themes);
        let query_embedding = self.embedder.encode(&enhanced_query)?;
        let chunks = self.search_chunks(&query_embedding, 20).await?;
        
        // Stage 3: Deduplicate to comics
        let comics = self.deduplicate_chunks(chunks);
        info!("Found {} unique comics", comics.len());
        
        // Stage 4: LLM re-rank
        let ranked = self.llm_rerank(query, &comics).await?;
        info!("LLM selected {} comics", ranked.len());
        
        Ok(ranked)
    }
}
```

---

## 5. Discord Bot Interface

### 5.1 Architecture
```
discord-bot/
├── src/
│   ├── main.rs              # Bot initialization
│   ├── commands.rs          # Command handlers
│   ├── handlers.rs          # Event handlers
│   ├── conversation.rs      # Context tracking
│   └── models.rs            # Discord-specific types
└── Cargo.toml
```

### 5.2 Command Interface

**Command**: `!xkcd <query>`

**Implementation**:
```rust
use serenity::framework::standard::{macros::command, CommandResult};

#[command]
#[description = "Find relevant XKCD comics"]
#[usage = "!xkcd <search query>"]
async fn xkcd(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let query = args.rest();
    
    if query.is_empty() {
        msg.reply(ctx, "Usage: `!xkcd <query>`\nExample: `!xkcd regex`").await?;
        return Ok(());
    }
    
    // Typing indicator
    msg.channel_id.broadcast_typing(&ctx.http).await?;
    
    // Get bot data
    let data = ctx.data.read().await;
    let retriever = data.get::<RetrieverKey>().unwrap();
    
    // Retrieve comics
    let results = retriever.retrieve(query).await?;
    
    if results.is_empty() {
        msg.reply(ctx, "No relevant XKCD found 😢").await?;
        return Ok(());
    }
    
    // Send results as embeds
    for comic in results {
        msg.channel_id.send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title(format!("XKCD #{}: {}", comic.number, comic.title))
                 .url(&comic.xkcd_url)
                 .description(comic.reason)
                 .field("Hover text", &comic.hover_text, false)
                 .color(0x96A8C8)
            })
        }).await?;
    }
    
    Ok(())
}
```

### 5.3 Conversation Tracking (Future Enhancement)
```rust
pub struct ConversationTracker {
    messages: HashMap<ChannelId, VecDeque<MessageData>>,
    max_messages: usize,     // 20
    max_age: Duration,        // 5 minutes
}

impl ConversationTracker {
    pub fn add_message(&mut self, channel: ChannelId, content: String) {
        let window = self.messages.entry(channel).or_default();
        
        window.push_back(MessageData {
            content,
            timestamp: Instant::now(),
        });
        
        // Prune old messages
        self.prune(channel);
    }
    
    fn prune(&mut self, channel: ChannelId) {
        if let Some(window) = self.messages.get_mut(&channel) {
            let now = Instant::now();
            
            // Remove messages older than max_age
            window.retain(|msg| now.duration_since(msg.timestamp) < self.max_age);
            
            // Keep only last N messages
            if window.len() > self.max_messages {
                let excess = window.len() - self.max_messages;
                window.drain(0..excess);
            }
        }
    }
    
    pub fn get_context(&self, channel: ChannelId) -> String {
        self.messages
            .get(&channel)
            .map(|w| w.iter().map(|m| &m.content).join("\n"))
            .unwrap_or_default()
    }
}
```

---

## 6. Deployment Architecture

### 6.1 Docker Compose Setup
```yaml
version: '3.8'

services:
  scraper-daemon:
    build: ./web-scraper
    container_name: xkcd-scraper
    command: ["scraper", "daemon"]
    volumes:
      - xkcd-data:/app/data
    environment:
      - DATABASE_URL=file:/app/data/xkcd.db
      - RUST_LOG=info
    restart: unless-stopped
    
  discord-bot:
    build: ./discord-bot
    container_name: xkcd-bot
    volumes:
      - xkcd-data:/app/data
    environment:
      - DATABASE_URL=file:/app/data/xkcd.db
      - DISCORD_TOKEN=${DISCORD_TOKEN}
      - RUST_LOG=info
    restart: unless-stopped
    depends_on:
      - scraper-daemon

volumes:
  xkcd-data:
```

### 6.2 Volume Sharing

**How it works**:
- Docker creates one volume: `xkcd-data`
- Volume mounted into both containers at `/app/data`
- Both containers see the exact same `xkcd.db` file
- libSQL handles cross-process locking automatically

**Database access pattern**:
- **scraper-daemon**: Writes daily (new comics) + weekly (updates)
- **discord-bot**: Reads continuously (user queries)
- **Concurrency**: libSQL handles multiple readers + occasional writer

### 6.3 Deployment Steps
```bash
# 1. Clone repo
git clone <repo>
cd insert-relevant-xkcd-discord-bot

# 2. Set environment variables
echo "DISCORD_TOKEN=your_token_here" > .env

# 3. Build and start
docker-compose up -d

# 4. Initialize database (first time)
docker-compose run --rm scraper-daemon scraper init

# 5. Initial scrape (one-time, ~5 minutes)
docker-compose run --rm scraper-daemon scraper scrape-all

# 6. Check status
docker-compose ps
docker-compose logs -f

# 7. View stats
docker-compose exec scraper-daemon scraper stats
```

### 6.4 Maintenance Operations
```bash
# Check for new comics manually
docker-compose exec scraper-daemon scraper scrape-new

# Force full update check
docker-compose exec scraper-daemon scraper check-updates

# Scrape specific comic
docker-compose exec scraper-daemon scraper scrape-comic 149

# View logs
docker-compose logs -f scraper-daemon
docker-compose logs -f discord-bot

# Restart services
docker-compose restart

# Update code and redeploy
git pull
docker-compose build
docker-compose up -d
```

---

## 7. Development Roadmap

### Phase 1: Web Scraper (Week 1)
- [ ] Set up project structure
- [ ] Implement MediaWiki client
- [ ] Implement wiki parser (explanation, transcript, trivia extraction)
- [ ] Implement text chunker
- [ ] Implement embedder wrapper (fastembed-rs)
- [ ] Implement database layer (libSQL)
- [ ] Implement CLI commands
- [ ] Implement daemon scheduler
- [ ] Test initial scrape on 10 comics
- [ ] Document scraper usage

### Phase 2: Backend/RAG (Week 2)
- [ ] Implement basic vector search
- [ ] Implement chunk deduplication
- [ ] Integrate LLM client (Claude API)
- [ ] Implement query enhancement
- [ ] Implement LLM re-ranking
- [ ] Test retrieval quality on sample queries
- [ ] Tune vector search parameters
- [ ] Document backend API

### Phase 3: Discord Bot (Week 3)
- [ ] Set up Serenity bot framework
- [ ] Implement `!xkcd` command
- [ ] Implement conversation tracker
- [ ] Format results as Discord embeds
- [ ] Add error handling
- [ ] Add typing indicators
- [ ] Test in development Discord server
- [ ] Document bot usage

### Phase 4: Integration & Testing (Week 4)
- [ ] Full scrape of all ~3000 comics
- [ ] End-to-end testing
- [ ] Performance optimization
- [ ] Docker Compose setup
- [ ] Deployment documentation
- [ ] README and user guide
- [ ] Deploy to production VPS

### Phase 5: Enhancements (Future)
- [ ] Proactive suggestions based on conversation
- [ ] User feedback (upvote/downvote)
- [ ] Multi-server support
- [ ] Custom LLM fine-tuning
- [ ] Image analysis for stub comics
- [ ] Web dashboard for statistics
- [ ] Prometheus metrics

---

## Implementation Checklist

### Pre-Development
- [x] Architecture design complete
- [x] Database schema finalized
- [x] Technology stack selected
- [x] Deployment strategy defined
- [ ] Development environment set up
- [ ] Discord bot token obtained
- [ ] Test Discord server created

### Web Scraper
- [ ] `Cargo.toml` dependencies configured
- [ ] Database schema created
- [ ] MediaWiki client implemented
- [ ] Wiki parser implemented
- [ ] Text chunker integrated
- [ ] Embedder integrated
- [ ] Database operations implemented
- [ ] CLI commands implemented
- [ ] Daemon scheduler implemented
- [ ] Unit tests written
- [ ] Integration tests written
- [ ] Initial scrape tested (10 comics)
- [ ] Full scrape tested (all comics)

### Backend
- [ ] Vector search implemented
- [ ] Chunk deduplication implemented
- [ ] LLM client integrated
- [ ] Query enhancement implemented
- [ ] LLM re-ranking implemented
- [ ] Unit tests written
- [ ] Integration tests written
- [ ] Retrieval quality validated

### Discord Bot
- [ ] Serenity framework configured
- [ ] Command framework set up
- [ ] `!xkcd` command implemented
- [ ] Conversation tracker implemented
- [ ] Embed formatting implemented
- [ ] Error handling implemented
- [ ] Unit tests written
- [ ] Integration tests written
- [ ] Tested in dev server

### Deployment
- [ ] Dockerfile created (scraper)
- [ ] Dockerfile created (bot)
- [ ] Docker Compose configured
- [ ] Volume sharing tested
- [ ] Environment variables documented
- [ ] Deployment guide written
- [ ] Deployed to production
- [ ] Monitoring set up

### Documentation
- [ ] README written
- [ ] Architecture documented
- [ ] API documentation
- [ ] Deployment guide
- [ ] User guide
- [ ] Contributing guide
- [ ] License added

---

## Testing Strategy

### Unit Tests
- Database operations (CRUD)
- Wiki parser (handle various formats)
- Text chunker (edge cases)
- Embedder (batch vs sequential)
- Query enhancement (theme extraction)

### Integration Tests
- End-to-end scraping (wiki → DB)
- End-to-end retrieval (query → results)
- Docker volume sharing
- Scheduler execution

### Manual Testing
- Initial scrape of 10 comics
- Full scrape of all comics
- Query quality on diverse searches
- Discord command functionality
- Error recovery (network failures)

---

## Performance Targets

- **Initial scrape**: < 10 minutes for 3000 comics
- **Daily scrape**: < 5 seconds
- **Weekly update check**: < 30 minutes
- **Query response time**: < 2 seconds (from Discord command to results)
- **Database size**: < 50 MB
- **Memory usage**: 
  - Scraper: < 500 MB
  - Bot: < 200 MB

---

## Known Limitations & Future Work

### Current Limitations
1. No proactive suggestions (only command-based)
2. Single Discord server only
3. No user feedback mechanism
4. No web interface
5. English language only

### Future Enhancements
1. **Proactive Mode**: Monitor conversation, suggest comics automatically
2. **Multi-Server**: Support multiple Discord servers with separate configs
3. **Feedback Loop**: Let users upvote/downvote suggestions to improve quality
4. **Fine-tuned LLM**: Train on XKCD corpus for better understanding
5. **Image Analysis**: OCR for comics without good explanations
6. **Web Dashboard**: View stats, search history, popular comics
7. **Internationalization**: Support non-English queries
8. **Hybrid Search**: Combine vector search with keyword/BM25 search

---

## Glossary

- **Chunk**: A semantically meaningful piece of text (200-500 chars)
- **Embedding**: Vector representation of text (384-dimensional)
- **Vector Search**: Finding similar items using cosine distance
- **RAG**: Retrieval-Augmented Generation (search + LLM)
- **libSQL**: SQLite fork with native vector support
- **MediaWiki**: Wiki software used by explainxkcd.com
- **Semantic Chunking**: Splitting text at natural boundaries (paragraphs, sentences)
- **Two-Stage Retrieval**: First vector search, then LLM re-ranking

---

## Appendix: Key Design Decisions

### Why Rust?
- Memory safety without garbage collection
- Excellent async/concurrency support
- Strong typing catches bugs at compile time
- Performance comparable to C/C++
- Great ecosystem for web, DB, and Discord

### Why libSQL over PostgreSQL?
- Embedded (no separate server process)
- Native vector support (no extensions)
- Perfect scale for this project (~3000 comics)
- SQLite-compatible (huge ecosystem)
- Simpler deployment

### Why Local Embeddings over API?
- No external dependencies or costs
- Fast enough for this scale
- Privacy (no data sent to third parties)
- Offline capable
- MiniLM-L6-v2 is surprisingly good

### Why Semantic Chunking?
- Better retrieval quality than fixed-size chunks
- Respects natural boundaries (paragraphs, jokes)
- Each chunk is self-contained and meaningful
- Multiple chunks per comic → more precise matching

### Why Two-Stage Retrieval?
- Pure vector search can miss nuance
- LLM understands XKCD culture and humor
- LLM can explain *why* a comic is relevant
- Better user experience with reasons

### Why Built-in Scheduler vs Cron?
- Self-contained (no host system dependencies)
- Cross-platform (works anywhere Docker runs)
- Easier to configure (just env vars)
- Better logging (integrated with app logs)
- Simpler deployment

---

## References

- [explainxkcd.com](https://www.explainxkcd.com/)
- [MediaWiki API](https://www.mediawiki.org/wiki/API:Main_page)
- [libSQL Documentation](https://docs.turso.tech/)
- [fastembed-rs](https://github.com/Anush008/fastembed-rs)
- [text-splitter crate](https://crates.io/crates/text-splitter)
- [Serenity Discord Library](https://github.com/serenity-rs/serenity)
- [tokio-cron-scheduler](https://crates.io/crates/tokio-cron-scheduler)

---

**Document Version**: 1.0  
**Last Updated**: 2025-01-23  
**Status**: Ready for Implementation
