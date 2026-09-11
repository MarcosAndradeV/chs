// Not a real module
module builtin

// Builtin types
type void void
type int int
type u8 u8
type usize usize
type bool bool
type float float
type string string
type noreturn noreturn

// --- Builtin Functions & Operators ---
// Note: These are compiler-lowered builtin operators rather than standard runtime functions.

// Appends one or more elements to a dynamic array.
// If a slice, array, or dynamic array is passed as the second argument,
// its elements are copied into the target dynamic array.
fn push(da: ^[dyn]Type, elems: ...Type)

// Deallocates a value. For dynamic arrays, this frees the underlying buffer.
fn drop(x: Type)

// Removes and returns the last element of a dynamic array.
// Panics if the dynamic array is empty.
fn pop(da: ^[dyn]Type) -> Type

// Clears all elements of a dynamic array, resetting its length to 0.
// Does not release the capacity buffer.
fn clear(da: ^[dyn]Type)

// Heap-allocates a zero-initialized instance of type `T` and returns a pointer `^T`.
// Note: It takes a type literal as the argument.
fn new(T: Type) -> ^Type

// Heap-allocates a slice of type `[]T` containing `count` elements.
// Note: It takes a type literal as the first argument.
fn make(T: Type, count: usize) -> []Type

// --- Compile-time & Syntax Directives ---
// Note: These are compile-time compiler directives rather than functions or operators.
//
// Expression-Level Directives:
//
// #sizeof(T)
//   Evaluates to the compile-time constant size of type literal `T` in bytes.
//
// #alignof(T)
//   Evaluates to the compile-time constant alignment of type literal `T` in bytes.
//
// #type_info(T)
//   Returns a pointer to runtime type metadata for type literal `T`.
//
// #anycast(expr)
//   Wrap expr in a Any type.
//
// #anycast[expr1, expr2, ...]
//   Creates a []Any by wraping each expr in a Any type.
//
// #default
//   Creates a default/zero value.
//
// Declaration-Level Attributes:
//
// #distinct
//   Used in type declarations (e.g. type MyInt #distinct int) to define a distinct
//   new type rather than a type alias.
//
// #test
//   Applied to struct or enum declarations to flag them.
//
// #foreign <Lib>
//   Marks a function declaration as imported from a foreign/C library.
//
// #link_name "<symbol>"
//   Specifies the foreign linker name/symbol for a foreign function.
//
// #private
//   Restricts function/declaration visibility to the current module.
