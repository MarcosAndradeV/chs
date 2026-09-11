module multifilemod

import "c/libc"

fn foo(val: MyType) {
    libc.printf(cast(libc.cstring) "Value: %d\n".data, cast(int) val);
}
