module main

import . "assert"
import "c/libc"
import "strings"

fn main() {
    var s1 = "hello";
    var s2 = "hello";
    var s3 = "world";

    assert(s1.len == 5);
    assert(s1 == s2);
    assert(!(s1 == s3));

    // 1. Character predicates
    assert(strings.is_digit('5'));
    assert(!strings.is_digit('A'));
    assert(strings.is_alpha('A'));
    assert(strings.is_whitespace(' '));
    assert(strings.is_upper('Z'));
    assert(strings.is_lower('z'));

    // 2. Case conversion
    assert(strings.to_lower_char('A') == 'a');
    assert(strings.to_upper_char('a') == 'A');

    // 3. Compare & Equals
    assert(strings.equals("abc", "abc"));
    assert(!strings.equals("abc", "def"));
    assert(strings.compare("abc", "def") < 0);
    assert(strings.compare("def", "abc") > 0);
    assert(strings.compare("abc", "abc") == 0);

    // 4. Index of & Last Index of
    assert(strings.index_of("foo bar foo", "foo") == 0);
    assert(strings.last_index_of("foo bar foo", "foo") == 8);

    // 5. Parse Int
    assert(strings.parse_int("12345") == 12345);
    assert(strings.parse_int("  -42 ") == -42);

    // 6. Split
    {
        var parts = strings.split("apple,banana,cherry", ",");
        defer drop(parts);
        assert(parts.len == 3);
        assert(strings.equals(parts[0], "apple"));
        assert(strings.equals(parts[1], "banana"));
        assert(strings.equals(parts[2], "cherry"));
    };

    libc.exit(0);
}
