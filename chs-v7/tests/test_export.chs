module main

fn add_numbers(a: int, b: int) -> int #export {
    return a + b;
}

fn multiply_numbers(a: int, b: int) -> int #export #link_name "custom_multiply" {
    return a * b;
}

fn main() {
    var sum: int = add_numbers(15, 27);
    assert(sum == 42);

    var prod: int = multiply_numbers(6, 7);
    assert(prod == 42);

    io.print("Exported functions test passed successfully\n");
}

import "io"
import . "assert"
