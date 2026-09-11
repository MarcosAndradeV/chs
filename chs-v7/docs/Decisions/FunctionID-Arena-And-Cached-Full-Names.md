# Decision: Central `FunctionID` Arena in `TypeDatabase` & Cached Full Names

**Date**: 2026-08-16  
**Status**: Approved / Implemented  

## Context

Previously, `TypeDatabase` only maintained type structures (`TypeID -> Type`) while function signatures were scattered across `Module.functions` maps. Method names and mangled paths (e.g. `module.TypeName.method_name`) were repeatedly concatenated and formatted on-the-fly during semantic typechecking and IR translation. `FunctionID` values were created ad-hoc from per-module map lengths.

## Decision

1. **Central `functions` Arena in `TypeDatabase`**:
   - `TypeDatabase` owns a global arena `pub functions: Vec<FunctionSignature>`.
   - Function registration (`register_function`) assigns a unique, global [`FunctionID`](file:///home/marcos/Projects/chs-v7/types/src/lib.rs#L37) handle (`FunctionID(u32)`).

2. **Cached `full_name` in `FunctionSignature`**:
   - [`FunctionSignature`](file:///home/marcos/Projects/chs-v7/types/src/lib.rs#L220) holds fields `pub id: Option<FunctionID>` and `pub full_name: String`.
   - The fully qualified/mangled symbol name is pre-computed during signature resolution (Pass 1.5) and cached in `full_name`.
   - Semantic call resolution and IR translator use `sig.full_name` directly, eliminating on-the-fly path string construction.

3. **Struct and Enum Method Binding**:
   - Associated methods, constructors (`make`), and destructors (`drop`) on `Type::Struct` and `Type::Enum` map method names to the assigned global `FunctionID`.

---

## Related Notes & Obsidian Links

- **[[Type Checking and Semantics]]**: Signature resolution pass and `TypeDatabase` functions vector.
- **[[CHS Compiler Architecture]]**: Overall compiler workflow and signature gathering.
