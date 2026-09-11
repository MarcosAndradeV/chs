module main

import . "assert"
import "c/libc"
import "io"
import "path"
import "runtime"

fn main() {
    var p = path.from_string("/usr/local/bin/chs.exe");

    assert(p.is_absolute());
    assert(!p.is_relative());

    var filename = p.file_name();
    assert(filename == "chs.exe");

    var parent_dir = p.parent();
    assert(parent_dir == "/usr/local/bin");

    var ext_str = p.extension();
    assert(ext_str == ".exe");

    var rel_p = path.from_string("src/main.chs");
    assert(rel_p.is_relative());
    assert(!rel_p.is_empty());
    assert(rel_p.len() == 12);

    var joined_p = rel_p.join_path("utils.chs");
    assert(joined_p.string() == "src/main.chs/utils.chs");

    var mut_p = path.from_string("build");
    mut_p.append("output.log");
    assert(mut_p.string() == "build/output.log");

    io.puts("Path method tests passed successfully\n");
    libc.exit(0);
}
