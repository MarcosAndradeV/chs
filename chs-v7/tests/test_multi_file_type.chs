module main

import "multifilemod"
import "c/libc"

fn main() {
    var x: multifilemod.MyType = cast(multifilemod.MyType) 42;
    multifilemod.foo(x);
    libc.exit(0);
}
