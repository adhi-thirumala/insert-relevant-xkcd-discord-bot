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

CREATE INDEX idx_updated_at ON xkcd_comics(updated_at);


-- Semantic chunks with embeddings
CREATE TABLE xkcd_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    comic_number INTEGER NOT NULL,
    chunk_text TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    section_type TEXT,              -- 'title_hover', 'explanation', 'transcript', 'trivia'
    embedding F32_BLOB(1024) NOT NULL,

    FOREIGN KEY (comic_number) REFERENCES xkcd_comics(comic_number) ON DELETE CASCADE
);

-- Vector search index
CREATE INDEX chunks_vec_idx ON xkcd_chunks(
    libsql_vector_idx(embedding, 'metric=cosine')
);

-- Fetch all chunks for a comic efficiently
CREATE INDEX idx_comic_chunks ON xkcd_chunks(comic_number, chunk_index);


CREATE TABLE metadata (
    key TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO metadata (key, value) VALUES ('INITIALIZED', 'true');
