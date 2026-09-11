module main

import "c/libc"

fn main() {
    if #feature(CHS_DEBUG_ALLOC) {
        libc.printf(cast(libc.cstring) "Feature CHS_DEBUG_ALLOC is enabled!\n".data);
    } else {
        libc.printf(cast(libc.cstring) "Feature CHS_DEBUG_ALLOC is disabled!\n".data);
    }
}
