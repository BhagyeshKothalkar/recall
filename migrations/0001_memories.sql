CREATE TABLE IF NOT EXISTS memories (
    id TEXT PRIMARY KEY NOT NULL,
    content TEXT NOT NULL,
    source_type TEXT NOT NULL,
    source_value TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (length(trim(content)) > 0),
    CHECK (
        (source_type = 'direct_input' AND source_value IS NULL)
        OR
        (source_type = 'file' AND source_value IS NOT NULL)
    ),
    CHECK (updated_at >= created_at)
);
