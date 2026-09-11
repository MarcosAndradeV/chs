module mem

import "c/libc"

fn copy(dest: ^void, src: ^void, size: usize) {
    libc.memcpy(dest, src, size);
}

fn move(dest: ^void, src: ^void, size: usize) {
    libc.memmove(dest, src, size);
}

fn set(ptr: ^void, val: int, size: usize) {
    libc.memset(ptr, val, size);
}

fn compare(ptr1: ^void, ptr2: ^void, size: usize) -> int {
    return libc.memcmp(ptr1, ptr2, size);
}

fn clone(ptr: ^void, size: usize) -> ^void {
    var new_ptr = make([]u8, size).data;
    if new_ptr != null {
        libc.memcpy(new_ptr, ptr, size);
    };
    return new_ptr;
}

fn zero(ptr: ^void, size: usize) {
    set(ptr, 0, size);
}

fn contains_u8(slice: []u8, val: u8) -> bool {
    foreach elem in slice {
        if elem == val {
            return true;
        };
    };
    return false;
}

fn index_of_u8(slice: []u8, val: u8) -> int {
    var i = 0;
    foreach elem in slice {
        if elem == val {
            return i;
        };
        i = i + 1;
    };
    return -1;
}

fn reverse_bytes(slice: []u8) {
    if slice.len <= 1 {
        return;
    };
    var left = 0;
    var right = slice.len - 1;
    for left < right {
        var tmp = slice.data[left];
        slice.data[left] = slice.data[right];
        slice.data[right] = tmp;
        left = left + 1;
        right = right - 1;
    };
}
