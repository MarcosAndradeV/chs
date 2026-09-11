module main

import "io"
import "strings"

type ToString struct {
    data: ^void,
    to_string: fn(data: ^void) -> string,
}

fn make_to_string(
    data: ^void,
    to_string: fn(data: ^void) -> string,
) -> ToString {
    return ToString.{
        data: data,
        to_string: to_string,
    };
}

fn int_to_string(data: ^void) -> string {
    if *cast(^int)data == 10 {
        return "10";
    } else {
        return "0";
    };
}

fn main() {
    var d = 10;
    var x = make_to_string(&d, int_to_string);
    var s = x.to_string(x.data);
    io.print("%\n", s);
}
