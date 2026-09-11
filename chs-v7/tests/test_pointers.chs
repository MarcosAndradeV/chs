module main

import . "assert"
import "c/libc"

fn swap(a: ^int, b: ^int) {
    var temp = *a;
    *a = *b;
    *b = temp;
}

fn main() {
    var x = 100;
    var y = 200;

    var px: ^int = &x;
    var py: ^int = &y;

    assert(*px == 100);
    assert(*py == 200);

    swap(&x, &y);

    assert(x == 200);
    assert(y == 100);
    libc.exit(0);
}
