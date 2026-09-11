module path

import "c/libc"
import "strings"

// --- Path Type ---

type Path struct {
    inner: string,
}

// Constructor for Path from a string
fn from_string(p: string) -> Path {
    return Path.{ inner: p };
}

// Returns the underlying string representation
fn (self: Path) string() -> string {
    return self.inner;
}

fn (self: Path) is_empty() -> bool {
    return self.inner.len == 0;
}

fn (self: Path) len() -> usize {
    return self.inner.len;
}

fn (self: Path) is_absolute() -> bool {
    if self.inner.len == 0 {
        return false;
    };
    return strings.starts_with(self.inner, "/");
}

fn (self: Path) is_relative() -> bool {
    return !self.is_absolute();
}

fn (self: Path) file_name() -> string {
    return base(self.inner);
}

fn (self: Path) base_name() -> string {
    return base(self.inner);
}

fn (self: Path) parent() -> string {
    return dir(self.inner);
}

fn (self: Path) extension() -> string {
    return ext(self.inner);
}

fn (self: Path) join_path(other: string) -> Path #owned_return {
    var joined = join(self.inner, other);
    return Path.{ inner: joined };
}

fn (self: ^Path) append(other: string) {
    self.inner = join(self.inner, other);
}

fn (self: Path) exists() -> bool {
    if self.inner.len == 0 || self.inner.data == null {
        return false;
    };
    var handle = libc.fopen(self.inner.data, "r".data);
    if handle == null {
        return false;
    };
    libc.fclose(handle);
    return true;
}

// --- Legacy & General Helper Functions ---

fn base(p: string) -> string {
    if p.len == 0 {
        return ".";
    };
    var idx = strings.last_index_of(p, "/");
    if idx == -1 {
        return p;
    };
    if idx == cast(int) (p.len - 1) {
        var trimmed = strings.substring(p, 0, p.len - 1);
        return base(trimmed);
    };
    return strings.substring(p, cast(usize) (idx + 1), p.len - cast(usize) (idx + 1));
}

fn dir(p: string) -> string {
    if p.len == 0 {
        return ".";
    };
    var idx = strings.last_index_of(p, "/");
    if idx == -1 {
        return ".";
    };
    if idx == 0 {
        return "/";
    };
    return strings.substring(p, 0, cast(usize) idx);
}

fn ext(p: string) -> string {
    var b = base(p);
    var idx = strings.last_index_of(b, ".");
    if idx <= 0 {
        return "";
    };
    return strings.substring(b, cast(usize) idx, b.len - cast(usize) idx);
}

fn join(p1: string, p2: string) -> string #owned_return {
    if p1.len == 0 { return p2; };
    if p2.len == 0 { return p1; };
    var b = make(strings.StringBuilder);
    b.write_string(p1);
    if !strings.ends_with(p1, "/") && !strings.starts_with(p2, "/") {
        b.write_string("/");
    };
    if strings.ends_with(p1, "/") && strings.starts_with(p2, "/") {
        b.write_string(strings.substring(p2, cast(usize) 1, p2.len - 1));
    } else {
        b.write_string(p2);
    };
    return b.to_string();
}
