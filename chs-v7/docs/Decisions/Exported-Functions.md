# Decision: Exported Functions (`#export`)

**Date**: 2026-08-14  
**Status**: Implemented

## Context
Functions declared in CHS previously used internal module mangling (`chs_<module>_<name>`) or local QBE functions (`function`). To allow C callers or foreign shared libraries to invoke CHS functions using standard C ABI symbol names, an `#export` directive was introduced.

## Decision
1. **Syntax**:
   - Added `FunctionDirective::Export` variant in [[CHS Compiler Architecture]] (`syntax/src/ast.rs` & `syntax/src/lib.rs`).
   - Parsed `#export` directive on function signatures:
     `fn add_numbers(a: int, b: int) -> int #export`
     `fn multiply(a: int, b: int) -> int #export #link_name "custom_mult"`

2. **IR Representation**:
   - Added `is_export: bool` and `link_name: TokenSource` to `Function::Default` in `ir/src/function.rs`.
   - `symbol_name()` returns `link_name` (unmangled C symbol name) when `is_export` is `true`.
   - Updated dead code elimination (`opt::strip_unused_functions` in `ir/src/opt.rs`) to treat `#export` functions as roots so they are preserved even if not called within the module.

3. **Codegen Emission**:
   - Updated `transpile_function` in QBE codegen (`codegen/src/qbe.rs`) to emit `export function w $<symbol>(...)` for functions with `func.is_export() == true`.
   - QBE generates `.globl <symbol>` in target assembly, making the symbol visible to C linkers and dynamic loaders (`dlopen`/`dlsym`).

4. **Test Suite**:
   - Added [`tests/test_export.chs`](file:///home/marcos/Projects/chs-v7/tests/test_export.chs).
