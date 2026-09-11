module main

import "c/libc"
import . "assert"

fn main() {
    var arr: [dyn]int = .{};

    // 1. Push a fixed-size array
    var fixed_arr: [3]int = .{10, 20, 30};
    push(&arr, fixed_arr);
    assert(arr.len == 3);
    assert(arr[0] == 10);
    assert(arr[1] == 20);
    assert(arr[2] == 30);

    // 2. Push a slice
    var sl: []int = []int.{10, 20, 30};
    push(&arr, sl);
    assert(arr.len == 6);
    assert(arr[3] == 10);
    assert(arr[4] == 20);
    assert(arr[5] == 30);

    // 3. Push another dynamic array
    var arr2: [dyn]int = .{};
    push(&arr2, 40, 50);
    push(&arr, arr2);
    assert(arr.len == 8);
    assert(arr[6] == 40);
    assert(arr[7] == 50);

    drop(arr);
    drop(arr2);
    libc.exit(0);
}
