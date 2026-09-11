---
tags: [compiler, semantics, modules, types, chs]
created: 2026-08-07
type: decision
---

# Multi-File Type Sharing Decision

We resolved an architectural issue where type declarations (such as type aliases, distinct types, structures, and enums) were not shared module-wide when a module consisted of multiple source files.

## Context & Problem

Previously, the compiler executed the first signature gathering pass (`check_signatures`) sequentially file-by-file. This meant:
1. File A would resolve its signatures and bodies before File B's type definitions were registered in the type database.
2. If File A referenced a type alias defined in File B, the compiler would encounter an unresolved type and fallback to register it as a dummy placeholder type, leading to subsequent semantic type errors.
3. This was especially apparent in multi-file standard library modules like `sys/libc`, where `stdio.chs` or `stdlib.chs` couldn't access common types (like `cstring` or `char`) declared in `libc.chs`.

## Resolution

We split the monolithic `check_signatures` pass into 7 distinct, public, modular phases inside `TypeChecker`:

1. `gather_imports` (Pass 1.0)
2. `gather_placeholders` (Pass 1.1)
3. `resolve_type_decls` (Pass 1.2)
4. `resolve_constants` (Pass 1.3)
5. `resolve_struct_enum_fields` (Pass 1.4)
6. `resolve_fn_signatures` (Pass 1.5)
7. `resolve_global_variables` (Pass 1.6)

In `compiler/src/lib.rs`, the driver now runs each pass globally across all compilation units and files sequentially. This ensures that every type is fully declared and alias definitions are completely resolved module-wide before function signatures, constants, or global variables are processed.

### Phase-Ordering Constraints

To allow enums to reference global constants in their default values and structs to use constants in array sizes, we resolved the dependencies by running constant evaluation (`resolve_constants`) *before* resolving structure/enum fields (`resolve_struct_enum_fields`).

### Unification & Coercion Fixes

We also corrected `coerce` in [[Type Checking and Semantics]]:
- Previously, direct type comparisons in `coerce` compared canonicalized type IDs against unresolved primitive aliases (like `int` and `usize`), causing coercion failures. We resolved aliases recursively using `resolve()` before comparison.
- We enabled `untyped_int` to coerce to any integer type (using `TypeDatabase::is_integer`).

## References

- [[Type Checking and Semantics]]
- [[CHS Compiler Architecture]]
