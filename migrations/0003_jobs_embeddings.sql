-- Durable derivation jobs.
CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    state TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_error TEXT,
    CHECK (kind = 'generate_embedding'),
    CHECK (state IN ('pending', 'running', 'completed', 'failed')),
    CHECK (updated_at >= created_at)
);

CREATE INDEX IF NOT EXISTS jobs_state_idx ON jobs(state, created_at);

-- Durable derived embeddings. The vector is encoded as little-endian f32 values.
CREATE TABLE IF NOT EXISTS embeddings (
    memory_id TEXT PRIMARY KEY NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    vector BLOB NOT NULL,
    dimensions INTEGER NOT NULL CHECK (dimensions > 0),
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS embeddings_model_idx ON embeddings(model);

-- Existing canonical memories receive durable embedding work on upgrade.
INSERT INTO jobs (id, kind, memory_id, state, attempts, created_at, updated_at, last_error)
SELECT
    lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-' ||
           substr('89ab', abs(random()) % 4 + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))),
    'generate_embedding', m.id, 'pending', 0, m.created_at, m.updated_at, NULL
FROM memories m
WHERE NOT EXISTS (
    SELECT 1 FROM jobs j
    WHERE j.memory_id = m.id AND j.kind = 'generate_embedding'
);
