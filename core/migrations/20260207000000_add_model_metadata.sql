-- Migration: Add extended model metadata fields and flags table
-- This extends the taxonomies table with additional fields for model profiles
-- and adds a flags table for country icons

-- Add extended columns to taxonomies for model metadata
-- NOTE: These columns were migrated previously outside of sqlx,
-- so we avoid ALTER TABLE here to prevent "duplicate column" errors.
-- hero_image, flag_id, nationality, birth_year, aliases already exist in taxonomies.

-- Create flags table for country icons
CREATE TABLE IF NOT EXISTS flags (
    id INTEGER PRIMARY KEY,
    code TEXT UNIQUE NOT NULL,  -- Country code (e.g., "us", "uk", "de")
    name TEXT                    -- Full country name (e.g., "United States")
);

CREATE INDEX IF NOT EXISTS idx_flags_code ON flags(code);
