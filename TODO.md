- [ ] Update parser to support consts like `const 1 : int = STDOUT`

- [ ] Update operations to struct with token field for error reporting

- [ ] Update vmv2 to support functions and let-bindings

- [ ] Improve typecheck errors

- [X] Add typecheck

- [X] (Testing yet) New syntax pattern `<Keyword> <Value> : <Types> = <Word>`

```haskell
let 10 : int = foo

const 1 : int = STDOUT

type : int option list = FOO

struct x y :
    int
    int
= Vector2

fn a b : int int -> int = add {
  a b +
}

```

- [X] `&n` copy relative value from stack

```haskell
1 2 &1 -- 1 2 2
3 4 &2 -- 3 4 3
```
