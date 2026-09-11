module main

import "c/libc"
import . "assert"
import "fs"

fn main() {
    var test_filename = "temp_test_file_handle.txt";

    if fs.file_exists(test_filename) {
        fs.remove_file(test_filename);
    };
    assert(!fs.file_exists(test_filename));

    // 1. Test File Handle Open & Write
    var f, ok = fs.open(test_filename, "wb");
    assert(ok);
    assert(f.is_open);

    var written = f.write_str("Hello File Handle!\nLine 2");
    assert(written > 0);
    f.close();
    assert(!f.is_open);

    // 2. Test File Handle Open, Seek, Tell, Read All
    var f_read, read_ok = fs.open(test_filename, "rb");
    assert(read_ok);
    defer f_read.close();

    assert(f_read.tell() == 0);

    {
        var content = f_read.read_all();
        assert(content == "Hello File Handle!\nLine 2");
    };

    // 3. Test Convenience Functions (read_to_string, append_string)
    {
        var str, ok_str = fs.read_to_string(test_filename);
        assert(ok_str);
        assert(str == "Hello File Handle!\nLine 2");
    };

    assert(fs.append_string(test_filename, "\nLine 3"));

    {
        var str_appended, ok_app = fs.read_to_string(test_filename);
        defer drop(str_appended.data);
        assert(ok_app);
        assert(str_appended == "Hello File Handle!\nLine 2\nLine 3");
    };

    // 4. Cleanup
    assert(fs.remove_file(test_filename));
    assert(!fs.file_exists(test_filename));

    libc.exit(0);
}
