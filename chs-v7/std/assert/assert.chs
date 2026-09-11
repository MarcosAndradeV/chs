module runtime

fn assert(cond: bool) {
    if !cond {
        chs_fatal_error("Assertion failed!");
    };
}

fn assert_msg(cond: bool, msg: string) {
    if !cond {
        chs_fatal_error(msg);
    };
}
