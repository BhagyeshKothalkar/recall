# Installing Recall

Recall is distributed as a Rust binary crate. The migrations are compiled into the binary, so an installed `recall` executable does not depend on the source checkout or the repository's `migrations/` directory at runtime.

## Install for the current user

From the repository root:

```bash
cargo install --path . --force
```

Cargo installs the executable as `recall` under Cargo's binary directory (normally `~/.cargo/bin`). Ensure that directory is on `PATH`.

Verify:

```bash
recall --version
recall --help
```

## Start and stop the daemon

The installed CLI manages the daemon itself:

```bash
recall start
recall status
recall stop
```

`recall start` launches the installed `recall daemon` as a detached background process. `recall stop` sends a clean shutdown request over the Unix socket.

The foreground form remains available for debugging:

```bash
recall daemon
```

## Runtime state

By default Recall stores its local runtime state under:

```text
$XDG_DATA_HOME/recall/
```

or, when `XDG_DATA_HOME` is unset:

```text
~/.local/share/recall/
```

The default files are:

```text
recall.db
recall.sock
```

They are not created in the source repository.

## Ollama

The daemon uses the centralized Ollama configuration documented in `OVERVIEW.md`. Ollama must be running and the configured generation/embedding models must be available before AI-backed operations are used.

Retrieval without AI remains available through:

```bash
recall --no-ai "your question"
```

## Development verification

Before installing a release build:

```bash
cargo check
cargo test
cargo install --path . --force
recall --help
```
