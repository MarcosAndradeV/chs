---
tags: [compiler, testing, development, workflow, chs]
created: 2026-08-07
type: documentation
---

# Testing and Development Workflow

This document details the testing infrastructure and development configurations for the CHS compiler repository.

## Development Mode Feature (`-Fdevelopment`)

The compiler provides a Cargo feature named **`development`**. When building or running tests during local development, compile with `-Fdevelopment`:

```bash
cargo build -Fdevelopment
cargo run -Fdevelopment -- run tests/test_char_literals.chs
cargo test -Fdevelopment
```

### Purpose & Behavior
- **Standard Library Resolution**: In release mode or when `development` is disabled, the compiler looks for standard library files in `$CHS_HOME/std`.
- **Local Fallback**: When `-Fdevelopment` is enabled, `compiler::get_std_path()` returns `./std` relative to the current working directory. This enables running and testing without configuring environment variables.

---

## Automated Test Suite (`runtest.py`)

The primary test runner for CHS is [`runtest.py`](runtest.py), located at the root of the workspace.

### Usage
Run the test suite using Python 3:

```bash
python3 runtest.py [FLAGS]
```

### CLI Flags

| Flag | Long Flag | Description |
| :--- | :--- | :--- |
| **`-d`** | `--debug` | Enables `CHS_DEBUG_ALLOC=1` in the environment to check runtime memory leak assertions. |
| **`-p`** | `--pause` | Pauses execution whenever a test fails, prompting for input before continuing. |

### Execution Pipeline
When `runtest.py` is executed, it performs the following sequential steps:

1. **Pre-build Compiler**:
   Executes `cargo build -q -Fdevelopment` to ensure the debug binary `target/debug/chs` is compiled up-to-date with local standard library paths.
2. **Execute Rust Unit Tests**:
   Runs `cargo test -q --no-fail-fast --all` across all workspace crates (`syntax`, `semantics`, `types`, `ir`, `codegen`, `compiler`, `cli`).
3. **Execute Standard Tests (`tests/*.chs`)**:
   Runs `target/debug/chs run tests/<name>.chs` for each `.chs` file in the `tests/` directory.
   - Expects returncode `0` (**PASS**).
   - If returncode is non-zero, logs **FAIL**.
   - If `--debug` is active, checks stderr for `"MEMORY LEAK DETECTED:"`.
4. **Execute Negative/Failure Tests (`tests/*.fail`)**:
   Runs `target/debug/chs run tests/<name>.fail` for each `.fail` file in `tests/`.
   - Expects non-zero returncode (**EXPECTED FAIL**).
   - If returncode is `0`, logs **UNEXPECTED PASS**.
5. **Summary Reporting**:
   Outputs total passed, failed, expected failed, and unexpected passed counts alongside any detected memory leaks.

---

## Related Notes

- **[[CHS Compiler Architecture]]**: Central compiler architecture overview.
- **[[Syntax and AST]]**: Abstract syntax tree & parsing.
- **[[Type Checking and Semantics]]**: Type checker and memory safety validations.
