# invatigate string.{} bug

- STATUS: OPEN
- PRIORITY: 100

module main

fn main() {
    var test = string.{};
}

Works only if

type string struct {
    data: ^u8,
    len: int
}

is defined in core.chs
