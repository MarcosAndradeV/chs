module io

import "c/libc"

fn puts(s: string) {
    libc.puts(s.data);
}

fn print(fmt: string, args: ...Any) {
    if args.len == 0 {
        libc.printf(fmt.data);
        return;
    };
    var arg_idx = 0;
    foreach c in fmt {
        if c == '%' {
            if arg_idx < args.len {
                var arg = args[arg_idx];
                arg_idx = arg_idx + 1;
                print_info(arg);
            } else {
                libc.printf("%%".data, 0);
            }
        } else {
            libc.printf("%c".data, cast(int) c);
        }
    }
}

fn print_info(arg: Any) #private {
    var info = arg.type_info;
    if info.kind == TypeKind.Integer {
        if info.bit_size == 8 {
            var val = *cast(^u8) arg.value;
            libc.printf("%u".data, cast(int) val);
        } else if info.bit_size == 32 {
            var val = *cast(^int) arg.value;
            if info.is_signed {
                libc.printf("%d".data, val);
            } else {
                libc.printf("%u".data, val);
            };
        } else if info.bit_size == 64 {
            var val = *cast(^usize) arg.value;
            if info.is_signed {
                libc.printf("%ld".data, val);
            } else {
                libc.printf("%lu".data, val);
            };
        };
    } else if info.kind == TypeKind.Bool {
        var val = *cast(^bool) arg.value;
        if val {
            libc.printf("true".data);
        } else {
            libc.printf("false".data);
        }
    } else if info.kind == TypeKind.Float {
        if info.bit_size == 32 {
            var val = *cast(^f32) arg.value;
            libc.printf("%g".data, cast(float) val);
        } else {
            var val = *cast(^f64) arg.value;
            libc.printf("%lg".data, val);
        }
    } else if info.kind == TypeKind.String {
        var s = cast(^string) arg.value;
        foreach char in s {
            libc.printf("%c".data, cast(int) char);
        };
    } else if info.kind == TypeKind.Pointer {
        libc.printf("%p".data, *cast(^^void)arg.value);
    } else if info.kind == TypeKind.Struct {
        libc.printf("{".data);
        var i = 0;
        foreach field in info.fields {
            if i > 0 {
                libc.printf(", ".data);
            };
            i += 1;
            print("%: ", #anycast[field.name]);
            print_info(Any.{
                value: arg.value + field.offset,
                type_info: field.type_info,
            });
        };
        libc.printf("}".data);
    } else if info.kind == TypeKind.Enum {
        var val_ptr = cast(^int) arg.value;
        var val = *val_ptr;
        var found = false;
        foreach variant in info.variants {
            if variant.value == val {
                print("%", #anycast[variant.name]);
                found = true;
            }
        };
        if !found {
            libc.printf("%d".data, val);
        }
    } else if info.kind == TypeKind.Array {
        libc.printf("[".data);
        var elem_size = info.element_type.size;
        var i = 0;
        for i < info.array_len {
            if i > 0 {
                libc.printf(", ".data);
            };
            print_info(Any.{
                value: arg.value + i * elem_size,
                type_info: info.element_type,
            });
            i = i + 1;
        };
        libc.printf("]".data);
    } else if info.kind == TypeKind.Slice || info.kind == TypeKind.DynArray {
        libc.printf("[".data);
        var slice = cast(^chs_slice_t) arg.value;
        var data_ptr = slice.data;
        var len = cast(int) slice.len;
        var elem_size = info.element_type.size;

        var i = 0;
        for i < len {
            if i > 0 {
                libc.printf(", ".data);
            };
            print_info(Any.{
                value: data_ptr + i * elem_size,
                type_info: info.element_type,
            });
            i = i + 1;
        };
        libc.printf("]".data);
    } else if info.kind == TypeKind.Any {
        var any_val = cast(^Any) arg.value;
        print_info(Any.{
            value: any_val.value,
            type_info: any_val.type_info,
        });
    } else {
        libc.printf("<unknown>".data, 0);
    }
}
