module main

import "c/libc"

fn main() {
    if #feature(CHS_USE_GC) {
        libc.printf(cast(libc.cstring) "Feature CHS_USE_GC is enabled!\n".data);
    } else {
        libc.printf(cast(libc.cstring) "Feature CHS_USE_GC is disabled!\n".data);
    }
}
