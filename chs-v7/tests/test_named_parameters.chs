module main

import . "assert"
import "c/libc"

fn calc(a: int, b: int, c: int) -> int {
    return (a - b) * c;
}

fn main() {
    // 1. All positional
    var res1 = calc(10, 4, 3);
    assert(res1 == 18);

    // 2. All named in order
    var res2 = calc(a: 10, b: 4, c: 3);
    assert(res2 == 18);

    // 3. All named out of order
    var res3 = calc(c: 3, a: 10, b: 4);
    assert(res3 == 18);

    // 4. Mixed (some positional, some named)
    var res4 = calc(10, c: 3, b: 4);
    assert(res4 == 18);

    libc.exit(0);
}
