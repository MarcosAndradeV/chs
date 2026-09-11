module runtime

fn chs_string_eq(s1: string, s2: string) -> bool {
    if s1.len != s2.len {
        return false;
    };
    if s1.len == 0 {
        return true;
    };
    return chs_memcmp(cast(^void) s1.data, cast(^void) s2.data, s1.len) == 0;
}

// builtin types overloaded operators
operator ==(s1: string, s2: string) -> bool {
    return chs_string_eq(s1, s2);
}

operator !=(s1: string, s2: string) -> bool {
    return !(s1 == s2);
}
