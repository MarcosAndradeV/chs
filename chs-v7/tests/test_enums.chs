module main

import . "assert"
import "c/libc"

type Color enum : u8 {
    Red = 1,
    Green = 2,
    Blue = 3,
}

type Status enum {
    Pending = 0,
    Active = 10,
    Finished = 100,
}

fn color_to_int(c: Color) -> int {
    switch c {
        Color.Red -> return 1;
        Color.Green -> return 2;
        Color.Blue -> return 3;
        _ -> return 0;
    };
}

fn main() {
    var c: Color = Color.Green;
    assert(c == Color.Green);
    assert(color_to_int(c) == 2);

    var s: Status = Status.Active;
    assert(s == Status.Active);
    libc.exit(0);
}
