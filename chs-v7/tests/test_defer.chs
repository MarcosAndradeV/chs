module main

import "runtime"
import "c/libc"

fn test_normal_defer() {
    libc.puts("test_normal_defer start".data);
    defer libc.puts("defer 1 (should be second)".data);
    defer libc.puts("defer 2 (should be first)".data);
    libc.puts("test_normal_defer end".data);
}

fn test_return_defer(x: int) {
    libc.puts("test_return_defer start".data);
    defer libc.puts("cleanup at function return".data);

    if x > 0 {
        libc.puts("returning early".data);
        return;
    };

    libc.puts("not returning early".data);
}

fn test_loop_defer() {
    libc.puts("test_loop_defer start".data);
    var i = 0;
    for i < 3 {
        var dummy = i;
        libc.puts("loop iteration start".data);
        defer libc.puts("loop iteration defer".data);

        if i == 1 {
            libc.puts("loop continue".data);
            i += 1;
            continue;
        };

        if i == 2 {
            libc.puts("loop break".data);
            break;
        };

        libc.puts("loop iteration end".data);
        i += 1;
    };
    libc.puts("test_loop_defer end".data);
}

fn main() {
    test_normal_defer();
    libc.puts("---".data);
    test_return_defer(1);
    libc.puts("---".data);
    test_return_defer(0);
    libc.puts("---".data);
    test_loop_defer();
}

// Expected output:

// test_normal_defer start
// test_normal_defer end
// defer 2 (should be first)
// defer 1 (should be second)
// ---
// test_return_defer start
// returning early
// cleanup at function return
// ---
// test_return_defer start
// not returning early
// cleanup at function return
// ---
// test_loop_defer start
// loop iteration start
// loop iteration end
// loop iteration defer
// loop iteration start
// loop continue
// loop iteration defer
// loop iteration start
// loop break
// loop iteration defer
// test_loop_defer end
