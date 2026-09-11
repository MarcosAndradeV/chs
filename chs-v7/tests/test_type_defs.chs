module main

import . "assert"
import "c/libc"

type UserId #distinct int
type IntAlias int

fn process_user(id: UserId) -> UserId {
    return id;
}

fn process_alias(val: IntAlias) -> int {
    return val + 1;
}

fn main() {
    var uid: UserId = cast(UserId) 42;
    var res: UserId = process_user(uid);
    assert(cast(int) res == 42);

    var a: IntAlias = 10;
    assert(process_alias(a) == 11);
    libc.exit(0);
}
