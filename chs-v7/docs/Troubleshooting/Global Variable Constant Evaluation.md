---
tags: [compiler, ir, global-variables, troubleshooting, chs]
created: 2026-08-13
type: troubleshooting
---

# Troubleshooting: Global Variable Constant Expression Evaluation & Member Access Panic

## Problem & Symptom

Compiling source code with global variable initializers (such as `var ball: ^Ball = null` in `examples/pong.chs`) caused the compiler to panic during IR translation:

```
thread 'main' panicked at ir/src/translate.rs:738:18:
Expected structural type, got Void
```

## Root Cause

1. During Intermediate Representation (IR) translation in [`ir::translate::translate_ast_items`](ir/src/translate.rs), global variable initializers are evaluated via `eval_global_var` -> `eval_const_expr`.
2. The `eval_const_expr` function lacked pattern matching handlers for `ExprKind::Null(_)`, `ExprKind::Cast(_)`, `ExprKind::AutoCast(_)`, and empty `ExprKind::InitList(_)`.
3. Consequently, `eval_const_expr` returned `None` for expressions like `null`. `eval_global_var` printed a non-fatal error diagnostic and returned `None`.
4. As a result, `module.add_global` was skipped for the global variable (e.g. `ball`), leaving it unregistered in the IR `Module`.
5. Later, when translating function bodies containing member accesses on that variable (e.g. `ball.pos`), `lookup_var("ball")` failed, returning `(Operand::Int(0), Type::Void)`.
6. Member resolution (`translate_lvalue` and `get_member_offset`) received `Type::Void` as the object type, triggering an assertion panic at line 738 (`Expected structural type, got Void`).

## Resolution

1. Updated `eval_const_expr` (standalone) and `Translator::eval_const_expr` in [`ir/src/translate.rs`](ir/src/translate.rs):
   - Added support for `s::ExprKind::Null(_)` => `ConstVal::Zero`.
   - Added support for `s::ExprKind::Cast(_, expr, _)` => recursively evaluates `expr`.
   - Added support for `s::ExprKind::AutoCast(expr, _)` => recursively evaluates `expr`.
   - Added support for `s::ExprKind::InitList(_)` => `ConstVal::Zero`.
2. Verified global pointer initializations now cleanly generate zero-initialized QBE data definitions (`z 8`).

## References

- [[CHS Compiler Architecture]]
- [[IR and Optimizations]]
- [[Type Checking and Semantics]]
- [[QBE Code Generation]]
