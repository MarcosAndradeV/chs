module main

import "c/libc"
import . "assert"

fn main() {
    var arr: [dyn]int = .{};

    push(&arr, 10);
    push(&arr, 20);
    push(&arr, 30);

    assert(arr.len == 3);

    var popped = pop(&arr);
    assert(popped == 30);
    assert(arr.len == 2);

    popped = pop(&arr);
    assert(popped == 20);
    assert(arr.len == 1);

    clear(&arr);
    assert(arr.len == 0);
    assert(arr.cap >= 2); // capacity is retained

    drop(arr);
    libc.exit(0);
}
