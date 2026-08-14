# Recall — Implementation Overview

## 1. Purpose of This Document

`OVERVIEW.md` describes the implementation that currently exists in the Recall crate.

It is intentionally different from `ENGINEERING.md` and the architectural/product documents:

- those documents define principles, decisions, invariants, and preferred engineering behavior;
- this document describes the **actual Rust constructs**, their contracts, their boundaries, their data flow, and the responsibilities of the functions and adapters currently implemented.

The implementation is organized around four concrete concerns:

```text
Domain
  ↓
Application
  ↓
Infrastructure
  ↓
Runtime
```

The CLI is a transport-facing presentation layer over the daemon.

---

# 2. Current Implemented Capability

The current implementation supports the complete primary memory path:

```text
recall store "memory"
        ↓
CLI
        ↓
Unix socket
        ↓
daemon
        ↓
StoreMemory
        ↓
atomic SQLite persistence
        ├── canonical memory
        └── embedding job
        ↓
background worker
        ↓
Ollama embedding
        ↓
SQLite embedding
```

And the question path:

```text
recall "question"
        ↓
CLI
        ↓
Unix socket
        ↓
AskRecall
        ├── SQLite FTS
        ├── semantic retrieval
        └── reciprocal-rank fusion
                ↓
          GenerationRequest
                ↓
          OllamaBackend
                ↓
             Answer
                ↓
          source memory IDs
```

Retrieval-only mode is:

```text
recall --no-ai "question"
        ↓
SQLite FTS
        ↓
retrieved memories
```

The retrieval-only path deliberately does not call the embedding backend.

---

# 3. Module Map

The crate is divided into these implementation areas:

```text
src/
├── main.rs
├── lib.rs
│
├── domain/
│   ├── memory.rs
│   ├── source.rs
│   ├── search.rs
│   ├── inference.rs
│   ├── job.rs
│   └── mod.rs
│
├── application/
│   ├── store.rs
│   ├── ask.rs
│   ├── retrieval.rs
│   ├── ports/
│   │   ├── memory_repository.rs
│   │   ├── store_persistence.rs
│   │   ├── job_repository.rs
│   │   ├── embedding_repository.rs
│   │   ├── memory_searcher.rs
│   │   ├── semantic_memory_searcher.rs
│   │   ├── text_embedder.rs
│   │   ├── inference_backend.rs
│   │   └── clock.rs
│   └── mod.rs
│
├── infrastructure/
│   ├── database/
│   │   ├── connection.rs
│   │   ├── migrations.rs
│   │   ├── memory_repository.rs
│   │   ├── store_persistence.rs
│   │   ├── job_repository.rs
│   │   ├── embedding_repository.rs
│   │   ├── search.rs
│   │   └── semantic_search.rs
│   │
│   ├── inference/
│   │   ├── config.rs
│   │   ├── ollama.rs
│   │   └── prompt.rs
│   │
│   ├── ipc/
│   │   ├── protocol.rs
│   │   ├── client.rs
│   │   └── server.rs
│   │
│   └── clock.rs
│
├── runtime/
│   ├── composition.rs
│   ├── daemon.rs
│   └── worker.rs
│
└── cli/
    ├── command.rs
    ├── input.rs
    ├── render.rs
    └── mod.rs
```

The important distinction is:

```text
domain/
    values and state machines

application/
    use cases and required capabilities

infrastructure/
    concrete implementations of those capabilities

runtime/
    composition and lifecycle

cli/
    user-facing command and rendering
```

---

# 4. Domain Layer

The domain layer contains values that describe Recall's actual concepts without depending on SQLite, Ollama, IPC, or CLI.

## 4.1 `domain::Memory`

`Memory` is the canonical memory representation.

Conceptually:

```rust
Memory {
    id: MemoryId,
    content: String,
    source: MemorySource,
    created_at: Timestamp,
    updated_at: Timestamp,
}
```

### `MemoryId`

`MemoryId` wraps a UUID.

Important operations:

```rust
MemoryId::new()
MemoryId::from_uuid(uuid)
MemoryId::as_uuid()
```

It implements `Display`, so it can cross the CLI/IPC boundary as a normal UUID string.

The ID is generated when canonical memory is created.

### `Timestamp`

`Timestamp` is the domain representation of time.

It stores Unix milliseconds and exposes:

```rust
Timestamp::from_unix_millis(...)
Timestamp::as_unix_millis()
```

The domain does not obtain system time itself. The application receives time through the `Clock` port.

### `Memory::new`

`Memory::new` is the canonical constructor.

It establishes memory invariants before persistence.

The current domain validation rejects:

- empty/whitespace-only content;
- invalid timestamp ordering.

Infrastructure reconstructs persisted memories through this constructor rather than bypassing domain validation.

---

# 5. `domain::MemorySource`

`MemorySource` represents how canonical content entered Recall.

Current variants:

```rust
MemorySource::DirectInput

MemorySource::File {
    path: PathBuf,
}
```

The source is typed rather than represented by an arbitrary string.

The distinction survives persistence:

```text
direct_input → no source_value

file         → source_value contains path
```

This means a file and direct text can contain identical text while retaining different provenance.

---

# 6. `domain::search`

The retrieval domain types are separate from canonical memory.

## `SearchQuery`

Contains:

```text
text
limit
```

Construction rejects:

- empty/whitespace-only text;
- zero result limits.

The query is the application-level contract passed into search implementations.

## `SearchScore`

`SearchScore` wraps `f32`.

Its constructor rejects `NaN`, because a NaN cannot participate reliably in ordering.

The underlying score remains implementation-defined.

## `SearchResult`

Contains:

```text
Memory
SearchScore
```

It provides:

```rust
memory()
score()
memory_id()
```

The score is not stored on `Memory`.

## `Relevance`

`Relevance` is the score assigned to a memory when it becomes generation context.

It is deliberately distinct from `SearchScore`.

## `RetrievedMemory`

Contains:

```text
Memory
Relevance
```

This is the explicit boundary between retrieval and generation.

It exposes:

```rust
memory()
relevance()
memory_id()
```

The application creates these values. The inference backend does not decide which memories become context.

---

# 7. `domain::inference`

The inference domain is backend-neutral.

## `InferenceModel`

Wraps a model name.

Construction rejects empty names.

It exposes:

```rust
name()
```

The domain does not know whether the model belongs to Ollama, llama.cpp, Candle, or another runtime.

## `GenerationRequest`

Contains:

```text
question
context: Vec<RetrievedMemory>
```

Construction rejects an empty question.

It exposes:

```rust
question()
context()
source_ids()
```

`source_ids()` derives provenance from the supplied context.

This is important: source identity is known before generation.

## `GenerationResponse`

Contains generated text.

Construction rejects empty output.

The backend returns this value to the application.

## `EmbeddingRequest`

Contains:

```text
memory_id
text
model
```

It represents embedding one canonical memory.

## `EmbeddingResponse`

Contains:

```text
memory_id
model
vector
```

Construction rejects:

- empty vectors;
- NaN vector values.

The vector is derived state, not part of `Memory`.

---

# 8. `domain::job`

The job model represents durable derivation work.

## `JobId`

UUID-backed identity.

## `JobKind`

Currently one kind exists:

```rust
JobKind::GenerateEmbedding {
    memory_id: MemoryId,
}
```

The job refers to canonical memory by ID rather than copying the memory.

## `JobState`

Current states:

```text
Pending
Running
Completed
Failed
```

## Job lifecycle

The valid state machine is:

```text
Pending
   ↓ start
Running
   ├── complete → Completed
   │
   └── fail → Failed
                  │
                  └── retry → Pending
```

Completed jobs cannot be restarted.

### `Job::new`

Creates a pending job with:

```text
attempts = 0
last_error = None
created_at = updated_at
```

### `Job::start`

Moves:

```text
Pending → Running
```

and increments attempts.

### `Job::complete`

Moves:

```text
Running → Completed
```

and clears any failure.

### `Job::fail`

Moves:

```text
Running → Failed
```

and records the failure string.

### `Job::retry`

Moves:

```text
Failed → Pending
```

### `Job::from_persisted`

Reconstructs a persisted job while validating:

- timestamp order;
- failed jobs have an error;
- non-failed jobs do not retain an error.

Infrastructure can restore state, but lifecycle changes remain domain operations.

---

# 9. Application Ports

Application ports define capabilities required by use cases.

They contain no SQLite or Ollama implementation.

## `MemoryRepository`

Contract:

```rust
create(&self, memory: &Memory)
get(&self, id: MemoryId)
delete(&self, id: MemoryId)
```

Purpose:

- persist canonical memories;
- retrieve canonical memories;
- authoritatively delete canonical memories.

It is the basic canonical-memory persistence abstraction.

## `StorePersistence`

Contract:

```rust
persist(&self, memory: &Memory, job: &Job)
```

This is the stronger persistence boundary used by the store use case.

Its contract is atomic:

```text
canonical memory
+
initial derivation job
```

must commit together.

This prevents a successfully stored memory from becoming invisible to the background derivation system.

## `JobRepository`

Contract:

```rust
create(&self, job)
claim_pending(&self, now)
update(&self, job)
get(&self, id)
list_by_state(&self, state)
```

`claim_pending` is the worker-facing operation.

The infrastructure implementation performs the durable claim transaction.

## `EmbeddingRepository`

Contract:

```rust
upsert(&self, embedding)
delete(&self, memory_id)
exists_for_model(&self, memory_id, model)
```

This repository owns derived embedding persistence.

It does not own canonical memory.

## `MemorySearcher`

Contract:

```rust
search(&self, query)
```

Returns lexical `SearchResult` values.

The application does not know that the implementation is SQLite FTS5.

## `SemanticMemorySearcher`

Contract:

```rust
search_semantic(&self, query)
```

Returns semantic `SearchResult` values.

The semantic implementation is free to change without changing `AskRecall`.

## `TextEmbedder`

Contract:

```rust
embed_text(&self, text, model)
```

This is separate from embedding a canonical memory.

Its purpose is query embedding.

This distinction prevents the semantic searcher from pretending that a query is itself a stored memory.

## `InferenceBackend`

Contract:

```rust
generate(&self, request)
embed(&self, request)
```

The application supplies complete domain requests.

The backend:

- does not query SQLite;
- does not select memories;
- does not assign provenance.

## `Clock`

Contract:

```rust
now()
```

The production implementation is `SystemClock`.

Tests can use deterministic clocks.

---

# 10. Store Use Case

`application::store::StoreMemory` owns the store workflow.

Its generic dependencies are:

```text
P: StorePersistence
C: Clock
```

Construction:

```rust
StoreMemory::new(persistence, clock, limits)
```

## `StoreInput`

Current variants:

```text
Text(String)
File(PathBuf)
```

## `MemoryLimits`

Contains:

```text
max_content_bytes
```

The current composition supplies the initial default maximum.

## `StoreMemory::execute`

The function performs:

```text
StoreInput
    ↓
capture
    ↓
size/content validation
    ↓
Memory::new
    ↓
Job::new(GenerateEmbedding)
    ↓
StorePersistence::persist
    ↓
StoreResponse
```

### `capture`

For:

```text
StoreInput::Text
```

the string is used directly and source becomes `DirectInput`.

For:

```text
StoreInput::File
```

the file is read as UTF-8 text and source becomes `File { path }`.

No document parser, MIME detection, OCR, or other ingestion layer is involved.

### `validate_size`

Rejects:

- whitespace-only content;
- content larger than `max_content_bytes`.

Validation happens before persistence.

### `StoreResponse`

Contains:

```text
memory_id
content_length
```

It does not expose the job or database implementation.

---

# 11. Atomic Store Persistence

`SqliteStorePersistence` implements `StorePersistence`.

Its `persist` function opens one SQLite transaction and performs:

```text
BEGIN

INSERT memories

INSERT jobs

COMMIT
```

If either insert fails, the transaction is not committed.

The canonical memory and its first embedding job therefore have one persistence boundary.

The job is initially:

```text
kind = generate_embedding
state = pending
attempts = 0
```

The worker performs the expensive model call later.

---

# 12. SQLite Canonical Memory Storage

`SqliteMemoryRepository` implements `MemoryRepository`.

Its operations map domain values to the `memories` table.

### Create

Persists:

```text
id
content
source_type
source_value
created_at
updated_at
```

### Get

Reads the row, reconstructs `MemorySource`, and passes the complete value through `Memory::new`.

### Delete

Deletes the canonical memory.

Foreign-key cascade behavior removes associated durable derived state.

The repository owns SQL mapping only. It does not orchestrate application workflows.

---

# 13. SQLite Database Connection

`DatabaseConnection` owns a SQLite connection.

Opening a database configures the local database for Recall's runtime requirements, including WAL and foreign-key behavior.

There are separate construction paths for:

```text
file-backed database
in-memory test database
```

The connection is owned by concrete repositories rather than borrowed from an application service, avoiding self-referential runtime ownership.

---

# 14. Database Migrations

`Migrator` executes migration files.

Current migrations:

```text
0001_memories.sql
0002_fts.sql
0003_jobs_embeddings.sql
```

The migration layer keeps schema SQL outside Rust application code.

---

# 15. Canonical Memory Schema

`memories` contains:

```text
id             TEXT PRIMARY KEY
content        TEXT NOT NULL
source_type    TEXT NOT NULL
source_value   TEXT
created_at     INTEGER NOT NULL
updated_at     INTEGER NOT NULL
```

Database constraints enforce:

```text
content is non-empty
source representation is internally consistent
updated_at >= created_at
```

The database therefore protects core persistence invariants in addition to the Rust domain.

---

# 16. SQLite FTS5 Search

`memories_fts` is a derived FTS5 table.

It is configured as a content-backed index over:

```text
memories.content
```

It is not canonical storage.

Triggers maintain the derived index for:

```text
INSERT
DELETE
UPDATE content
```

The migration also performs an initial FTS rebuild so memories that existed before the FTS migration become searchable.

---

# 17. `SqliteMemorySearcher`

The lexical search implementation is:

```text
SqliteMemorySearcher
```

It implements:

```rust
MemorySearcher
```

## Search operation

The search path is:

```text
SearchQuery
    ↓
FTS expression
    ↓
memories_fts MATCH
    ↓
join canonical memories
    ↓
bm25 ranking
    ↓
SearchResult[]
```

The implementation uses SQLite FTS5 `bm25`.

The canonical `memories` table is joined after FTS candidate selection so returned objects are reconstructed from authoritative memory data.

The FTS index is never treated as the canonical memory.

---

# 18. FTS Query Normalization

The current implementation converts whitespace-separated query tokens into an OR expression with quoted tokens.

Conceptually:

```text
SQLite WAL
```

becomes an FTS expression equivalent to:

```text
"SQLite" OR "WAL"
```

Quotes inside tokens are escaped for the FTS expression.

This is intentionally a small lexical implementation rather than a larger query-language subsystem.

---

# 19. Durable Embedding Storage

The `embeddings` table stores derived vectors:

```text
memory_id
model
vector
dimensions
created_at
```

The vector is stored as a SQLite BLOB containing little-endian `f32` values.

`SqliteEmbeddingRepository` provides:

```rust
upsert
delete
exists_for_model
```

The repository contains the encoding/decoding implementation.

The canonical memory is unaffected if an embedding is absent or regenerated.

---

# 20. Embedding Worker

`EmbeddingWorker` owns execution of durable embedding jobs.

Its generic dependencies are:

```text
J: JobRepository
M: MemoryRepository
R: EmbeddingRepository
E: InferenceBackend
C: Clock
```

The worker does not own any of those systems.

## `run_once`

The worker:

```text
claim_pending
    ↓
inspect JobKind
    ↓
load canonical memory
    ↓
construct EmbeddingRequest
    ↓
InferenceBackend::embed
    ↓
EmbeddingRepository::upsert
    ↓
Job::complete
    ↓
JobRepository::update
```

On failure:

```text
Job::fail
    ↓
JobRepository::update
```

The canonical memory is not deleted.

## `run_forever`

The daemon worker loops with a bounded polling interval.

The current implementation polls every 250 ms.

This is a local durable queue rather than a distributed job system.

---

# 21. Existing-Memory Upgrade Behavior

Migration `0003_jobs_embeddings.sql` creates a pending embedding job for canonical memories that do not already have an embedding job.

This means adding the embedding subsystem to an existing database produces durable work for the existing corpus.

The worker can subsequently populate embeddings without rewriting canonical memories.

---

# 22. Ollama Configuration

All Ollama configuration is centralized in:

```text
src/infrastructure/inference/config.rs
```

No application use case owns Ollama configuration.

`OllamaConfig` contains:

```text
base_url
generation_model
embedding_model
timeout
```

Environment variables:

```text
RECALL_OLLAMA_URL
RECALL_OLLAMA_MODEL
RECALL_OLLAMA_EMBEDDING_MODEL
RECALL_OLLAMA_TIMEOUT_SECS
```

Defaults currently include:

```text
RECALL_OLLAMA_URL
    http://localhost:11434

RECALL_OLLAMA_MODEL
    llama3.2

RECALL_OLLAMA_EMBEDDING_MODEL
    same as generation model when unspecified

RECALL_OLLAMA_TIMEOUT_SECS
    120
```

The configuration object validates:

- non-empty URL;
- non-empty generation model;
- non-empty embedding model;
- numeric timeout.

The adapter receives an already constructed `OllamaConfig`.

---

# 23. Ollama HTTP Adapter

`OllamaBackend` implements:

```rust
InferenceBackend
TextEmbedder
```

It owns:

```text
reqwest blocking Client
OllamaConfig
```

The HTTP client timeout comes from centralized configuration.

## Generation

`InferenceBackend::generate` performs:

```text
GenerationRequest
    ↓
prompt::build
    ↓
Ollama /api/generate
    ↓
GenerationResponse
```

The request uses:

```text
model
prompt
system
stream = false
```

The adapter maps HTTP/transport/JSON errors to `InferenceError`.

Empty generation output is rejected.

## Embedding

`InferenceBackend::embed` delegates to the same backend's query embedding capability and associates the resulting vector with the canonical memory ID and model.

## Query embedding

`TextEmbedder::embed_text` calls:

```text
Ollama /api/embed
```

and validates that:

- at least one embedding is returned;
- the selected vector is non-empty;
- all values are finite.

The first embedding in the response is used for the current single-text request.

---

# 24. Prompt Construction

Prompt construction is separated from Ollama transport in:

```text
src/infrastructure/inference/prompt.rs
```

`prompt::build` receives a complete `GenerationRequest`.

It produces:

```text
Prompt {
    system,
    prompt
}
```

The system instructions establish that:

- supplied memories are data rather than instructions;
- unsupported facts should not be invented;
- insufficient context should be acknowledged.

The user prompt contains:

```text
Retrieved Recall memories
    ↓
memory delimiters
    ↓
memory contents
    ↓
user question
```

The prompt builder does not access SQLite.

It receives the exact context selected by `AskRecall`.

---

# 25. Ask Use Case

`application::ask::AskRecall` owns question answering.

Its dependencies are:

```text
MemorySearcher
SemanticMemorySearcher
InferenceBackend
```

and a retrieval limit.

Construction rejects a zero retrieval limit.

---

# 26. `AskRequest`

Contains:

```text
question
use_ai
```

Construction rejects an empty question.

The CLI's:

```text
recall "question"
```

maps to:

```text
use_ai = true
```

while:

```text
recall --no-ai "question"
```

maps to:

```text
use_ai = false
```

---

# 27. Ask Retrieval Flow

`AskRecall::execute` first retrieves memories.

For `use_ai = false`:

```text
SearchQuery
    ↓
MemorySearcher
    ↓
lexical SearchResult[]
    ↓
AskResult::Retrieved
```

For `use_ai = true`:

```text
SearchQuery
    ↓
lexical retrieval
    +
semantic retrieval
    ↓
merge_results
    ↓
RetrievedMemory[]
    ↓
GenerationRequest
    ↓
InferenceBackend
    ↓
Answer
```

The AI path therefore uses both retrieval mechanisms.

The no-AI path intentionally remains lexical-only so that it can retrieve memories without invoking Ollama.

---

# 28. Hybrid Retrieval

`application::retrieval::merge_results` combines lexical and semantic candidate lists.

The current algorithm is reciprocal rank fusion.

For each candidate rank:

```text
1 / (60 + rank + 1)
```

is added to that memory's combined score.

The candidate maps are keyed by `MemoryId`.

A memory appearing in both result sets receives contributions from both rankings.

The merged candidates are sorted by combined score and truncated to the configured retrieval limit.

The result is represented again as `SearchResult`.

This keeps the hybrid ranking policy in the application layer rather than embedding it into either SQLite search implementation.

---

# 29. Provenance

When AI is enabled:

```text
SearchResult[]
    ↓
RetrievedMemory[]
    ↓
GenerationRequest
```

The `GenerationRequest` contains the exact memories supplied to the model.

After generation:

```rust
let sources = generation.source_ids();
```

constructs the answer's source IDs from the request context.

The model output itself does not determine provenance.

The resulting `Answer` contains:

```text
text
sources: Vec<MemoryId>
```

This is the concrete implementation of application-owned provenance.

---

# 30. Retrieval-Only Result

When AI is disabled, `AskResult::Retrieved` contains the actual search results.

The IPC layer converts them to:

```text
RetrievedResponse
    └── memories[]
          ├── memory_id
          ├── content
          └── score
```

The CLI renders these directly.

No generation request is constructed.

No Ollama call is made.

No query embedding is made.

---

# 31. IPC Protocol

The wire protocol lives in:

```text
src/infrastructure/ipc/protocol.rs
```

The protocol uses serde serialization.

Requests:

```text
Request::Store
Request::Ask
Request::Status
```

Responses:

```text
Response::Store
Response::Answer
Response::Retrieved
Response::Status
Response::Error
```

The wire structs are separate from application/domain structs.

This allows the protocol representation to evolve without making domain types depend on serialization.

---

# 32. Store IPC Contract

A store request contains:

```text
StoreRequest {
    input: StoreInput
}
```

Wire `StoreInput` is:

```text
Text(String)
File(PathBuf)
```

The server converts the wire value into application `StoreInput`.

It then invokes:

```text
StoreService::execute
```

On success it returns:

```text
StoreResponse {
    memory_id,
    content_length
}
```

---

# 33. Ask IPC Contract

An ask request contains:

```text
AskRequest {
    question,
    use_ai
}
```

The server converts this to the application `AskRequest`.

The application decides whether to:

```text
retrieve only
```

or:

```text
retrieve + generate
```

The wire response distinguishes those outcomes:

```text
Response::Retrieved
Response::Answer
```

---

# 34. IPC Server

`IpcServer` is deliberately thin.

It owns:

```text
UnixListener
```

and references the already-composed:

```text
StoreService
AskService
```

Its responsibilities are:

```text
bind socket
accept connection
decode request
dispatch
encode response
```

It does not contain:

- SQL;
- FTS ranking;
- embedding logic;
- prompt construction;
- Ollama HTTP calls;
- memory construction rules.

The server is transport glue.

A maximum IPC message size is enforced by the framing implementation.

---

# 35. IPC Client

The CLI uses `IpcClient`.

Its responsibilities are:

```text
connect Unix socket
serialize Request
send request
read Response
deserialize Response
```

It does not execute application logic.

This keeps the CLI independent from SQLite and Ollama.

---

# 36. Runtime Composition

`runtime::composition` is the concrete dependency assembly point.

It selects:

```text
SqliteStorePersistence
SqliteMemorySearcher
SqliteSemanticMemorySearcher
SqliteJobRepository
SqliteMemoryRepository
SqliteEmbeddingRepository
OllamaBackend
SystemClock
```

and connects them to application services.

The composition root is the only location where the application is tied to the current concrete infrastructure choices.

---

# 37. Store Service Composition

The store service is constructed with:

```text
SqliteStorePersistence
SystemClock
MemoryLimits
```

The current composition uses the central memory-size limit and system clock.

This creates:

```text
StoreMemory<
    SqliteStorePersistence,
    SystemClock
>
```

No CLI dependency enters the service.

---

# 38. Ask Service Composition

The ask service is constructed with:

```text
SqliteMemorySearcher
SqliteSemanticMemorySearcher<OllamaBackend>
OllamaBackend
retrieval_limit
```

The same configured Ollama backend is shared between generation and embedding/query-embedding operations.

The application sees only:

```text
MemorySearcher
SemanticMemorySearcher
InferenceBackend
```

---

# 39. Worker Composition

The embedding worker is constructed from:

```text
SqliteJobRepository
SqliteMemoryRepository
SqliteEmbeddingRepository
OllamaBackend
SystemClock
InferenceModel
```

The embedding model is obtained from the centralized Ollama configuration.

The worker is started by the daemon and runs independently from client requests.

---

# 40. Daemon Runtime

`Daemon` owns:

```text
DaemonConfig
StoreService
AskService
EmbeddingWorker thread
```

`DaemonConfig` contains:

```text
database_path
socket_path
```

`Daemon::build`:

```text
build store service
load Ollama configuration
construct Ollama backend
build ask service
spawn embedding worker
construct daemon
```

`Daemon::run` creates the IPC server over the composed services and serves requests.

The daemon is therefore the owner of long-lived infrastructure and application state.

---

# 41. CLI Command Model

`cli::command::Cli` defines the user-facing command structure.

Primary question form:

```text
recall "question"
```

Optional flag:

```text
--no-ai
```

Operational subcommands currently include:

```text
recall store <input>
recall daemon
recall status
```

`StoreArgs` contains one input string.

`DaemonArgs` contains database and socket paths.

`StatusArgs` contains a socket path.

---

# 42. CLI Input Conversion

`cli::input::store_input` implements the initial store ambiguity rule:

```text
argument resolves to existing path
        ↓
StoreInput::File

otherwise
        ↓
StoreInput::Text
```

The CLI does not read the file itself.

The application store use case performs file acquisition so the same behavior can later be used by another interface.

---

# 43. CLI Orchestration

`cli::run` selects the runtime path.

Question:

```text
question
    ↓
IpcClient
    ↓
Request::Ask
```

with:

```text
use_ai = !no_ai
```

Store:

```text
store input
    ↓
StoreInput
    ↓
Request::Store
```

Daemon:

```text
DaemonConfig
    ↓
Daemon::build
    ↓
Daemon::run
```

Status:

```text
Request::Status
```

The CLI does not directly construct application services for ordinary client operations.

---

# 44. CLI Rendering

`cli::render::render` maps wire responses to terminal output.

Store:

```text
Stored memory <id>
```

Answer:

```text
answer text

Sources:
- <id>
- <id>
```

Retrieved:

```text
1. [<id>]
memory content
```

Remote errors become CLI failures.

Rendering contains no retrieval or inference decisions.

---

# 45. Main Entry Point

`main.rs` is the process entry point.

Its role is limited to:

```text
parse CLI
    ↓
cli::run
    ↓
convert failure to process exit
```

It does not own database or model behavior.

---

# 46. Data Lifecycle

A newly stored memory has two persistence layers.

Immediately after store:

```text
memories
    ready

jobs
    pending

embeddings
    absent
```

After successful background derivation:

```text
memories
    ready

jobs
    completed

embeddings
    ready for configured model
```

If embedding fails:

```text
memories
    ready

jobs
    failed + error

embeddings
    absent or previous derived version
```

Canonical memory remains intact.

---

# 47. Delete Semantics at the Database Layer

Canonical memory is represented by the `memories` table.

Derived records reference it with foreign keys.

The schema uses:

```text
ON DELETE CASCADE
```

for jobs and embeddings.

Therefore deletion of canonical memory removes its directly associated durable derived records.

The application-level public forget operation is not yet part of the CLI command set.

---

# 48. Search Consistency

Lexical search is maintained synchronously through SQLite triggers.

Therefore:

```text
memory inserted
    ↓
FTS entry inserted
```

and:

```text
memory deleted
    ↓
FTS entry removed
```

Semantic search is intentionally asynchronous:

```text
memory inserted
    ↓
embedding job pending
    ↓
worker
    ↓
embedding
```

This produces different readiness states:

```text
canonical memory: immediate
lexical search: immediate
semantic search: eventual
```

---

# 49. Model Changes

Embedding persistence includes the model name.

Semantic search filters embeddings by the currently configured model.

Therefore changing the embedding model does not silently treat an old vector as though it were produced by the new model.

The existing canonical memory remains unchanged.

The worker can generate derived embeddings for the selected model.

---

# 50. Error Boundaries

Errors remain local to the subsystem that understands them.

Examples:

```text
Memory validation
    ↓
MemoryValidationError

SQLite persistence
    ↓
MemoryRepositoryError /
StorePersistenceError

FTS retrieval
    ↓
SearchError

Ollama transport/API
    ↓
InferenceError

Ollama configuration
    ↓
OllamaConfigError

Job lifecycle
    ↓
InvalidJobTransition

Worker execution
    ↓
WorkerError
```

The IPC layer converts application/runtime failures into coarse `RemoteErrorKind` categories.

The CLI turns those into user-facing failures.

---

# 51. Test Structure and Important Contracts

The implementation contains focused tests around the principal contracts.

## Domain tests

Current coverage includes:

```text
memory rejects invalid content
memory rejects invalid timestamp ordering

search query rejects empty text
search query rejects zero limit
scores reject NaN

source variants preserve their semantics

job follows lifecycle
completed job cannot run again
failed job can be requeued
```

## Store tests

The store tests verify:

```text
direct text is persisted
empty content is rejected before persistence
content over the limit is rejected before persistence
a derivation job is created with successful persistence
```

## Database tests

Canonical persistence tests verify:

```text
direct memory round trip
file-source round trip
canonical deletion
```

FTS tests verify:

```text
matching memory is returned
canonical deletion removes searchability
```

Semantic tests verify:

```text
similar embeddings are ranked first
```

## Ask tests

The ask tests explicitly protect:

```text
retrieved memory reaches inference
provenance comes from retrieved memory IDs
--no-ai does not invoke inference
zero retrieval limit is rejected
```

## Prompt tests

Prompt construction tests verify that:

```text
memory is represented as supplied data
question is preserved
```

The tests therefore protect information flow rather than private implementation details.

---

# 52. Current Runtime Configuration Surface

The current important configuration inputs are:

```text
DaemonConfig
├── database_path
└── socket_path

OllamaConfig
├── base_url
├── generation_model
├── embedding_model
└── timeout
```

Application retrieval configuration currently includes the retrieval limit.

The central runtime composition selects the current defaults rather than spreading configuration literals across application code.

---

# 53. Current Concrete Dependency Graph

The running daemon is effectively composed as:

```text
Daemon
│
├── StoreMemory
│   ├── SqliteStorePersistence
│   │   └── SQLite
│   ├── SystemClock
│   └── MemoryLimits
│
├── AskRecall
│   ├── SqliteMemorySearcher
│   │   └── SQLite FTS5
│   ├── SqliteSemanticMemorySearcher
│   │   ├── SQLite embeddings
│   │   └── OllamaBackend
│   └── OllamaBackend
│
├── EmbeddingWorker
│   ├── SqliteJobRepository
│   ├── SqliteMemoryRepository
│   ├── SqliteEmbeddingRepository
│   ├── OllamaBackend
│   ├── SystemClock
│   └── InferenceModel
│
└── IpcServer
    ├── StoreMemory
    └── AskRecall
```

The same Ollama backend instance is shared by the ask service and embedding worker.

---

# 54. Actual User-Level Flows

## Store

```text
recall store "I use SQLite WAL mode."
```

becomes:

```text
CLI
  ↓
StoreInput::Text
  ↓
IPC Request::Store
  ↓
StoreMemory::execute
  ↓
Memory::new
  ↓
Job::new(GenerateEmbedding)
  ↓
SqliteStorePersistence::persist
  ↓
SQLite transaction
  ├── memories
  └── jobs
  ↓
StoreResponse
  ↓
CLI
```

## File store

```text
recall store ./idea.txt
```

becomes:

```text
CLI path detection
  ↓
StoreInput::File
  ↓
StoreMemory
  ↓
read_to_string
  ↓
MemorySource::File
  ↓
atomic SQLite persistence
```

## Retrieval-only ask

```text
recall --no-ai "what did I say about SQLite?"
```

becomes:

```text
CLI
  ↓
AskRequest(use_ai = false)
  ↓
MemorySearcher
  ↓
SQLite FTS5
  ↓
SearchResult[]
  ↓
RetrievedResponse
  ↓
CLI
```

## AI-backed ask

```text
recall "what did I say about SQLite?"
```

becomes:

```text
CLI
  ↓
AskRequest(use_ai = true)
  ↓
lexical retrieval
  +
semantic retrieval
  ↓
RRF merge
  ↓
RetrievedMemory[]
  ↓
GenerationRequest
  ↓
PromptBuilder
  ↓
Ollama /api/generate
  ↓
GenerationResponse
  ↓
Answer {
    text,
    sources
}
  ↓
CLI
```

## Background embedding

```text
pending Job
  ↓
EmbeddingWorker
  ↓
load Memory
  ↓
EmbeddingRequest
  ↓
Ollama /api/embed
  ↓
EmbeddingResponse
  ↓
SqliteEmbeddingRepository
  ↓
Job::complete
```

---

# 55. Boundaries That Must Remain Visible in Code

The current implementation depends on several concrete boundaries.

## Canonical vs derived

```text
Memory
    ≠
Embedding
    ≠
SearchScore
    ≠
GenerationResponse
```

## Retrieval vs generation

```text
MemorySearcher
    ↓
RetrievedMemory
    ↓
InferenceBackend
```

## Application vs infrastructure

```text
application::ports
    ↓
infrastructure implementations
```

## Transport vs application

```text
IPC protocol
    ↓
application request
```

## Configuration vs adapter

```text
OllamaConfig
    ↓
OllamaBackend
```

## Worker vs persistence

```text
EmbeddingWorker
    orchestrates

Repositories
    persist

OllamaBackend
    performs inference
```

These boundaries are represented directly by Rust traits, structs, and module ownership rather than only by convention.

---

# 56. Current Non-Core/Incomplete Operational Surface

The primary store and ask flows are implemented.

The current `status` wire command exists structurally, but the server still treats it as unsupported rather than implementing a complete status application use case.

The command exists so the protocol shape does not need to be invented later, but it is not currently a completed operational feature.

Likewise, the current implementation does not add future commands such as:

```text
search
show
forget
jobs
doctor
logs
```

unless they are explicitly implemented.

---

# 57. Extension Points

The implementation has explicit replacement points for future infrastructure.

## Inference

Current:

```text
OllamaBackend
```

Can be replaced by another `InferenceBackend` implementation.

## Lexical search

Current:

```text
SqliteMemorySearcher
```

implements `MemorySearcher`.

## Semantic search

Current:

```text
SqliteSemanticMemorySearcher
```

implements `SemanticMemorySearcher`.

A future vector index can replace it without changing `AskRecall`.

## Persistence

Current:

```text
SqliteMemoryRepository
SqliteStorePersistence
SqliteJobRepository
SqliteEmbeddingRepository
```

implement application-owned persistence ports.

## Clock

Current:

```text
SystemClock
```

implements `Clock`.

Tests can use deterministic implementations.

---

# 58. Implementation-Level Invariants

The current code directly enforces these implementation contracts:

1. `Memory` is the canonical persisted representation.
2. Search results contain a memory plus a search-specific score.
3. Generation context contains explicitly selected memories.
4. Generation provenance is derived from generation context.
5. The inference backend does not retrieve memories.
6. Store persistence commits canonical memory and its first embedding job atomically.
7. Embedding is asynchronous relative to store.
8. FTS is derived from canonical memory content.
9. Semantic embeddings are derived from canonical memory content.
10. Failed embedding work does not delete canonical memory.
11. `--no-ai` does not invoke inference.
12. Ollama-specific configuration is centralized.
13. Ollama-specific HTTP structures remain inside the adapter.
14. Prompt construction is separate from Ollama transport.
15. The CLI communicates with application behavior through IPC rather than SQLite.
16. Domain state transitions are validated by domain methods.
17. Infrastructure reconstructs domain values through domain constructors.
18. Model identity is stored alongside derived embeddings.
19. Derived records are tied to canonical memory IDs.
20. Retrieval can function without successful generation.

---

# 59. The Implementation's Core Contract

The entire current implementation can be reduced to two primary contracts.

### Remember

```text
StoreInput
    ↓
canonical Memory
    ↓
durable SQLite state
    ↓
durable derivation Job
    ↓
eventual embedding
```

### Recall

```text
AskRequest
    ↓
lexical + optional semantic retrieval
    ↓
selected Memory values
    ↓
optional generation
    ↓
Answer + application-owned provenance
```

Everything else in the implementation exists to make those two contracts durable, replaceable, testable, and executable through the daemon.
