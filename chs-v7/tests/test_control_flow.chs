module main

import . "assert"
import "c/libc"

fn sum_range(n: int) -> int {
    var total = 0;
    var i = 1;
    for i <= n {
        total = total + i;
        i = i + 1;
    };
    return total;
}

fn classify(val: int) -> int {
    if val < 0 {
        return -1;
    } else if val == 0 {
        return 0;
    } else {
        return 1;
    };
}

fn foreach_sum() -> int {
    var sum = 0;
    foreach x in .{1, 2, 3, 4} {
        sum += x;
    };
    return sum;
}

fn main() {
    assert(sum_range(10) == 55);
    assert(classify(-42) == -1);
    assert(classify(0) == 0);
    assert(classify(99) == 1);
    assert(foreach_sum() == 10);
    libc.exit(0);
}
