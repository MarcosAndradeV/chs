module main

import "c/libc"
import "strings"
import . "assert"

fn test_drop_dyn_array() {
    var arr: [dyn]int = .{};
    push(&arr, 10, 20, 30);
    assert(arr.len == 3);
    drop(arr);
    assert(arr.len == 0);
    assert(arr.cap == 0);
    assert(arr.data == null);
}

fn test_drop_pointer() {
    var ptr: ^int = new(int);
    *ptr = 42;
    drop(ptr);
    assert(ptr == null);
}

fn test_drop_slice() {
    var s: []int = make([]int, 5);
    drop(s);
    assert(s.len == 0);
    assert(s.data == null);
}

fn test_drop_string() {
    var buffer = make([]u8, 13);
    var s: string = strings.from_slice(buffer);
    drop(s);
    assert(s.len == 0);
    assert(s.data == null);
}

fn main() {
    test_drop_dyn_array();
    test_drop_pointer();
    test_drop_slice();
    test_drop_string();
    libc.exit(0);
}
