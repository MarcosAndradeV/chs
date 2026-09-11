module main

import "io"
import . "assert"

fn main() {
    var args = get_command_line_arguments();
    assert(args.len >= 1);
    assert(args[0].len > 0);

    io.print("Command line args test passed successfully!\n");
}
