-- Add covering index for video_sources aggregation
-- This optimizes the "get_library_paginated" query by allowing index-only scans
-- for page_id and status lookups.

CREATE INDEX IF NOT EXISTS idx_video_sources_page_status ON video_sources(page_id, status);
