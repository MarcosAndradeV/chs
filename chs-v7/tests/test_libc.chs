module main

import "c/libc"
import . "assert"
import "io"

fn main() {
    // Verify foreign global variable pointers are bound cleanly
    var in_ptr: ^libc.FILE = libc.stdin;
    var out_ptr: ^libc.FILE = libc.stdout;
    var err_ptr: ^libc.FILE = libc.stderr;

    assert(cast(rawptr) in_ptr != null);
    assert(cast(rawptr) out_ptr != null);
    assert(cast(rawptr) err_ptr != null);

    // Verify MB_CUR_MAX
    var cur_max: libc.size_t = libc.MB_CUR_MAX();
    assert(cur_max >= 1);

    io.puts("Libc foreign globals test passed successfully!");
}
