module main

import . "assert"
import "c/libc"
import "io"

fn main() {
    assert(1 == 1);
    assert(true);
    assert(2 + 2 == 4);
    io.puts("Math works!");
    libc.exit(0);
}
