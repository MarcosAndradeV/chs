module runtime

library Runtime {
    link_name = "chs_runtime",
    kind = "static",
}

type chs_dyn_array_t struct {
    data: rawptr,
    len: usize,
    cap: usize,
}

type chs_slice_t struct {
    data: rawptr,
    len: usize,
}

type chs_string_t struct {
    data: ^u8,
    len: usize,
}

const USIZE_MAX: usize = 9223372036854775807

fn chs_alloc(size: usize) -> rawptr #foreign Runtime #link_name "chs_alloc" #private
fn chs_realloc(ptr: rawptr, size: usize) -> rawptr #foreign Runtime #link_name "chs_realloc" #private
fn chs_dealloc(ptr: rawptr) #foreign Runtime #link_name "chs_dealloc" #private

fn chs_fatal_error(msg: string) #foreign Runtime #link_name "chs_fatal_error" #private

fn chs_memset(s: rawptr, c: int, n: usize) -> rawptr #foreign Runtime #link_name "memset" #private
fn chs_memcpy(d: rawptr, s: rawptr, n: usize) -> rawptr #foreign Runtime #link_name "memcpy" #private
fn chs_memcmp(s1: rawptr, s2: rawptr, n: usize) -> int #foreign Runtime #link_name "memcmp" #private
fn chs_strlen(s: rawptr) -> int #foreign Runtime #link_name "strlen" #private

fn dyn_array_grow(arr: ^chs_dyn_array_t, elem_size: usize) #private {
    if arr == null || elem_size == 0 {
        chs_fatal_error("invalid dynamic array metadata");
    };
    var new_cap: usize = 4;
    if arr.cap != 0 {
        var growth = arr.cap / 2;
        if growth == 0 {
            growth = 1;
        };
        if arr.cap > USIZE_MAX - growth {
            chs_fatal_error("dynamic array capacity growth overflows size_t");
        };
        new_cap = arr.cap + growth;
    };
    if new_cap < arr.len || new_cap > USIZE_MAX / elem_size {
        chs_fatal_error("dynamic array byte size overflows size_t");
    };

    var new_data = chs_realloc(arr.data, new_cap * elem_size);
    if new_data == null {
        chs_fatal_error("out of memory");
    };
    arr.data = new_data;
    arr.cap = new_cap;
}

fn chs_dyn_array_push(arr_ptr: rawptr, val_ptr: rawptr, elem_size: usize) #private {
    if arr_ptr == null {
        return;
    };
    var arr = cast(^chs_dyn_array_t) arr_ptr;
    if arr.len >= arr.cap {
        dyn_array_grow(arr, elem_size);
    };
    var dest = arr.data + arr.len * elem_size;
    chs_memcpy(dest, val_ptr, elem_size);
    arr.len = arr.len + 1;
}

fn chs_dyn_array_push_slice(arr_ptr: rawptr, src_data: rawptr, src_len: usize, elem_size: usize) #private {
    if arr_ptr == null || src_len == 0 || src_data == null {
        return;
    };
    var arr = cast(^chs_dyn_array_t) arr_ptr;
    if src_len > USIZE_MAX - arr.len {
        chs_fatal_error("dynamic array length overflows size_t");
    };
    var required_len = arr.len + src_len;
    for required_len > arr.cap {
        dyn_array_grow(arr, elem_size);
    };
    var dest = arr.data + arr.len * elem_size;
    chs_memcpy(dest, src_data, src_len * elem_size);
    arr.len = required_len;
}

fn chs_dyn_array_pop(arr_ptr: rawptr, out_ptr: rawptr, elem_size: usize) #private {
    if arr_ptr == null {
        return;
    };
    var arr = cast(^chs_dyn_array_t) arr_ptr;
    if arr.len == 0 {
        chs_fatal_error("Pop from empty dynamic array. abort()");
    };
    arr.len = arr.len - 1;
    if out_ptr != null {
        var src = arr.data + arr.len * elem_size;
        chs_memcpy(out_ptr, src, elem_size);
    };
}

fn chs_dyn_array_clear(arr_ptr: rawptr) #private {
    if arr_ptr != null {
        var arr = cast(^chs_dyn_array_t) arr_ptr;
        arr.len = 0;
    };
}

fn chs_dyn_array_drop(arr_ptr: rawptr) #private {
    if arr_ptr != null {
        var arr = cast(^chs_dyn_array_t) arr_ptr;
        if arr.data != null {
            chs_dealloc(arr.data);
            arr.data = null;
            arr.len = 0;
            arr.cap = 0;
        };
    };
}

fn chs_slice_drop(slice_ptr: rawptr) #private {
    if slice_ptr != null {
        var slice = cast(^chs_slice_t) slice_ptr;
        if slice.data != null {
            chs_dealloc(slice.data);
            slice.data = null;
            slice.len = 0;
        };
    };
}

fn chs_string_drop(str_ptr: rawptr) #private {
    if str_ptr != null {
        var str = cast(^chs_string_t) str_ptr;
        if str.data != null {
            chs_dealloc(cast(rawptr) str.data);
            str.data = null;
            str.len = 0;
        };
    };
}

fn chs_ptr_drop(ptr_addr: ^rawptr) #private {
    if ptr_addr != null && *ptr_addr != null {
        chs_dealloc(*ptr_addr);
        *ptr_addr = null;
    };
}

var chs_args: []string #foreign Runtime #link_name "chs_args" #private

fn get_command_line_arguments() -> []string {
    return chs_args;
}
