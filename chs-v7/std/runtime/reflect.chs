module runtime

type TypeEnumVariant struct {
    name: string,
    value: int,
}

type TypeField struct {
    name: string,
    offset: int,
    type_info: ^TypeInfo,
}

type TypeKind enum : u8 {
    Void = 1,
    Bool = 2,
    Float = 3,
    NoReturn = 4,
    UntypedInt = 5,
    Integer = 6,
    Pointer = 7,
    Array = 8,
    Slice = 9,
    Struct = 10,
    Enum = 11,
    String = 12,
    Any = 13,
    FnPointer = 14,
    DynArray = 15,
}

type TypeInfo struct {
    kind: TypeKind,
    name: string,
    size: int,
    align: int,
    element_type: ^TypeInfo,
    array_len: int,
    fields: []TypeField,
    variants: []TypeEnumVariant,
    is_signed: bool,
    bit_size: int,
}

type Any struct {
    value: ^void,
    type_info: ^TypeInfo
}
