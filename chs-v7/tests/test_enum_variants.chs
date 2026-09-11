module main

import . "assert"
import "c/libc"

type LocalEnum enum {
    A = 10,
    B = 20,
    C = 30,
}

fn check_local(e: LocalEnum) -> int {
    switch e {
        .A -> return 1;
        .B -> return 2;
        .C -> return 3;
        _ -> return 0;
    };
}

fn check_whence(w: libc.Whence) -> int {
    switch w {
        .SET -> return 100;
        .CUR -> return 200;
        .END -> return 300;
        _ -> return 0;
    };
}

fn main() {
    // 1. Module-qualified enum variant access
    var w: libc.Whence = libc.Whence.END;
    assert(w == libc.Whence.END);

    // 2. Implicit enum variant in local variable assignment
    var w2: libc.Whence = .CUR;
    assert(w2 == libc.Whence.CUR);

    // 3. Implicit enum variant comparison
    assert(w2 == .CUR);
    assert(w == .END);

    // 4. Passing implicit enum variants to function calls
    assert(check_local(.B) == 2);
    assert(check_local(.C) == 3);
    assert(check_whence(.SET) == 100);
    assert(check_whence(.END) == 300);

    // 5. Assignment update with implicit enum variant
    w = .SET;
    assert(w == .SET);

    libc.exit(0);
}
