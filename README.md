# Recall

**Recall is a local-first personal memory system for your terminal.**

It stores things you tell it, retrieves relevant memories later, and can optionally use a local AI model to turn those memories into an answer.

Everything is designed to run locally — your memories stay on your machine.

## Features

* **Store memories from the terminal**

  ```bash
  recall store "I prefer SQLite for small local applications."
  ```

* **Store text files**

  ```bash
  recall store ./notes.txt
  ```

* **Ask questions about your memories**

  ```bash
  recall "What database do I prefer?"
  ```

* **AI-powered answers**

  Recall can use a locally running Ollama model to generate answers from the memories it retrieves.

* **Retrieval without AI**

  If you just want to see what Recall finds:

  ```bash
  recall --no-ai "What did I say about SQLite?"
  ```

* **Hybrid memory retrieval**

  Recall combines traditional text search with semantic retrieval to find memories that are relevant even when the wording differs.

* **Background processing**

  Memory embeddings are generated in the background, so storing a memory does not have to wait for AI processing.

* **Local and private**

  The database, search index, and embeddings are stored locally. AI inference is performed through your local Ollama instance.

* **Simple daemon management**

  ```bash
  recall start
  recall status
  recall stop
  ```

* **Works as an installed command**

  Once installed, `recall` can be used normally from your terminal without running it from the project directory.

## Installation

With Rust and Cargo installed:

```bash
cargo install --path .
```

Then:

```bash
recall --help
```

Start Recall:

```bash
recall start
```

## Ollama

AI-powered questions require a local Ollama installation and a model.

For example:

```bash
ollama pull llama3.2
```

Then:

```bash
recall "What do I remember about my project?"
```

You can also use Recall without AI:

```bash
recall --no-ai "What do I remember about my project?"
```

## Data location

Recall keeps its local runtime data outside the project directory, normally under:

```text
~/.local/share/recall/
```

This includes the local database and daemon socket.

## Project status

Recall is a personal project focused on building a useful, local-first memory system with a clean separation between memory storage, retrieval, AI generation, and the command-line interface.

---

## Disclaimer

**This is a personal project and is completely AI-assisted.**

The implementation, architecture, debugging, documentation, and development process were carried out with substantial assistance from AI. This project is not presented as a commercial product or as independently authored software.
