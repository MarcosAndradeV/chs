module main

import "c/libc" // normal import
import stdio "io" // alias import
import . "strings" // all import

import . "assert"

fn main() {
    assert_msg(is_empty(""), "strings.is_empty() without module prefix");
    stdio.print("stdio.print() with module alias\n");
    libc.exit(0) // normal module import
}
