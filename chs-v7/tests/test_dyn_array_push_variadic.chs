module main

import "c/libc"
import . "assert"

fn main() {
    var arr: [dyn]int = .{};

    // Push 3 elements at once
    push(&arr, 10, 20, 30);
    assert(arr.len == 3);
    assert(arr[0] == 10);
    assert(arr[1] == 20);
    assert(arr[2] == 30);

    // Push 2 more elements
    push(&arr, 40, 50);
    assert(arr.len == 5);
    assert(arr[4] == 50);

    // Push 0 elements (no-op)
    push(&arr);
    assert(arr.len == 5);

    drop(arr);
    libc.exit(0);
}
