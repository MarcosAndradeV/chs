---
tags: [compiler, types, enums, namespacing]
created: 2026-08-07
type: decision
---

# Enum Access and Implicit Variants

We implemented qualified namespaced enum variant access (e.g., `libc.Whence.END`) and implicit enum variants (e.g., `.END`) to make CHS more ergonomic and modular.

## Context and Problem

1. **Qualified Enum Variant Access**: In modules, when an enum was defined (e.g., `Whence` in `libc`), expressions like `libc.Whence.END` did not resolve. The typechecker only checked for variables in the module (`lookup_var_in_module`), failing to identify `Whence` as an enum type within the module path.
2. **Implicit Enum Variants**: CHS lacked support for implicit enum variant syntax (e.g., `.END`), which is useful when the type of an expression can be inferred from context (like the expected parameter type of a function call, variable declarations, or comparisons).

## Solution and Design Decisions

### 1. Bidirectional Type Checking (Expected Type Propagation)
We introduced `expected_type: Option<TypeID>` to the `TypeChecker`. During semantic checking of expressions:
- **Variable Declarations & Thread-Locals**: If a type annotation is present, we propagate it as the expected type when type-checking the initializer.
- **Assignments**: The type of the LHS expression is inferred first and used as the expected type for the RHS.
- **Returns**: The return type of the function is propagated as the expected type.
- **Function Arguments**: The expected parameter type is resolved from the function signature and propagated when inferring each argument.
- **Binary Expressions**: If one side is implicit (e.g., `.CUR`), the other side is type-checked first, and its type is set as the expected type for the implicit side.
- **Initializer Lists**: Falling back to the expected type if `type_hint` is omitted (e.g. `.{ x = 1, y = 2 }`).

### 2. Implicit Member Expression Syntax
- The parser (`syntax/src/lib.rs`) intercepts `TokenKind::Dot` followed by `TokenKind::Identifier` (e.g., `.END`) and parses it as a `MemberExpr` with a dummy identifier `.` as its base object.
- The typechecker identifies the dummy `.` object. It resolves `expected_type` to its underlying enum layout, finds the matching variant, and returns the enum type. Crucially, the typechecker populates the `resolved_type` of the dummy object to ensure code generation/IR translation has access to the enum type info.

### 3. Namespace/Module Type Lookup
In `infer_expr` for `ExprKind::Member(mem)`, if the base evaluates to `self.type_db.module()` (i.e. `libc`), we query the module for types (`lookup_type_in_module`) if variable lookup returns `None`. This returns the enum type (e.g. `Whence`), allowing the outer member expression to evaluate the variant against the resolved enum type.

### 4. QBE IR Translation
In `ir/src/translate.rs`, we check if the base object is an enum type at the beginning of `translate_expr` for `ExprKind::Member(mem)`. If the object has type `Type::Enum`, we directly translate the variant access to its constant integer default value, preventing the compiler from attempting to emit code for the type path.

## References
- [[Type Checking and Semantics]]
- [[Syntax and AST]]
- [[CHS Compiler Architecture]]
