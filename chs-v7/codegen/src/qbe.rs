use ir::{
    EnumLayout, Function, InstId, Instruction, Module, Operand, StructLayout, Type as TypeID,
    type_layout,
};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use types::{self as t, Type as LangType, TypeDatabase};

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeMapMode {
    Abi,
    Extended,
    Base,
}

pub struct QbeTranspiler<'a, 'b> {
    module: &'a Module,
    type_db: &'b TypeDatabase,
    output: String,
    string_map: HashMap<std::rc::Rc<str>, usize>,
}

fn escape_string(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\x08' => escaped.push_str("\\b"),
            '\x0c' => escaped.push_str("\\f"),
            c if (c as u32) < 32 || (c as u32) == 127 => {
                escaped.push_str(&format!("\\{:03o}", c as u32));
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

fn collect_string_literals(
    type_db: &TypeDatabase,
    module: &Module,
) -> HashMap<std::rc::Rc<str>, usize> {
    let mut map = HashMap::new();
    let mut next_id = 0;

    let mut add_str = |s: &std::rc::Rc<str>| {
        map.entry(s.clone()).or_insert_with(|| {
            let id = next_id;
            next_id += 1;
            id
        });
    };

    // Collect reflection strings
    let mut visited = std::collections::HashSet::new();
    let mut queue: Vec<t::TypeID> = type_db.queried_types.iter().copied().collect();
    while let Some(ty) = queue.pop() {
        let canonical = type_db.resolve(ty);
        if visited.insert(canonical) {
            let type_name = type_db.type_to_string(canonical);
            add_str(&std::rc::Rc::from(type_name));

            match type_db.get_type(canonical) {
                t::Type::Pointer(inner) => {
                    queue.push(*inner);
                }
                t::Type::Array(inner, _) => {
                    queue.push(*inner);
                }
                t::Type::Slice(inner) | t::Type::DynArray(inner) => {
                    queue.push(*inner);
                }
                t::Type::Struct {
                    fields: Some(fields),
                    ..
                } => {
                    for field in fields {
                        add_str(&std::rc::Rc::from(field.name.as_str()));
                        queue.push(field.ty);
                    }
                }
                t::Type::Enum { variants, .. } => {
                    for variant in variants {
                        add_str(&std::rc::Rc::from(variant.name.as_str()));
                    }
                }
                t::Type::FnPointer {
                    params,
                    return_type,
                } => {
                    for param in params {
                        queue.push(*param);
                    }
                    queue.push(*return_type);
                }
                _ => {}
            }
        }
    }

    for func in module.functions().values() {
        if let Function::Default { instructions, .. } = func {
            for inst_data in instructions {
                match &inst_data.inst {
                    Instruction::Add(l, r)
                    | Instruction::Sub(l, r)
                    | Instruction::Mul(l, r)
                    | Instruction::Div(l, r)
                    | Instruction::Mod(l, r)
                    | Instruction::Eq(_, l, r)
                    | Instruction::NotEq(_, l, r)
                    | Instruction::Lt(_, l, r)
                    | Instruction::LtEq(_, l, r)
                    | Instruction::Gt(_, l, r)
                    | Instruction::GtEq(_, l, r)
                    | Instruction::And(l, r)
                    | Instruction::Or(l, r)
                    | Instruction::BitAnd(l, r)
                    | Instruction::BitOr(l, r)
                    | Instruction::BitXor(l, r)
                    | Instruction::Index(l, r)
                    | Instruction::GetIndexPtr(l, r)
                    | Instruction::Store(_, l, r) => {
                        if let Operand::String(s) = l {
                            add_str(s);
                        }
                        if let Operand::String(s) = r {
                            add_str(s);
                        }
                    }
                    Instruction::Neg(o)
                    | Instruction::Not(o)
                    | Instruction::Load(o)
                    | Instruction::CondBr(o, _, _)
                    | Instruction::GetMemberPtr(o, _)
                    | Instruction::Cast(o) => {
                        if let Operand::String(s) = o {
                            add_str(s);
                        }
                    }
                    Instruction::Return(Some(Operand::String(s))) => {
                        add_str(s);
                    }
                    Instruction::Call(_callee, args) => {
                        // if let Operand::String(s) = callee {
                        //     add_str(s);
                        // }
                        for arg in args.as_ref() {
                            if let Operand::String(s) = arg {
                                add_str(s);
                            }
                        }
                    }
                    Instruction::OOBCheck(Operand::String(s), ..) => {
                        add_str(s);
                    }
                    _ => {}
                }
            }
        }
    }
    map
}

impl<'a, 'b> QbeTranspiler<'a, 'b> {
    pub fn new(module: &'a Module, type_db: &'b TypeDatabase) -> Self {
        let string_map = collect_string_literals(type_db, module);
        Self {
            module,
            type_db,
            string_map,
            output: String::new(),
        }
    }

    pub fn transpile(mut self) -> String {
        self.emit_types();
        self.emit_strings();
        self.emit_reflection_data();

        let mut funcs: Vec<&Function> = self.module.functions().values().collect();
        funcs.sort_by_key(|f| f.name().as_str());
        for func in funcs {
            if func.is_default() {
                self.emit_function(func);
            }
        }

        self.emit_globals();

        self.output
    }

    fn emit_globals(&mut self) {
        let globals = self.module.globals();
        for g in globals {
            if g.is_foreign {
                continue;
            }
            let (size, align) = ir::types::type_layout(g.ty, self.type_db);
            let prefix = if g.is_thread_local {
                "thread "
            } else if g.is_export {
                "export "
            } else {
                ""
            };

            write!(
                self.output,
                "{}data ${} = align {} {{ ",
                prefix, g.name, align
            )
            .unwrap();
            match g.init_val {
                ir::module::ConstVal::Int(v) => {
                    if size == 8 {
                        write!(self.output, "l {}", v).unwrap();
                    } else if size == 4 {
                        write!(self.output, "w {}", v).unwrap();
                    } else if size == 2 {
                        write!(self.output, "h {}", v).unwrap();
                    } else if size == 1 {
                        write!(self.output, "b {}", v).unwrap();
                    } else {
                        write!(self.output, "z {}", size).unwrap();
                    }
                }
                ir::module::ConstVal::Float(v) => {
                    let mut s = format!("{}", v);
                    if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                        s.push_str(".0");
                    }
                    if size == 4 {
                        write!(self.output, "s {}", s).unwrap();
                    } else {
                        write!(self.output, "d {}", s).unwrap();
                    }
                }
                ir::module::ConstVal::Bool(v) => {
                    let val = if v { 1 } else { 0 };
                    write!(self.output, "b {}", val).unwrap();
                }
                ir::module::ConstVal::Zero => {
                    write!(self.output, "z {}", size).unwrap();
                }
            }
            writeln!(self.output, " }}").unwrap();
        }
    }

    fn emit_types(&mut self) {
        writeln!(self.output, "type :string = align 8 {{ l, l }}").unwrap();

        // 1. Emit all slice and dyn_array types first (since they don't have layout dependencies on other types)
        let mut slices = Vec::new();
        let mut dyn_arrays = Vec::new();
        for &id in self.type_db.types.keys() {
            let canonical = self.type_db.resolve(id);
            match self.type_db.get_type(canonical) {
                LangType::Slice(inner) => slices.push(*inner),
                LangType::DynArray(inner) => dyn_arrays.push(*inner),
                _ => {}
            }
        }
        slices.sort();
        slices.dedup();
        dyn_arrays.sort();
        dyn_arrays.dedup();

        for &inner in &slices {
            let mangled_inner = self.mangle_type(inner);
            writeln!(
                self.output,
                "type :chs_slice_{} = align 8 {{ l, l }}",
                mangled_inner
            )
            .unwrap();
        }

        for &inner in &dyn_arrays {
            let mangled_inner = self.mangle_type(inner);
            writeln!(
                self.output,
                "type :chs_dyn_array_{} = align 8 {{ l, l, l }}",
                mangled_inner
            )
            .unwrap();
        }

        // 2. Gather all structs and arrays to define
        let mut types_to_define = Vec::new();
        // Add all structs and enums
        for &type_id in self.type_db.types.keys() {
            let canonical = self.type_db.resolve(type_id);
            match self.type_db.get_type(canonical) {
                LangType::Struct {
                    fields: Some(_), ..
                } => {
                    if !types_to_define.contains(&canonical) {
                        types_to_define.push(canonical);
                    }
                }
                LangType::Enum { .. } => {
                    if !types_to_define.contains(&canonical) {
                        types_to_define.push(canonical);
                    }
                }
                _ => {}
            }
        }
        // Add all arrays and tuples
        for &id in self.type_db.types.keys() {
            let canonical = self.type_db.resolve(id);
            match self.type_db.get_type(canonical) {
                LangType::Array(..) | LangType::Tuple(..) => {
                    types_to_define.push(canonical);
                }
                _ => {}
            }
        }
        // Deduplicate
        types_to_define.sort();
        types_to_define.dedup();

        // 3. Perform topological sort
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();
        let mut order = Vec::new();

        fn visit(
            ty: t::TypeID,
            type_db: &TypeDatabase,
            visited: &mut HashSet<t::TypeID>,
            temp_visited: &mut HashSet<t::TypeID>,
            order: &mut Vec<t::TypeID>,
        ) {
            let canonical = type_db.resolve(ty);
            if temp_visited.contains(&canonical) {
                return;
            }
            if !visited.contains(&canonical) {
                temp_visited.insert(canonical);

                match type_db.get_type(canonical) {
                    LangType::Struct {
                        fields: Some(fields),
                        ..
                    } => {
                        for field in fields {
                            let field_canon = type_db.resolve(field.ty);
                            match type_db.get_type(field_canon) {
                                LangType::Struct { name, .. } if name != "string" => {
                                    visit(field_canon, type_db, visited, temp_visited, order);
                                }
                                LangType::Array(..)
                                | LangType::Tuple(..)
                                | LangType::Enum { .. } => {
                                    visit(field_canon, type_db, visited, temp_visited, order);
                                }
                                _ => {}
                            }
                        }
                    }
                    LangType::Array(inner, _) => {
                        let inner_canon = type_db.resolve(*inner);
                        match type_db.get_type(inner_canon) {
                            LangType::Struct { name, .. } if name != "string" => {
                                visit(inner_canon, type_db, visited, temp_visited, order);
                            }
                            LangType::Array(..) | LangType::Tuple(..) | LangType::Enum { .. } => {
                                visit(inner_canon, type_db, visited, temp_visited, order);
                            }
                            _ => {}
                        }
                    }
                    LangType::Tuple(elements) => {
                        for &elem in elements {
                            let elem_canon = type_db.resolve(elem);
                            match type_db.get_type(elem_canon) {
                                LangType::Struct { name, .. } if name != "string" => {
                                    visit(elem_canon, type_db, visited, temp_visited, order);
                                }
                                LangType::Array(..)
                                | LangType::Tuple(..)
                                | LangType::Enum { .. } => {
                                    visit(elem_canon, type_db, visited, temp_visited, order);
                                }
                                _ => {}
                            }
                        }
                    }
                    LangType::Enum { repr, variants, .. } => {
                        let repr_canon = type_db.resolve(*repr);
                        match type_db.get_type(repr_canon) {
                            LangType::Struct { name, .. } if name != "string" => {
                                visit(repr_canon, type_db, visited, temp_visited, order);
                            }
                            LangType::Array(..) | LangType::Tuple(..) | LangType::Enum { .. } => {
                                visit(repr_canon, type_db, visited, temp_visited, order);
                            }
                            _ => {}
                        }
                        for variant in variants {
                            if let Some(payload_ty) = variant.payload {
                                let payload_canon = type_db.resolve(payload_ty);
                                match type_db.get_type(payload_canon) {
                                    LangType::Struct { name, .. } if name != "string" => {
                                        visit(payload_canon, type_db, visited, temp_visited, order);
                                    }
                                    LangType::Array(..)
                                    | LangType::Tuple(..)
                                    | LangType::Enum { .. } => {
                                        visit(payload_canon, type_db, visited, temp_visited, order);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }

                temp_visited.remove(&canonical);
                visited.insert(canonical);
                order.push(canonical);
            }
        }

        for &ty in &types_to_define {
            visit(
                ty,
                self.type_db,
                &mut visited,
                &mut temp_visited,
                &mut order,
            );
        }

        // 4. Emit the sorted types
        for ty in order {
            let canonical = self.type_db.resolve(ty);
            match self.type_db.get_type(canonical) {
                LangType::Struct {
                    name,
                    fields: Some(_),
                    ..
                } => {
                    let layout = StructLayout::compute(canonical, self.type_db);
                    write!(self.output, "type :{} = align {} {{ ", name, layout.align).unwrap();
                    if let LangType::Struct {
                        fields: Some(fields),
                        ..
                    } = self.type_db.get_type(canonical)
                    {
                        for (i, field) in fields.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.output.push_str(&self.map_extended_type(field.ty));
                        }
                    }
                    self.output.push_str(" }\n");
                }
                LangType::Tuple(elements) => {
                    // Lower structural tuples as anonymous, flat, byte-aligned struct definitions in QBE.
                    // QBE's ABI handler automatically decides whether small tuples/structs are passed/returned
                    // via registers or whether large aggregates are passed via caller-allocated stack storage
                    // with a pointer passed as an implicit first argument, conforming to the target C ABI.
                    let name = self.mangle_type(canonical);
                    let layout = StructLayout::compute(canonical, self.type_db);
                    write!(self.output, "type :{} = align {} {{ ", name, layout.align).unwrap();
                    for (i, &elem) in elements.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.output.push_str(&self.map_extended_type(elem));
                    }
                    self.output.push_str(" }\n");
                }
                LangType::Array(inner, size) => {
                    let mangled_inner = self.mangle_type(*inner);
                    let elem_qbe = self.map_extended_type(*inner);
                    let (_, elem_align) = type_layout(*inner, self.type_db);
                    writeln!(
                        self.output,
                        "type :chs_array_{}_{} = align {} {{ {} {} }}",
                        mangled_inner, size, elem_align, elem_qbe, size
                    )
                    .unwrap();
                }
                LangType::Enum { name, repr, .. } => {
                    let layout = EnumLayout::compute(canonical, self.type_db);
                    let tag_qbe = self.map_extended_type(*repr);
                    let (tag_size, _) = type_layout(*repr, self.type_db);
                    if layout.size > tag_size {
                        let payload_size = layout.size - tag_size;
                        writeln!(
                            self.output,
                            "type :{} = align {} {{ {}, b {} }}",
                            name, layout.align, tag_qbe, payload_size
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            self.output,
                            "type :{} = align {} {{ {} }}",
                            name, layout.align, tag_qbe
                        )
                        .unwrap();
                    }
                }
                _ => {}
            }
        }
        self.output.push('\n');
    }

    fn emit_strings(&mut self) {
        let mut strings: Vec<(&std::rc::Rc<str>, &usize)> = self.string_map.iter().collect();
        strings.sort_by_key(|&(_, id)| *id);
        for (val, &id) in strings {
            let escaped = escape_string(val);
            writeln!(
                self.output,
                "data $str_data_{} = {{ b \"{}\", b 0 }}",
                id, escaped
            )
            .unwrap();
            writeln!(
                self.output,
                "data $str_{} = align 8 {{ l $str_data_{}, l {} }}",
                id,
                id,
                val.len()
            )
            .unwrap();
        }
        if !self.string_map.is_empty() {
            self.output.push('\n');
        }
    }

    fn map_type_impl(&self, ty: t::TypeID, mode: TypeMapMode) -> String {
        let canonical = self.type_db.resolve(ty);
        if canonical == self.type_db.f64() {
            return "d".to_string();
        }
        if canonical == self.type_db.f32() {
            return "s".to_string();
        }

        if canonical == self.type_db.void() {
            return match mode {
                TypeMapMode::Abi => "w".to_string(),
                TypeMapMode::Extended => "b".to_string(),
                TypeMapMode::Base => "".to_string(),
            };
        }
        if canonical == self.type_db.u8() || canonical == self.type_db.bool() {
            return match mode {
                TypeMapMode::Abi | TypeMapMode::Base => "w".to_string(),
                TypeMapMode::Extended => "b".to_string(),
            };
        }
        if canonical == self.type_db.int() {
            return "w".to_string();
        }
        if canonical == self.type_db.usize() {
            return "l".to_string();
        }

        match self.type_db.get_type(canonical) {
            LangType::Void
            | LangType::Bool
            | LangType::NoReturn
            | LangType::UntypedInt
            | LangType::Module => "w".to_string(),
            LangType::Float(bitsize) => match bitsize {
                t::FloatBitSize::_32 => "s".to_string(),
                t::FloatBitSize::_64 => "d".to_string(),
            },
            LangType::UntypedFloat => "d".to_string(),
            LangType::Integer(_, bitsize) => match bitsize {
                t::BitSize::_8 => match mode {
                    TypeMapMode::Abi | TypeMapMode::Base => "w".to_string(),
                    TypeMapMode::Extended => "b".to_string(),
                },
                t::BitSize::_16 => match mode {
                    TypeMapMode::Abi | TypeMapMode::Base => "w".to_string(),
                    TypeMapMode::Extended => "h".to_string(),
                },
                t::BitSize::_32 => "w".to_string(),
                t::BitSize::_64 => "l".to_string(),
            },
            LangType::Pointer(_) | LangType::FnPointer { .. } | LangType::TypeVar(_) => {
                "l".to_string()
            }
            LangType::Distinct { base, .. } => self.map_type_impl(*base, mode),
            other => {
                if mode == TypeMapMode::Base {
                    "l".to_string()
                } else {
                    match other {
                        LangType::Array(..) | LangType::Slice(..) | LangType::DynArray(..) => {
                            let mangled = self.mangle_type(canonical);
                            format!(":chs_{}", mangled)
                        }
                        LangType::Struct { name, .. } | LangType::Enum { name, .. } => {
                            format!(":{}", name)
                        }
                        LangType::Tuple(..) => {
                            let mangled = self.mangle_type(canonical);
                            format!(":{}", mangled)
                        }
                        LangType::Any(_) => ":Any".to_string(),
                        LangType::String => ":string".to_string(),
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    fn map_abi_type(&self, ty: t::TypeID) -> String {
        self.map_type_impl(ty, TypeMapMode::Abi)
    }

    fn map_extended_type(&self, ty: t::TypeID) -> String {
        self.map_type_impl(ty, TypeMapMode::Extended)
    }

    fn map_base_type(&self, ty: t::TypeID) -> String {
        self.map_type_impl(ty, TypeMapMode::Base)
    }

    fn get_load_inst(&self, ty: t::TypeID) -> &'static str {
        let canonical = self.type_db.resolve(ty);
        if canonical == self.type_db.int() {
            "loadw"
        } else if canonical == self.type_db.usize() {
            "loadl"
        } else if canonical == self.type_db.f64() {
            "loadd"
        } else if canonical == self.type_db.f32() {
            "loads"
        } else if canonical == self.type_db.u8() || canonical == self.type_db.bool() {
            "loadub"
        } else {
            match self.type_db.get_type(canonical) {
                LangType::Float(bitsize) => match bitsize {
                    t::FloatBitSize::_32 => "loads",
                    t::FloatBitSize::_64 => "loadd",
                },
                LangType::UntypedFloat => "loadd",
                LangType::Pointer(_) | LangType::FnPointer { .. } | LangType::TypeVar(_) => "loadl",
                LangType::Distinct { base, .. } => self.get_load_inst(*base),
                LangType::Enum { repr, .. } => self.get_load_inst(*repr),
                _ => "loadw",
            }
        }
    }

    fn get_store_inst(&self, ty: t::TypeID) -> &'static str {
        let canonical = self.type_db.resolve(ty);
        if canonical == self.type_db.int() {
            "storew"
        } else if canonical == self.type_db.usize() {
            "storel"
        } else if canonical == self.type_db.f64() {
            "stored"
        } else if canonical == self.type_db.f32() {
            "stores"
        } else if canonical == self.type_db.u8() || canonical == self.type_db.bool() {
            "storeb"
        } else {
            match self.type_db.get_type(canonical) {
                LangType::Float(bitsize) => match bitsize {
                    t::FloatBitSize::_32 => "stores",
                    t::FloatBitSize::_64 => "stored",
                },
                LangType::UntypedFloat => "stored",
                LangType::Pointer(_) | LangType::FnPointer { .. } | LangType::TypeVar(_) => {
                    "storel"
                }
                LangType::Distinct { base, .. } => self.get_store_inst(*base),
                LangType::Enum { repr, .. } => self.get_store_inst(*repr),
                _ => "storew",
            }
        }
    }

    fn get_comparison_suffix(&self, ty: t::TypeID) -> &'static str {
        let canonical = self.type_db.resolve(ty);
        if canonical == self.type_db.f64() {
            "d"
        } else if canonical == self.type_db.f32() {
            "s"
        } else if canonical == self.type_db.int() {
            "w"
        } else if canonical == self.type_db.usize() {
            "l"
        } else {
            match self.type_db.get_type(canonical) {
                LangType::Float(bitsize) => match bitsize {
                    t::FloatBitSize::_32 => "s",
                    t::FloatBitSize::_64 => "d",
                },
                LangType::UntypedFloat => "d",
                LangType::Pointer(_) | LangType::FnPointer { .. } | LangType::TypeVar(_) => "l",
                LangType::Distinct { base, .. } => self.get_comparison_suffix(*base),
                LangType::Enum { repr, .. } => self.get_comparison_suffix(*repr),
                _ => "w",
            }
        }
    }

    fn emit_function_signature(&mut self, func: &Function) {
        let ret_ty = &func.signature().return_type;
        let canonical_ret = self.type_db.resolve(*ret_ty);
        let ret_str =
            if canonical_ret == self.type_db.void() || canonical_ret == self.type_db.noreturn() {
                "".to_string()
            } else {
                format!("{} ", self.map_abi_type(*ret_ty))
            };

        let visibility_str = if func.is_export() {
            "export function"
        } else {
            "function"
        };

        write!(
            self.output,
            "{} {}${}(",
            visibility_str,
            ret_str,
            func.symbol_name()
        )
        .unwrap();
        for (i, param) in func.signature().params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            write!(self.output, "{} %param_{}", self.map_abi_type(*param), i).unwrap();
        }
        self.output.push(')');
    }

    fn emit_function(&mut self, func: &Function) {
        self.emit_function_signature(func);
        let Function::Default {
            blocks,
            instructions,
            ..
        } = func
        else {
            return;
        };
        self.output.push_str(" {\n");

        writeln!(self.output, "@start").unwrap();
        writeln!(self.output, "    jmp @block_0").unwrap();

        for block in blocks {
            writeln!(self.output, "@block_{}", block.id.0).unwrap();
            for inst_id in &block.instructions {
                let inst_data = &instructions[inst_id.0 as usize];
                self.emit_instruction(func, inst_id, &inst_data.inst, &inst_data.ty);
            }
        }

        self.output.push_str("}\n\n");
    }

    fn emit_instruction(&mut self, func: &Function, id: &InstId, inst: &Instruction, ty: &TypeID) {
        let base_ty = self.map_base_type(*ty);

        match inst {
            Instruction::Add(l, r) => {
                let l_ty = self.get_operand_type(l, func);
                let r_ty = self.get_operand_type(r, func);
                let l_canon = self.type_db.resolve(l_ty);
                let r_canon = self.type_db.resolve(r_ty);

                let l_is_ptr = matches!(self.type_db.get_type(l_canon), LangType::Pointer(_));
                let r_is_ptr = matches!(self.type_db.get_type(r_canon), LangType::Pointer(_));

                if l_is_ptr {
                    let LangType::Pointer(inner_ty) = self.type_db.get_type(l_canon) else {
                        unreachable!()
                    };
                    let (raw_size, _) = type_layout(*inner_ty, self.type_db);
                    let elem_size = std::cmp::max(raw_size, 1);

                    if r_canon == self.type_db.usize() {
                        writeln!(
                            self.output,
                            "    %offset_{} =l mul {}, {}",
                            id.0,
                            self.op(r),
                            elem_size
                        )
                        .unwrap();
                    } else {
                        let ext_inst = if r_canon == self.type_db.int()
                            || r_canon == self.type_db.untyped_int()
                        {
                            "extsw"
                        } else {
                            "extuw"
                        };
                        writeln!(
                            self.output,
                            "    %idx_l_{} =l {} {}",
                            id.0,
                            ext_inst,
                            self.op(r)
                        )
                        .unwrap();
                        writeln!(
                            self.output,
                            "    %offset_{} =l mul %idx_l_{}, {}",
                            id.0, id.0, elem_size
                        )
                        .unwrap();
                    }
                    writeln!(
                        self.output,
                        "    %r_{} =l add {}, %offset_{}",
                        id.0,
                        self.op(l),
                        id.0
                    )
                    .unwrap();
                } else if r_is_ptr {
                    let LangType::Pointer(inner_ty) = self.type_db.get_type(r_canon) else {
                        unreachable!()
                    };
                    let (raw_size, _) = type_layout(*inner_ty, self.type_db);
                    let elem_size = std::cmp::max(raw_size, 1);

                    if l_canon == self.type_db.usize() {
                        writeln!(
                            self.output,
                            "    %offset_{} =l mul {}, {}",
                            id.0,
                            self.op(l),
                            elem_size
                        )
                        .unwrap();
                    } else {
                        let ext_inst = if l_canon == self.type_db.int()
                            || l_canon == self.type_db.untyped_int()
                        {
                            "extsw"
                        } else {
                            "extuw"
                        };
                        writeln!(
                            self.output,
                            "    %idx_l_{} =l {} {}",
                            id.0,
                            ext_inst,
                            self.op(l)
                        )
                        .unwrap();
                        writeln!(
                            self.output,
                            "    %offset_{} =l mul %idx_l_{}, {}",
                            id.0, id.0, elem_size
                        )
                        .unwrap();
                    }
                    writeln!(
                        self.output,
                        "    %r_{} =l add {}, %offset_{}",
                        id.0,
                        self.op(r),
                        id.0
                    )
                    .unwrap();
                } else {
                    writeln!(
                        self.output,
                        "    %r_{} ={} add {}, {}",
                        id.0,
                        base_ty,
                        self.op_with_base(l, &base_ty),
                        self.op_with_base(r, &base_ty)
                    )
                    .unwrap();
                }
            }
            Instruction::Sub(l, r) => {
                let l_ty = self.get_operand_type(l, func);
                let r_ty = self.get_operand_type(r, func);
                let l_canon = self.type_db.resolve(l_ty);
                let r_canon = self.type_db.resolve(r_ty);

                let l_is_ptr = matches!(self.type_db.get_type(l_canon), LangType::Pointer(_));
                let r_is_ptr = matches!(self.type_db.get_type(r_canon), LangType::Pointer(_));

                if l_is_ptr && r_is_ptr {
                    let LangType::Pointer(inner_ty) = self.type_db.get_type(l_canon) else {
                        unreachable!()
                    };
                    let (raw_size, _) = type_layout(*inner_ty, self.type_db);
                    let elem_size = std::cmp::max(raw_size, 1);

                    let dest_base = self.map_base_type(*ty);
                    if dest_base == "w" {
                        writeln!(
                            self.output,
                            "    %diff_l_{} =l sub {}, {}",
                            id.0,
                            self.op(l),
                            self.op(r)
                        )
                        .unwrap();
                        writeln!(
                            self.output,
                            "    %div_l_{} =l div %diff_l_{}, {}",
                            id.0, id.0, elem_size
                        )
                        .unwrap();
                        writeln!(self.output, "    %r_{} =w copy %div_l_{}", id.0, id.0).unwrap();
                    } else {
                        writeln!(
                            self.output,
                            "    %diff_{} =l sub {}, {}",
                            id.0,
                            self.op(l),
                            self.op(r)
                        )
                        .unwrap();
                        writeln!(
                            self.output,
                            "    %r_{} =l div %diff_{}, {}",
                            id.0, id.0, elem_size
                        )
                        .unwrap();
                    }
                } else if l_is_ptr {
                    let LangType::Pointer(inner_ty) = self.type_db.get_type(l_canon) else {
                        unreachable!()
                    };
                    let (raw_size, _) = type_layout(*inner_ty, self.type_db);
                    let elem_size = std::cmp::max(raw_size, 1);

                    if r_canon == self.type_db.usize() {
                        writeln!(
                            self.output,
                            "    %offset_{} =l mul {}, {}",
                            id.0,
                            self.op(r),
                            elem_size
                        )
                        .unwrap();
                    } else {
                        let ext_inst = if r_canon == self.type_db.int()
                            || r_canon == self.type_db.untyped_int()
                        {
                            "extsw"
                        } else {
                            "extuw"
                        };
                        writeln!(
                            self.output,
                            "    %idx_l_{} =l {} {}",
                            id.0,
                            ext_inst,
                            self.op(r)
                        )
                        .unwrap();
                        writeln!(
                            self.output,
                            "    %offset_{} =l mul %idx_l_{}, {}",
                            id.0, id.0, elem_size
                        )
                        .unwrap();
                    }
                    writeln!(
                        self.output,
                        "    %r_{} =l sub {}, %offset_{}",
                        id.0,
                        self.op(l),
                        id.0
                    )
                    .unwrap();
                } else {
                    writeln!(
                        self.output,
                        "    %r_{} ={} sub {}, {}",
                        id.0,
                        base_ty,
                        self.op_with_base(l, &base_ty),
                        self.op_with_base(r, &base_ty)
                    )
                    .unwrap();
                }
            }
            Instruction::Mul(l, r) => {
                let op = "mul";
                writeln!(
                    self.output,
                    "    %r_{} ={} {} {}, {}",
                    id.0,
                    base_ty,
                    op,
                    self.op_with_base(l, &base_ty),
                    self.op_with_base(r, &base_ty)
                )
                .unwrap();
            }
            Instruction::Div(l, r) => {
                let op = "div";
                writeln!(
                    self.output,
                    "    %r_{} ={} {} {}, {}",
                    id.0,
                    base_ty,
                    op,
                    self.op_with_base(l, &base_ty),
                    self.op_with_base(r, &base_ty)
                )
                .unwrap();
            }
            Instruction::Mod(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} ={} rem {}, {}",
                    id.0,
                    base_ty,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }

            Instruction::Eq(comp_ty, l, r)
            | Instruction::NotEq(comp_ty, l, r)
            | Instruction::Lt(comp_ty, l, r)
            | Instruction::LtEq(comp_ty, l, r)
            | Instruction::Gt(comp_ty, l, r)
            | Instruction::GtEq(comp_ty, l, r) => {
                let canonical_comp = self.type_db.resolve(*comp_ty);
                if let LangType::Enum { repr, .. } = self.type_db.get_type(canonical_comp) {
                    let load_inst = self.get_load_inst(*repr);

                    writeln!(
                        self.output,
                        "    %tag_l_{} =w {} {}",
                        id.0,
                        load_inst,
                        self.op(l)
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "    %tag_r_{} =w {} {}",
                        id.0,
                        load_inst,
                        self.op(r)
                    )
                    .unwrap();

                    let op_name = match inst {
                        Instruction::Eq(..) => "ceqw",
                        Instruction::NotEq(..) => "cnew",
                        Instruction::Lt(..) => "csltw",
                        Instruction::LtEq(..) => "cslew",
                        Instruction::Gt(..) => "csgtw",
                        Instruction::GtEq(..) => "csgew",
                        _ => unreachable!(),
                    };
                    writeln!(
                        self.output,
                        "    %r_{} =w {} %tag_l_{}, %tag_r_{}",
                        id.0, op_name, id.0, id.0
                    )
                    .unwrap();
                } else {
                    let comp_suffix = self.get_comparison_suffix(*comp_ty);
                    let is_float = comp_suffix == "d" || comp_suffix == "s";

                    let op_name = match inst {
                        Instruction::Eq(..) => format!("ceq{}", comp_suffix),
                        Instruction::NotEq(..) => format!("cne{}", comp_suffix),
                        Instruction::Lt(..) => {
                            if is_float {
                                format!("clt{}", comp_suffix)
                            } else {
                                format!("cslt{}", comp_suffix)
                            }
                        }
                        Instruction::LtEq(..) => {
                            if is_float {
                                format!("cle{}", comp_suffix)
                            } else {
                                format!("csle{}", comp_suffix)
                            }
                        }
                        Instruction::Gt(..) => {
                            if is_float {
                                format!("cgt{}", comp_suffix)
                            } else {
                                format!("csgt{}", comp_suffix)
                            }
                        }
                        Instruction::GtEq(..) => {
                            if is_float {
                                format!("cge{}", comp_suffix)
                            } else {
                                format!("csge{}", comp_suffix)
                            }
                        }
                        _ => unreachable!(),
                    };

                    writeln!(
                        self.output,
                        "    %r_{} =w {} {}, {}",
                        id.0,
                        op_name,
                        self.op_with_base(l, comp_suffix),
                        self.op_with_base(r, comp_suffix)
                    )
                    .unwrap();
                }
            }

            Instruction::And(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w and {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::Or(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w or {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::BitAnd(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w and {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::BitOr(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w or {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::BitXor(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w xor {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }

            Instruction::Neg(op) => {
                let op_ty = self.get_operand_type(op, func);
                let op_base_ty = self.map_base_type(op_ty);
                if op_base_ty == "d" {
                    writeln!(
                        self.output,
                        "    %r_{} =d sub d_0.0, {}",
                        id.0,
                        self.op_with_base(op, "d")
                    )
                    .unwrap();
                } else if op_base_ty == "s" {
                    writeln!(
                        self.output,
                        "    %r_{} =s sub s_0.0, {}",
                        id.0,
                        self.op_with_base(op, "s")
                    )
                    .unwrap();
                } else {
                    writeln!(
                        self.output,
                        "    %r_{} ={} sub 0, {}",
                        id.0,
                        op_base_ty,
                        self.op(op)
                    )
                    .unwrap();
                }
            }
            Instruction::Not(op) => {
                writeln!(self.output, "    %r_{} =w ceqw {}, 0", id.0, self.op(op)).unwrap();
            }
            Instruction::Cast(op) => {
                let src_ty = self.get_operand_type(op, func);
                let dest_ty = *ty;

                let is_src_agg = self.is_aggregate(src_ty);
                let is_dest_agg = self.is_aggregate(dest_ty);

                if is_src_agg && is_dest_agg {
                    // Aggregate-to-aggregate cast (e.g. distinct struct type casts)
                    let (size, align) = type_layout(dest_ty, self.type_db);
                    let alloc_inst = if align >= 16 {
                        "alloc16"
                    } else if align == 8 {
                        "alloc8"
                    } else {
                        "alloc4"
                    };
                    writeln!(self.output, "    %r_{} =l {} {}", id.0, alloc_inst, size).unwrap();
                    writeln!(
                        self.output,
                        "    call $memcpy(l %r_{}, l {}, l {})",
                        id.0,
                        self.op(op),
                        size
                    )
                    .unwrap();
                } else if !is_src_agg && is_dest_agg {
                    // cast primitive to aggregate (e.g. int -> Enum)
                    let (size, align) = type_layout(dest_ty, self.type_db);
                    let alloc_inst = if align >= 16 {
                        "alloc16"
                    } else if align == 8 {
                        "alloc8"
                    } else {
                        "alloc4"
                    };
                    writeln!(self.output, "    %r_{} =l {} {}", id.0, alloc_inst, size).unwrap();

                    let canonical_dest = self.type_db.resolve(dest_ty);
                    if let LangType::Enum { repr, .. } = self.type_db.get_type(canonical_dest) {
                        let store_inst = self.get_store_inst(*repr);
                        writeln!(
                            self.output,
                            "    {} {}, %r_{}",
                            store_inst,
                            self.op(op),
                            id.0
                        )
                        .unwrap();
                    } else {
                        writeln!(self.output, "    storew {}, %r_{}", self.op(op), id.0).unwrap();
                    }
                } else {
                    // normal cast or aggregate-to-primitive cast
                    let src_base = if is_src_agg {
                        "w".to_string()
                    } else {
                        self.map_base_type(src_ty)
                    };

                    let src_val = if is_src_agg {
                        let load_inst = self.get_load_inst(src_ty);
                        let temp_reg = format!("%cast_src_{}", id.0);
                        writeln!(
                            self.output,
                            "    {} =w {} {}",
                            temp_reg,
                            load_inst,
                            self.op(op)
                        )
                        .unwrap();
                        temp_reg
                    } else {
                        self.op_with_base(op, &src_base)
                    };
                    let dest_base = self.map_base_type(dest_ty);

                    if src_base == dest_base {
                        writeln!(
                            self.output,
                            "    %r_{} ={} copy {}",
                            id.0, dest_base, src_val
                        )
                        .unwrap();
                    } else if src_base == "w" && dest_base == "l" {
                        let canonical_src = self.type_db.resolve(src_ty);
                        let repr_is_int = if let LangType::Enum { repr, .. } =
                            self.type_db.get_type(canonical_src)
                        {
                            let canon_repr = self.type_db.resolve(*repr);
                            canon_repr == self.type_db.int()
                                || canon_repr == self.type_db.untyped_int()
                        } else {
                            canonical_src == self.type_db.int()
                                || canonical_src == self.type_db.untyped_int()
                        };
                        let ext_inst = if repr_is_int { "extsw" } else { "extuw" };
                        writeln!(self.output, "    %r_{} =l {} {}", id.0, ext_inst, src_val)
                            .unwrap();
                    } else if src_base == "l" && dest_base == "w" {
                        writeln!(self.output, "    %r_{} =w copy {}", id.0, src_val).unwrap();
                    } else if src_base == "w" && dest_base == "s" {
                        writeln!(self.output, "    %r_{} =s swtof {}", id.0, src_val).unwrap();
                    } else if src_base == "l" && dest_base == "s" {
                        writeln!(self.output, "    %r_{} =s sltof {}", id.0, src_val).unwrap();
                    } else if src_base == "w" && dest_base == "d" {
                        writeln!(self.output, "    %r_{} =d swtod {}", id.0, src_val).unwrap();
                    } else if src_base == "l" && dest_base == "d" {
                        writeln!(self.output, "    %r_{} =d sltod {}", id.0, src_val).unwrap();
                    } else if src_base == "s" && dest_base == "w" {
                        writeln!(self.output, "    %r_{} =w stosi {}", id.0, src_val).unwrap();
                    } else if src_base == "s" && dest_base == "l" {
                        writeln!(self.output, "    %r_{} =l stosl {}", id.0, src_val).unwrap();
                    } else if src_base == "d" && dest_base == "w" {
                        writeln!(self.output, "    %r_{} =w dtosi {}", id.0, src_val).unwrap();
                    } else if src_base == "d" && dest_base == "l" {
                        writeln!(self.output, "    %r_{} =l dtosl {}", id.0, src_val).unwrap();
                    } else if src_base == "s" && dest_base == "d" {
                        writeln!(self.output, "    %r_{} =d exts {}", id.0, src_val).unwrap();
                    } else if src_base == "d" && dest_base == "s" {
                        writeln!(self.output, "    %r_{} =s truncd {}", id.0, src_val).unwrap();
                    } else {
                        panic!("Unsupported QBE cast from {} to {}", src_base, dest_base);
                    }
                }
            }

            Instruction::Alloca(alloc_ty) => {
                let (size, align) = type_layout(*alloc_ty, self.type_db);
                let size = std::cmp::max(size, 1);
                let alloc_inst = if align >= 16 {
                    "alloc16"
                } else if align == 8 {
                    "alloc8"
                } else {
                    "alloc4"
                };
                writeln!(self.output, "    %r_{} =l {} {}", id.0, alloc_inst, size).unwrap();
            }

            Instruction::Load(ptr) => {
                let val_ty = *ty;
                if self.is_aggregate(val_ty) {
                    let (size, align) = type_layout(val_ty, self.type_db);
                    let alloc_inst = if align >= 16 {
                        "alloc16"
                    } else if align == 8 {
                        "alloc8"
                    } else {
                        "alloc4"
                    };
                    writeln!(self.output, "    %r_{} =l {} {}", id.0, alloc_inst, size).unwrap();
                    writeln!(
                        self.output,
                        "    call $memcpy(l %r_{}, l {}, l {})",
                        id.0,
                        self.op(ptr),
                        size
                    )
                    .unwrap();
                } else {
                    let underlying_val = self.type_db.get_underlying_type(val_ty);
                    let load_inst = self.get_load_inst(underlying_val);
                    writeln!(
                        self.output,
                        "    %r_{} ={} {} {}",
                        id.0,
                        base_ty,
                        load_inst,
                        self.op(ptr)
                    )
                    .unwrap();
                }
            }

            Instruction::Store(val_ty, ptr, val) => {
                let underlying_val = self.type_db.get_underlying_type(*val_ty);
                if self.is_aggregate(underlying_val) {
                    let (size, _) = type_layout(underlying_val, self.type_db);
                    writeln!(
                        self.output,
                        "    call $memcpy(l {}, l {}, l {})",
                        self.op(ptr),
                        self.op(val),
                        size
                    )
                    .unwrap();
                } else {
                    let store_inst = self.get_store_inst(underlying_val);
                    let val_base = self.map_base_type(underlying_val);
                    writeln!(
                        self.output,
                        "    {} {}, {}",
                        store_inst,
                        self.op_with_base(val, &val_base),
                        self.op(ptr)
                    )
                    .unwrap();
                }
            }

            Instruction::Index(arr, idx) => {
                let arr_ty = self.get_operand_type(arr, func);
                let canonical_arr = self.type_db.resolve(arr_ty);
                let elem_ty = match self.type_db.get_type(canonical_arr) {
                    LangType::Pointer(inner) => *inner,
                    _ => panic!("Expected pointer type for Index"),
                };
                let (elem_size, _) = type_layout(elem_ty, self.type_db);

                writeln!(self.output, "    %idx_l_{} =l extsw {}", id.0, self.op(idx)).unwrap();
                writeln!(
                    self.output,
                    "    %offset_{} =l mul %idx_l_{}, {}",
                    id.0, id.0, elem_size
                )
                .unwrap();
                writeln!(
                    self.output,
                    "    %elem_ptr_{} =l add {}, %offset_{}",
                    id.0,
                    self.op(arr),
                    id.0
                )
                .unwrap();

                let canonical_elem = self.type_db.resolve(elem_ty);
                if self.is_aggregate(elem_ty) {
                    writeln!(self.output, "    %r_{} =l copy %elem_ptr_{}", id.0, id.0).unwrap();
                } else {
                    let load_inst = self.get_load_inst(canonical_elem);
                    writeln!(
                        self.output,
                        "    %r_{} ={} {} %elem_ptr_{}",
                        id.0, base_ty, load_inst, id.0
                    )
                    .unwrap();
                }
            }

            Instruction::GetMemberPtr(obj, offset) => {
                writeln!(
                    self.output,
                    "    %r_{} =l add {}, {}",
                    id.0,
                    self.op(obj),
                    offset
                )
                .unwrap();
            }

            Instruction::GetIndexPtr(arr, idx) => {
                let arr_ty = self.get_operand_type(arr, func);
                let canonical_arr = self.type_db.resolve(arr_ty);
                let elem_ty = match self.type_db.get_type(canonical_arr) {
                    LangType::Pointer(inner) => {
                        let canonical_inner = self.type_db.resolve(*inner);
                        match self.type_db.get_type(canonical_inner) {
                            LangType::Array(inner_elem, _) => *inner_elem,
                            LangType::Slice(inner_elem) | LangType::DynArray(inner_elem) => {
                                *inner_elem
                            }
                            _ => *inner,
                        }
                    }
                    _ => panic!("Expected pointer type for GetIndexPtr"),
                };
                let (elem_size, _) = type_layout(elem_ty, self.type_db);

                writeln!(self.output, "    %idx_l_{} =l extsw {}", id.0, self.op(idx)).unwrap();
                writeln!(
                    self.output,
                    "    %offset_{} =l mul %idx_l_{}, {}",
                    id.0, id.0, elem_size
                )
                .unwrap();
                writeln!(
                    self.output,
                    "    %r_{} =l add {}, %offset_{}",
                    id.0,
                    self.op(arr),
                    id.0
                )
                .unwrap();
            }
            Instruction::OOBCheck(msg, idx, len) => {
                let mut arg_strings = Vec::new();
                for arg in [msg, idx, len] {
                    let arg_ty = self.get_operand_type(arg, func);
                    let arg_abi = self.map_abi_type(arg_ty);
                    arg_strings.push(format!("{} {}", arg_abi, self.op(arg)));
                }

                let args_formatted = arg_strings.join(", ");

                writeln!(self.output, "    call $chs__oob_check({})", args_formatted).unwrap();
            }
            Instruction::DynArrayGrow(ptr, elem_size) => {
                let mut arg_strings = Vec::new();
                for arg in [ptr, elem_size] {
                    let arg_ty = self.get_operand_type(arg, func);
                    let arg_abi = self.map_abi_type(arg_ty);
                    arg_strings.push(format!("{} {}", arg_abi, self.op(arg)));
                }

                let args_formatted = arg_strings.join(", ");

                writeln!(
                    self.output,
                    "    call $chs_dyn_array_grow({})",
                    args_formatted
                )
                .unwrap();
            }
            Instruction::Dealloc(ptr) => {
                let arg_ty = self.get_operand_type(ptr, func);
                let arg_abi = self.map_abi_type(arg_ty);
                writeln!(
                    self.output,
                    "    call $chs_dealloc({} {})",
                    arg_abi,
                    self.op(ptr)
                )
                .unwrap();
            }
            Instruction::Call(callee, args) => {
                let is_indirect = !matches!(callee, Operand::String(_) | Operand::Global(_));

                let (return_type, symbol_name) = if is_indirect {
                    let callee_ty = self.get_operand_type(callee, func);
                    let canonical = self.type_db.resolve(callee_ty);
                    if let LangType::FnPointer { return_type, .. } =
                        self.type_db.get_type(canonical).clone()
                    {
                        (return_type, self.op(callee))
                    } else {
                        panic!("Expected function pointer type for indirect QBE call");
                    }
                } else {
                    let callee_name = match callee {
                        Operand::String(name) => name.clone(),
                        Operand::Global(name) => name.clone(),
                        _ => unreachable!(),
                    };
                    let callee_func = self
                        .module
                        .functions()
                        .get(callee_name.as_ref())
                        .expect("Callee function signature not found");
                    (
                        callee_func.signature().return_type,
                        format!("${}", callee_func.symbol_name()),
                    )
                };

                let canonical_ret = self.type_db.resolve(return_type);
                let mut arg_strings = Vec::new();

                if is_indirect {
                    for arg in args.iter() {
                        let arg_ty = self.get_operand_type(arg, func);
                        let arg_abi = self.map_abi_type(arg_ty);
                        arg_strings.push(format!(
                            "{} {}",
                            arg_abi,
                            self.op_with_base(arg, &arg_abi)
                        ));
                    }
                } else {
                    let callee_name = match callee {
                        Operand::String(name) => name.clone(),
                        Operand::Global(name) => name.clone(),
                        _ => unreachable!(),
                    };
                    let callee_func = self
                        .module
                        .functions()
                        .get(callee_name.as_ref())
                        .expect("Callee function signature not found");
                    let params_len = callee_func.signature().params.len();
                    for (i, arg) in args.iter().enumerate() {
                        if callee_func.signature().has_va_args && i == params_len {
                            arg_strings.push("...".to_string());
                        }
                        let abi_type = if i < params_len {
                            let param_ty = callee_func.signature().params[i];
                            self.map_abi_type(param_ty)
                        } else {
                            let arg_ty = self.get_operand_type(arg, func);
                            self.map_abi_type(arg_ty)
                        };
                        arg_strings.push(format!(
                            "{} {}",
                            abi_type,
                            self.op_with_base(arg, &abi_type)
                        ));
                    }
                }

                let args_formatted = arg_strings.join(", ");

                if canonical_ret == self.type_db.void() {
                    writeln!(self.output, "    call {}({})", symbol_name, args_formatted).unwrap();
                } else {
                    let ret_abi = self.map_abi_type(return_type);
                    writeln!(
                        self.output,
                        "    %r_{} ={} call {}({})",
                        id.0, ret_abi, symbol_name, args_formatted
                    )
                    .unwrap();
                }
            }

            Instruction::Br(target) => {
                writeln!(self.output, "    jmp @block_{}", target.0).unwrap();
            }
            Instruction::CondBr(cond, true_block, false_block) => {
                writeln!(
                    self.output,
                    "    jnz {}, @block_{}, @block_{}",
                    self.op(cond),
                    true_block.0,
                    false_block.0
                )
                .unwrap();
            }
            Instruction::Return(val_opt) => {
                if let Some(val) = val_opt {
                    let ret_ty = func.signature().return_type;
                    let ret_base = self.map_abi_type(ret_ty);
                    writeln!(self.output, "    ret {}", self.op_with_base(val, &ret_base)).unwrap();
                } else {
                    writeln!(self.output, "    ret").unwrap();
                }
            }
        }
    }

    fn get_operand_type(&self, op: &Operand, func: &Function) -> t::TypeID {
        match op {
            Operand::Null => self.type_db.pointer_type(self.type_db.void()).unwrap(),
            Operand::Reg(reg_id) => {
                let Function::Default { instructions, .. } = func else {
                    return self.type_db.void();
                };
                instructions[reg_id.0 as usize].ty
            }
            Operand::Int(_) => self.type_db.int(),
            Operand::Bool(_) => self.type_db.bool(),
            Operand::Float(_) => self.type_db.float(),
            Operand::String(_) => self.type_db.string(),
            Operand::Param(i) => func.signature().params[*i as usize],
            Operand::Global(_) | Operand::ThreadLocalGlobal(_) => {
                let type_info_id = self.type_db.u8();
                self.type_db.pointer_type(type_info_id).unwrap()
            }
        }
    }

    fn mangle_type(&self, ty: t::TypeID) -> String {
        let canonical = self.type_db.resolve(ty);
        match self.type_db.get_type(canonical) {
            t::Type::Void => "void".to_string(),
            t::Type::Bool => "bool".to_string(),
            t::Type::Float(bitsize) => match bitsize {
                t::FloatBitSize::_32 => "f32".to_string(),
                t::FloatBitSize::_64 => "float".to_string(),
            },
            t::Type::UntypedFloat => "float".to_string(),
            t::Type::NoReturn => "noreturn".to_string(),
            t::Type::UntypedInt => "int".to_string(),
            t::Type::Module => "module".to_string(),
            t::Type::Integer(sign, bitsize) => match (sign, bitsize) {
                (t::Sign::Signed, t::BitSize::_8) => "i8".to_string(),
                (t::Sign::Unsigned, t::BitSize::_8) => "u8".to_string(),
                (t::Sign::Signed, t::BitSize::_16) => "i16".to_string(),
                (t::Sign::Unsigned, t::BitSize::_16) => "u16".to_string(),
                (t::Sign::Signed, t::BitSize::_32) => "int".to_string(),
                (t::Sign::Unsigned, t::BitSize::_32) => "u32".to_string(),
                (t::Sign::Signed, t::BitSize::_64) => "i64".to_string(),
                (t::Sign::Unsigned, t::BitSize::_64) => "usize".to_string(),
            },
            t::Type::String => "string".to_string(),
            t::Type::Any(_) => "any".to_string(),
            t::Type::Distinct { name, .. } => format!("distinct_{}", name),
            t::Type::Pointer(inner) => {
                format!("ptr_{}", self.mangle_type(*inner))
            }
            t::Type::Array(inner, size) => {
                format!("array_{}_{}", self.mangle_type(*inner), size)
            }
            t::Type::Slice(inner) => {
                format!("slice_{}", self.mangle_type(*inner))
            }
            t::Type::DynArray(inner) => {
                format!("dyn_array_{}", self.mangle_type(*inner))
            }
            t::Type::Tuple(elements) => {
                let mut parts = vec!["tuple".to_string()];
                for &e in elements {
                    parts.push(self.mangle_type(e));
                }
                parts.join("_")
            }
            t::Type::Struct { name, .. } => {
                format!("struct_{}", name)
            }
            t::Type::Enum { name, .. } => {
                format!("enum_{}", name)
            }
            _ => format!("type_{}", canonical.0),
        }
    }

    fn is_aggregate(&self, ty: t::TypeID) -> bool {
        let canonical = self.type_db.resolve(ty);
        matches!(
            self.type_db.get_type(canonical),
            LangType::Struct { .. }
                | LangType::Enum { .. }
                | LangType::Array(..)
                | LangType::Slice(..)
                | LangType::DynArray(..)
                | LangType::Tuple(..)
                | LangType::String
        )
    }

    fn op_with_base(&self, op: &Operand, base_ty: &str) -> String {
        match op {
            Operand::Float(v) => {
                let mut s = format!("{}", v);
                if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                    s.push_str(".0");
                }
                if base_ty == "s" {
                    format!("s_{}", s)
                } else {
                    format!("d_{}", s)
                }
            }
            _ => self.op(op),
        }
    }

    fn op(&self, op: &Operand) -> String {
        match op {
            Operand::Null => "0".to_string(),
            Operand::Reg(reg_id) => format!("%r_{}", reg_id.0),
            Operand::Int(v) => format!("{}", v),
            Operand::Float(v) => {
                let mut s = format!("{}", v);
                if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                    s.push_str(".0");
                }
                format!("d_{}", s)
            }
            Operand::Bool(v) => {
                if *v {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            Operand::String(s) => {
                let id = self
                    .string_map
                    .get(s)
                    .expect("String literal not found in map");
                format!("$str_{}", id)
            }
            Operand::Param(i) => format!("%param_{}", i),
            Operand::Global(name) => {
                if let Some(func) = self.module.functions().get(name.as_ref()) {
                    format!("${}", func.symbol_name())
                } else {
                    format!("${}", name)
                }
            }
            Operand::ThreadLocalGlobal(name) => {
                format!("thread ${}", name)
            }
        }
    }

    fn emit_reflection_data(&mut self) {
        let mut visited = std::collections::HashSet::new();
        let mut queue: Vec<t::TypeID> = self.type_db.queried_types.iter().copied().collect();
        let mut reflection_types = Vec::new();

        while let Some(ty) = queue.pop() {
            let canonical = self.type_db.resolve(ty);
            if visited.insert(canonical) {
                reflection_types.push(canonical);
                match self.type_db.get_type(canonical) {
                    t::Type::Pointer(inner) => {
                        queue.push(*inner);
                    }
                    t::Type::Array(inner, _) => {
                        queue.push(*inner);
                    }
                    t::Type::Slice(inner) | t::Type::DynArray(inner) => {
                        queue.push(*inner);
                    }
                    t::Type::Struct {
                        fields: Some(fields),
                        ..
                    } => {
                        for field in fields {
                            queue.push(field.ty);
                        }
                    }
                    t::Type::FnPointer {
                        params,
                        return_type,
                    } => {
                        queue.push(*return_type);
                        for param in params {
                            queue.push(*param);
                        }
                    }
                    _ => {}
                }
            }
        }

        if reflection_types.is_empty() {
            return;
        }

        writeln!(self.output, "# --- Reflection Metadata ---").unwrap();

        // Helper to format string slice in QBE data block
        let get_str_data = |s: &str| -> String {
            let id = self.string_map.get(s).expect("Reflection string not found");
            format!("l $str_data_{}, w {}", id, s.len())
        };

        let type_field_id = self
            .type_db
            .lookup_type_in_module("runtime", &t::TokenSource::from("TypeField"))
            .or_else(|| {
                self.type_db
                    .lookup_type_in_module("", &t::TokenSource::from("TypeField"))
            })
            .expect("TypeField not found");
        let canonical_field = self.type_db.resolve(type_field_id);
        let layout_field = StructLayout::compute(canonical_field, self.type_db);
        let fields_of_type_field = match self.type_db.get_type(canonical_field) {
            t::Type::Struct {
                fields: Some(f), ..
            } => f,
            _ => panic!("TypeField is not a struct"),
        };

        let type_enum_variant_id = self
            .type_db
            .lookup_type_in_module("runtime", &t::TokenSource::from("TypeEnumVariant"))
            .or_else(|| {
                self.type_db
                    .lookup_type_in_module("", &t::TokenSource::from("TypeEnumVariant"))
            })
            .expect("TypeEnumVariant not found");
        let canonical_variant = self.type_db.resolve(type_enum_variant_id);
        let layout_variant = StructLayout::compute(canonical_variant, self.type_db);
        let fields_of_variant = match self.type_db.get_type(canonical_variant) {
            t::Type::Struct {
                fields: Some(f), ..
            } => f,
            _ => panic!("TypeEnumVariant is not a struct"),
        };

        let type_info_id = self.type_db.type_info();
        let canonical_info = self.type_db.resolve(type_info_id);
        let layout_info = StructLayout::compute(canonical_info, self.type_db);
        let fields_of_info = match self.type_db.get_type(canonical_info) {
            t::Type::Struct {
                fields: Some(f), ..
            } => f,
            _ => panic!("TypeInfo is not a struct"),
        };

        let get_qbe_field_val = |field_ty: t::TypeID, field_size: u32, raw_val: &str| -> String {
            let canon = self.type_db.resolve(field_ty);
            match self.type_db.get_type(canon) {
                t::Type::String | t::Type::Slice(_) => {
                    format!("{}, z 4", raw_val)
                }
                t::Type::Pointer(_) => {
                    format!("l {}", raw_val)
                }
                _ => {
                    if field_size == 1 {
                        format!("b {}", raw_val)
                    } else if field_size == 4 {
                        format!("w {}", raw_val)
                    } else {
                        format!("l {}", raw_val)
                    }
                }
            }
        };

        let emit_struct_data = |fields: &[t::StructField],
                                layout: &StructLayout,
                                values: &std::collections::HashMap<&str, String>|
         -> String {
            let mut out = String::new();
            let mut current_offset = 0;
            for (idx, field) in fields.iter().enumerate() {
                let fl = &layout.fields[idx];
                if fl.offset > current_offset {
                    out.push_str(&format!("z {}, ", fl.offset - current_offset));
                    current_offset = fl.offset;
                }
                let val_rep = values
                    .get(field.name.as_str())
                    .expect("Value for field not provided");
                let formatted = get_qbe_field_val(field.ty, fl.size, val_rep);
                out.push_str(&format!("{}, ", formatted));
                current_offset += fl.size;
            }
            if layout.size > current_offset {
                out.push_str(&format!("z {}, ", layout.size - current_offset));
            }
            if out.ends_with(", ") {
                out.truncate(out.len() - 2);
            }
            out
        };

        // Emit arrays of fields and variants
        for &ty in &reflection_types {
            let canonical = self.type_db.resolve(ty);
            match self.type_db.get_type(canonical) {
                t::Type::Struct {
                    fields: Some(fields),
                    ..
                } => {
                    if !fields.is_empty() {
                        let layout = StructLayout::compute(canonical, self.type_db);
                        writeln!(self.output, "data $chs_type_fields_{} = align 8 {{", ty.0)
                            .unwrap();
                        for (idx, field) in fields.iter().enumerate() {
                            let field_offset = layout.fields[idx].offset;
                            let field_canon = self.type_db.resolve(field.ty);

                            let mut vals = std::collections::HashMap::new();
                            vals.insert("name", get_str_data(&field.name));
                            vals.insert("offset", field_offset.to_string());
                            vals.insert("type_info", format!("$chs_type_info_{}", field_canon.0));

                            let struct_bytes =
                                emit_struct_data(fields_of_type_field, &layout_field, &vals);
                            writeln!(self.output, "    {},", struct_bytes).unwrap();
                        }
                        writeln!(self.output, "}}").unwrap();
                    }
                }
                t::Type::Enum { variants, .. } if !variants.is_empty() => {
                    writeln!(self.output, "data $chs_type_variants_{} = align 8 {{", ty.0).unwrap();
                    for variant in variants {
                        let mut vals = std::collections::HashMap::new();
                        vals.insert("name", get_str_data(&variant.name));
                        vals.insert("value", variant.default_value.to_string());

                        let struct_bytes =
                            emit_struct_data(fields_of_variant, &layout_variant, &vals);
                        writeln!(self.output, "    {},", struct_bytes).unwrap();
                    }
                    writeln!(self.output, "}}").unwrap();
                }
                _ => {}
            }
        }

        // Emit TypeInfo structures
        for &ty in &reflection_types {
            let canonical = self.type_db.resolve(ty);
            fn get_type_kind(db: &t::TypeDatabase, ty: t::TypeID) -> u32 {
                let canonical = db.resolve(ty);
                match db.types.get(&canonical).unwrap() {
                    t::Type::Void => 1,
                    t::Type::Bool => 2,
                    t::Type::Float(..) | t::Type::UntypedFloat => 3,
                    t::Type::NoReturn => 4,
                    t::Type::UntypedInt => 5,
                    t::Type::Integer(..) => 6,
                    t::Type::Pointer(_) => 7,
                    t::Type::Array(_, _) => 8,
                    t::Type::Slice(_) => 9,
                    t::Type::Struct { .. } => 10,
                    t::Type::Enum { .. } => 11,
                    t::Type::String => 12,
                    t::Type::Any(_) => 13,
                    t::Type::FnPointer { .. } => 14,
                    t::Type::DynArray(_) => 15,
                    t::Type::Distinct { base, .. } => get_type_kind(db, *base),
                    _ => 0,
                }
            }
            let kind = get_type_kind(self.type_db, ty);
            let name = self.type_db.type_to_string(canonical);
            let (size, align) = type_layout(canonical, self.type_db);

            let elem_ptr = match self.type_db.get_type(canonical) {
                t::Type::Pointer(inner)
                | t::Type::Array(inner, _)
                | t::Type::Slice(inner)
                | t::Type::DynArray(inner) => {
                    format!("$chs_type_info_{}", self.type_db.resolve(*inner).0)
                }
                t::Type::FnPointer { return_type, .. } => {
                    format!("$chs_type_info_{}", self.type_db.resolve(*return_type).0)
                }
                _ => "0".to_string(),
            };

            let arr_len = match self.type_db.get_type(canonical) {
                t::Type::Array(_, size) => *size as u64,
                _ => 0,
            };

            let fields_slice = match self.type_db.get_type(canonical) {
                t::Type::Struct {
                    fields: Some(fields),
                    ..
                } if !fields.is_empty() => {
                    format!("l $chs_type_fields_{}, w {}", ty.0, fields.len())
                }
                _ => "l 0, w 0".to_string(),
            };

            let variants_slice = match self.type_db.get_type(canonical) {
                t::Type::Enum { variants, .. } if !variants.is_empty() => {
                    format!("l $chs_type_variants_{}, w {}", ty.0, variants.len())
                }
                _ => "l 0, w 0".to_string(),
            };

            let is_signed = match self.type_db.get_type(canonical) {
                t::Type::Integer(sign, _) => matches!(sign, t::Sign::Signed),
                _ => false,
            };
            let bit_size = match self.type_db.get_type(canonical) {
                t::Type::Integer(_, bitsize) => match bitsize {
                    t::BitSize::_8 => 8,
                    t::BitSize::_16 => 16,
                    t::BitSize::_32 => 32,
                    t::BitSize::_64 => 64,
                },
                t::Type::Float(bitsize) => match bitsize {
                    t::FloatBitSize::_32 => 32,
                    t::FloatBitSize::_64 => 64,
                },
                t::Type::UntypedFloat => 64,
                _ => 0,
            };

            let mut vals = std::collections::HashMap::new();
            vals.insert("kind", kind.to_string());
            vals.insert("name", get_str_data(&name));
            vals.insert("size", size.to_string());
            vals.insert("align", align.to_string());
            vals.insert("element_type", elem_ptr);
            vals.insert("array_len", arr_len.to_string());
            vals.insert("fields", fields_slice);
            vals.insert("variants", variants_slice);
            vals.insert("is_signed", (is_signed as u32).to_string());
            vals.insert("bit_size", bit_size.to_string());

            let struct_bytes = emit_struct_data(fields_of_info, &layout_info, &vals);
            writeln!(self.output, "data $chs_type_info_{} = align 8 {{", ty.0).unwrap();
            writeln!(self.output, "    {}", struct_bytes).unwrap();
            writeln!(self.output, "}}").unwrap();
        }
        writeln!(self.output).unwrap();
    }
}
