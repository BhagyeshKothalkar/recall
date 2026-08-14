# Recall — Concrete System Architecture & Implementation Specification

## 1. Architectural Decision

Recall will be implemented as a **single Rust binary containing two runtime roles**:

```
recall <command>
    |
    +-- CLI client
    |
    +-- Unix-domain-socket IPC
    |
    v
Recall daemon
    |
    +-- Application layer
    |
    +-- Domain layer
    |
    +-- Infrastructure
            |
            +-- SQLite
            +-- Ollama
            +-- filesystem
            +-- background workers

```

The same binary provides both roles.

The daemon is started with:

```
recall daemon

```

Normal user operations are:

```
recall store "some memory"
recall store ./some-file.txt
recall "some question"

```

The CLI does not directly access SQLite or Ollama.

The CLI translates user input into an application request, sends that request to the daemon, and renders the response.

This is a deliberate architectural boundary.

---

# 2. Why the Daemon Owns the Application

The daemon owns:

- the database connection
- memory persistence
- indexing
- background jobs
- inference clients
- retrieval
- application state
- lifecycle of long-running resources

The CLI owns:

- argument parsing
- input acquisition
- IPC connection
- request serialization
- response rendering
- exit status

Therefore:

```
CLI
  │
  │ Request
  ▼
Daemon
  │
  ▼
Application
  │
  ├── Domain
  ├── Repository
  ├── Search
  ├── Jobs
  └── Inference

```

The CLI must not duplicate application logic.

There should be no:

```
if command == "store" {
    open sqlite...
}

```

inside CLI code.

Instead:

```
CLI
    ↓
StoreRequest
    ↓
IPC
    ↓
StoreMemory use case

```

---

# 3. User Interface

The intentionally minimal public interface is:

```
recall store "text"
recall store path/to/file
recall "question"

```

Additional operational commands:

```
recall daemon
recall status
recall help

```

Potential future commands:

```
recall search "query"
recall show <id>
recall forget <id>
recall jobs

```

Do not implement future commands until they have a concrete use.

---

# 4. `store` Command

The user invokes:

```
recall store "I use SQLite WAL mode."

```

The argument is interpreted as follows:

1. If it resolves to an existing file path, read the file.
2. Otherwise treat the argument literally as text.

Therefore:

```
recall store "hello world"

```

stores:

```
hello world

```

while:

```
recall store ./idea.txt

```

reads `idea.txt` and stores its contents.

For the initial implementation, file input is deliberately treated as **text input**.

Do not introduce document parsing, MIME detection, PDF extraction, Markdown semantics, OCR, etc.

Those are future ingestion concerns.

---

# 5. `ask` Command

There is deliberately no `ask` subcommand in the primary UX.

The question itself is the root positional argument:

```
recall "what was that Rust project idea?"

```

This is intentional.

Recall is supposed to feel like a memory tool rather than a conventional command-line application.

The CLI should eventually support useful optional flags without changing this fundamental UX.

Potential future examples:

```
recall --verbose "what was that Rust project idea?"
recall --no-ai "where did I mention SQLite?"

```

Do not add flags until they have a real use.

---

# 6. Command Classification

At the CLI boundary there are three conceptual operations.

```
enum Command {
    Store(StoreArgs),
    Ask(AskArgs),
    Daemon,
    Status,
}

```

The exact `clap` representation may differ because the root positional question needs to coexist with subcommands.

The parser must normalize raw CLI arguments into a small internal command representation.

Everything after that point should operate on typed application requests.

---

# 7. Main System Tasks

Recall has five fundamental product tasks.

```
1. Capture
2. Persist
3. Derive
4. Retrieve
5. Answer

```

They form the core pipeline:

```
User input
    │
    ▼
Capture
    │
    ▼
Persist canonical memory
    │
    ▼
Derive indexes / embeddings
    │
    ▼
Retrieve relevant memories
    │
    ▼
Generate answer

```

The first three happen when storing.

The last two happen when asking.

---

# 8. Task 1 — Capture

Capture converts user-facing input into canonical domain input.

Input:

```
StoreInput

```

which can represent:

```
Text("...")
File(PathBuf)

```

The CLI is responsible for acquiring the initial input.

The application layer is responsible for turning it into a canonical memory.

---

# 9. Capture Subtasks

Capture performs:

```
1. Determine input kind.
2. If file:
       read file.
3. Produce text.
4. Validate text.
5. Construct Memory.

```

For the initial version, there is no distinction in the stored content between:

```
store "hello"

```

and:

```
store ./hello.txt

```

Both eventually become canonical text.

The source metadata should still distinguish them.

---

# 10. Capture State

The capture operation is synchronous.

Conceptual state:

```
enum CaptureState {
    Received,
    Reading,
    Validated,
    Failed,
}

```

There is no reason to persist this state.

It is transient execution state.

The result of successful capture is:

```
CapturedMemory

```

containing:

```
content
source

```

---

# 11. Capture Information

The capture task receives:

```
StoreInput

```

and produces:

```
CapturedMemory

```

Conceptually:

```
StoreInput
    │
    ▼
Capture
    │
    ▼
CapturedMemory

```

The capture task must not know:

- SQLite
- embeddings
- Ollama
- IPC
- search
- background workers

---

# 12. `StoreInput`

Use an enum rather than ambiguous strings:

```
enum StoreInput {
    Text(String),
    File(PathBuf),
}

```

This makes the distinction explicit.

The CLI decides which variant to create.

The application decides how to consume it.

---

# 13. `CapturedMemory`

Do not immediately use the final persisted `Memory` type for raw input.

Use an intermediate domain value:

```
struct CapturedMemory {
    content: String,
    source: MemorySource,
}

```

This makes the lifecycle explicit:

```
raw CLI input
    ↓
CapturedMemory
    ↓
Memory

```

---

# 14. `MemorySource`

Use an enum.

Initial variants:

```
enum MemorySource {
    DirectInput,
    File { path: PathBuf },
}

```

This is preferable to:

```
source: String

```

because source semantics will eventually matter.

Future variants can include:

```
Url
ImportedFile
Terminal
Integration

```

without changing the fundamental memory model.

---

# 15. Task 2 — Persist

Persistence turns:

```
CapturedMemory

```

into:

```
Memory

```

and stores it as canonical state.

The persistence boundary is:

```
MemoryRepository

```

The application layer owns the operation.

The SQLite adapter implements the repository.

---

# 16. Canonical `Memory`

The canonical memory should contain only authoritative information.

Initial conceptual structure:

```
struct Memory {
    id: MemoryId,
    content: String,
    source: MemorySource,
    created_at: Timestamp,
    updated_at: Timestamp,
}

```

Do not put:

```
embedding
summary
search score
LLM response

```

inside `Memory`.

Those are derived concepts.

---

# 17. `MemoryId`

Use a dedicated domain identifier:

```
struct MemoryId(Uuid);

```

The ID is generated when the canonical memory is constructed.

The database stores it.

Derived data refers back to it.

---

# 18. Persistence Contract

The repository should expose domain operations.

Conceptually:

```
trait MemoryRepository {
    fn create(&mut self, memory: &Memory) -> Result<(), RepositoryError>;

    fn get(&mut self, id: MemoryId) -> Result<Option<Memory>, RepositoryError>;

    fn delete(&mut self, id: MemoryId) -> Result<(), RepositoryError>;
}

```

The exact signatures may use async or connection-scoped abstractions as implementation dictates.

The semantic contract is what matters.

The repository must never expose SQL concepts to the application layer.

---

# 19. Persistence Ownership

Ownership is:

```
Application
    owns Memory value
        ↓
MemoryRepository
    borrows / persists Memory
        ↓
SQLite adapter
    owns database representation

```

The repository does not become the owner of application state.

The database owns persisted state.

The Rust process owns the live representation.

---

# 20. Persistence Invariant

The most important store invariant is:

> **Canonical memory persistence must not depend on successful AI processing.**

If Ollama is:

- unavailable
- slow
- misconfigured
- broken
- removed

the memory must still be stored.

Therefore:

```
store
  ↓
persist memory
  ↓
success returned to user

```

is independent from:

```
embedding
summarization
classification

```

---

# 21. Task 3 — Derive

After persistence, Recall may produce derived information.

Initial derived task:

```
CreateEmbedding

```

Later:

```
ExtractMetadata
SummarizeMemory
ClassifyMemory

```

The first implementation should only require what retrieval actually needs.

---

# 22. Derived State

Derived information is associated with a canonical memory.

Conceptually:

```
struct MemoryEmbedding {
    memory_id: MemoryId,
    model: EmbeddingModel,
    vector: Vec<f32>,
    created_at: Timestamp,
}

```

This is deliberately not part of `Memory`.

---

# 23. Derived State Lifecycle

An individual memory can have:

```
Canonical:
    Stored

Embedding:
    Missing
    Pending
    Processing
    Ready
    Failed

```

Conceptually:

```
enum DerivationState {
    Missing,
    Pending,
    Processing,
    Ready,
    Failed,
}

```

The exact persisted representation may instead use a job table and derived-record existence.

Prefer not to duplicate state unnecessarily.

---

# 24. Job Model

Background work should be represented explicitly.

Use:

```
struct Job {
    id: JobId,
    kind: JobKind,
    state: JobState,
    created_at: Timestamp,
    attempts: u32,
    last_error: Option<String>,
}

```

Initial:

```
enum JobKind {
    GenerateEmbedding { memory_id: MemoryId },
}

```

Initial job states:

```
enum JobState {
    Pending,
    Running,
    Completed,
    Failed,
}

```

Retry semantics can be added without changing the fundamental model.

---

# 25. Why Jobs Are Persisted

A persisted job allows:

```
memory stored
    ↓
embedding job pending
    ↓
daemon crashes
    ↓
daemon restarts
    ↓
job remains discoverable

```

This prevents inference availability from becoming memory availability.

---

# 26. Store Pipeline

The complete store operation is:

```
CLI
 │
 │ recall store "..."
 ▼
StoreInput
 │
 ▼
Capture
 │
 ▼
CapturedMemory
 │
 ▼
MemoryFactory
 │
 ▼
Memory
 │
 ├───────────────┐
 ▼               ▼
MemoryRepository JobRepository
 │               │
 ▼               ▼
SQLite           Pending embedding job
 │
 ▼
StoreResponse
 │
 ▼
CLI

```

The user does not wait for the embedding.

---

# 27. Store Response

The application should return enough information to make the operation understandable.

Conceptually:

```
struct StoreResponse {
    memory_id: MemoryId,
    content_length: usize,
    derivation: DerivationStatus,
}

```

Initial output could be:

```
Stored memory 01J...

```

The CLI should not dump internal job/database details by default.

---

# 28. Task 4 — Retrieve

The question pipeline begins with:

```
recall "question"

```

The CLI sends:

```
AskRequest {
    question: String,
}

```

to the daemon.

The daemon passes it to the retrieval application service.

---

# 29. Retrieval Subtasks

Retrieval consists of:

```
1. Validate question.
2. Search lexical index.
3. Search semantic index if available.
4. Combine candidates.
5. Rank candidates.
6. Select context.
7. Return evidence.

```

The retrieval system does not generate language.

It finds memory.

---

# 30. Retrieval Must Work Without AI

The retrieval layer must remain useful if:

```
Ollama unavailable
embedding model unavailable
GPU unavailable

```

Therefore lexical retrieval must be a complete capability.

The minimum retrieval path is:

```
question
    ↓
SQLite FTS
    ↓
ranked memories

```

Semantic retrieval is additive.

---

# 31. Search Abstraction

Use a domain/application abstraction such as:

```
trait MemorySearcher {
    fn search(
        &self,
        query: &SearchQuery,
    ) -> Result<Vec<SearchResult>, SearchError>;
}

```

The searcher is responsible for retrieval.

It does not know that the results will eventually be passed to an LLM.

---

# 32. Search Query

Use:

```
struct SearchQuery {
    text: String,
    limit: usize,
}

```

Do not pass raw CLI strings throughout the system.

The application creates a validated query.

---

# 33. Search Result

Use a separate result type:

```
struct SearchResult {
    memory: Memory,
    score: SearchScore,
}

```

Do not put search score inside `Memory`.

A memory exists independently of any search.

---

# 34. Search Score

Use a named type if the scoring model becomes meaningful:

```
struct SearchScore(f32);

```

Initially, a simple floating-point score is sufficient.

The internal scoring mechanism must remain an implementation detail.

---

# 35. Retrieval Pipeline

Initial implementation:

```
SearchQuery
    │
    ▼
LexicalSearcher
    │
    ▼
SearchResult[]

```

Next stage:

```
SearchQuery
    │
    ├── lexical search
    │
    └── semantic search
            │
            ▼
       candidate sets
            │
            ▼
       candidate merger
            │
            ▼
          ranking
            │
            ▼
       SearchResult[]

```

Do not implement hybrid retrieval until there is a working lexical path.

---

# 36. Task 5 — Answer

Answering is a separate task from retrieval.

Input:

```
AskContext {
    question: String,
    memories: Vec<RetrievedMemory>,
}

```

Output:

```
Answer {
    text: String,
    sources: Vec<MemoryId>,
}

```

The answer generator is an inference concern.

---

# 37. Retrieved Memory

Do not pass generic `SearchResult` objects directly into the inference backend.

Create an explicit context representation:

```
struct RetrievedMemory {
    memory: Memory,
    relevance: Relevance,
}

```

This is the information contract between retrieval and generation.

---

# 38. `AskContext`

Use:

```
struct AskContext {
    question: String,
    memories: Vec<RetrievedMemory>,
}

```

This structure is deliberately owned.

The inference backend receives the complete context needed to generate an answer.

It does not query the database.

---

# 39. Critical Boundary

The inference backend must never do this:

```
LLM backend
    ↓
query SQLite

```

Instead:

```
Application
    ↓
retrieve memories
    ↓
construct AskContext
    ↓
InferenceBackend

```

This guarantees that:

> **The LLM does not own the memory.**

---

# 40. Inference Contract

The core abstraction is:

```
trait InferenceBackend {
    fn generate(
        &self,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse, InferenceError>;

    fn embed(
        &self,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse, InferenceError>;
}

```

Whether these methods are synchronous or asynchronous depends on the runtime design.

The semantic boundary is fixed.

---

# 41. Generation Request

Use:

```
struct GenerationRequest {
    question: String,
    context: Vec<RetrievedMemory>,
}

```

The backend transforms this into whatever representation it needs.

For Ollama:

```
GenerationRequest
    ↓
Ollama-specific prompt/messages
    ↓
HTTP
    ↓
Ollama

```

For future llama.cpp:

```
GenerationRequest
    ↓
llama.cpp-specific request

```

The application does not change.

---

# 42. Generation Response

Use:

```
struct GenerationResponse {
    text: String,
}

```

The application combines it with known source IDs to construct:

```
struct Answer {
    text: String,
    sources: Vec<MemoryId>,
}

```

The backend does not get to decide what memories were authoritative.

The retrieval system already knows that.

---

# 43. Source Attribution

Source IDs must originate from retrieval.

Never ask the LLM to invent source IDs.

The answer pipeline is:

```
retrieval
    ↓
[Memory A, Memory C]
    ↓
AskContext
    ↓
LLM
    ↓
text
    ↓
Answer {
    text,
    sources: [A, C]
}

```

The LLM generates language.

The application determines provenance.

---

# 44. Answer Pipeline

Complete question flow:

```
CLI
 │
 │ recall "question"
 ▼
AskRequest
 │
 ▼
Application
 │
 ▼
SearchQuery
 │
 ▼
Retriever
 │
 ├── lexical index
 │
 └── semantic index
 │
 ▼
Ranked memories
 │
 ▼
AskContext
 │
 ▼
InferenceBackend
 │
 ▼
GenerationResponse
 │
 ▼
Answer
 │
 ▼
IPC response
 │
 ▼
CLI rendering

```

This is the central Recall pipeline.

---

# 45. Full Store-to-Recall Relationship

The complete lifecycle is:

```
                    STORE

User
 │
 │ recall store "..."
 ▼
Capture
 │
 ▼
Memory
 │
 ▼
SQLite canonical store
 │
 └───────────────┐
                 │
                 ▼
            Derivation Job
                 │
                 ▼
             Embedding
                 │
                 ▼
          Semantic Index


                    RECALL

User
 │
 │ recall "question"
 ▼
Retrieval
 │
 ├── lexical index
 │
 └── semantic index
 │
 ▼
Relevant Memories
 │
 ▼
Prompt Context
 │
 ▼
Ollama
 │
 ▼
Answer
 │
 ▼
User

```

---

# 46. Application Services

The application layer should contain explicit use cases.

Initial services:

```
StoreMemory
AskRecall
GetStatus

```

Potential later services:

```
DeleteMemory
SearchMemory
ReindexMemory
ProcessJobs

```

Each use case should have one responsibility.

---

# 47. `StoreMemory`

Conceptual API:

```
struct StoreMemory {
    repository: Arc<dyn MemoryRepository>,
    jobs: Arc<dyn JobRepository>,
    clock: Arc<dyn Clock>,
}

```

Its operation:

```
fn execute(
    &self,
    input: StoreInput,
) -> Result<StoreResponse, StoreError>

```

The exact ownership mechanism may use concrete types rather than `Arc<dyn ...>` where appropriate.

The important contract is:

```
StoreInput
    ↓
CapturedMemory
    ↓
Memory
    ↓
repository.create()
    ↓
job creation
    ↓
StoreResponse

```

---

# 48. `AskRecall`

Conceptual API:

```
struct AskRecall {
    searcher: Arc<dyn MemorySearcher>,
    inference: Arc<dyn InferenceBackend>,
}

```

Operation:

```
fn execute(
    &self,
    request: AskRequest,
) -> Result<Answer, AskError>

```

Pipeline:

```
AskRequest
    ↓
SearchQuery
    ↓
MemorySearcher
    ↓
RetrievedMemory[]
    ↓
GenerationRequest
    ↓
InferenceBackend
    ↓
Answer

```

---

# 49. Application Error Boundaries

Use specific errors at each layer.

```
CaptureError
StoreError
SearchError
AskError
RepositoryError
InferenceError
JobError
IpcError

```

Do not collapse everything immediately into:

```
RecallError

```

Low-level errors should be translated at meaningful boundaries.

For example:

```
Ollama HTTP timeout
    ↓
OllamaError
    ↓
InferenceError
    ↓
AskError
    ↓
user-facing error

```

---

# 50. Daemon Runtime

The daemon owns application dependencies.

Conceptually:

```
struct App {
    memory_repository: ...,
    job_repository: ...,
    searcher: ...,
    inference: ...,
}

```

Then:

```
daemon startup
    ↓
load configuration
    ↓
open database
    ↓
construct repositories
    ↓
construct searcher
    ↓
construct inference backend
    ↓
construct application services
    ↓
start job workers
    ↓
start IPC listener
    ↓
serve requests

```

The runtime performs composition.

It should not contain business logic.

---

# 51. Composition Root

Dependency construction belongs in one place.

The composition root should answer:

```
Which database implementation?
Which inference backend?
Which search implementation?
Which job runner?
Which IPC transport?

```

The rest of the application receives dependencies.

This makes replacing:

```
OllamaBackend

```

with:

```
LlamaCppBackend

```

a composition change rather than an application rewrite.

---

# 52. Ownership Model

The daemon owns long-lived infrastructure.

```
DaemonRuntime
 ├── Database
 ├── JobRunner
 ├── InferenceBackend
 ├── Searcher
 └── IPC listener

```

Application services borrow or share these dependencies according to their concurrency requirements.

Do not let individual requests open their own independent database/inference infrastructure unless there is a concrete reason.

---

# 53. State Ownership

Canonical state:

```
SQLite

```

Derived persistent state:

```
SQLite / derived index storage

```

Transient application state:

```
Daemon process

```

Job state:

```
SQLite

```

Inference process state:

```
Ollama / future backend

```

CLI state:

```
single request lifecycle

```

This ownership distinction should remain explicit.

---

# 54. IPC Protocol

Use a Unix domain socket for daemon communication on Linux.

The initial protocol should be a simple request/response protocol.

Conceptually:

```
enum Request {
    Store(StoreRequest),
    Ask(AskRequest),
    Status(StatusRequest),
}

```

and:

```
enum Response {
    Store(StoreResponse),
    Answer(Answer),
    Status(StatusResponse),
    Error(RemoteError),
}

```

Serialize the protocol using a structured format.

JSON is acceptable initially because debuggability matters more than protocol efficiency for a single-user daemon.

The protocol is internal, not a public network API.

---

# 55. IPC Ownership

The IPC layer should only translate:

```
wire representation
    ↔
application request/response

```

It should not:

- access SQLite
- perform searches
- call Ollama
- construct prompts
- implement store logic

---

# 56. IPC Request Lifecycle

For:

```
recall store "hello"

```

the CLI does:

```
parse
  ↓
StoreRequest
  ↓
serialize
  ↓
Unix socket

```

The daemon does:

```
deserialize
  ↓
StoreMemory.execute()
  ↓
StoreResponse
  ↓
serialize

```

The CLI does:

```
deserialize
  ↓
render

```

---

# 57. CLI Ownership

CLI code should know about:

- command syntax
- input paths
- terminal output
- exit codes
- IPC

CLI code should not know about:

- SQLite schemas
- SQL
- embeddings
- prompt construction
- search ranking
- Ollama endpoints

This keeps the interface replaceable.

---

# 58. Input Path Detection

For:

```
recall store <argument>

```

the CLI should pass the argument to the application as a `StoreInput`.

The application/input layer determines whether the path exists.

Do not make the CLI responsible for filesystem semantics beyond argument parsing.

This keeps the behavior available to future interfaces.

---

# 59. Ambiguous File/Text Input

If:

```
recall store foo

```

and a file named `foo` exists:

```
treat as file

```

Otherwise:

```
treat as text

```

This is the initial UX decision.

A future explicit flag can resolve ambiguity if needed:

```
--text
--file

```

Do not add it now.

---

# 60. Empty Input

Reject empty or whitespace-only memories.

The invariant should be established before persistence.

For example:

```
recall store ""

```

should return a user-facing validation error.

No database row should be created.

Likewise an empty file should be rejected.

---

# 61. Large Input

Do not impose arbitrary tiny limits.

However, the application must eventually define a maximum acceptable memory size.

The initial implementation should have a named configuration value:

```
struct MemoryLimits {
    max_content_bytes: usize,
}

```

Validation occurs before persistence.

The limit should be easy to change locally.

---

# 62. Timestamp Ownership

Do not let SQLite silently generate business timestamps if the domain requires them.

The application should obtain a timestamp from a `Clock` abstraction if deterministic testing requires it.

Conceptually:

```
trait Clock {
    fn now(&self) -> Timestamp;
}

```

Production:

```
SystemClock

```

Tests:

```
FakeClock

```

This is a justified abstraction because time directly affects deterministic testing.

---

# 63. Database Schema

Initial logical tables:

```
memories
jobs
embeddings

```

Potential schema:

```
memories
--------
id
content
source_type
source_value
created_at
updated_at

```

```
jobs
----
id
kind
payload
state
attempts
created_at
updated_at
last_error

```

```
embeddings
----------
memory_id
model
vector
created_at

```

The exact SQL representation can differ.

The conceptual separation must remain.

---

# 64. FTS

Lexical search should use SQLite full-text search.

The FTS representation is derived from `memories`.

Therefore:

```
memories
   │
   ▼
FTS index

```

The FTS index must not become the canonical memory.

If the FTS index is destroyed, it can be rebuilt.

---

# 65. Embeddings

Embedding records similarly derive from:

```
Memory.content

```

If the embedding model changes:

```
old embeddings
    ↓
discard / invalidate
    ↓
recompute

```

The memory itself remains untouched.

---

# 66. Job Creation Transaction

When storing a memory, the application should ensure the relationship between canonical storage and initial derivation work is consistent.

Preferred initial transaction:

```
BEGIN
    insert memory
    insert embedding job
COMMIT

```

If the transaction fails:

```
memory does not exist
job does not exist

```

This avoids a memory that is persistently invisible to the derivation system.

The actual embedding work happens later.

---

# 67. Job Worker

The worker performs:

```
fetch pending job
    ↓
mark running
    ↓
load memory
    ↓
call embedding backend
    ↓
store embedding
    ↓
mark completed

```

On failure:

```
mark failed
record error

```

The worker must never delete the memory because embedding failed.

---

# 68. Worker Ownership

The job worker owns job execution state.

The repository owns persistence.

The inference backend owns communication with the model runtime.

Therefore:

```
Worker
  orchestrates

Repository
  persists

InferenceBackend
  infers

```

No component should absorb the responsibilities of all three.

---

# 69. Search Data Freshness

A newly stored memory may initially have:

```
canonical memory: ready
FTS: ready
embedding: pending

```

This is acceptable.

The user should be able to search immediately through lexical search.

Semantic availability can lag behind.

This is an explicit consistency model:

> **Canonical memory is immediately durable; derived intelligence is eventually consistent.**

---

# 70. Asking During Derivation

If a memory has not yet received an embedding:

```
lexical retrieval

```

can still find it.

Therefore a newly stored memory can participate in Recall immediately.

This is important for usability.

---

# 71. Inference Failure During `ask`

If retrieval succeeds but inference fails:

```
recall "question"

```

should return a clear inference error.

It should not pretend that an answer exists.

Potential future behavior:

```
--no-ai

```

could expose retrieval-only results.

Do not implement that flag yet.

The underlying architecture should make it easy later.

---

# 72. Retrieval Failure During `ask`

If the database/search layer fails:

```
AskError::Retrieval(...)

```

should be returned.

Do not invoke the LLM with no context unless that behavior is explicitly requested.

Recall is a memory system.

An answer without retrieved memory defeats the central trust model.

---

# 73. Prompt Construction

Prompt construction should be a separate pure component.

For example:

```
struct PromptBuilder;

```

with:

```
fn build(
    question: &str,
    memories: &[RetrievedMemory],
) -> GenerationRequest

```

or equivalent.

It should:

- preserve source identity
- clearly delimit memories
- distinguish memory from instructions
- tell the model to answer from supplied context
- avoid claiming unsupported facts

The prompt builder must not access the database.

---

# 74. Prompt Injection Boundary

Stored memory is data, not trusted instructions.

The prompt should clearly distinguish:

```
SYSTEM INSTRUCTIONS

```

from:

```
RETRIEVED MEMORY

```

A memory containing:

```
"ignore previous instructions..."

```

must remain content.

The application must not treat retrieved memory as executable instruction.

---

# 75. Answer Trust Contract

The generation prompt should instruct the model approximately:

```
Use the supplied memories to answer the user's question.
Do not invent memories.
If the supplied memories do not contain enough information,
say so.

```

The exact wording belongs in the prompt builder.

The architectural requirement is more important than the exact prompt.

---

# 76. LLM Backend Adapter

The Ollama adapter should have one responsibility:

```
GenerationRequest / EmbeddingRequest
        ↓
Ollama API
        ↓
GenerationResponse / EmbeddingResponse

```

It should contain:

- HTTP client
- endpoint construction
- request serialization
- response parsing
- Ollama-specific error mapping
- timeout configuration

Nothing else.

---

# 77. Future llama.cpp Adapter

The future architecture should permit:

```
inference/
    ollama.rs
    llama_cpp.rs

```

with both implementing the same backend contract.

The application should not contain:

```
if backend == Ollama

```

throughout its logic.

Backend selection happens at composition time.

---

# 78. Future Candle Adapter

Likewise:

```
inference/
    candle.rs

```

may eventually implement:

```
InferenceBackend

```

if the experiment justifies it.

Nothing in:

```
domain
application
retrieval

```

should need to know that Candle exists.

---

# 79. Main Runtime Contract

`main.rs` should be extremely small.

Conceptually:

```
fn main() -> ExitCode {
    run().unwrap_or_else(render_fatal_error)
}

```

or an equivalent structured runtime.

Its responsibility is:

```
initialize
 ↓
parse CLI
 ↓
select runtime
 ↓
execute
 ↓
exit

```

Do not put application logic in `main.rs`.

---

# 80. File Structure

The initial repository should be a **single Rust crate**, not a Cargo workspace.

A workspace is not justified yet.

Structure:

```
recall/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── ENGINEERING.md
├── ARCHITECTURE.md
├── migrations/
│   ├── 0001_initial.sql
│   └── 0002_fts.sql
├── src/
│   ├── main.rs
│   │
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── command.rs
│   │   ├── input.rs
│   │   └── render.rs
│   │
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── memory.rs
│   │   ├── source.rs
│   │   ├── search.rs
│   │   ├── inference.rs
│   │   └── job.rs
│   │
│   ├── application/
│   │   ├── mod.rs
│   │   ├── store.rs
│   │   ├── ask.rs
│   │   └── status.rs
│   │
│   ├── infrastructure/
│   │   ├── mod.rs
│   │   │
│   │   ├── database/
│   │   │   ├── mod.rs
│   │   │   ├── connection.rs
│   │   │   ├── migrations.rs
│   │   │   ├── memory_repository.rs
│   │   │   ├── job_repository.rs
│   │   │   └── search.rs
│   │   │
│   │   ├── inference/
│   │   │   ├── mod.rs
│   │   │   ├── ollama.rs
│   │   │   └── prompt.rs
│   │   │
│   │   ├── ipc/
│   │   │   ├── mod.rs
│   │   │   ├── protocol.rs
│   │   │   ├── client.rs
│   │   │   └── server.rs
│   │   │
│   │   └── clock.rs
│   │
│   └── runtime/
│       ├── mod.rs
│       ├── daemon.rs
│       ├── composition.rs
│       └── worker.rs
│
└── tests/
    ├── store.rs
    ├── ask.rs
    ├── retrieval.rs
    └── database.rs

```

This is the target organization, not a demand that every empty file be created immediately.

---

# 81. `src/main.rs`

Responsibility:

```
process entrypoint

```

It should:

1. initialize minimal runtime facilities
2. parse CLI
3. dispatch to CLI runtime
4. convert errors into exit codes

It must not contain:

- SQL
- prompt construction
- inference
- search
- memory creation

Target size:

```
< 100 lines

```

Ideally much smaller.

---

# 82. `src/cli/command.rs`

Defines the user-facing CLI model.

Contains:

```
Command
StoreArgs
AskArgs
DaemonArgs
StatusArgs

```

This file knows `clap`.

It does not know application implementation.

---

# 83. `src/cli/input.rs`

Converts command arguments into application input.

Responsibilities:

```
string
    ↓
StoreInput::Text

existing path
    ↓
StoreInput::File

```

It may perform lightweight CLI input validation.

It must not persist anything.

---

# 84. `src/cli/render.rs`

Converts application responses into terminal output.

Responsibilities:

```
StoreResponse → terminal text
Answer → terminal text
StatusResponse → terminal text
Error → user-facing diagnostic

```

It should not decide what the answer means.

It only renders.

---

# 85. `src/cli/mod.rs`

Coordinates:

```
Command
    ↓
client
    ↓
response
    ↓
render

```

It is the CLI orchestration layer.

It must remain thin.

---

# 86. `src/domain/memory.rs`

Defines:

```
Memory
MemoryId
memory construction/validation

```

Responsibilities:

- canonical memory invariants
- ID semantics
- memory construction

No SQLite.

No CLI.

No Ollama.

---

# 87. `src/domain/source.rs`

Defines:

```
MemorySource

```

and source-related domain behavior.

Initial variants:

```
DirectInput
File

```

This module should remain small.

---

# 88. `src/domain/search.rs`

Defines:

```
SearchQuery
SearchResult
SearchScore
RetrievedMemory

```

These are retrieval concepts, not database concepts.

---

# 89. `src/domain/inference.rs`

Defines domain-level inference structures:

```
GenerationRequest
GenerationResponse
EmbeddingRequest
EmbeddingResponse

```

and potentially:

```
InferenceModel

```

Do not place Ollama HTTP structures here.

Those belong to the adapter.

---

# 90. `src/domain/job.rs`

Defines:

```
Job
JobId
JobKind
JobState

```

and state-transition rules.

The job state machine should be tested heavily.

---

# 91. `src/application/store.rs`

Contains `StoreMemory`.

Responsibilities:

```
StoreInput
    ↓
capture
    ↓
validate
    ↓
construct Memory
    ↓
persist
    ↓
schedule derivation
    ↓
StoreResponse

```

This is the authoritative implementation of the store use case.

No CLI knowledge.

No SQL.

---

# 92. `src/application/ask.rs`

Contains `AskRecall`.

Responsibilities:

```
AskRequest
    ↓
SearchQuery
    ↓
MemorySearcher
    ↓
RetrievedMemory
    ↓
GenerationRequest
    ↓
InferenceBackend
    ↓
Answer

```

This is the authoritative implementation of asking Recall.

---

# 93. `src/application/status.rs`

Produces application status.

It may aggregate:

```
database status
job status
inference status

```

It should not itself know how those systems are implemented.

---

# 94. `src/infrastructure/database/connection.rs`

Owns SQLite connection setup.

Responsibilities:

- database path
- connection configuration
- WAL configuration
- connection initialization

It should not contain repository business logic.

---

# 95. `src/infrastructure/database/migrations.rs`

Owns migration execution.

Responsibilities:

```
locate migrations
execute migrations
report migration errors

```

Do not put schema SQL directly in Rust strings if migration files are sufficient.

---

# 96. `src/infrastructure/database/memory_repository.rs`

Implements:

```
MemoryRepository

```

against SQLite.

Contains:

- SQL
- row mapping
- persistence errors

Does not contain application behavior.

---

# 97. `src/infrastructure/database/job_repository.rs`

Implements job persistence.

Responsibilities:

```
insert job
claim pending job
mark running
mark completed
mark failed
list pending work

```

State transitions should be validated at the application/domain boundary.

Database operations should enforce necessary persistence invariants.

---

# 98. `src/infrastructure/database/search.rs`

Implements:

```
MemorySearcher

```

using SQLite FTS.

This file should contain search-specific SQL and mapping.

It should not contain LLM ranking logic.

---

# 99. `src/infrastructure/inference/ollama.rs`

The Ollama adapter.

Contains:

```
HTTP client
Ollama request structures
Ollama response structures
error translation
generation
embedding

```

The adapter translates between:

```
domain inference types
        ↕
Ollama wire types

```

Keep Ollama-specific JSON structs private to this module where possible.

---

# 100. `src/infrastructure/inference/prompt.rs`

Contains prompt construction.

This is intentionally separate from `ollama.rs`.

Reason:

Prompt semantics are Recall behavior.

Ollama transport is infrastructure.

Changing the prompt should not require changing HTTP code.

This is exactly the desired locality-of-change property.

---

# 101. `src/infrastructure/ipc/protocol.rs`

Defines wire protocol types:

```
Request
Response
RemoteError

```

These should be serialization-friendly.

Do not expose domain internals directly if the wire representation should evolve independently.

---

# 102. `src/infrastructure/ipc/client.rs`

Used by the CLI.

Responsibilities:

```
connect socket
serialize request
send request
receive response
deserialize response

```

No application behavior.

---

# 103. `src/infrastructure/ipc/server.rs`

Used by daemon runtime.

Responsibilities:

```
listen socket
accept connection
decode request
dispatch request to application service
encode response

```

The server should be dispatch glue.

Large repetitive dispatch code is an explicitly acceptable exception to the file-size guideline if necessary.

But it must remain mechanically boring.

---

# 104. `src/infrastructure/clock.rs`

Contains:

```
SystemClock

```

and the `Clock` implementation.

This isolates system time from deterministic domain/application tests.

---

# 105. `src/runtime/composition.rs`

This is the **composition root**.

It constructs:

```
database
repositories
searcher
inference backend
application services
workers
IPC server

```

This is the only place where concrete implementations should be assembled.

For example:

```
MemoryRepository
    = SqliteMemoryRepository

MemorySearcher
    = SqliteMemorySearcher

InferenceBackend
    = OllamaBackend

```

Changing the backend should primarily be a composition change.

---

# 106. `src/runtime/daemon.rs`

Owns daemon lifecycle.

Responsibilities:

```
initialize runtime
start IPC
start workers
wait for shutdown
shutdown workers
close resources

```

It should not implement store or ask behavior.

---

# 107. `src/runtime/worker.rs`

Owns background derivation execution.

Initial responsibility:

```
GenerateEmbedding jobs

```

Pipeline:

```
claim job
    ↓
load memory
    ↓
construct EmbeddingRequest
    ↓
InferenceBackend.embed()
    ↓
persist embedding
    ↓
mark job completed

```

The worker should not know about CLI.

---

# 108. Directory Relationships

The dependency direction should be:

```
                 ┌───────────┐
                 │    CLI    │
                 └─────┬─────┘
                       │
                       ▼
                 ┌───────────┐
                 │Application│
                 └─────┬─────┘
                       │
                       ▼
                  ┌────────┐
                  │ Domain │
                  └────────┘
                       ▲
                       │
              implements interfaces
                       │
                ┌──────┴──────┐
                │Infrastructure│
                └──────┬──────┘
                       ▲
                       │
                  ┌────┴─────┐
                  │ Runtime  │
                  └──────────┘

```

More precisely, runtime composes everything.

Infrastructure implements application/domain boundaries.

CLI consumes the application through IPC rather than directly.

---

# 109. One Important Refinement

The domain layer should not necessarily own every trait.

Repository and backend traits are **ports**.

They should live at the layer that consumes the abstraction.

For example, if `application::store` requires a memory repository, the repository contract should be owned by the application/domain-facing layer rather than by SQLite.

Conceptually:

```
application/
    ports/
        memory_repository.rs
        job_repository.rs
        inference_backend.rs

```

This may therefore refine the earlier directory structure into:

```
application/
├── mod.rs
├── store.rs
├── ask.rs
├── status.rs
└── ports/
    ├── mod.rs
    ├── memory_repository.rs
    ├── job_repository.rs
    ├── memory_searcher.rs
    ├── inference_backend.rs
    └── clock.rs

```

This is the preferred design.

Infrastructure implements these ports.

This ensures that the direction of dependency is correct:

```
Application defines what it needs.
Infrastructure defines how it provides it.

```

---

# 110. Revised Dependency Structure

The resulting structure is:

```
                    CLI
                     │
                     ▼
                Application
                /    |    \
               /     |     \
          Domain    Ports    Use cases
                     ▲
                     │
              Infrastructure
             /       |        \
        SQLite     Ollama      IPC
                     │
                   Runtime

```

The runtime is the composition mechanism, not a business layer.

---

# 111. Trait Placement

Concrete decisions:

```
MemoryRepository
    → application::ports

JobRepository
    → application::ports

MemorySearcher
    → application::ports

InferenceBackend
    → application::ports

Clock
    → application::ports

```

Implementations:

```
SQLite
    → infrastructure::database

Ollama
    → infrastructure::inference

SystemClock
    → infrastructure::clock

```

This creates strong inversion-of-control boundaries.

---

# 112. Test Implementations

Test doubles should live near the tests rather than production infrastructure.

For example:

```
tests/support/

```

or module-local test implementations.

Possible:

```
InMemoryMemoryRepository
FakeInferenceBackend
FixedClock
InMemoryJobRepository

```

Do not add production dependencies solely to support tests.

---

# 113. Integration Test Structure

Use:

```
tests/
├── store.rs
├── ask.rs
├── retrieval.rs
├── database.rs
└── support/
    ├── mod.rs
    ├── repository.rs
    ├── inference.rs
    └── clock.rs

```

Integration tests should exercise public subsystem contracts.

---

# 114. Store Test

The most important initial integration test should be:

```
store_text_persists_memory

```

Arrange:

```
empty database

```

Act:

```
StoreMemory.execute(
    StoreInput::Text("hello")
)

```

Assert:

```
response contains MemoryId
database contains memory
memory content == "hello"
source == DirectInput

```

No Ollama required.

---

# 115. File Store Test

Test:

```
store_file_reads_file_contents

```

Arrange:

```
temporary file containing "hello"

```

Act:

```
StoreInput::File(path)

```

Assert:

```
canonical memory content == "hello"
source == File(path)

```

This confirms that file ingestion remains a thin conversion to text.

---

# 116. Store Failure Test

Test:

```
embedding_failure_does_not_lose_memory

```

Arrange:

```
FakeInferenceBackend returns failure

```

Act:

```
store memory
process embedding job

```

Assert:

```
memory exists
job is failed
memory content remains unchanged

```

This test protects one of Recall's most important architectural guarantees.

---

# 117. Ask Test

Test:

```
ask_uses_retrieved_memories_as_context

```

Arrange:

```
repository contains known memories
fake searcher returns memory A
fake inference records request

```

Act:

```
AskRecall.execute(...)

```

Assert:

```
inference received the question
inference received memory A
answer contains inference response
answer sources contains A

```

The test should verify information flow, not Ollama.

---

# 118. Provenance Test

Test:

```
answer_sources_come_from_retrieval

```

The fake inference backend should not be able to inject arbitrary source IDs.

The application determines the final source set.

This explicitly protects the memory trust model.

---

# 119. Retrieval Test

Test:

```
lexical_search_returns_matching_memory

```

Use a real temporary SQLite database.

Insert several memories.

Run:

```
SearchQuery("SQLite")

```

Assert the relevant memory is among the results.

Do not test SQL implementation details.

Test behavior.

---

# 120. Job State Test

Test the state machine independently.

Examples:

```
pending_job_can_be_claimed
running_job_can_complete
running_job_can_fail
completed_job_cannot_run_again

```

These tests should not require an actual worker.

---

# 121. File Size Enforcement

The project should use CI or a lightweight script to detect files exceeding the 300-line guideline.

The check should report rather than blindly reject legitimate exceptions.

For example:

```
WARNING: src/infrastructure/ipc/server.rs = 327 lines

```

The exception can then be reviewed.

Do not distort code purely to satisfy a line-count script.

---

# 122. The First Implementation Boundary

The first implementation should stop at:

```
CLI
    ↓
IPC
    ↓
StoreMemory
    ↓
SQLite

```

and:

```
CLI
    ↓
IPC
    ↓
AskRecall
    ↓
SQLite FTS
    ↓
Ollama

```

with embedding/index jobs added after the basic flow works.

Do not start by implementing every planned subsystem.

---

# 123. First Vertical Slice

The first meaningful vertical slice is:

```
recall store "I like Rust."

```

produces:

```
Stored memory <id>

```

and then:

```
recall "what do I like?"

```

produces an answer based on:

```
"I like Rust."

```

The complete path must work end-to-end before substantial infrastructure is added.

---

# 124. Development Sequence

Implementation should proceed in this order:

```
1. Domain types
2. Application ports
3. SQLite memory persistence
4. Store use case
5. CLI store command
6. IPC protocol
7. Daemon composition
8. End-to-end store
9. SQLite lexical search
10. Ask use case
11. Ollama generation
12. End-to-end ask
13. Background jobs
14. Embeddings
15. Semantic retrieval

```

This order maximizes early feedback.

---

# 125. Explicitly Deferred

Do not implement yet:

```
TUI
filesystem watching
automatic ingestion
summarization
classification
memory relationships
reranking models
llama.cpp
SYCL
Candle
GPU benchmarking
agent/tool use
web search
cloud synchronization

```

The architecture should permit them.

The implementation should not depend on them.

---

# 126. The Resulting Mental Model

The system should ultimately be understandable as four cooperating machines:

```
                 ┌─────────────────────┐
                 │       Capture       │
                 │ "What did user give │
                 │       us?"          │
                 └──────────┬──────────┘
                            │
                            ▼
                 ┌─────────────────────┐
                 │      Memory         │
                 │ "What do we know?"  │
                 └──────────┬──────────┘
                            │
                   derived indexes
                            │
                            ▼
                 ┌─────────────────────┐
                 │      Retrieval      │
                 │ "What is relevant?" │
                 └──────────┬──────────┘
                            │
                            ▼
                 ┌─────────────────────┐
                 │      Inference      │
                 │ "What can we say    │
                 │  about it?"         │
                 └─────────────────────┘

```

The daemon coordinates these machines.

The CLI merely asks the machine to do something.

---

# 127. Final Architectural Invariants

These are the rules the implementation must preserve.

### Invariant 1

```
Memory is canonical.

```

### Invariant 2

```
Embeddings, indexes, summaries and LLM output are derived.

```

### Invariant 3

```
Storing memory never requires successful inference.

```

### Invariant 4

```
Retrieval can work without inference.

```

### Invariant 5

```
Inference never queries memory directly.

```

### Invariant 6

```
The application chooses what memory is supplied to the LLM.

```

### Invariant 7

```
The application, not the LLM, determines provenance.

```

### Invariant 8

```
Ollama is an adapter, not an application dependency.

```

### Invariant 9

```
Future llama.cpp/SYCL and Candle implementations replace adapters.

```

### Invariant 10

```
The CLI contains no business logic.

```

### Invariant 11

```
SQLite contains no application orchestration.

```

### Invariant 12

```
Prompt construction contains no database access.

```

### Invariant 13

```
Background derivation is eventually consistent with canonical memory.

```

### Invariant 14

```
Every subsystem has one clear owner for its state.

```

### Invariant 15

```
A local behavior change should normally require a local code change.

```

---

# 128. Final Target Architecture

The implementation should converge on:

```
                         USER
                          │
              ┌───────────┴───────────┐
              │                       │
              ▼                       ▼
     recall store "..."      recall "question"
              │                       │
              └──────────┬────────────┘
                         │
                         ▼
                       CLI
                         │
                    Unix socket
                         │
                         ▼
                 ┌───────────────┐
                 │ Recall Daemon │
                 └───────┬───────┘
                         │
                 Application Layer
                         │
          ┌──────────────┼──────────────┐
          │              │              │
          ▼              ▼              ▼
       Capture       Retrieval       Answer
          │              │              │
          ▼              ▼              ▼
       Memory        Searchers      Inference
          │              │              │
          ▼              │              ▼
       SQLite ◄──────────┘           Ollama
          │
          ▼
    Derived Jobs
          │
          ▼
     Embeddings
          │
          ▼
   Semantic Index

```

And eventually:

```
Inference
    │
    ├── Ollama
    │
    ├── llama.cpp
    │      └── SYCL
    │           └── Intel GPU
    │
    └── Candle

```

without changing the memory model, retrieval model, CLI, or application semantics.

That is the architecture.

The implementation should now be driven from this specification rather than continuing to make architectural decisions opportunistically while coding.
