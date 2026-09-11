module main

import "c/libc"
import stdio "io"
import . "assert"

fn test_file_type(f: ^libc.FILE) {
    assert(f == null);
}

fn main() {
    var f: ^libc.FILE = null;
    test_file_type(f);
    
    stdio.print("Namespaced types test passed successfully\n");
}
