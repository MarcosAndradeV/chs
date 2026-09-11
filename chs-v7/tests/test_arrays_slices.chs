module main

import . "assert"
import "c/libc"

fn sum_slice(s: []int) -> int {
    var sum = 0;
    var i = 0;
    for i < s.len {
        sum = sum + s[i];
        i = i + 1;
    };
    return sum;
}

fn main() {
    var arr: [5]int = .{10, 20, 30, 40, 50};

    assert(arr[0] == 10);
    assert(arr[4] == 50);

    assert(sum_slice(arr) == 150);
    libc.exit(0);
}
