# Decision: Foreign Global Variables (`#foreign`)

**Date**: 2026-08-14  
**Status**: Implemented

## Context
Standard C library headers (`std/c/libc/stdio.chs`) declare global C stream pointers (`stdin`, `stdout`, `stderr`). Previously, CHS only supported `#foreign` on function signatures. Foreign global variables required compiler support to prevent local memory allocations (`data $name = ...`) and bind external C library symbol references directly.

## Decision
1. **Syntax**:
   - Extended `FileItem::VarDecl` / `VarDeclStmt` in [[CHS Compiler Architecture]] parser (`syntax`) to accept `#foreign <Library>` and `#link_name "<Symbol>"` attributes on global variable declarations:
     `var stdin: ^FILE #foreign Libc`
   - Allowed optional initializers for foreign variables (`ExprKind::Null` default).

2. **Semantics**:
   - Added `is_foreign: bool` field to `VariableSignature` in [[Type Checking and Semantics]] (`types`).
   - `infer_global_var` validates that the target foreign library is declared and skips local initializer type inference.

3. **IR & Codegen**:
   - Added `is_foreign: bool` to `ir::Global`.
   - `emit_globals` in QBE codegen (`codegen`) skips emitting local `data` declarations for `g.is_foreign == true`, allowing QBE assembly to reference external C library symbols (`$stdin`, `$stdout`, `$stderr`) directly.

4. **Standard Library Integration**:
   - Enabled `stdin`, `stdout`, and `stderr` in [`std/c/libc/stdio.chs`](std/c/libc/stdio.chs).
   - Implemented `MB_CUR_MAX` wrapper function in [`std/c/libc/stdlib.chs`](std/c/libc/stdlib.chs).
   - Bound foreign runtime global `var chs_args: []string #foreign Runtime #link_name "chs_args"` in [`std/runtime/core.chs`](std/runtime/core.chs) and exposed `get_command_line_arguments() -> []string`.
   - Updated `pass2_check_bodies` in [[Type Checking and Semantics]] (`semantics`) to bypass initializer expression checks for foreign globals (`if decl.is_foreign { continue; }`).
