# Method Implementation Integration

## Overview
Method implementations (`FileItem::MethodImplementation`) allow defining functions attached to structs or enums with an explicit receiver syntax, e.g., `fn (self: Point) sum() -> int` or `fn (self: ^Point) add_x(dx: int)`.

This document records the architectural design, semantics, and IR translation for methods in the CHS compiler, referencing [[CHS Compiler Architecture]] and [[Type Checking and Semantics]].

## Key Architectural Decisions

1. **Type Database Function Association**:
   - `Type::Struct` and `Type::Enum` in [`types`](types/src/lib.rs) hold `functions: HashMap<TokenSource, FunctionID>`, `constructor: Option<FunctionID>`, and `destructor: Option<FunctionID>`.
   - Method signatures are registered in `module.functions` with key `ReceiverName.method_name` (e.g. `Point.add_x`), while mangled symbols are formatted as `module_name.ReceiverName.method_name`.

2. **Pointer Receiver Auto-Referencing / Auto-Dereferencing**:
   - When calling an instance method `p.method(...)`:
     - If `method` expects a pointer receiver `self: ^T` but `p` is a value of type `T`, semantics auto-references the call and IR translation evaluates `translate_lvalue(&p)` to pass `p`'s memory address directly without copying.
     - If `method` expects a value receiver `self: T` but `p` is a pointer `^T`, semantics and IR handle dereferencing.

3. **Method Resolution in Callee & Member Expressions**:
   - Member lookup `ExprKind::Member` falls back to method signature lookups on receiver struct/enum types when a property name is not a struct field or enum variant.
   - Non-variadic method calls respect `param_offset = 1` for `self` in arity and argument index mapping.

4. **Cross-Module & Imported Type Method Resolution**:
   - Helper `lookup_method` searches for method signatures across:
     - The current module's `functions` map.
     - Imported modules (both direct and aliased imports like `import stdio "io"`).
     - All modules registered in `TypeDatabase`.
   - Resolves qualified `def_module.ReceiverName.method_name` symbol names for instance calls, value/pointer receivers, and static type method calls (`b.Point.sum(p)`).
   - Qualified type name lookups (`mod.Type`) are handled in `TypeDatabase::lookup_type_in_module` and `lookup_var_in_module`.

## Verification
- Clean compilation of all Rust unit test suites (`cargo test`).
- Full integration test suite passing with 35 tests (`python3 runtest.py`), including [`tests/test_methods.chs`](tests/test_methods.chs) and [`tests/test_imported_methods.chs`](tests/test_imported_methods.chs).
