# Decision: 32-Bit Floating-Point Variant (`f32`) & `Float(FloatBitSize)`

**Date**: 2026-08-12  
**Status**: Implemented

## Context
Prior to this decision, CHS only possessed a single 64-bit float primitive (`Type::Float`). To mirror the design of [[Type Checking and Semantics|integer types]] (`Integer(Sign, BitSize)`), a 32-bit floating-point type (`f32`) was needed.

## Decision
1. **Internal Representation**:
   - Refactored `Type::Float` into `Type::Float(FloatBitSize)` where `FloatBitSize` has variants `_32` and `_64`.
   - Introduced `Type::UntypedFloat` for literal inference and auto-coercion.
2. **Primitives & Aliases**:
   - `f32`: 32-bit single-precision IEEE 754 float (`Float(FloatBitSize::_32)`).
   - `f64`: 64-bit double-precision IEEE 754 float (`Float(FloatBitSize::_64)`).
   - `float`: Standard alias to `f64` (`FLOAT_ID = F64_ID`) for full backward compatibility.
3. **QBE Codegen Mapping**:
   - `f32` maps to QBE base type `s` (single precision). Loads use `loads`, stores use `stores`, negation uses `=s sub s_0.0, %op`, comparisons use suffix `s`.
   - `f64` maps to QBE base type `d` (double precision).
   - Conversions between integers (`w`, `l`), single (`s`), and double (`d`) utilize QBE instructions: `swtof`, `sltof`, `swtod`, `sltod`, `stosi`, `stosl`, `dtosi`, `dtosl`, `exts`, and `truncd`.
   - **Literal Formatting**: Float constants passed to `f32` parameters, return values, or single-precision operations are formatted with `op_with_base(op, "s")` producing `s_<val>` (e.g. `s_50.0`), preventing ABI type mismatch errors with C foreign functions expecting 32-bit floats.
   - **Constant Inference**: Added `untyped_float` type resolution in [[CHS Compiler Architecture]] (`syntax/src/lib.rs`) for `const` float literal declarations without explicit type annotations.


