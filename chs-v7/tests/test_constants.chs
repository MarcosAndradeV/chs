module main

import . "assert"
import "c/libc"

const SIZE: int = 5
const DOUBLE_SIZE: int = SIZE * 2
const MSG: string = "Hello, constants!"
const VERBOSE: bool = true

type Buffer struct {
    data: [SIZE]int,
    len: int,
}

fn main() {
    // 1. Check compile-time global constants values
    assert(SIZE == 5);
    assert(DOUBLE_SIZE == 10);
    assert(VERBOSE);

    // 2. Check local constants
    const LOCAL_VAL: int = 15;
    const LOCAL_EXPR: int = LOCAL_VAL + 5;
    assert(LOCAL_VAL == 15);
    assert(LOCAL_EXPR == 20);

    // 3. Constant-sized array declaration and manipulation
    var arr: [SIZE]int = #default;
    arr[0] = 42;
    arr[SIZE - 1] = 99;
    assert(arr[0] == 42);
    assert(arr[4] == 99);

    // 4. Struct with constant-sized array
    var buf: Buffer = #default;
    buf.len = SIZE;
    buf.data[0] = 100;
    assert(buf.len == 5);
    assert(buf.data[0] == 100);

    libc.exit(0);
}
