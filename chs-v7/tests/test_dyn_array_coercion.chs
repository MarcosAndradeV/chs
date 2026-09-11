module main

import "c/libc"
import . "assert"

fn sum_slice(s: []int) -> int {
    var total = 0;
    var i = 0;
    for i < s.len {
        total = total + s[i];
        i = i + 1;
    };
    return total;
}

fn main() {
    var arr: [dyn]int = .{};
    push(&arr, 10, 20, 30);

    // Coerce dyn array to slice
    var total = sum_slice(arr);
    assert(total == 60);

    drop(arr);
    libc.exit(0);
}
