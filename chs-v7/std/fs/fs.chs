module fs

import "c/libc"
import "mem"
import "strings"

type Whence libc.Whence

// --- File Handle Interface ---

type File struct {
    handle: ^libc.FILE,
    is_open: bool,
}

// Opens a file with given mode ("r", "w", "a", "rb", "wb", "ab", etc.)
fn open(path: string, mode: string) -> (File, bool) {
    if path.data == null || mode.data == null {
        return File.{}, false;
    };
    var handle = libc.fopen(path.data, mode.data);
    if handle == null {
        return File.{}, false;
    };
    return File.{ handle: handle, is_open: true }, true;
}

// Closes an open file handle
fn (file: ^File) close() -> bool {
    if file == null || !file.is_open || file.handle == null {
        return false;
    };
    var res = libc.fclose(file.handle) == 0;
    file.handle = null;
    file.is_open = false;
    return res;
}

// Reads up to `size` bytes into buffer
fn (file: ^File) read(buffer: ^void, size: int) -> int {
    if file == null || !file.is_open || file.handle == null || buffer == null || size <= 0 {
        return 0;
    };
    return cast(int) libc.fread(buffer, 1, size, file.handle);
}

// Reads remaining content from current offset to EOF as a string
fn (file: ^File) read_all() -> string #owned_return {
    if file == null || !file.is_open || file.handle == null {
        return "";
    };
    var cur_pos = libc.ftell(file.handle);
    libc.fseek(file.handle, 0, .END);
    var end_pos = libc.ftell(file.handle);
    var size = end_pos - cur_pos;
    libc.fseek(file.handle, cur_pos, .SET);

    if size <= 0 {
        return "";
    };

    var buf = make([]u8, autocast (size + 1));
    var read_bytes = libc.fread(cast(^void) buf.data, 1, autocast size, file.handle);
    buf[read_bytes] = cast(u8) 0;

    return strings.from_raw_parts(data: buf.data, len: read_bytes);
}

// Writes `size` bytes from buffer to file
fn (file: ^File) write(buffer: ^void, size: int) -> int {
    if file == null || !file.is_open || file.handle == null || buffer == null || size <= 0 {
        return 0;
    };
    return cast(int) libc.fwrite(buffer, 1, size, file.handle);
}

// Writes a string content to file
fn (file: ^File) write_str(content: string) -> int {
    if file == null || !file.is_open || file.handle == null || content.len <= 0 || content.data == null {
        return 0;
    };
    return cast(int) libc.fwrite(cast(^void) content.data, 1, content.len, file.handle);
}

// Repositions the file offset
fn (file: ^File) seek(offset: int, whence: Whence) -> bool {
    if file == null || !file.is_open || file.handle == null {
        return false;
    };
    return libc.fseek(file.handle, autocast offset, whence) == 0;
}

// Returns the current file offset
fn (file: ^File) tell() -> int {
    if file == null || !file.is_open || file.handle == null {
        return -1;
    };
    return cast(int) libc.ftell(file.handle);
}

// Flushes buffered file output
fn (file: ^File) flush() -> bool {
    if file == null || !file.is_open || file.handle == null {
        return false;
    };
    return libc.fflush(file.handle) == 0;
}

// --- Convenience Utility Functions ---

// Reads an entire file into a heap-allocated string
fn read_to_string(path: string) -> (string, bool) #owned_return {
    var f, ok = open(path, "rb");
    if !ok {
        return "", false;
    };
    defer f.close();
    return f.read_all(), true;
}

// Overwrites or creates a file with content
fn write_string(path: string, content: string) -> bool {
    var f, ok = open(path, "wb");
    if !ok {
        return false;
    };
    defer f.close();
    var written = f.write_str(content);
    return written == content.len;
}

// Appends content to a file
fn append_string(path: string, content: string) -> bool {
    var f, ok = open(path, "ab");
    if !ok {
        return false;
    };
    defer f.close();
    var written = f.write_str(content);
    return written == content.len;
}

// Checks if a file exists
fn file_exists(path: string) -> bool {
    var f, ok = open(path, "r");
    if !ok {
        return false;
    };
    f.close();
    return true;
}

// Deletes a file
fn remove_file(path: string) -> bool {
    if path.data == null {
        return false;
    };
    return libc.remove(path.data) == 0;
}
