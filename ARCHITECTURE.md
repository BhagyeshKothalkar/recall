# Recall --- Product Philosophy & Design Principles

## 1. Purpose

Recall is a **personal memory system for one person**, running primarily
on a consumer laptop.

Its purpose is simple:

> Make the user's own computer remember things for them.

Recall is not fundamentally a chatbot, an autonomous agent, or an AI
productivity suite. AI is a mechanism used to make the user's stored
memory easier to retrieve, connect, and understand.

The implementation specification establishes the concrete API and
runtime model. This document defines the product philosophy and the
design choices behind that API.

------------------------------------------------------------------------

## 2. The Central Principle

> **The LLM does not own the memory. The database does.**

This is the defining design decision of Recall.

A language model can interpret retrieved information, summarize it, and
answer questions about it. It must not become the implicit source of
truth about the user's past.

The authoritative chain is:

``` text
user input
    ↓
canonical memory
    ↓
retrieval
    ↓
LLM interpretation
    ↓
answer + provenance
```

Not:

``` text
user
  ↓
LLM
  ↓
"I remember..."
```

The implementation API reflects this directly:

``` text
StoreInput
    ↓
CapturedMemory
    ↓
Memory
    ↓
MemoryRepository
```

and:

``` text
AskRequest
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

The inference backend receives memory selected by Recall. It does not
query the database itself.

------------------------------------------------------------------------

## 3. Personal Tool First

Recall is designed for **one user**, not a multi-tenant service.

That changes the engineering priorities.

The system should optimize for:

-   low friction
-   local operation
-   fast startup and interaction
-   easy backup
-   understandable failure modes
-   modest resource consumption
-   maintainability by one developer
-   useful behavior on an ordinary consumer laptop

It does **not** need:

-   distributed databases
-   cloud-scale infrastructure
-   multi-user authorization
-   horizontally scalable workers
-   remote orchestration
-   elaborate service meshes
-   a specialized vector database merely for architectural fashion

The right question is not:

> "Can this architecture scale to millions of users?"

It is:

> "Will I actually use `recall ...` when I need to remember something?"

------------------------------------------------------------------------

## 4. Local-First Is a Product Property

The default assumption is:

> **Memory stays on the user's machine unless the user explicitly
> chooses otherwise.**

Core Recall should not require:

-   cloud storage
-   hosted vector databases
-   hosted LLM APIs
-   SaaS authentication
-   telemetry
-   remote synchronization

The implementation target is a consumer laptop, so local resources are
the natural system boundary:

``` text
Recall
├── local database
├── local artifacts
├── local indexes
├── local daemon
└── local inference
```

Network access may be useful for explicit future integrations, but it
must not be a prerequisite for the core memory product.

This also makes backup and recovery understandable: the canonical
database and configuration should be sufficient to recover the user's
memory, with derived indexes rebuildable.

------------------------------------------------------------------------

## 5. Canonical Memory vs Derived Intelligence

Recall deliberately separates **what the user has stored** from **what
the system has computed about it**.

Canonical data includes:

-   memory content
-   source
-   identity
-   timestamps
-   authoritative metadata

Derived data may include:

-   FTS indexes
-   embeddings
-   summaries
-   classifications
-   cached model output
-   other retrieval structures

The rule is:

> **Derived intelligence may improve memory retrieval, but it must never
> become the only representation of memory.**

This gives Recall important properties:

-   embeddings can be regenerated
-   search indexes can be rebuilt
-   models can be changed
-   inference backends can be replaced
-   corrupted derived state does not destroy the memory itself
-   deletion can be authoritative

This is why `Memory` does not contain an embedding or search score in
the implementation specification.

------------------------------------------------------------------------

## 6. Capture Must Not Require AI

Memory creation is a first-class operation independent of inference.

A user should be able to do something equivalent to:

``` text
recall store "try SQLite WAL mode"
```

and have the information safely persisted even if:

-   Ollama is unavailable
-   the model is missing
-   the GPU is unavailable
-   embeddings fail
-   enrichment is broken

The store pipeline therefore separates:

``` text
capture
    ↓
persist
```

from:

``` text
derive
```

The canonical memory should become durable before expensive or
failure-prone AI work occurs.

This makes Recall useful even when the AI subsystem is completely
offline.

------------------------------------------------------------------------

## 7. Search Before AI

Recall must be useful without an LLM.

The first retrieval capability is therefore deterministic local search,
particularly SQLite FTS.

The conceptual progression is:

``` text
exact / lexical
    ↓
FTS
    ↓
metadata filtering
    ↓
fuzzy / improved lexical retrieval
    ↓
semantic retrieval
    ↓
hybrid ranking
```

Semantic retrieval is an additional index, not a replacement for lexical
retrieval.

This is important for both reliability and product behavior. If Ollama
stops working tonight, the user should still be able to ask Recall where
they mentioned SQLite.

------------------------------------------------------------------------

## 8. Retrieval Is a Product Capability, Not an LLM Trick

Recall's core intelligence is not merely generation.

It is:

> **Finding the right underlying memory.**

The long-term retrieval pipeline is:

``` text
query
  ↓
normalization
  ↓
lexical + semantic retrieval
  ↓
candidate memories
  ↓
ranking
  ↓
relevant context
  ↓
LLM
```

The retrieval pipeline must be independently testable.

A plausible LLM answer is not evidence that retrieval was correct.

The system should be able to inspect and test which memories were
selected.

------------------------------------------------------------------------

## 9. Provenance Is Part of Trust

Recall should preserve the relationship between an answer and the
memories that informed it.

The application, not the LLM, determines provenance.

For example:

``` text
Answer
├── text
└── sources
    ├── Memory A
    └── Memory C
```

The model generates language.

Recall knows which memories were actually retrieved.

This prevents the model from inventing citations or source IDs and gives
the user a path back to the underlying information.

The desired experience is:

> "Why did Recall say this?"

→ inspect the source memory.

That makes Recall fundamentally different from opaque conversational
memory.

------------------------------------------------------------------------

## 10. Trust Hierarchy

Recall should distinguish different kinds of information.

### User-authored memory

Highest authority as a record of what the user entered.

### Imported material

Authoritative as stored source material, but not necessarily
authoritative as factual truth.

### LLM summaries

Derived representations.

### LLM conclusions

Useful interpretations, but non-canonical.

The system should never silently replace a user's original memory with a
model-generated paraphrase.

If derived information is stored, its relationship to its source should
remain recoverable.

------------------------------------------------------------------------

## 11. The LLM Is a Replaceable Capability

The first inference backend is Ollama because it provides a practical
route to a useful local system quickly.

Ollama is **not** the architecture.

Recall therefore exposes an inference capability boundary conceptually
like:

``` text
InferenceBackend
├── OllamaBackend
├── LlamaCppBackend
├── CandleBackend
└── future implementations
```

The application should depend on the capability rather than on a
particular inference vendor.

This allows the system to change inference technology without changing
the memory model or retrieval semantics.

Generation and embedding should also remain conceptually separate
capabilities. They may eventually use different models or even different
backends.

------------------------------------------------------------------------

## 12. Consumer Laptop as the Primary Deployment Target

Recall is not being designed around a server-class machine.

The baseline environment is a **consumer laptop**.

That implies:

-   finite RAM
-   finite storage
-   potentially modest CPU capacity
-   integrated or shared-memory graphics
-   intermittent resource availability
-   user-facing workloads competing with Recall
-   no assumption of 24/7 service availability

The architecture should therefore favor:

-   embedded storage
-   bounded background work
-   low operational complexity
-   graceful degradation
-   recoverable jobs
-   CPU fallback
-   optional acceleration
-   measurable resource usage

The daemon exists to make background work possible, not to turn the
laptop into a miniature distributed system.

------------------------------------------------------------------------

## 13. Intel Integrated XPU: Experiment, Not Assumption

A later goal is to investigate local inference using the laptop's Intel
integrated XPU/GPU.

This is deliberately treated as an engineering experiment.

Recall should not assume:

-   a particular Intel device
-   a particular memory capacity
-   a particular backend
-   a particular performance level
-   that GPU inference will be better than CPU inference

Instead, the system should eventually distinguish:

``` text
detected
supported
configured
working
```

The first objective is capability discovery.

The second is useful inference.

The third is performance optimization.

This ordering prevents hardware-specific assumptions from contaminating
the core product architecture.

------------------------------------------------------------------------

## 14. llama.cpp / SYCL as the Native Experiment

For the eventual Intel acceleration investigation, the architecture
should remain compatible with a future llama.cpp adapter using Intel
SYCL.

The intended evolution is:

``` text
InferenceBackend
       │
       ├── Ollama
       │
       └── llama.cpp
              │
              ├── CPU
              └── SYCL
                    │
                    └── Intel integrated XPU/GPU
```

The first experiment should preferably keep llama.cpp outside the Recall
process boundary where practical, for example as a local server/process.

Tighter integration should only happen if measurements demonstrate a
real benefit.

The product should remain fully useful if the Intel acceleration
experiment produces disappointing results.

------------------------------------------------------------------------

## 15. CPU Fallback Is a Correctness Requirement

Hardware acceleration is an optimization.

It is not a prerequisite for Recall to function.

If the XPU disappears, is unsupported, or performs poorly:

``` text
Recall
  ↓
CPU inference
```

should remain possible where practical.

A laptop-oriented application should degrade gracefully rather than make
hardware availability part of its correctness model.

------------------------------------------------------------------------

## 16. Why a Daemon?

The implementation specification chooses a single Rust binary with two
runtime roles:

``` text
recall <command>
    ↓
CLI client
    ↓
Unix-domain socket
    ↓
Recall daemon
```

The daemon owns:

-   database connections
-   repositories
-   retrieval
-   inference clients
-   background jobs
-   long-lived state
-   resource lifecycle

The CLI owns:

-   argument parsing
-   input acquisition
-   IPC
-   response rendering
-   exit status

This separation gives Recall a clean boundary without requiring multiple
deployable services.

The daemon also enables an important product behavior:

``` text
store
  ↓
persist immediately
  ↓
background derivation
```

The user does not need to wait for embedding generation just to safely
remember something.

------------------------------------------------------------------------

## 17. Why One Binary?

A single binary keeps deployment appropriate for a personal laptop.

There is no need to coordinate:

``` text
CLI package
+
daemon package
+
database server
+
vector server
+
model service
```

Instead:

``` text
recall
```

can act as either client or daemon.

The architectural separation is logical rather than deployment-heavy.

This preserves the engineering benefits of a daemon while keeping
installation and backup simple.

------------------------------------------------------------------------

## 18. Unix-Domain Socket IPC

The CLI should not directly access application infrastructure.

Instead:

``` text
CLI
  ↓
typed request
  ↓
Unix socket
  ↓
daemon
  ↓
application use case
```

This provides a strong boundary:

-   CLI cannot accidentally bypass application rules
-   database ownership remains in the daemon
-   inference ownership remains in the daemon
-   future clients can use the same application protocol
-   the CLI remains a replaceable presentation layer

For an internal, single-user laptop protocol, a simple structured format
such as JSON is appropriate initially because debuggability matters more
than protocol micro-optimization.

------------------------------------------------------------------------

## 19. The Public UX Should Be Extremely Small

The primary interaction should feel like a memory tool, not an
enterprise CLI.

The implementation specification intentionally centers the UX around:

``` text
recall store "some memory"
recall store ./some-file.txt
recall "some question"
```

with operational commands such as:

``` text
recall daemon
recall status
recall help
```

This matters because Recall's value comes from repeated low-friction
use.

The user should not need to open a complicated interface merely to save
a thought.

A TUI can come later, after the underlying workflows prove useful.

------------------------------------------------------------------------

## 20. Store Text and Files, But Keep Ingestion Simple

The initial `store` behavior deliberately treats a file as text:

``` text
existing path → read contents → canonical text memory
otherwise      → literal text
```

This is a product choice to avoid premature ingestion complexity.

The first version should not immediately attempt:

-   PDF parsing
-   OCR
-   MIME systems
-   document semantics
-   rich Markdown interpretation
-   arbitrary filesystem crawling

The initial question is simply:

> Can Recall reliably remember what I intentionally give it?

More sophisticated ingestion can be added when actual use demonstrates
its value.

------------------------------------------------------------------------

## 21. Eventual Consistency Is Intentional

A newly stored memory may have:

``` text
canonical memory: ready
FTS: ready
embedding: pending
```

That is acceptable.

The product invariant is:

> **Canonical memory is immediately durable; derived intelligence is
> eventually consistent.**

This is a deliberate tradeoff for a personal laptop.

It allows the user to continue working while background enrichment
occurs, and it prevents inference latency from becoming memory latency.

------------------------------------------------------------------------

## 22. Background Jobs Should Be Small and Durable

The system may eventually perform:

``` text
GenerateEmbedding
ExtractMetadata
SummarizeMemory
ReindexMemory
```

but it should not require a distributed job system.

A small persisted local job queue is sufficient.

The job model exists to survive:

-   daemon crashes
-   model failures
-   GPU failures
-   interrupted indexing
-   database restarts

A failed enrichment job should be retryable without changing or losing
the canonical memory.

------------------------------------------------------------------------

## 23. Failure Isolation

Recall should fail in layers.

If inference fails:

``` text
memory remains
search remains
job can fail/retry
```

If embeddings fail:

``` text
lexical retrieval remains
```

If GPU inference fails:

``` text
CPU fallback where possible
```

If a derived index is corrupted:

``` text
canonical memory remains
index can be rebuilt
```

This is not merely defensive engineering. It follows directly from the
product philosophy that memory is more important than AI.

------------------------------------------------------------------------

## 24. Security Boundary: Read-Oriented First

The initial Recall LLM should not be an autonomous system.

It should not initially be allowed to:

-   execute shell commands
-   modify files
-   delete memories
-   inspect arbitrary filesystem locations
-   control processes
-   autonomously browse the web
-   create background agent loops

The initial direction is:

``` text
retrieve
  ↓
provide context
  ↓
generate
  ↓
show answer + provenance
```

This keeps the model's authority narrow.

Tool use can be considered later if a concrete personal workflow
justifies it.

------------------------------------------------------------------------

## 25. Deletion Must Be Authoritative

"Forget" is a real product operation, not merely removal from one search
index.

The canonical database determines whether a memory exists.

When deletion is implemented, derived representations must not retain an
inaccessible semantic copy indefinitely.

This reinforces the canonical/derived distinction:

``` text
canonical deletion
    ↓
derived data invalidated / removed
```

------------------------------------------------------------------------

## 26. Backup and Portability

Recall should be easy to back up and restore.

The desired recovery model is approximately:

``` text
database backup
+
configuration
+
optional model cache
```

rather than:

``` text
database
+
vector server
+
cloud credentials
+
remote services
+
special deployment machinery
```

This is particularly important for a personal laptop tool. The user's
memory should not become operationally dependent on a collection of
services.

------------------------------------------------------------------------

## 27. Observability Is Part of the Product

A background daemon introduces questions that a simple CLI does not:

-   Is the daemon running?
-   Is the database healthy?
-   Are jobs stuck?
-   Is Ollama reachable?
-   Which backend is active?
-   Is the GPU detected?
-   Are embeddings complete?
-   Why did a memory fail to become searchable?

Therefore commands such as:

``` text
recall status
recall doctor
recall jobs
recall logs
```

are natural operational extensions.

Diagnostics should make system behavior understandable without requiring
source-code archaeology.

------------------------------------------------------------------------

## 28. Rust Is a Fit for the System, Not a Branding Choice

Rust is appropriate because Recall combines:

-   a daemon
-   IPC
-   local persistence
-   filesystem interaction
-   background workers
-   process management
-   inference orchestration
-   hardware discovery
-   CLI/TUI interfaces

The goal is not to reimplement every dependency in Rust.

Mature external systems such as SQLite and Ollama should be used where
they are appropriate.

Rust's role is to provide a small, explicit, maintainable system
boundary around them.

------------------------------------------------------------------------

## 29. SQLite Is the Natural Initial Store

For one user's memory on one laptop, SQLite provides an unusually good
fit:

-   embedded
-   transactional
-   local
-   easy to back up
-   no database daemon
-   mature
-   suitable for FTS
-   compatible with WAL
-   straightforward to rebuild and migrate

A separate database server would add operational complexity without an
identified product benefit.

Likewise, a specialized vector database should only be introduced if
actual measurements show that SQLite plus a suitable local vector/search
mechanism is insufficient.

------------------------------------------------------------------------

## 30. API Design Reflects the Philosophy

The implementation API intentionally separates concepts.

### Capture

``` text
StoreInput
    ↓
CapturedMemory
```

### Persistence

``` text
CapturedMemory
    ↓
Memory
    ↓
MemoryRepository
```

### Retrieval

``` text
SearchQuery
    ↓
MemorySearcher
    ↓
SearchResult / RetrievedMemory
```

### Generation

``` text
GenerationRequest
    ↓
InferenceBackend
    ↓
GenerationResponse
```

### Final answer

``` text
GenerationResponse
+
retrieval provenance
    ↓
Answer
```

This is more than code organization. It prevents conceptual
responsibilities from collapsing into one "AI service."

------------------------------------------------------------------------

## 31. The Application Owns the Meaning of a Request

The application layer is the product behavior.

For example:

``` text
StoreMemory
AskRecall
GetStatus
```

are use cases.

Infrastructure knows how to perform external operations.

The CLI knows how to express them to the user.

The inference backend knows how to talk to a model runtime.

This produces the dependency direction:

``` text
CLI
 ↓
Application
 ↓
Domain + Ports
 ↓
Infrastructure
```

The runtime composition root assembles the concrete implementations.

------------------------------------------------------------------------

## 32. Inference Must Never Query Memory

This deserves explicit repetition because it is the strongest
architectural boundary.

Wrong:

``` text
InferenceBackend
    ↓
SQLite
    ↓
"find memories"
```

Correct:

``` text
Application
    ↓
MemorySearcher
    ↓
RetrievedMemory[]
    ↓
GenerationRequest
    ↓
InferenceBackend
```

This guarantees that the model sees only the context Recall
intentionally selected.

It also makes retrieval independently testable and makes provenance
deterministic.

------------------------------------------------------------------------

## 33. Product Benchmarks

Recall should be judged primarily by product behavior, not by AI
sophistication.

### Benchmark 1 --- Capture reliability

A memory should be safely stored without requiring AI.

Success means:

``` text
store succeeds
→ canonical memory exists
```

even when inference is unavailable.

### Benchmark 2 --- Retrieval usefulness

Given a small known memory corpus, representative natural-language
questions should surface the correct memory in the top results.

The benchmark should measure retrieval, not whether an LLM can
hallucinate a plausible answer.

### Benchmark 3 --- Provenance

Every generated answer that relies on memory should be traceable to the
memories actually supplied as context.

### Benchmark 4 --- Failure isolation

Inference failure must not destroy memory persistence or basic lexical
search.

### Benchmark 5 --- Rebuildability

Delete/rebuild derived indexes and embeddings without losing canonical
memories.

### Benchmark 6 --- Laptop suitability

The system should remain practical on the target consumer laptop:

-   bounded background resource use
-   no unnecessary always-on external services
-   predictable storage growth
-   recoverable jobs
-   graceful CPU fallback

Exact numeric thresholds should be established from the actual target
machine rather than invented prematurely.

### Benchmark 7 --- Intel XPU experiment

When hardware acceleration is investigated, compare actual measurements
for:

``` text
CPU
Ollama path
llama.cpp CPU
llama.cpp SYCL / Intel XPU
```

where technically applicable.

Measure:

-   useful model capability
-   latency
-   prompt processing
-   tokens/sec
-   memory consumption
-   stability
-   CPU/GPU utilization
-   practical context size

The question is not "Can the GPU run inference?"

It is:

> "Does using this laptop's XPU materially improve Recall enough to
> justify the complexity?"

------------------------------------------------------------------------

## 34. Engineering Decision Rule

When choosing between implementations, prefer the smallest solution that
advances the product.

Ask:

1.  What user problem does this solve?
2.  Which Recall invariant does it protect?
3.  Can the behavior remain local?
4.  Does it introduce a new permanent dependency?
5.  Does it make canonical memory less authoritative?
6.  Does it require AI when AI is not necessary?
7.  Does it make the laptop deployment more fragile?
8.  Can it be removed or replaced later?
9.  Can we measure whether it actually helps?

This keeps implementation decisions subordinate to product value.

------------------------------------------------------------------------

## 35. What Recall Is Not

At least initially, Recall is not:

-   a generic chatbot
-   an autonomous personal agent
-   a cloud memory service
-   a vector database product
-   a document management suite
-   a filesystem crawler
-   an AI operating system
-   a multi-user platform
-   a GPU benchmark project

Those may contain useful technologies or future features, but none is
the core product.

------------------------------------------------------------------------

## 36. Definition of Success

Recall succeeds when the user starts treating it as an external memory.

Examples:

``` text
"I remember having an idea about this."

recall "what was that Rust project idea?"
```

or:

``` text
"I know I solved this before."

recall "how did I fix that Docker problem?"
```

or simply:

``` text
"I don't want to organize this thought right now."

recall store "..."
```

The user should be able to trust that the original thought was stored,
find it later, and understand why Recall returned it.

The ultimate benchmark is behavioral:

> **Does Recall become something the user naturally reaches for when
> they need to remember?**

------------------------------------------------------------------------

## 47. Guiding Mental Model

Recall can be understood as four cooperating layers:

``` text
┌──────────────────────────────────────┐
│             EXPERIENCE               │
│          CLI / future TUI            │
├──────────────────────────────────────┤
│               RECALL                 │
│      memory / search / retrieval     │
├──────────────────────────────────────┤
│              INFERENCE               │
│    Ollama / llama.cpp / Candle       │
├──────────────────────────────────────┤
│               SYSTEM                │
│   Rust / SQLite / IPC / filesystem   │
└──────────────────────────────────────┘
```

The product center is the second layer: **memory and retrieval**.

Inference supports it.

The system infrastructure enables it.

The interface exposes it.

------------------------------------------------------------------------

## 48. Final Product Position

Recall should remain deliberately small in purpose:

> **A local personal memory system that makes the user's own computer
> remember things for them.**

The LLM makes that memory conversational.

Search makes it dependable.

Provenance makes it trustworthy.

SQLite makes it local and durable.

Rust makes the surrounding system explicit and maintainable.

The Intel XPU experiment makes local inference an interesting future
capability.

None of those should displace the central product.

**Recall is a memory system first.\
The LLM is the guy you ask about the memory.**


---

## 39. Documentation Is Part of the Product

Recall's engineering documentation is not incidental.

The repository should maintain clear documentation for:

- product philosophy
- architectural boundaries
- implementation decisions
- public interfaces
- important invariants
- failure behavior
- operational behavior
- meaningful technical decisions
- deferred decisions and why they remain deferred

Documentation should explain **why** a decision exists, not merely restate what the code does.

Public Rust APIs should have Rustdoc where their purpose, invariants, ownership expectations, failure behavior, side effects, or lifecycle requirements are not immediately obvious.

Documentation must evolve with the architecture. Stale documentation is a design defect because it causes future changes to be made against a false model of the system.

---

## 40. Code Quality Is Primarily About Change Locality

The central code-quality principle for Recall is:

> **A behavior change should require the smallest possible change surface.**

The preferred order of change locality is:

```text
lines
  ↓
function
  ↓
impl / object / struct
  ↓
file
  ↓
directory / subsystem
  ↓
entire application
```

A change that logically belongs to a few lines should not require editing an entire file.

A change that belongs to one function should not require unrelated functions to change.

A change that belongs to one implementation should not require changes across the application.

A change that belongs to one subsystem should terminate within that subsystem.

When a small behavioral change repeatedly requires a large change surface, that is evidence of architectural coupling and should trigger a design review.

This is the primary maintainability benchmark for Recall.

---

## 41. The 300-Line Rule

No single handwritten Rust source file should exceed **300 lines** unless there is a concrete, documented reason why splitting it would make the code materially worse.

The 300-line threshold is an architectural warning, not a formatting target.

When a file approaches or exceeds the threshold, ask:

- Does it contain multiple responsibilities?
- Can a coherent implementation boundary be extracted?
- Are infrastructure details mixed with domain/application logic?
- Are request/response types mixed with transport code?
- Are unrelated use cases sharing one module?

Prefer splitting by responsibility.

Legitimate exceptions may include:

- generated code
- mechanically repetitive code
- large static mappings
- protocol dispatch tables
- parser tables
- unavoidable platform declarations
- mundane glue where splitting would reduce readability

An exception should be intentional rather than a justification after the fact.

The goal is not artificially small files.

The goal is **small change surfaces and clear ownership**.

---

## 42. Error Fixes and Behavioral Changes

Every bug fix or behavioral change should be evaluated by asking:

> **What is the smallest scope in which this change can correctly live?**

Prefer:

```text
a few lines
    ↓
one function
    ↓
one impl / object
    ↓
one file
    ↓
one directory / subsystem
```

Only change a larger scope when the requirement genuinely crosses that boundary.

For example:

```text
Change:
"Fix Ollama response parsing."

Preferred:
inference/ollama.rs

Bad:
application + retrieval + CLI + database + runtime
```

Similarly:

```text
Change:
"Change memory ranking."

Preferred:
retrieval/ranking implementation + focused tests

Bad:
changes throughout domain, CLI, database, daemon, and inference
```

A change that unexpectedly spreads across unrelated modules is a signal that the existing boundary may be wrong.

---

## 43. Accidental Coupling Is a Defect

Recall should continuously resist coupling that makes ordinary changes expensive.

Examples of undesirable coupling include:

- CLI code knowing SQLite details
- domain code knowing Ollama APIs
- retrieval code knowing prompt transport
- prompts accessing the database
- inference adapters deciding provenance
- database code orchestrating application workflows
- unrelated features sharing mutable global state
- model-specific assumptions leaking into domain types

The architecture should make implementation details terminate at the boundary where they originate.

The desired result is:

```text
small requirement
    ↓
small code change
    ↓
small test change
```

not:

```text
small requirement
    ↓
wide architectural ripple
    ↓
many unrelated files
```

---

## 44. Tests Protect Change Locality

Tests should reinforce the architecture, not make refactoring harder.

A good test verifies observable behavior and important invariants.

Tests should therefore protect statements such as:

- memory persists without successful inference
- lexical search works without embeddings
- inference only receives explicitly retrieved context
- provenance comes from retrieval
- derived data can be rebuilt
- failed enrichment does not destroy canonical memory
- job state transitions are valid
- backend implementations can be replaced without changing application semantics

Tests should not unnecessarily encode private implementation details.

This allows internal refactoring while preserving behavioral guarantees.

---

## 45. Code Review Standard

Every meaningful change should be reviewed against four questions:

### Responsibility

Does this code belong here?

### Locality

Could the change have been smaller?

### Coupling

Did an implementation detail leak across a boundary?

### Evidence

Is the behavior protected by an appropriate test?

A change that works but unnecessarily increases the architectural change surface is not considered high-quality Recall code.

---

## 46. Long-Term Code Quality Benchmark

The codebase should continuously move toward this ideal:

```text
                         Change locality

                    ┌─────────────────────┐
                    │        lines        │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │      function       │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │    impl / object    │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │        file         │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │      directory      │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │ entire application  │
                    └─────────────────────┘
```

The lower the required scope for a legitimate change, the healthier the architecture.

The goal is not maximal abstraction.

The goal is:

> **Minimal accidental coupling, small files, explicit responsibilities, strong documentation, and the smallest possible change surface.**
