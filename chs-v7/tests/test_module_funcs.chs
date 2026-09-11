module main
import "c/libc"

fn normal_function(x: int) -> int {
    return x + 1;
}

type fptr fn(x: int) -> int

fn test_function_ptr(x: int) -> int {
    return x + 1;
}

fn main() {
    var f: fptr = test_function_ptr;
    if f(10) == normal_function(10) {
        libc.exit(0);
    } else {
        libc.exit(1);
    }
}
