# Use a array and constants for the types instead of db fields

- STATUS: OPEN
- PRIORITY: 0

macro_rules! define_primitive_types {
    (
        $(
            $const_name:ident = ($str_name:expr, $id:expr);
        )+
    ) => {
        // 1. Generate individual constants
        $(
            pub const $const_name: TypeID = TypeID($id);
        )+

        // 2. Count the entries automatically for the array size
        const PRIMITIVE_TYPE_COUNT: usize = 0 $(+ { let _ = $id; 1 })+;

        // 3. Generate the combined array
        pub const PRIMITIVE_TYPE_IDS: [(&'static str, TypeID); PRIMITIVE_TYPE_COUNT] = [
            $(
                ($str_name, $const_name),
            )+
        ];
    };
}

// --- Usage ---

define_primitive_types! {
    VOID     = ("void", 0);
    INT      = ("int", 1);
    U8       = ("u8", 2);
    USIZE    = ("usize", 3);
    BOOL     = ("bool", 4);
    FLOAT    = ("float", 5);
    STRING   = ("string", 6);
    NORETURN = ("noreturn", 7);
}
