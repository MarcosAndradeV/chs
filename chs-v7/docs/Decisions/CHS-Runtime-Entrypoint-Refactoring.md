# Decision: CHS Runtime Entrypoint & Exported Functions Refactoring

**Date**: 2026-08-14  
**Status**: Implemented

## Context
Previously, QBE codegen (`codegen/src/qbe.rs`) generated a hardcoded `w $main()` wrapper (`emit_main_wrapper`) that called `$chs_main()`. This created a dependency on hardcoded compiler codegen logic for main entrypoint generation and required special handling during IR dead code elimination (`opt::strip_unused_functions`).

## Decision
1. **Entrypoint Definition in CHS**:
   - Refactored `main` entrypoint generation out of QBE codegen into standard CHS code in [`std/runtime/core.chs`](std/runtime/core.chs):
     ```chs
     fn main() #foreign Runtime #link_name "chs_main"

     fn entrypoint(argc: int, argv: ^^char) -> int #export #link_name "main" #private {
         main();
         return 0;
     }
     ```
   - Removed `emit_main_wrapper` from `QbeTranspiler` in `codegen/src/qbe.rs`.

2. **`#export` Directive Integration**:
   - `semantics/src/typecheck.rs` automatically adds `FunctionDirective::Export` to user's `fn main()`.
   - `ir/src/function.rs` generates C symbol names for exported functions:
     - Explicit `#link_name "custom"` -> `"custom"`
     - Exported `main` without explicit link name -> `"chs_main"`
     - Other exported functions -> `signature.name` (unmangled short C symbol)

3. **Unified Dead Code Elimination**:
   - Simplified `strip_unused_functions` in [[IR and Translation Architecture]] (`ir/src/opt.rs`) to eliminate the compiler `is_main: bool` flag.
   - Root reachability is determined strictly by `Function::Foreign` and `Function::Default { is_export: true, .. }`.

4. **C Runtime Decoupling Milestone (`-nostartfiles`)**:
   - By passing `-nostartfiles` to the linker driver in [`compiler/src/lib.rs`](compiler/src/lib.rs) and providing `_start` in [`std/runtime/lib/chs_runtime.c`](std/runtime/lib/chs_runtime.c), CHS executables bypass standard C toolchain startup objects (`crt1.o`, `crti.o`, `crtn.o`) and link dynamically against `libc.so`.
   - `_start` parses initial Linux ELF stack arguments (`argc`, `argv`, `stack_end`) and invokes `__libc_start_main` in `libc.so`, establishing a clean entrypoint while maintaining full `libc.so` runtime initialization.
