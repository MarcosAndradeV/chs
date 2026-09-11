module main

import . "assert"

fn main() {
    var values = make([]int, 0);
    assert(values.len == 0);
    assert(values.data == null);
    drop(values);
}
