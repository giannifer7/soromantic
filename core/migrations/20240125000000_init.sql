-- Soromantic Database Schema (Consolidated)
-- Page-centric model with ID-based asset paths

-- Pages table (central entity)
-- Status values: 0=none, 1=pending, 2=downloading, 3=done, 4=error
CREATE TABLE IF NOT EXISTS pages (
    id INTEGER PRIMARY KEY,
    url TEXT UNIQUE,
    title TEXT,
    cover_status INTEGER DEFAULT 0,
    thumb_status INTEGER DEFAULT 0,
    preview_status INTEGER DEFAULT 0,
    video_status INTEGER DEFAULT 0,
    -- Denormalized relations for fast forward lookup
    model_ids TEXT,      -- Comma-separated Taxonomy IDs (e.g. ",1,5,")
    studio_id INTEGER,   -- Single Studio Taxonomy ID
    related_ids TEXT     -- Comma-separated Page IDs (e.g. ",102,405,")
);
-- CREATE INDEX IF NOT EXISTS idx_pages_studio_id ON pages(studio_id);

CREATE INDEX IF NOT EXISTS idx_pages_title ON pages(title);
CREATE INDEX IF NOT EXISTS idx_pages_thumb_status ON pages(thumb_status);
CREATE INDEX IF NOT EXISTS idx_pages_id_desc ON pages(id DESC);

-- Video sources table
CREATE TABLE IF NOT EXISTS video_sources (
    id INTEGER PRIMARY KEY,
    page_id INTEGER,
    resolution INTEGER,
    status INTEGER DEFAULT 0,
    duration REAL,
    start_time REAL DEFAULT 0.0,
    stop_time REAL DEFAULT 0.0,
    UNIQUE(page_id, resolution)
);

CREATE INDEX IF NOT EXISTS idx_video_sources_page_id ON video_sources(page_id);

-- Taxonomies table (unified models/studios)
CREATE TABLE IF NOT EXISTS taxonomies (
    id INTEGER PRIMARY KEY,
    url TEXT UNIQUE,
    name TEXT,
    type INTEGER -- 1=model, 2=studio
);

CREATE INDEX IF NOT EXISTS idx_taxonomies_name ON taxonomies(name);
CREATE INDEX IF NOT EXISTS idx_taxonomies_type ON taxonomies(type);

-- Links table (taxonomy associations)
CREATE TABLE IF NOT EXISTS links (
    id INTEGER PRIMARY KEY,
    page_id INTEGER,
    taxonomy_id INTEGER,
    FOREIGN KEY (page_id) REFERENCES pages(id),
    FOREIGN KEY (taxonomy_id) REFERENCES taxonomies(id)
);

CREATE INDEX IF NOT EXISTS idx_links_page_id ON links(page_id);
-- CREATE INDEX IF NOT EXISTS idx_links_taxonomy_id ON links(taxonomy_id);
-- CREATE INDEX IF NOT EXISTS idx_links_taxonomy_page ON links(taxonomy_id, page_id);

-- Page relations (Legacy directed graph, replaced by denormalized related_ids in pages)
-- Legacy tables will be dropped after data migration in a subsequent script.
-- DROP TABLE IF EXISTS page_relations;
-- DROP TABLE IF EXISTS grid_boxes;
