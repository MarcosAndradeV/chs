use std::collections::HashMap;

pub use lex_just_parse::lexer::TokenSource;

/// A lightweight, copyable handle to a registered type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TypeID(pub u32);

impl TypeID {
    /// Returns the constructor function ID for a type if set.
    pub fn constructor(self, db: &TypeDatabase) -> Option<FunctionID> {
        let canonical = db.resolve(self);
        db.get_type(canonical).constructor()
    }

    /// Returns the destructor function ID for a type if set.
    pub fn destructor(self, db: &TypeDatabase) -> Option<FunctionID> {
        let canonical = db.resolve(self);
        db.get_type(canonical).destructor()
    }

    pub fn get_type(self, db: &TypeDatabase) -> &Type {
        db.get_type(self)
    }

    pub fn unwrap_pointer_type(self, db: &TypeDatabase) -> TypeID {
        let mut canonical = db.resolve(self);
        while let Type::Pointer(inner) = db.get_type(canonical).clone() {
            canonical = db.resolve(inner);
        }
        db.get_underlying_type(canonical)
    }
}

/// A lightweight, copyable handle to a registered function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FunctionID(pub u32);

pub const VOID_ID: TypeID = TypeID(0);
pub const BOOL_ID: TypeID = TypeID(1);
pub const FLOAT_ID: TypeID = TypeID(2);
pub const NORETURN_ID: TypeID = TypeID(3);
pub const UNTYPED_INT_ID: TypeID = TypeID(4);
pub const STRING_ID: TypeID = TypeID(5);
pub const MODULE_TYPE_ID: TypeID = TypeID(6);
pub const I8_ID: TypeID = TypeID(7);
pub const U8_ID: TypeID = TypeID(8);
pub const I16_ID: TypeID = TypeID(9);
pub const U16_ID: TypeID = TypeID(10);
pub const I32_ID: TypeID = TypeID(11);
pub const U32_ID: TypeID = TypeID(12);
pub const I64_ID: TypeID = TypeID(13);
pub const U64_ID: TypeID = TypeID(14);
pub const F64_ID: TypeID = TypeID(15);
pub const UNTYPED_FLOAT_ID: TypeID = TypeID(16);
pub const RAWPTR_ID: TypeID = TypeID(17);

pub const INT_ID: TypeID = I32_ID;
pub const USIZE_ID: TypeID = U64_ID;
pub const F32_ID: TypeID = FLOAT_ID;

pub fn lookup_builtin_type(name: &str) -> Option<TypeID> {
    match name {
        "void" => Some(VOID_ID),
        "bool" => Some(BOOL_ID),
        "f32" | "float" => Some(F32_ID),
        "f64" => Some(F64_ID),
        "untyped_float" => Some(UNTYPED_FLOAT_ID),
        "noreturn" => Some(NORETURN_ID),
        "untyped_int" => Some(UNTYPED_INT_ID),
        "string" => Some(STRING_ID),
        "module" => Some(MODULE_TYPE_ID),
        "i8" => Some(I8_ID),
        "u8" => Some(U8_ID),
        "i16" => Some(I16_ID),
        "u16" => Some(U16_ID),
        "i32" | "int" => Some(I32_ID),
        "u32" => Some(U32_ID),
        "i64" => Some(I64_ID),
        "u64" | "usize" => Some(U64_ID),
        "rawptr" => Some(RAWPTR_ID),
        _ => None,
    }
}

/// A field within a struct type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    pub name: TokenSource,
    pub ty: TypeID,
}

/// A variant within an enum type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumVariant {
    pub name: TokenSource,
    pub default_value: u64,
    pub payload: Option<TypeID>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sign {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitSize {
    _8,
    _16,
    _32,
    _64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatBitSize {
    _32,
    _64,
}

/// The internal representation of all language types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Any(TypeID),
    Array(TypeID, usize),
    Bool,
    Distinct {
        name: TokenSource,
        base: TypeID,
    },
    DynArray(TypeID),
    Enum {
        name: TokenSource,
        repr: TypeID,
        variants: Vec<EnumVariant>, // None means the enum is a forward-declared placeholder
        functions: HashMap<TokenSource, FunctionID>,
    },
    Float(FloatBitSize),
    FnPointer {
        params: Vec<TypeID>,
        return_type: TypeID,
    },
    Integer(Sign, BitSize),
    Module,
    NoReturn,
    Pointer(TypeID),
    Slice(TypeID),
    String,
    Struct {
        name: TokenSource,
        fields: Option<Vec<StructField>>, // None means the struct is a forward-declared placeholder
        functions: HashMap<TokenSource, FunctionID>,
        constructor: Option<FunctionID>,
        destructor: Option<FunctionID>,
    },
    Tuple(Vec<TypeID>),
    TypeVar(u32),
    UntypedFloat,
    UntypedInt,
    Void,
}

impl Type {
    /// Returns `true` if the type is primitive.
    #[must_use]
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            Self::Void
                | Self::Bool
                | Self::Float(..)
                | Self::NoReturn
                | Self::UntypedInt
                | Self::UntypedFloat
                | Self::Module
                | Self::Integer(..)
        )
    }

    /// Returns `true` if the type is [`Pointer`].
    ///
    /// [`Pointer`]: Type::Pointer
    #[must_use]
    pub fn is_pointer(&self) -> bool {
        matches!(self, Self::Pointer(..))
    }

    pub fn functions(&self) -> Option<&HashMap<TokenSource, FunctionID>> {
        match self {
            Self::Struct { functions, .. } | Self::Enum { functions, .. } => Some(functions),
            _ => None,
        }
    }

    pub fn constructor(&self) -> Option<FunctionID> {
        match self {
            Self::Struct { constructor, .. } => *constructor,
            _ => None,
        }
    }

    pub fn destructor(&self) -> Option<FunctionID> {
        match self {
            Self::Struct { destructor, .. } => *destructor,
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FunctionParam {
    pub name: TokenSource,
    pub typ: TypeID,
    pub is_variadic: bool,
}

#[derive(Clone, Debug)]
pub struct FunctionSignature {
    pub id: Option<FunctionID>,
    pub name: TokenSource,
    pub full_name: String,
    pub params: Vec<FunctionParam>,
    pub return_type: TypeID,
    pub has_va_args: bool,
    pub is_private: bool,
    pub is_foreign: bool,
    pub has_owned_return: bool,
}

#[derive(Clone, Debug)]
pub struct VariableSignature {
    pub name: TokenSource,
    pub ty: TypeID,
    pub is_private: bool,
    pub is_thread_local: bool,
    pub is_foreign: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ImportInfo {
    original_name: Option<TokenSource>,
    is_all: bool,
}

impl ImportInfo {
    pub fn all() -> Self {
        Self {
            original_name: None,
            is_all: true,
        }
    }
    pub fn with_original_name(original_name: TokenSource) -> Self {
        Self {
            original_name: Some(original_name),
            is_all: true,
        }
    }

    pub fn is_all(&self) -> bool {
        self.is_all
    }

    pub fn original_name(&self) -> Option<&TokenSource> {
        self.original_name.as_ref()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Module {
    pub functions: HashMap<TokenSource, FunctionSignature>,
    pub variables: HashMap<TokenSource, VariableSignature>,
    pub structs: HashMap<TokenSource, TypeID>,
    pub enums: HashMap<TokenSource, TypeID>,
    pub types: HashMap<TokenSource, TypeID>,
    pub imported_modules: HashMap<TokenSource, ImportInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ModuleID(pub u32);

impl Module {
    pub fn register_type(&mut self, name: TokenSource, id: TypeID) {
        self.types.insert(name, id);
    }

    pub fn register_struct(&mut self, name: TokenSource, id: TypeID) {
        self.structs.insert(name, id);
    }

    pub fn register_enum(&mut self, name: TokenSource, id: TypeID) {
        self.enums.insert(name, id);
    }

    pub fn lookup_type(&self, name: &TokenSource) -> Option<TypeID> {
        if let Some(&id) = self.types.get(name) {
            return Some(id);
        }
        if let Some(&id) = self.structs.get(name) {
            return Some(id);
        }
        if let Some(&id) = self.enums.get(name) {
            return Some(id);
        }
        None
    }

    pub fn lookup_var(&self, name: &TokenSource) -> Option<&VariableSignature> {
        self.variables.get(name)
    }
}

/// The central type registry and unification arena.
#[derive(Debug, Clone)]
pub struct TypeDatabase {
    pub types: HashMap<TypeID, Type>,
    pub functions: Vec<FunctionSignature>,
    pub substitutions: HashMap<u32, TypeID>,
    pub queried_types: std::collections::HashSet<TypeID>,
    pub aliases: HashMap<TypeID, TypeID>,
    modules: HashMap<String, Module>,
    next_var_id: u32,
}

impl Default for TypeDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeDatabase {
    /// Creates a new TypeDatabase pre-populated with primitive types.
    pub fn new() -> Self {
        let mut db = Self {
            types: HashMap::new(),
            functions: Vec::new(),
            substitutions: HashMap::new(),
            modules: HashMap::new(),
            next_var_id: 0,
            queried_types: std::collections::HashSet::new(),
            aliases: HashMap::new(),
        };

        db.types.insert(VOID_ID, Type::Void);
        db.types.insert(BOOL_ID, Type::Bool);
        db.types.insert(F32_ID, Type::Float(FloatBitSize::_32));
        db.types.insert(F64_ID, Type::Float(FloatBitSize::_64));
        db.types.insert(UNTYPED_FLOAT_ID, Type::UntypedFloat);
        db.types.insert(NORETURN_ID, Type::NoReturn);
        db.types.insert(UNTYPED_INT_ID, Type::UntypedInt);
        db.types.insert(STRING_ID, Type::String);
        db.types.insert(MODULE_TYPE_ID, Type::Module);

        db.types
            .insert(I8_ID, Type::Integer(Sign::Signed, BitSize::_8));
        db.types
            .insert(U8_ID, Type::Integer(Sign::Unsigned, BitSize::_8));
        db.types
            .insert(I16_ID, Type::Integer(Sign::Signed, BitSize::_16));
        db.types
            .insert(U16_ID, Type::Integer(Sign::Unsigned, BitSize::_16));
        db.types
            .insert(I32_ID, Type::Integer(Sign::Signed, BitSize::_32));
        db.types
            .insert(U32_ID, Type::Integer(Sign::Unsigned, BitSize::_32));
        db.types
            .insert(I64_ID, Type::Integer(Sign::Signed, BitSize::_64));
        db.types
            .insert(U64_ID, Type::Integer(Sign::Unsigned, BitSize::_64));
        db.types.insert(RAWPTR_ID, Type::Pointer(VOID_ID));

        db
    }

    pub fn rawptr(&self) -> TypeID {
        RAWPTR_ID
    }
    pub fn void(&self) -> TypeID {
        VOID_ID
    }
    pub fn int(&self) -> TypeID {
        INT_ID
    }
    pub fn noreturn(&self) -> TypeID {
        NORETURN_ID
    }
    pub fn module(&self) -> TypeID {
        MODULE_TYPE_ID
    }
    pub fn bool(&self) -> TypeID {
        BOOL_ID
    }
    pub fn u8(&self) -> TypeID {
        U8_ID
    }
    pub fn float(&self) -> TypeID {
        FLOAT_ID
    }
    pub fn f32(&self) -> TypeID {
        F32_ID
    }
    pub fn f64(&self) -> TypeID {
        F64_ID
    }
    pub fn untyped_float(&self) -> TypeID {
        UNTYPED_FLOAT_ID
    }
    pub fn string(&self) -> TypeID {
        STRING_ID
    }
    pub fn usize(&self) -> TypeID {
        USIZE_ID
    }
    pub fn untyped_int(&self) -> TypeID {
        UNTYPED_INT_ID
    }
    pub fn is_float(&self, ty: TypeID) -> bool {
        let canon = self.resolve(ty);
        matches!(self.get_type(canon), Type::Float(..))
    }
    pub fn type_kind(&self) -> TypeID {
        self.lookup_type_in_module("", &TokenSource::from("TypeKind"))
            .expect("runtime")
    }
    pub fn type_info(&self) -> TypeID {
        self.lookup_type_in_module("", &TokenSource::from("TypeInfo"))
            .expect("runtime")
    }
    pub fn any(&self) -> TypeID {
        self.lookup_type_in_module("", &TokenSource::from("Any"))
            .expect("runtime")
    }
    pub fn pointer_type(&self, inner: TypeID) -> Option<TypeID> {
        for (&id, ty) in &self.types {
            if let Type::Pointer(existing_inner) = ty
                && *existing_inner == inner
            {
                return Some(id);
            }
        }
        None
    }

    pub fn is_integer(&self, ty: TypeID) -> bool {
        let canon = self.resolve(ty);
        matches!(self.get_type(canon), Type::Integer(..))
    }

    pub fn is_primitive_castable(&self, ty: TypeID) -> bool {
        let canon = self.resolve(ty);
        if matches!(self.get_type(canon), Type::Integer(..) | Type::Float(..))
            || canon == self.bool()
            || canon == self.untyped_int()
            || canon == self.untyped_float()
        {
            return true;
        }
        false
    }

    /// Registers a function signature into the arena, assigns a unique FunctionID, and returns it.
    pub fn register_function(&mut self, mut sig: FunctionSignature) -> FunctionID {
        let id = FunctionID(self.functions.len() as u32);
        sig.id = Some(id);
        self.functions.push(sig);
        id
    }

    pub fn get_function(&self, id: FunctionID) -> &FunctionSignature {
        &self.functions[id.0 as usize]
    }

    pub fn get_function_mut(&mut self, id: FunctionID) -> &mut FunctionSignature {
        &mut self.functions[id.0 as usize]
    }

    pub fn try_get_function(&self, id: FunctionID) -> Option<&FunctionSignature> {
        self.functions.get(id.0 as usize)
    }

    /// Inserts a new type into the registry and returns its unique TypeID.
    pub fn insert_type(&mut self, ty: Type) -> TypeID {
        let id = TypeID(self.types.len() as u32);
        self.types.insert(id, ty);
        id
    }

    pub fn alias(&mut self, _name: impl ToString, _base_id: TypeID) {}

    pub fn queried_types_mut(&mut self) -> &mut std::collections::HashSet<TypeID> {
        &mut self.queried_types
    }

    /// Returns a TypeID for a Pointer to the inner type, interning it.
    pub fn pointer(&mut self, inner: TypeID) -> TypeID {
        self.insert_type(Type::Pointer(inner))
    }

    /// Returns a TypeID for an Array of the element type and size, interning it.
    pub fn array(&mut self, element: TypeID, size: usize) -> TypeID {
        for (&id, ty) in &self.types {
            if let Type::Array(existing_element, existing_size) = ty
                && *existing_element == element
                && *existing_size == size
            {
                return id;
            }
        }
        self.insert_type(Type::Array(element, size))
    }

    /// Returns a TypeID for a Slice of the element type, interning it.
    pub fn slice(&mut self, element: TypeID) -> TypeID {
        for (&id, ty) in &self.types {
            if let Type::Slice(existing_element) = ty
                && *existing_element == element
            {
                return id;
            }
        }
        self.insert_type(Type::Slice(element))
    }

    /// Returns a TypeID for a DynArray of the element type, interning it.
    pub fn dyn_array(&mut self, element: TypeID) -> TypeID {
        for (&id, ty) in &self.types {
            if let Type::DynArray(existing_element) = ty
                && *existing_element == element
            {
                return id;
            }
        }
        self.insert_type(Type::DynArray(element))
    }

    /// Returns a TypeID for a Tuple of the given element types, interning it.
    pub fn tuple(&mut self, elements: Vec<TypeID>) -> TypeID {
        for (&id, ty) in &self.types {
            if let Type::Tuple(existing_elements) = ty
                && *existing_elements == elements
            {
                return id;
            }
        }
        self.insert_type(Type::Tuple(elements))
    }

    pub fn fn_pointer(&mut self, params: Vec<TypeID>, return_type: TypeID) -> TypeID {
        let ty = Type::FnPointer {
            params,
            return_type,
        };
        for (id, existing_ty) in &self.types {
            if existing_ty == &ty {
                return *id;
            }
        }
        self.insert_type(ty)
    }

    /// Retrieves a reference to the Type associated with a TypeID.
    pub fn get_type(&self, id: TypeID) -> &Type {
        self.types.get(&id).unwrap()
    }

    pub fn get_inner_type_id(&self, id: TypeID) -> TypeID {
        match self.types.get(&id).unwrap() {
            Type::Any(any_id) => *any_id,
            Type::Slice(inner_id) => *inner_id,
            Type::DynArray(inner_id) => *inner_id,
            Type::Array(inner_id, ..) => *inner_id,
            _ => id,
        }
    }

    pub fn get_underlying_type(&self, id: TypeID) -> TypeID {
        let canonical = self.resolve(id);
        match self.types.get(&canonical).unwrap() {
            Type::Distinct { base, .. } => self.get_underlying_type(*base),
            _ => canonical,
        }
    }

    /// Retrieves a mutable reference to the Type associated with a TypeID.
    pub fn get_type_mut(&mut self, id: TypeID) -> &mut Type {
        self.types.get_mut(&id).unwrap()
    }

    /// Creates and registers a new unique inference type variable.
    pub fn new_inference_var(&mut self) -> TypeID {
        let var_id = self.next_var_id;
        self.next_var_id += 1;
        self.insert_type(Type::TypeVar(var_id))
    }

    /// Resolves a TypeID to its canonical representative, following substitution chains.
    pub fn resolve(&self, mut id: TypeID) -> TypeID {
        loop {
            let mut updated = false;
            while let Type::TypeVar(var_id) = self.get_type(id) {
                if let Some(&resolved) = self.substitutions.get(var_id) {
                    if resolved == id {
                        break;
                    }
                    id = resolved;
                    updated = true;
                } else {
                    break;
                }
            }
            if let Some(&aliased) = self.aliases.get(&id)
                && aliased != id
            {
                id = aliased;
                updated = true;
            }
            if !updated {
                break;
            }
        }
        id
    }

    /// Unifies two types, updating the internal substitutions if type variables are involved.
    pub fn unify(&mut self, id1: TypeID, id2: TypeID) -> Result<(), String> {
        let canonical1 = self.resolve(id1);
        let canonical2 = self.resolve(id2);

        if canonical1 == canonical2 {
            return Ok(());
        }

        let noreturn_id = self.noreturn();
        if canonical1 == noreturn_id || canonical2 == noreturn_id {
            return Ok(());
        }

        let void_id = self.void();

        match (
            self.get_type(canonical1).clone(),
            self.get_type(canonical2).clone(),
        ) {
            (Type::TypeVar(v1), _) => {
                self.substitutions.insert(v1, canonical2);
                Ok(())
            }
            (_, Type::TypeVar(v2)) => {
                self.substitutions.insert(v2, canonical1);
                Ok(())
            }
            (Type::Integer(s1, b1), Type::Integer(s2, b2)) => {
                if s1 == s2 && b1 == b2 {
                    Ok(())
                } else {
                    Err(format!(
                        "Cannot unify integer {:?}/{:?} with {:?}/{:?}",
                        s1, b1, s2, b2
                    ))
                }
            }
            (Type::Float(b1), Type::Float(b2)) => {
                if b1 == b2 {
                    Ok(())
                } else {
                    Err(format!("Cannot unify float {:?} with {:?}", b1, b2))
                }
            }
            (Type::Pointer(p1), Type::Pointer(..)) if p1 == void_id => Ok(()),
            (Type::Pointer(..), Type::Pointer(p2)) if p2 == void_id => Ok(()),
            (Type::Pointer(inner1), Type::Pointer(inner2)) => self.unify(inner1, inner2),
            (Type::Array(inner1, size1), Type::Array(inner2, size2)) => {
                if size1 == size2 {
                    self.unify(inner1, inner2)
                } else {
                    Err(format!(
                        "Cannot unify array of size {} with array of size {}",
                        size1, size2
                    ))
                }
            }
            (Type::Slice(inner1), Type::Slice(inner2)) => self.unify(inner1, inner2),
            (Type::DynArray(inner1), Type::DynArray(inner2)) => self.unify(inner1, inner2),
            (Type::Tuple(elems1), Type::Tuple(elems2)) => {
                if elems1.len() != elems2.len() {
                    return Err(format!(
                        "Cannot unify tuples of different lengths ({} and {})",
                        elems1.len(),
                        elems2.len()
                    ));
                }
                for (&e1, &e2) in elems1.iter().zip(elems2.iter()) {
                    self.unify(e1, e2)?;
                }
                Ok(())
            }
            (
                Type::FnPointer {
                    params: params1,
                    return_type: ret1,
                },
                Type::FnPointer {
                    params: params2,
                    return_type: ret2,
                },
            ) => {
                if params1.len() != params2.len() {
                    return Err(format!(
                        "Cannot unify function pointer types with different parameter counts ({} and {})",
                        params1.len(),
                        params2.len()
                    ));
                }
                for (&p1, &p2) in params1.iter().zip(params2.iter()) {
                    self.unify(p1, p2)?;
                }
                self.unify(ret1, ret2)
            }
            (t1, t2) => Err(format!("Cannot unify {:?} with {:?}", t1, t2)),
        }
    }

    /// Populates/updates fields for a placeholder Struct.
    pub fn set_struct_fields(
        &mut self,
        id: TypeID,
        fields: Vec<StructField>,
    ) -> Result<(), String> {
        let canonical = self.resolve(id);
        match self.get_type_mut(canonical) {
            Type::Struct { fields: f, .. } => {
                *f = Some(fields);
                Ok(())
            }
            _ => Err("Type is not a struct".to_string()),
        }
    }

    /// Populates/updates variants for a placeholder Enum.
    pub fn set_enum_variants(
        &mut self,
        id: TypeID,
        variants: Vec<EnumVariant>,
    ) -> Result<(), String> {
        let canonical = self.resolve(id);
        match self.get_type_mut(canonical) {
            Type::Enum { variants: v, .. } => {
                *v = variants;
                Ok(())
            }
            _ => Err("Type is not an enum".to_string()),
        }
    }

    /// Associates a function ID with a type by name.
    pub fn add_function_to_type(
        &mut self,
        id: TypeID,
        name: TokenSource,
        fn_id: FunctionID,
    ) -> Result<(), String> {
        let canonical = self.resolve(id);
        match self.get_type_mut(canonical) {
            Type::Struct { functions, .. } => {
                functions.insert(name, fn_id);
                Ok(())
            }
            _ => Err("Type cannot have associated functions".to_string()),
        }
    }

    /// Sets the constructor function ID for a type.
    pub fn set_constructor_for_type(
        &mut self,
        id: TypeID,
        fn_id: FunctionID,
    ) -> Result<(), String> {
        let canonical = self.resolve(id);
        match self.get_type_mut(canonical) {
            Type::Struct { constructor, .. } => {
                *constructor = Some(fn_id);
                Ok(())
            }
            _ => Err("Type cannot have a constructor".to_string()),
        }
    }

    /// Sets the destructor function ID for a type.
    pub fn set_destructor_for_type(&mut self, id: TypeID, fn_id: FunctionID) -> Result<(), String> {
        let canonical = self.resolve(id);
        match self.get_type_mut(canonical) {
            Type::Struct { destructor, .. } => {
                *destructor = Some(fn_id);
                Ok(())
            }
            _ => Err("Type cannot have a destructor".to_string()),
        }
    }

    /// Returns the constructor function ID for a type if set.
    pub fn constructor(&self, id: TypeID) -> Option<FunctionID> {
        let canonical = self.resolve(id);
        self.get_type(canonical).constructor()
    }

    /// Returns the destructor function ID for a type if set.
    pub fn destructor(&self, id: TypeID) -> Option<FunctionID> {
        let canonical = self.resolve(id);
        self.get_type(canonical).destructor()
    }

    /// Returns a user-friendly string representation of a type.
    pub fn type_to_string(&self, id: TypeID) -> String {
        let canonical = self.resolve(id);
        match self.get_type(canonical) {
            Type::Void => "void".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Float(bitsize) => match bitsize {
                FloatBitSize::_32 => "f32".to_string(),
                FloatBitSize::_64 => "float".to_string(),
            },
            Type::NoReturn => "noreturn".to_string(),
            Type::UntypedInt => "untyped_int".to_string(),
            Type::UntypedFloat => "untyped_float".to_string(),
            Type::Module => "module".to_string(),
            Type::Integer(sign, bitsize) => match (sign, bitsize) {
                (Sign::Signed, BitSize::_8) => "i8".to_string(),
                (Sign::Unsigned, BitSize::_8) => "u8".to_string(),
                (Sign::Signed, BitSize::_16) => "i16".to_string(),
                (Sign::Unsigned, BitSize::_16) => "u16".to_string(),
                (Sign::Signed, BitSize::_32) => "int".to_string(),
                (Sign::Unsigned, BitSize::_32) => "u32".to_string(),
                (Sign::Signed, BitSize::_64) => "i64".to_string(),
                (Sign::Unsigned, BitSize::_64) => "u64".to_string(),
            },
            Type::Pointer(inner) => {
                format!("^{}", self.type_to_string(*inner))
            }
            Type::Array(inner, size) => format!("[{}]{}", size, self.type_to_string(*inner)),
            Type::Slice(inner) => format!("[]{}", self.type_to_string(*inner)),
            Type::DynArray(inner) => format!("[dyn]{}", self.type_to_string(*inner)),
            Type::Struct { name, .. } => name.to_string(),
            Type::Enum { name, .. } => name.to_string(),
            Type::TypeVar(v) => format!("_t{}", v),
            Type::Tuple(elems) => {
                let elems_str: Vec<String> =
                    elems.iter().map(|&e| self.type_to_string(e)).collect();
                format!("({})", elems_str.join(", "))
            }
            Type::Any(_) => "Any".to_string(),
            Type::String => "string".to_string(),
            Type::Distinct { name, .. } => name.to_string(),
            Type::FnPointer {
                params,
                return_type,
            } => {
                let params_str: Vec<String> =
                    params.iter().map(|&p| self.type_to_string(p)).collect();
                format!(
                    "fn({}) -> {}",
                    params_str.join(", "),
                    self.type_to_string(*return_type)
                )
            }
        }
    }

    pub fn modules_add(&mut self, name: &str) -> &mut Module {
        match self.modules.entry(name.to_string()) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                occupied_entry.into_mut()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(Module::default())
            }
        }
    }

    pub fn modules_get_mut(&mut self, name: &str) -> &mut Module {
        self.modules.get_mut(name).unwrap()
    }

    pub fn modules_get(&self, name: &str) -> &Module {
        self.modules.get(name).unwrap()
    }

    pub fn try_modules_get(&self, name: &str) -> Option<&Module> {
        self.modules.get(name)
    }

    pub fn try_modules_get_mut(&mut self, name: &str) -> Option<&mut Module> {
        self.modules.get_mut(name)
    }

    pub fn modules(&self) -> &HashMap<String, Module> {
        &self.modules
    }

    pub fn lookup_type_in_module(&self, module_name: &str, name: &TokenSource) -> Option<TypeID> {
        if let Some(id) = lookup_builtin_type(name.as_str()) {
            return Some(id);
        }
        if let Some((mod_prefix, type_name)) = name.as_str().split_once('.') {
            let actual_module = if let Some(module) = self.modules.get(module_name) {
                if let Some(info) = module.imported_modules.get(&TokenSource::from(mod_prefix)) {
                    info.original_name
                        .as_ref()
                        .map(|n| n.as_str())
                        .unwrap_or(mod_prefix)
                } else {
                    mod_prefix
                }
            } else {
                mod_prefix
            };
            let type_tok = TokenSource::from(type_name);
            if let Some(module) = self.modules.get(actual_module) {
                if let Some(id) = module.lookup_type(&type_tok) {
                    return Some(id);
                }
            }
        }
        if let Some(module) = self.modules.get(module_name) {
            if let Some(id) = module.lookup_type(name) {
                return Some(id);
            }
            for (imported_name, info) in &module.imported_modules {
                if info.is_all {
                    let mod_name = info
                        .original_name
                        .as_ref()
                        .unwrap_or(imported_name)
                        .as_str();
                    if let Some(imp_mod) = self.modules.get(mod_name)
                        && let Some(id) = imp_mod.lookup_type(name)
                    {
                        return Some(id);
                    }
                }
            }
        }
        for module in self.modules.values() {
            if let Some(id) = module.lookup_type(name) {
                return Some(id);
            }
        }
        None
    }

    pub fn lookup_var_in_module(
        &self,
        module_name: &str,
        name: &TokenSource,
    ) -> Option<VariableSignature> {
        if let Some((mod_prefix, var_name)) = name.as_str().split_once('.') {
            let actual_module = if let Some(module) = self.modules.get(module_name) {
                if let Some(info) = module.imported_modules.get(&TokenSource::from(mod_prefix)) {
                    info.original_name
                        .as_ref()
                        .map(|n| n.as_str())
                        .unwrap_or(mod_prefix)
                } else {
                    mod_prefix
                }
            } else {
                mod_prefix
            };
            let var_tok = TokenSource::from(var_name);
            if let Some(module) = self.modules.get(actual_module) {
                if let Some(sig) = module.lookup_var(&var_tok) {
                    if !sig.is_private || actual_module == module_name {
                        return Some(sig.clone());
                    }
                }
            }
        }
        if let Some(module) = self.modules.get(module_name) {
            if let Some(sig) = module.lookup_var(name) {
                return Some(sig.clone());
            }
            for (imported_name, info) in &module.imported_modules {
                if info.is_all {
                    let mod_name = info
                        .original_name
                        .as_ref()
                        .unwrap_or(imported_name)
                        .as_str();
                    if let Some(imp_mod) = self.modules.get(mod_name)
                        && let Some(sig) = imp_mod.lookup_var(name)
                        && !sig.is_private
                    {
                        return Some(sig.clone());
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_types() {
        let db = TypeDatabase::new();
        assert_eq!(db.get_type(db.void()), &Type::Void);
        assert_eq!(db.get_type(db.bool()), &Type::Bool);
        assert_eq!(db.get_type(db.f32()), &Type::Float(FloatBitSize::_32));
        assert_eq!(db.get_type(db.f64()), &Type::Float(FloatBitSize::_64));
        assert_eq!(db.get_type(db.float()), &Type::Float(FloatBitSize::_32));
        assert_eq!(db.get_type(db.noreturn()), &Type::NoReturn);
        assert_eq!(
            db.get_type(db.int()),
            &Type::Integer(Sign::Signed, BitSize::_32)
        );
        assert_eq!(
            db.get_type(db.u8()),
            &Type::Integer(Sign::Unsigned, BitSize::_8)
        );
        assert_eq!(
            db.get_type(db.usize()),
            &Type::Integer(Sign::Unsigned, BitSize::_64)
        );
    }

    #[test]
    fn test_recursive_struct() {
        let mut db = TypeDatabase::new();

        // Forward declare struct Node
        let node_id = db.insert_type(Type::Struct {
            name: TokenSource::from("Node"),
            fields: None,
            functions: HashMap::new(),
            constructor: None,
            destructor: None,
        });

        let int_id = db.int();
        let next_ptr_id = db.pointer(node_id);

        let fields = vec![
            StructField {
                name: TokenSource("value"),
                ty: int_id,
            },
            StructField {
                name: TokenSource("next"),
                ty: next_ptr_id,
            },
        ];

        assert!(db.set_struct_fields(node_id, fields.clone()).is_ok());

        // Retrieve Node type and verify fields are correct
        if let Type::Struct {
            name,
            fields: Some(f),
            ..
        } = db.get_type(node_id)
        {
            let name: &str = name.as_ref();
            assert_eq!(name, "Node");
            assert_eq!(f, &fields);
        } else {
            panic!("Expected struct type Node with fields populated");
        }
    }

    #[test]
    fn test_unification_and_resolution() {
        let mut db = TypeDatabase::new();

        let var1 = db.new_inference_var();
        let var2 = db.new_inference_var();
        let int_id = db.int();

        // Unify two inference variables
        assert!(db.unify(var1, var2).is_ok());
        // Unify one of them with int
        assert!(db.unify(var2, int_id).is_ok());

        // Both variables should now resolve to int_id
        assert_eq!(db.resolve(var1), int_id);
        assert_eq!(db.resolve(var2), int_id);
    }

    #[test]
    fn test_unification_mismatch() {
        let mut db = TypeDatabase::new();
        let int_id = db.int();
        let bool_id = db.bool();

        assert!(db.unify(int_id, bool_id).is_err());
    }

    #[test]
    fn test_enum_variants() {
        let mut db = TypeDatabase::new();
        let int_id = db.int();

        // Forward declare Enum Option
        let option_id = db.insert_type(Type::Enum {
            name: TokenSource("Option"),
            repr: int_id,
            variants: Vec::new(),
            functions: HashMap::new(),
        });

        let variants = vec![
            EnumVariant {
                name: TokenSource("None"),
                default_value: 0,
                payload: None,
            },
            EnumVariant {
                name: TokenSource("Some"),
                default_value: 1,
                payload: None,
            },
        ];

        assert!(db.set_enum_variants(option_id, variants.clone()).is_ok());

        if let Type::Enum {
            name,
            repr,
            variants: v,
            ..
        } = db.get_type(option_id)
        {
            assert_eq!(name, "Option");
            assert_eq!(*repr, int_id);
            assert_eq!(v, &variants);
        } else {
            panic!("Expected enum type Option with variants populated");
        }
    }

    #[test]
    fn test_type_associated_functions() {
        let mut db = TypeDatabase::new();
        let struct_id = db.insert_type(Type::Struct {
            name: TokenSource::from("MyStruct"),
            fields: Some(vec![]),
            functions: HashMap::new(),
            constructor: None,
            destructor: None,
        });

        let ctor_fn = FunctionID(10);
        let dtor_fn = FunctionID(20);
        let helper_fn = FunctionID(30);

        assert!(db.set_constructor_for_type(struct_id, ctor_fn).is_ok());
        assert!(db.set_destructor_for_type(struct_id, dtor_fn).is_ok());
        assert!(
            db.add_function_to_type(struct_id, TokenSource::from("helper"), helper_fn)
                .is_ok()
        );

        let ty = db.get_type(struct_id);
        assert_eq!(ty.constructor(), Some(ctor_fn));
        assert_eq!(ty.destructor(), Some(dtor_fn));
        assert_eq!(
            ty.functions().unwrap().get(&TokenSource::from("helper")),
            Some(&helper_fn)
        );
    }

    #[test]
    fn test_fn_pointer_unification() {
        let mut db = TypeDatabase::new();
        let int_id = db.int();
        let bool_id = db.bool();

        let fn1 = db.fn_pointer(vec![int_id], bool_id);
        let fn2 = db.fn_pointer(vec![int_id], bool_id);

        // They should be identical IDs (interned)
        assert_eq!(fn1, fn2);

        // Unifying identical function pointer types should succeed
        assert!(db.unify(fn1, fn2).is_ok());

        // Unifying with a different parameter count or type should fail
        let fn3 = db.fn_pointer(vec![int_id, bool_id], bool_id);
        assert!(db.unify(fn1, fn3).is_err());

        let fn4 = db.fn_pointer(vec![bool_id], bool_id);
        assert!(db.unify(fn1, fn4).is_err());

        // Type variable unification
        let var1 = db.new_inference_var();
        let fn_var = db.fn_pointer(vec![var1], bool_id);
        assert!(db.unify(fn1, fn_var).is_ok());
        assert_eq!(db.resolve(var1), int_id);
    }

    #[test]
    fn test_module_declarations() {
        let mut db = TypeDatabase::new();
        let int_id = db.int();

        // Create main module
        let main_mod = db.modules_add("main");
        main_mod
            .structs
            .insert(TokenSource::from("MyStruct"), int_id);
        main_mod.variables.insert(
            TokenSource::from("g_val"),
            VariableSignature {
                name: TokenSource::from("g_val"),
                ty: int_id,
                is_private: false,
                is_thread_local: false,
                is_foreign: false,
            },
        );

        assert_eq!(
            db.lookup_type_in_module("main", &TokenSource::from("MyStruct")),
            Some(int_id)
        );
        let var_lookup = db.lookup_var_in_module("main", &TokenSource::from("g_val"));
        assert!(var_lookup.is_some());
        assert_eq!(var_lookup.unwrap().ty, int_id);
    }
}
