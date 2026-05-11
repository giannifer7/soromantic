-- Rompla Database Schema (port from Nim project)
-- Replaces the old Soromantic schema with normalized tables

-- Drop old tables (data migration not performed — existing data will be lost)
DROP TABLE IF EXISTS links;
DROP TABLE IF EXISTS taxonomies;
DROP TABLE IF EXISTS flags;
DROP TABLE IF EXISTS grid_boxes;
DROP TABLE IF EXISTS page_relations;

-- ============================================================
-- Sites — configured scrapers with URL prefix matching
-- ============================================================
CREATE TABLE IF NOT EXISTS sites (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE,
    url_prefix TEXT,
    scraper TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_sites_name ON sites (name);

-- ============================================================
-- Nations (flags)
-- ============================================================
CREATE TABLE IF NOT EXISTS nations (
    id INTEGER PRIMARY KEY,
    code TEXT UNIQUE NOT NULL,
    name TEXT,
    flag_status INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_nations_code ON nations (code);

-- ============================================================
-- Studios
-- ============================================================
CREATE TABLE IF NOT EXISTS studios (
    id INTEGER PRIMARY KEY,
    url TEXT UNIQUE,
    name TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_studios_name ON studios (name);

-- ============================================================
-- Performers (models)
-- ============================================================
CREATE TABLE IF NOT EXISTS performers (
    id INTEGER PRIMARY KEY,
    name TEXT,
    star INTEGER DEFAULT 0,
    sex INTEGER DEFAULT 0,
    birth_year INTEGER,
    aliases TEXT,
    thumb_status INTEGER DEFAULT 0,
    nation_id INTEGER,
    FOREIGN KEY(nation_id) REFERENCES nations(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_performers_name ON performers (name);
CREATE INDEX IF NOT EXISTS idx_performers_star ON performers (star);

-- ============================================================
-- Pages — now with site_id, relative URLs
-- ============================================================

-- Add site_id to existing pages table if it exists
-- If old pages table doesn't have site_id, we need to alter it
-- Since we might be starting fresh, use CREATE IF NOT EXISTS

-- Check if pages table exists without site_id
-- SQLite doesn't support IF NOT EXISTS for ALTER, so we handle this in Rust init
-- For the migration, we recreate the table if needed

-- First, preserve old pages data if any (unlikely in a fresh migration)
-- Then recreate with new schema

CREATE TABLE IF NOT EXISTS pages_new (
    id INTEGER PRIMARY KEY,
    site_id INTEGER DEFAULT 0,
    url TEXT,
    title TEXT,
    studio_id INTEGER,
    thumb_status INTEGER DEFAULT 0,
    preview_status INTEGER DEFAULT 0,
    video_status INTEGER DEFAULT 0,
    cover_status INTEGER DEFAULT 0,
    FOREIGN KEY(site_id) REFERENCES sites(id)
);

-- Copy data from old pages if the table exists and has the old schema
-- (skip if pages_new was just created empty or pages doesn't exist)
INSERT OR IGNORE INTO pages_new (id, site_id, url, title, studio_id, thumb_status, preview_status, video_status, cover_status)
SELECT id, 0, url, title, studio_id, thumb_status, preview_status, COALESCE(video_status, 0), cover_status
FROM pages
WHERE NOT EXISTS (SELECT 1 FROM pages_new LIMIT 1);

-- Drop old pages and rename
DROP TABLE IF EXISTS pages;
ALTER TABLE pages_new RENAME TO pages;

CREATE INDEX IF NOT EXISTS idx_pages_studio_id ON pages (studio_id);
CREATE INDEX IF NOT EXISTS idx_pages_site_id ON pages (site_id);
CREATE INDEX IF NOT EXISTS idx_pages_title ON pages (title);
CREATE INDEX IF NOT EXISTS idx_pages_thumb_status ON pages (thumb_status);
CREATE INDEX IF NOT EXISTS idx_pages_id_desc ON pages (id DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_pages_site_url ON pages (site_id, url);

-- ============================================================
-- pages_full view — reconstructs full URLs
-- ============================================================
DROP VIEW IF EXISTS pages_full;
CREATE VIEW pages_full AS
SELECT
  p.id,
  CASE WHEN s.url_prefix IS NOT NULL THEN s.url_prefix || p.url ELSE p.url END AS url,
  p.title,
  p.studio_id,
  p.site_id,
  p.thumb_status,
  p.preview_status,
  p.video_status,
  p.cover_status
FROM pages p
LEFT JOIN sites s ON p.site_id = s.id;

-- ============================================================
-- Performer URLs
-- ============================================================
CREATE TABLE IF NOT EXISTS performer_urls (
    id INTEGER PRIMARY KEY,
    performer_id INTEGER,
    site_id INTEGER,
    url TEXT UNIQUE,
    FOREIGN KEY(performer_id) REFERENCES performers(id) ON DELETE CASCADE,
    FOREIGN KEY(site_id) REFERENCES sites(id)
);

CREATE INDEX IF NOT EXISTS idx_performer_urls_performer_id ON performer_urls (performer_id);
CREATE INDEX IF NOT EXISTS idx_performer_urls_site_id ON performer_urls (site_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_performer_urls_performer_site ON performer_urls (performer_id, site_id);

-- ============================================================
-- Cast — performer↔page associations
-- ============================================================
CREATE TABLE IF NOT EXISTS cast (
    id INTEGER PRIMARY KEY,
    page_id INTEGER,
    performer_id INTEGER,
    starring INTEGER DEFAULT 1,
    FOREIGN KEY (page_id) REFERENCES pages (id),
    FOREIGN KEY (performer_id) REFERENCES performers (id)
);

CREATE INDEX IF NOT EXISTS idx_cast_performer_id ON cast (performer_id);
CREATE INDEX IF NOT EXISTS idx_cast_performer_page ON cast (performer_id, page_id);
CREATE INDEX IF NOT EXISTS idx_cast_page_id ON cast (page_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_cast_unique ON cast (page_id, performer_id);

-- ============================================================
-- Studio Links — studio↔page associations
-- ============================================================
CREATE TABLE IF NOT EXISTS studio_links (
    id INTEGER PRIMARY KEY,
    page_id INTEGER,
    studio_id INTEGER,
    FOREIGN KEY (page_id) REFERENCES pages (id),
    FOREIGN KEY (studio_id) REFERENCES studios (id)
);

CREATE INDEX IF NOT EXISTS idx_studio_links_studio_id ON studio_links (studio_id);
CREATE INDEX IF NOT EXISTS idx_studio_links_studio_page ON studio_links (studio_id, page_id);
CREATE INDEX IF NOT EXISTS idx_studio_links_page_id ON studio_links (page_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_studio_links_unique ON studio_links (page_id, studio_id);

-- ============================================================
-- Page Relations — related pages
-- ============================================================
CREATE TABLE IF NOT EXISTS page_relations (
    source_id INTEGER NOT NULL,
    target_id INTEGER NOT NULL,
    PRIMARY KEY (source_id, target_id),
    FOREIGN KEY (source_id) REFERENCES pages (id) ON DELETE CASCADE,
    FOREIGN KEY (target_id) REFERENCES pages (id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_page_relations_target ON page_relations (target_id);

-- ============================================================
-- Video Sources
-- ============================================================
CREATE TABLE IF NOT EXISTS video_sources (
    id INTEGER PRIMARY KEY,
    page_id INTEGER,
    resolution INTEGER,
    duration REAL,
    start_time REAL DEFAULT 0.0,
    stop_time REAL DEFAULT 0.0,
    status INTEGER DEFAULT 0,
    UNIQUE (page_id, resolution)
);

CREATE INDEX IF NOT EXISTS idx_video_sources_page_id ON video_sources (page_id);
CREATE INDEX IF NOT EXISTS idx_video_sources_page_status ON video_sources (page_id, status);

-- ============================================================
-- Insert _orphaned_ sentinel site
-- ============================================================
INSERT OR IGNORE INTO sites (name, url_prefix, scraper) VALUES ('_orphaned_', NULL, '');
