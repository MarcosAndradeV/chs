// WARN: MEMORY LEAK
module main

import . "assert"
import "strings"
import "path"
import "mem"

fn main() {
    // 1. StringBuilder
    var sb = make(strings.StringBuilder);
    sb.write_string("Hello");
    sb.write_char(' ');
    sb.write_string("World");
    var res_str = sb.to_string();
    assert(res_str == "Hello World");
    drop(sb);

    // 2. String transformations & searching
    assert(strings.to_lower("Hello WORLD") == "hello world");
    assert(strings.to_upper("hello world") == "HELLO WORLD");
    assert(strings.repeat("ab", 3) == "ababab");

    var parts: [3]string = [3]string.{ "one", "two", "three" };
    var joined = strings.join(parts, ", ");
    assert(joined == "one, two, three");

    assert(strings.count("banana", "an") == 2);
    assert(strings.count("aaaa", "aa") == 2);

    // 3. ASCII & Character Classifiers
    assert(strings.is_hex_digit('a'));
    assert(strings.is_hex_digit('F'));
    assert(strings.is_hex_digit('9'));
    assert(!strings.is_hex_digit('z'));

    assert(strings.is_ascii('A'));
    assert(strings.is_printable(' '));
    assert(strings.is_punct('!'));
    assert(!strings.is_punct('A'));

    // 4. Path Utilities
    assert(path.base("/usr/local/bin/chs") == "chs");
    assert(path.base("file.chs") == "file.chs");
    assert(path.dir("/usr/local/bin/chs") == "/usr/local/bin");
    assert(path.ext("/path/to/file.chs") == ".chs");
    assert(path.ext("README") == "");
    assert(path.join("/usr/local", "bin") == "/usr/local/bin");
    assert(path.join("/usr/local/", "/bin") == "/usr/local/bin");

    // 5. Memory & Slice Helpers
    var arr: [4]u8 = [4]u8.{ 'a', 'b', 'c', 'd' };
    assert(mem.contains_u8(arr, 'c'));
    assert(!mem.contains_u8(arr, 'z'));
    assert(mem.index_of_u8(arr, 'c') == 2);
    assert(mem.index_of_u8(arr, 'z') == -1);

    mem.reverse_bytes(arr);
    assert(arr[0] == 'd');
    assert(arr[3] == 'a');

    mem.zero(cast(^void) &arr, 4);
    assert(arr[0] == 0);
    assert(arr[3] == 0);
}
