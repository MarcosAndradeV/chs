module main

import . "assert"

var calls: int = 0

fn next_index() -> int {
    calls += 1;
    return 0;
}

fn main() {
    var pointers: [dyn]^int = .{};
    var ptr = new(int);
    push(&pointers, ptr);

    drop(pointers[next_index()]);
    assert(calls == 1);

    // The element pointer was cleared by `drop`; this releases only the backing
    // dynamic-array allocation.
    drop(pointers);
}
