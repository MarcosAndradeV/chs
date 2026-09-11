module main

import "c/libc"
import . "assert"

fn main() {
    var arr: [dyn]int = .{};
    assert(arr.data == null);
    assert(arr.len == 0);
    assert(arr.cap == 0);

    push(&arr, 10);
    push(&arr, 20);
    push(&arr, 30);
    push(&arr, 40);
    push(&arr, 50);

    assert(arr.len == 5);
    assert(arr[0] == 10);
    assert(arr[4] == 50);

    drop(arr);

    assert(arr.data == null);
    assert(arr.len == 0);
    assert(arr.cap == 0);

    libc.exit(0);
}
