# Troubleshooting: Undefined Type Error Reporting

## Symptom
Referencing an un-declared or missing type name (e.g., `foo: Foo` in `type Test struct { foo: Foo }`) failed to trigger a semantic analysis type error (`type 'Foo' not found`). Instead, the compiler proceeded to QBE codegen, emitting `:Foo` in QBE SSA assembly and causing a low-level build failure:
`qbe: out.ssa: undefined type :Foo`

## Root Cause
When type names were mapped in `s::map_type_ext`, unknown type names inserted placeholder struct types (`t::Type::Struct { name: "Foo", fields: None, .. }`) into `TypeDatabase` to allow recursive pointer types and forward declarations. Valid struct declarations updated their fields to `fields: Some(...)` during `resolve_struct_enum_fields`. However, if a referenced type was never declared anywhere, it remained as `fields: None`. Prior to this fix, the type checker never validated if placeholder types remained `fields: None`.

## Fix
1. Introduced `check_type_defined` helper in [[Type Checking and Semantics]] (`semantics/src/typecheck.rs`) to verify that referenced type IDs do not resolve to `Type::Struct { fields: None, .. }`.
2. Validated type references across struct field definitions, function parameter/return types, global and local variable declarations, and cast targets.
3. Added missing primitive type aliases (`uint`, `ulong`, `ulonglong`, `double`) to [`std/c/libc/ffi.chs`](std/c/libc/ffi.chs).
4. Added test case [`tests/test_undefined_type.fail`](tests/test_undefined_type.fail).
