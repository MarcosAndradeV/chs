# Decision: Universal `make` Constructor and `drop` Destructor System

**Date**: 2026-08-16  
**Status**: Approved / Implemented  

## Overview & Context

Previously, CHS used distinct primitives for memory allocation (`new(T)` for single heap instances, `make(T, count)` for slices and dynamic arrays) and limited `drop(x)` to built-in dynamic memory types (`^T`, `[]T`, `[dyn]T`, `string`). 

To establish a uniform, predictable memory management model across primitive types, aggregate structs, and user-defined objects, CHS unifies all constructor operations under `make` and all destructor/deallocation operations under `drop`.

---

## Key Architectural Decisions

### 1. Unified Constructor Syntax (`make`)
- **Deprecate `new(T)`**: `new(T)` is absorbed into `make`.
- **Heap Allocation of Single Items**: `make(T)` (or `make(^T)`) heap-allocates a zero-initialized instance of type `T` and returns a pointer `^T`.
- **Slices & Dynamic Arrays**:
  - `make([]T, n)` allocates a heap slice of `n` elements of type `T`.
  - `make([dyn]T, n)` allocates a dynamic array with initial capacity `n`.
- **Custom Struct Constructors**:
  - Defined via method naming convention: `fn TypeName.make(args...) -> TypeName`.
  - Calling `make(TypeName, args...)` returns a value of type `TypeName` (stack/value construction).
  - Calling `make(^TypeName, args...)` heap-allocates memory for `TypeName`, executes `TypeName.make(args...)`, and returns pointer `^TypeName`.

### 2. Universal Destructor System (`drop`)
- **Custom Destructor Convention**: Defined as `fn (self: ^TypeName) drop()` (or `fn (self: TypeName) drop()`).
- **Multi-Stage Teardown Process**:
  1. **Custom Destructor**: Calls `TypeName.drop(&x)` if defined on the type.
  2. **Field Teardown**: Recursively drops any droppable fields inside `x` (strings, dynamic arrays, slices, pointers, or nested structs with destructors).
  3. **Heap Deallocation**: If `x` is a heap pointer (`^TypeName`), frees the underlying heap memory block.
- **Stack & Heap Compatibility**: `drop(x)` can be explicitly called on stack-backed variables containing droppable resources or custom destructors. It executes stages 1 & 2 without freeing stack memory.

### 3. Automatic Scope Drop (RAII) & Ownership Move Semantics
- **Automatic Scope Cleanup**: At block scope exit, the compiler automatically inserts `drop` calls for all remaining owned variables in scope that have not been explicitly dropped or moved.
- **Move Semantics**: Assigning an owned variable (`y = x`) or passing it by value to a function (`foo(x)`) transfers ownership to `y` or `foo`.
- **Static Safety Checks**: Variables whose ownership has been moved are marked unowned in static analysis (`owned_vars`). Accessing or re-dropping a moved variable triggers a compile-time error. POD/Primitive types (integers, floats, booleans, rawptr) retain copy semantics.

---

## Related Notes & Obsidian Links

- **[[Memory Management and Drop Safety]]**: Static analysis checks for ownership, multi-stage drop execution, and RAII scope drop.
- **[[Standard Library]]**: Updated builtin declarations for `make` and `drop`.
- **[[Type Checking and Semantics]]**: Constructor and destructor symbol association in `TypeDatabase`.
- **[[Method Implementation Integration]]**: Method lookup conventions for `TypeName.make` and `TypeName.drop`.
