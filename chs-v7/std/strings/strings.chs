module strings

import "c/libc"
import "mem"

// --- String Inspection & Search ---

fn is_empty(s: string) -> bool {
    return s.len == 0;
}

fn substring(s: string, start: usize, len: usize) -> string {
    if start + len > s.len {
        return "";
    };
    s.data = s.data + start;
    s.len = len;
    return s;
}

fn starts_with(s: string, prefix: string) -> bool {
    if s.len < prefix.len {
        return false;
    };
    var i = 0;
    for i < prefix.len {
        if s.data[i] != prefix.data[i] {
            return false;
        };
        i = i + 1;
    };
    return true;
}

fn ends_with(s: string, suffix: string) -> bool {
    if s.len < suffix.len {
        return false;
    };
    var offset = s.len - suffix.len;
    var i = 0;
    for i < suffix.len {
        if s.data[offset + i] != suffix.data[i] {
            return false;
        };
        i = i + 1;
    };
    return true;
}

fn index_of(s: string, substr: string) -> int {
    if substr.len == 0 {
        return 0;
    };
    if s.len < substr.len {
        return -1;
    };
    var i = 0;
    var limit = s.len - substr.len;
    for i <= limit {
        var match = true;
        var j = 0;
        for j < substr.len {
            if s.data[i + j] != substr.data[j] {
                match = false;
                break;
            };
            j = j + 1;
        };
        if match {
            return i;
        };
        i = i + 1;
    };
    return -1;
}

fn last_index_of(s: string, substr: string) -> int {
    if substr.len == 0 {
        return cast(int) s.len;
    };
    if s.len < substr.len {
        return -1;
    };
    var i = cast(int) (s.len - substr.len);
    for i >= 0 {
        var match = true;
        var j = 0;
        for j < substr.len {
            if s.data[i + j] != substr.data[j] {
                match = false;
                break;
            };
            j = j + 1;
        };
        if match {
            return i;
        };
        i = i - 1;
    };
    return -1;
}

fn contains(s: string, substr: string) -> bool {
    return index_of(s, substr) != -1;
}

fn equals(s1: string, s2: string) -> bool {
    if s1.len != s2.len {
        return false;
    };
    if s1.len == 0 {
        return true;
    };
    return libc.memcmp(cast(^void) s1.data, cast(^void) s2.data, s1.len) == 0;
}

fn compare(s1: string, s2: string) -> int {
    var min_len = s1.len;
    if s2.len < min_len {
        min_len = s2.len;
    };
    if min_len > 0 {
        var cmp = libc.memcmp(cast(^void) s1.data, cast(^void) s2.data, min_len);
        if cmp != 0 {
            return cmp;
        };
    };
    if s1.len < s2.len {
        return -1;
    } else if s1.len > s2.len {
        return 1;
    };
    return 0;
}

fn trim_left(s: string) -> string {
    var start = 0;
    for start < s.len {
        var c = s.data[start];
        if !is_whitespace(c) {
            break;
        };
        start = start + 1;
    };
    return substring(s, start, s.len - start);
}

fn trim_right(s: string) -> string {
    var end = s.len;
    for end > 0 {
        var c = s.data[end - 1];
        if !is_whitespace(c) {
            break;
        };
        end = end - 1;
    };
    return substring(s, 0, end);
}

fn trim(s: string) -> string {
    return trim_right(trim_left(s));
}

fn from_slice(s: []u8) -> string #owned_return {
    return *cast(^string)cast(^void)&s;
}

fn from_raw_parts(data: ^u8, len: usize) -> string #owned_return {
    var s: string = #default;
    s.data = data;
    s.len  = len;
    return s;
}

// --- StringBuilder ---

type StringBuilder struct {
    buf: [dyn]u8,
}

// constructor
fn (b: StringBuilder) make() -> StringBuilder #owned_return {
    var b: StringBuilder = #default;
    return b;
}

fn (b: ^StringBuilder) write_char(c: u8) {
    push(&b.buf, c);
}

fn (b: ^StringBuilder) write_string(s: string) {
    foreach c in s {
        push(&b.buf, c);
    }
}

fn (b: ^StringBuilder) to_string() -> string #owned_return {
    return from_raw_parts(b.buf.data, b.buf.len);
}

// destructor
fn (b: StringBuilder) drop() {
    drop(b.buf);
}

// --- Extended String Transformations ---

fn to_lower(s: string) -> string #owned_return {
    var b = make(StringBuilder);
    foreach c in s {
        b.write_char(to_lower_char(c));
    };
    return b.to_string();
}

fn to_upper(s: string) -> string #owned_return {
    var b = make(StringBuilder);
    foreach c in s {
        b.write_char(to_upper_char(c));
    };
    return b.to_string();
}

fn repeat(s: string, count: int) -> string #owned_return {
    if count <= 0 || s.len == 0 {
        return "";
    };
    var b = make(StringBuilder);
    var i = 0;
    for i < count {
        b.write_string(s);
        i = i + 1;
    };
    return b.to_string();
}

fn join(elems: []string, sep: string) -> string #owned_return {
    if elems.len == 0 {
        return "";
    };
    var b = make(StringBuilder);
    var idx = 0;
    foreach elem in elems {
        if idx > 0 {
            b.write_string(sep);
        };
        b.write_string(elem);
        idx = idx + 1;
    };
    return b.to_string();
}

fn count(s: string, substr: string) -> int {
    if s.len == 0 || substr.len == 0 || s.len < substr.len {
        return 0;
    };
    var n = 0;
    var limit = cast(int) (s.len - substr.len);
    var idx = 0;
    for idx <= limit {
        var sub = substring(s, cast(usize) idx, s.len - cast(usize) idx);
        var found = index_of(sub, substr);
        if found == -1 {
            break;
        };
        n = n + 1;
        idx = idx + found + cast(int) substr.len;
    };
    return n;
}

// --- Character Utilities ---

fn is_digit(c: u8) -> bool {
    return c >= '0' && c <= '9';
}

fn is_alpha(c: u8) -> bool {
    return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z');
}

fn is_alnum(c: u8) -> bool {
    return is_alpha(c) || is_digit(c);
}

fn is_whitespace(c: u8) -> bool {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r';
}

fn is_upper(c: u8) -> bool {
    return c >= 'A' && c <= 'Z';
}

fn is_lower(c: u8) -> bool {
    return c >= 'a' && c <= 'z';
}

fn to_lower_char(c: u8) -> u8 {
    if is_upper(c) {
        return c + 32;
    };
    return c;
}

fn to_upper_char(c: u8) -> u8 {
    if is_lower(c) {
        return c - 32;
    };
    return c;
}

fn is_hex_digit(c: u8) -> bool {
    return is_digit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
}

fn is_ascii(c: u8) -> bool {
    return c <= 127;
}

fn is_printable(c: u8) -> bool {
    return c >= ' ' && c <= '~';
}

fn is_punct(c: u8) -> bool {
    return is_printable(c) && !is_alnum(c) && c != ' ';
}

// --- Parsing ---

fn parse_int(s: string) -> int {
    var trimmed = trim(s);
    if trimmed.len == 0 {
        return 0;
    };
    var i = 0;
    var sign = 1;
    if trimmed.data[0] == '-' {
        sign = -1;
        i = 1;
    } else if trimmed.data[0] == '+' {
        i = 1;
    };
    var val = 0;
    for i < trimmed.len {
        var c = trimmed.data[i];
        if !is_digit(c) {
            break;
        };
        val = val * 10 + (cast(int) c - '0');
        i = i + 1;
    };
    return val * sign;
}

// --- Splitting ---

fn split(s: string, delim: string) -> [dyn]string #owned_return {
    var result: [dyn]string = .{};
    if delim.len == 0 {
        var i = 0;
        for i < s.len {
            push(&result, substring(s, i, 1));
            i = i + 1;
        };
        return result;
    };
    var start: usize = 0;
    var len = s.len;
    for start <= len {
        var sub = substring(s, start, len - start);
        var idx = index_of(sub, delim);
        if idx == -1 {
            push(&result, sub);
            break;
        };
        push(&result, substring(s, start, idx));
        start = start + idx + delim.len;
    };
    return result;
}
