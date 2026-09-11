# Decision: Builtin `rawptr` Type & Syntax Parser Enhancements

**Date**: 2026-08-14  
**Status**: Implemented

## Context
1. **Raw Pointer Primitive**: In C FFI and low-level memory allocation routines, `rawptr` represents untyped pointers (`^void` / `void*`).
2. **Float Constant Inference**: Floating-point `const` declarations without explicit type annotations (e.g., `const PI = 3.14159`) needed automatic type mapping to `untyped_float`.
3. **Trailing Commas in Struct Fields**: Struct field declarations in `type S struct { a: int, b: int, }` should permit optional trailing commas.
4. **C FFI Type Aliases**: Additional C primitive type aliases (`uint`, `ulong`, `ulonglong`, `double`) were required for complete standard C library bindings in [`std/c/libc/ffi.chs`](std/c/libc/ffi.chs).

## Decision
1. **Builtin `rawptr` Registration**:
   - Added `RAWPTR_ID` (`TypeID(17)`) in [[Type Checking and Semantics]] (`types/src/lib.rs`) pointing to `Type::Pointer(VOID_ID)`.
   - Registered `"rawptr"` in `lookup_builtin_type` mapping to `RAWPTR_ID`.

2. **Float Constant Type Inference**:
   - Updated `parse_const_declaration` in [[CHS Compiler Architecture]] (`syntax/src/lib.rs`) to automatically assign scalar type `untyped_float` when a `const` item initializes with `ExprKind::Float(_)`.

3. **Struct Parser Trailing Commas**:
   - Updated `parse_struct_declaration` in `syntax/src/lib.rs` to use `parse_maybe_comma` for struct field separation, permitting optional trailing commas.

4. **C Primitive Aliases in `libc`**:
   - Added `uint` (`u32`), `ulong` (`u64`), `ulonglong` (`u64`), and `double` (`f64`) to [`std/c/libc/ffi.chs`](std/c/libc/ffi.chs).
