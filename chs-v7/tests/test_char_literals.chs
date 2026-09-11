module main

import . "assert"

fn main() {
    // 1. Basic Character Literals
    var a: int = 'a';
    assert(a == 97);

    var b: u8 = 'B';
    assert(b == 66);

    var zero: i8 = '0';
    assert(zero == 48);

    // 2. Escape Sequences
    var newline: u8 = '\n';
    assert(newline == 10);

    var tab: u8 = '\t';
    assert(tab == 9);

    var backslash: u8 = '\\';
    assert(backslash == 92);

    var quote: u8 = '\'';
    assert(quote == 39);

    // 3. UTF-8 Character Literal
    var unicode_char: int = '⚡'; // U+26A1 = 9889
    assert(unicode_char == 9889);

    // 4. Operations with Character Literals
    assert('b' - 'a' == 1);
    assert('a' + 1 == 'b');
}
