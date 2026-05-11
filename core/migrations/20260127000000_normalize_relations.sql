-- Normalize Page Relations
CREATE TABLE IF NOT EXISTS page_relations (
    source_id INTEGER NOT NULL,
    target_id INTEGER NOT NULL,
    PRIMARY KEY(source_id, target_id),
    FOREIGN KEY(source_id) REFERENCES pages(id) ON DELETE CASCADE,
    FOREIGN KEY(target_id) REFERENCES pages(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_page_relations_target ON page_relations(target_id);
