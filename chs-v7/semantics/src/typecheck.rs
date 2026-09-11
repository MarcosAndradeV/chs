use std::collections::{HashMap, HashSet};
use std::vec;

use diagnostic::DiagnosticReporter;
use lex_just_parse::lexer::{Loc, TokenSource};
use syntax::ast as s;
use types::{
    EnumVariant, FunctionParam, FunctionSignature, ImportInfo, StructField, Type, TypeDatabase,
    TypeID,
};

use super::errors::SemanticError;

// #[derive(Clone, Debug)]
// pub struct FunctionParam {
//     name: TokenSource,
//     typ: TypeID,
// }

// #[derive(Clone, Debug)]
// pub struct FunctionSignature {
//     pub name: TokenSource,
//     pub params: Vec<FunctionParam>,
//     pub return_type: TypeID,
//     pub has_va_args: bool,
//     pub is_private: bool,
// }

#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
}

pub struct TypeChecker<'a> {
    pub type_db: &'a mut TypeDatabase,
    pub reporter: &'a mut DiagnosticReporter,
    pub declared_libraries: &'a mut HashMap<String, s::Library>,
    pub features: &'a HashSet<TokenSource>,
    pub scopes: Vec<HashMap<TokenSource, TypeID>>,
    pub expected_return_type: TypeID,
    pub declared_modules: HashSet<TokenSource>,
    pub operator_overloads: HashMap<(&'static str, TypeID, TypeID), FunctionSignature>,
    pub struct_decls: HashMap<TokenSource, s::Struct>,
    pub enum_decls: HashMap<TokenSource, s::Enum>,
    pub resolve_recursive_visited: HashSet<TypeID>,
    pub constants: Vec<HashMap<String, ConstValue>>,
    current_module: TokenSource,
    pub expected_type: Option<TypeID>,
    pub unowned_vars: HashSet<TokenSource>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(
        type_db: &'a mut TypeDatabase,
        reporter: &'a mut DiagnosticReporter,
        declared_libraries: &'a mut HashMap<String, s::Library>,
        features: &'a HashSet<TokenSource>,
    ) -> Self {
        let expected_return_type = type_db.void();
        Self {
            type_db,
            features,
            operator_overloads: HashMap::new(),
            reporter,
            scopes: vec![HashMap::new()],
            expected_return_type,
            declared_libraries,
            declared_modules: HashSet::new(),
            struct_decls: HashMap::new(),
            enum_decls: HashMap::new(),
            resolve_recursive_visited: HashSet::new(),
            constants: vec![HashMap::new()],
            current_module: TokenSource::from("module"),
            expected_type: None,
            unowned_vars: HashSet::new(),
        }
    }

    fn map_type(&mut self, ast_ty: &s::Type) -> TypeID {
        let constants = &self.constants;
        let current_mod = self.current_module.clone();
        s::map_type_ext(ast_ty, self.type_db, current_mod.as_str(), &|name| {
            for scope in constants.iter().rev() {
                if let Some(val) = scope.get(name)
                    && let ConstValue::Int(v) = val
                {
                    return Some(*v as usize);
                }
            }
            None
        })
    }

    fn check_type_defined(&mut self, loc: Loc, type_id: TypeID) {
        let canonical = self.resolve_recursive(type_id);
        let underlying = self.type_db.get_underlying_type(canonical);
        if let Type::Struct {
            fields: None,
            ref name,
            ..
        } = self.type_db.get_type(underlying).clone()
        {
            self.reporter
                .report(loc, format!("type '{}' not found", name));
        }
    }

    fn unwrap_pointer_type(&self, id: TypeID) -> TypeID {
        let mut canonical = self.type_db.resolve(id);
        while let Type::Pointer(inner) = self.type_db.get_type(canonical).clone() {
            canonical = self.type_db.resolve(inner);
        }
        self.type_db.get_underlying_type(canonical)
    }

    fn lookup_method(
        &self,
        receiver_type_id: TypeID,
        property_name: &TokenSource,
    ) -> Option<(String, FunctionSignature)> {
        let receiver_type_id = self.unwrap_pointer_type(receiver_type_id);
        let full_receiver_name = self.type_db.type_to_string(receiver_type_id);
        let short_receiver_name = full_receiver_name
            .rsplit('.')
            .next()
            .unwrap_or(&full_receiver_name);

        let keys = [
            TokenSource::from(format!("{}.{}", full_receiver_name, property_name)),
            TokenSource::from(format!("{}.{}", short_receiver_name, property_name)),
            property_name.clone(),
        ];

        if let Some(module) = self.type_db.try_modules_get(&self.current_module) {
            for key in &keys {
                if let Some(sig) = module.functions.get(key) {
                    return Some((self.current_module.to_string(), sig.clone()));
                }
            }
        }

        if let Some(module) = self.type_db.try_modules_get(&self.current_module) {
            for (imported_name, info) in &module.imported_modules {
                let mod_name = info
                    .original_name()
                    .map(|n| n.as_str())
                    .unwrap_or(imported_name.as_str());
                if let Some(imp_mod) = self.type_db.try_modules_get(mod_name) {
                    for key in &keys {
                        if let Some(sig) = imp_mod.functions.get(key) {
                            if !sig.is_private {
                                return Some((mod_name.to_string(), sig.clone()));
                            }
                        }
                    }
                }
            }
        }

        for (mod_name, imp_mod) in self.type_db.modules() {
            for key in &keys {
                if let Some(sig) = imp_mod.functions.get(key) {
                    if !sig.is_private || mod_name == self.current_module.as_str() {
                        return Some((mod_name.clone(), sig.clone()));
                    }
                }
            }
        }

        None
    }

    pub fn gather_imports(&mut self, ast: &mut s::FileAst) {
        self.current_module = ast.module.source.clone();
        self.type_db.modules_add(self.current_module.as_str());
        if self.current_module.as_str() != "runtime" {
            self.type_db
                .modules_get_mut(self.current_module.as_str())
                .imported_modules
                .insert(TokenSource::from("runtime"), ImportInfo::all());
        }

        for item in &ast.items {
            match item {
                s::FileItem::Library(lib) => {
                    self.declared_libraries
                        .insert(lib.name.source.to_string(), lib.clone());
                }
                s::FileItem::Import(import) => {
                    let token_source: TokenSource = import
                        .path
                        .source()
                        .split('/')
                        .next_back()
                        .expect("import should be at least one name")
                        .into();
                    let mod_name = if let s::ImportMode::Alias(alias) = &import.mode {
                        alias.clone()
                    } else {
                        token_source.clone()
                    };
                    self.type_db
                        .modules_get_mut(&self.current_module)
                        .imported_modules
                        .insert(
                            mod_name.clone(),
                            if import.mode.is_all() {
                                ImportInfo::all()
                            } else {
                                ImportInfo::with_original_name(token_source)
                            },
                        );
                }
                _ => (),
            }
        }
    }

    pub fn gather_placeholders(&mut self, ast: &mut s::FileAst) {
        self.current_module = ast.module.source.clone();
        for item in &mut ast.items {
            match item {
                s::FileItem::Struct(decl) => {
                    let name = decl.name.source.clone();
                    self.struct_decls.insert(name.clone(), decl.clone());
                    let struct_id = if let Some(id) = self
                        .type_db
                        .lookup_type_in_module(&self.current_module, &name)
                    {
                        id
                    } else {
                        self.type_db.insert_type(Type::Struct {
                            name: name.clone(),
                            fields: None,
                            functions: HashMap::new(),
                            constructor: None,
                            destructor: None,
                        })
                    };
                    self.type_db
                        .modules_get_mut(&self.current_module)
                        .register_struct(name, struct_id);
                }
                s::FileItem::Enum(decl) => {
                    let name = &decl.name.source;
                    self.enum_decls.insert(name.clone(), decl.clone());
                    let enum_id = if let Some(id) = self
                        .type_db
                        .lookup_type_in_module(&self.current_module, name)
                    {
                        id
                    } else {
                        let mut repr = decl
                            .inner_type
                            .as_ref()
                            .map(|t| self.map_type(t))
                            .unwrap_or(self.type_db.int());
                        repr = self.resolve_recursive(repr);
                        self.type_db.insert_type(Type::Enum {
                            name: name.clone(),
                            repr,
                            variants: Vec::new(),
                            functions: HashMap::new(),
                        })
                    };
                    self.type_db
                        .modules_get_mut(&self.current_module)
                        .register_enum(name.clone(), enum_id);
                }
                _ => {}
            }
        }
    }

    pub fn resolve_type_decls(&mut self, ast: &mut s::FileAst) {
        self.current_module = ast.module.source.clone();
        for item in &mut ast.items {
            if let s::FileItem::TypeDecl(td) = item {
                let name = &td.name.source;
                let mut base_id = self.map_type(&td.base_type);
                base_id = self.resolve_recursive(base_id);
                let type_id = if td.is_distinct {
                    self.type_db.insert_type(Type::Distinct {
                        name: name.clone(),
                        base: base_id,
                    })
                } else {
                    self.type_db.alias(name.to_string(), base_id);
                    base_id
                };

                self.type_db
                    .modules_get_mut(&self.current_module)
                    .register_type(name.clone(), type_id);
            }
        }
    }

    pub fn resolve_struct_enum_fields(&mut self, ast: &mut s::FileAst) {
        self.current_module = ast.module.source.clone();
        for item in &ast.items {
            match item {
                s::FileItem::Struct(decl) => {
                    let name = &decl.name.source;
                    let struct_id = self
                        .type_db
                        .lookup_type_in_module(&self.current_module, name)
                        .unwrap();
                    let mut fields = Vec::new();
                    for field in &decl.fields {
                        let mut field_ty = self.map_type(&field.typ);
                        field_ty = self.resolve_recursive(field_ty);
                        self.check_type_defined(field.typ.loc(), field_ty);
                        fields.push(StructField {
                            name: field.name.source.clone(),
                            ty: field_ty,
                        });
                    }
                    if let Err(e) = self.type_db.set_struct_fields(struct_id, fields) {
                        self.reporter.report(decl.name.loc, e);
                    }
                }
                s::FileItem::Enum(decl) => {
                    let name = &decl.name.source;
                    let enum_id = self
                        .type_db
                        .lookup_type_in_module(&self.current_module, name)
                        .unwrap();

                    // Update enum repr in case it was a custom type alias
                    if let Some(ref ast_ty) = decl.inner_type {
                        let mut repr = self.map_type(ast_ty);
                        repr = self.resolve_recursive(repr);
                        if let Type::Enum { repr: old_repr, .. } =
                            self.type_db.get_type_mut(enum_id)
                        {
                            *old_repr = repr;
                        }
                    }

                    let mut variants = HashSet::new();
                    let mut counter = 0;
                    for variant in &decl.variants {
                        match variant {
                            s::EnumVariant::Name(tok) => {
                                let loc = tok.loc;
                                if !variants.insert(EnumVariant {
                                    name: tok.source.clone(),
                                    default_value: counter,
                                    payload: None,
                                }) {
                                    self.reporter.report(loc, "Duplicated enum variant");
                                }
                            }
                            s::EnumVariant::DefaultValue(tok, lit) => {
                                let loc = tok.loc;
                                if !variants.insert(EnumVariant {
                                    name: tok.source.clone(),
                                    default_value: {
                                        match self.eval_const_expr(lit) {
                                            Ok(ConstValue::Int(value)) => {
                                                counter = value as u64; // TODO: check conversion
                                                counter
                                            }
                                            _ => {
                                                self.reporter.report(
                                                    loc,
                                                    "Enum variant default value must be a integer",
                                                );
                                                continue;
                                            }
                                        }
                                    },
                                    payload: None,
                                }) {
                                    self.reporter
                                        .report(loc, "Duplicated enum variant or variant value");
                                }
                            }
                        }
                        counter += 1;
                    }
                    if let Err(e) = self
                        .type_db
                        .set_enum_variants(enum_id, variants.into_iter().collect())
                    {
                        self.reporter.report(decl.name.loc, e);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn resolve_constants(&mut self, ast: &mut s::FileAst) {
        self.current_module = ast.module.source.clone();
        for item in &mut ast.items {
            if let s::FileItem::Const(decl) = item {
                let decl_ty = {
                    let mut ty = self.map_type(&decl.typ);
                    ty = self.resolve_recursive(ty);
                    ty
                };
                let mut expr_copy = decl.expr.clone();
                let expr_ty = self.infer_expr(&mut expr_copy);
                if decl_ty != self.type_db.void() && self.type_db.unify(decl_ty, expr_ty).is_err() {
                    if let Some(coerced) = self.coerce(expr_ty, decl_ty) {
                        decl.expr.resolved_type = Some(coerced);
                    } else {
                        let expected_str = self.type_db.type_to_string(decl_ty);
                        let found_str = self.type_db.type_to_string(expr_ty);
                        let err = SemanticError::TypeMismatch {
                            loc: decl.name.loc,
                            expected: expected_str,
                            found: found_str,
                        };
                        self.reporter.report(err.loc(), err.to_string());
                    }
                }
                match self.eval_const_expr(&decl.expr) {
                    Ok(val) => {
                        self.constants
                            .last_mut()
                            .unwrap()
                            .insert(decl.name.source.to_string(), val);
                        let sig = types::VariableSignature {
                            name: decl.name.source.clone(),
                            ty: decl_ty,
                            is_private: false,
                            is_thread_local: false,
                            is_foreign: false,
                        };
                        self.type_db
                            .modules_get_mut(&self.current_module)
                            .variables
                            .insert(decl.name.source.clone(), sig);
                    }
                    Err(err_msg) => {
                        self.reporter.report(decl.loc, err_msg);
                    }
                }
            }
        }
    }

    pub fn resolve_fn_signatures(&mut self, ast: &mut s::FileAst) {
        self.current_module = ast.module.source.clone();
        for item in &mut ast.items {
            match item {
                s::FileItem::Operator(decl) => {
                    let name = decl.op.get_name();
                    let mut params = Vec::new();
                    let p1 = {
                        let p = &decl.parameters[0];
                        let mut param_ty = self.map_type(&p.typ);
                        param_ty = self.resolve_recursive(param_ty);
                        params.push(FunctionParam {
                            name: p.name.source.clone(),
                            typ: param_ty,
                            is_variadic: false,
                        });
                        param_ty
                    };
                    let p2 = {
                        let p = &decl.parameters[1];
                        let mut param_ty = self.map_type(&p.typ);
                        param_ty = self.resolve_recursive(param_ty);
                        params.push(FunctionParam {
                            name: p.name.source.clone(),
                            typ: param_ty,
                            is_variadic: false,
                        });
                        param_ty
                    };
                    let return_type = {
                        let ret_ty = self.map_type(&decl.return_type);
                        self.resolve_recursive(ret_ty)
                    };
                    if return_type == self.type_db.void() {
                        self.reporter
                            .report(decl.loc, "operators cannot return `void`");
                    }
                    let full_name = format!("{}.{}_{}_{}", self.current_module, name, p1.0, p2.0);
                    let tsrc = TokenSource::from(&full_name);
                    decl.resolved_name = Some(tsrc.clone());
                    let mut sig = FunctionSignature {
                        id: None,
                        name: tsrc,
                        full_name,
                        params,
                        return_type,
                        is_private: false,
                        has_va_args: false,
                        is_foreign: false,
                        has_owned_return: false,
                    };
                    let fn_id = self.type_db.register_function(sig.clone());
                    sig.id = Some(fn_id);
                    if self
                        .operator_overloads
                        .insert((name, p1, p2), sig)
                        .is_some()
                    {
                        self.reporter.report(
                            decl.loc,
                            format!(
                                "Operator '{}' is already defined with this signature",
                                decl.op.get_name()
                            ),
                        );
                    }
                }
                s::FileItem::Function(decl) => {
                    let is_main = decl.signature.name.source == "main";
                    if is_main {
                        decl.directives.push(s::FunctionDirective::Export);
                    }

                    let name = &decl.signature.name.source;
                    if is_main && !decl.signature.parameters.is_empty() {
                        self.reporter
                            .report(decl.signature.name.loc, "main function take 0 arguments");
                    }
                    let mut params = Vec::new();
                    for p in &decl.signature.parameters {
                        let mut param_ty = self.map_type(&p.typ);
                        param_ty = self.resolve_recursive(param_ty);
                        self.check_type_defined(p.typ.loc(), param_ty);
                        params.push(FunctionParam {
                            name: p.name.source.clone(),
                            typ: param_ty,
                            is_variadic: p.is_variadic,
                        });
                    }
                    let return_type = decl
                        .signature
                        .return_type
                        .as_ref()
                        .map(|t| {
                            let mut ret_ty = self.map_type(t);
                            ret_ty = self.resolve_recursive(ret_ty);
                            self.check_type_defined(t.loc(), ret_ty);
                            ret_ty
                        })
                        .unwrap_or_else(|| self.type_db.void());
                    if is_main && return_type != self.type_db.void() {
                        self.reporter
                            .report(decl.signature.name.loc, "main function must return `void`");
                    }

                    let mut is_private = false;
                    let mut is_foreign = false;
                    let mut has_owned_return = false;
                    let mut foreign_lib = None;
                    for directive in &decl.directives {
                        match directive {
                            s::FunctionDirective::Foreign(loc, lib_token) => {
                                is_foreign = true;
                                foreign_lib = Some((*loc, lib_token.source.clone()));
                            }
                            s::FunctionDirective::Export | s::FunctionDirective::LinkName(_) => {}
                            s::FunctionDirective::Private => {
                                is_private = true;
                            }
                            s::FunctionDirective::OwnedReturn => {
                                has_owned_return = true;
                            }
                        }
                    }

                    if let Some((loc, lib_name)) = foreign_lib
                        && !self.declared_libraries.contains_key(lib_name.as_str())
                    {
                        self.reporter.report(
                            loc,
                            format!(
                                "Library '{}' not declared/imported for foreign function",
                                lib_name
                            ),
                        );
                    }

                    if is_foreign && decl.body.is_some() {
                        self.reporter.report(
                            decl.signature.name.loc,
                            "foreign functions cannot have a body",
                        );
                    }

                    let full_name = format!("{}.{}", self.current_module, name);
                    decl.resolved_name = Some(full_name.clone());

                    let mut sig = FunctionSignature {
                        id: None,
                        name: name.clone(),
                        full_name,
                        params,
                        return_type,
                        is_private,
                        has_va_args: decl.signature.va_args,
                        is_foreign,
                        has_owned_return,
                    };

                    let fn_id = self.type_db.register_function(sig.clone());
                    sig.id = Some(fn_id);

                    if self
                        .type_db
                        .modules_get_mut(&self.current_module)
                        .functions
                        .insert(sig.name.clone(), sig)
                        .is_some()
                    {
                        self.reporter.report(
                            decl.signature.name.loc,
                            format!(
                                "Function '{}' is already defined",
                                decl.signature.name.source()
                            ),
                        );
                    }
                }
                s::FileItem::MethodImplementation(method) => {
                    let self_ast_ty = &method.self_parameter.typ;
                    let mut self_ty = self.map_type(self_ast_ty);
                    self_ty = self.resolve_recursive(self_ty);

                    let receiver_type_id = self.unwrap_pointer_type(self_ty);
                    let receiver_name = self.type_db.type_to_string(receiver_type_id);

                    let mut params = Vec::new();
                    let self_param_name = method.self_parameter.name.source.clone();
                    params.push(FunctionParam {
                        name: self_param_name,
                        typ: self_ty,
                        is_variadic: false,
                    });

                    for p in &method.function.signature.parameters {
                        let mut param_ty = self.map_type(&p.typ);
                        param_ty = self.resolve_recursive(param_ty);
                        params.push(FunctionParam {
                            name: p.name.source.clone(),
                            typ: param_ty,
                            is_variadic: p.is_variadic,
                        });
                    }

                    let return_type = method
                        .function
                        .signature
                        .return_type
                        .as_ref()
                        .map(|t| {
                            let mut ret_ty = self.map_type(t);
                            ret_ty = self.resolve_recursive(ret_ty);
                            ret_ty
                        })
                        .unwrap_or_else(|| self.type_db.void());

                    let method_name = &method.function.signature.name.source;
                    let func_key = TokenSource::from(format!("{}.{}", receiver_name, method_name));
                    let full_name =
                        format!("{}.{}.{}", self.current_module, receiver_name, method_name);
                    method.function.resolved_name = Some(full_name.clone());

                    let mut is_private = false;
                    let mut has_owned_return = false;
                    for directive in &method.function.directives {
                        match directive {
                            s::FunctionDirective::Private => is_private = true,
                            s::FunctionDirective::OwnedReturn => has_owned_return = true,
                            _ => {}
                        }
                    }

                    let mut sig = FunctionSignature {
                        id: None,
                        name: func_key.clone(),
                        full_name,
                        params,
                        return_type,
                        is_private,
                        has_va_args: method.function.signature.va_args,
                        is_foreign: false,
                        has_owned_return,
                    };

                    let fn_id = self.type_db.register_function(sig.clone());
                    sig.id = Some(fn_id);

                    self.type_db
                        .modules_get_mut(&self.current_module)
                        .functions
                        .insert(func_key, sig);

                    let _ = self.type_db.add_function_to_type(
                        receiver_type_id,
                        method_name.clone(),
                        fn_id,
                    );
                    if method_name.as_str() == "make" {
                        let _ = self
                            .type_db
                            .set_constructor_for_type(receiver_type_id, fn_id);
                    } else if method_name.as_str() == "drop" {
                        let _ = self
                            .type_db
                            .set_destructor_for_type(receiver_type_id, fn_id);
                    }
                }
                _ => (),
            }
        }
    }

    pub fn resolve_global_variables(&mut self, ast: &mut s::FileAst) {
        self.current_module = ast.module.source.clone();
        for item in &mut ast.items {
            match item {
                s::FileItem::VarDecl(decl) => {
                    self.infer_global_var(decl, false);
                }
                _ => {}
            }
        }
    }

    pub fn check_bodies(&mut self, ast: &mut s::FileAst) -> bool {
        self.current_module = ast.module.source.clone();
        self.pass2_check_bodies(&mut ast.items);
        !self.reporter.has_errors()
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.constants.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
        self.constants.pop();
    }

    fn insert_var(&mut self, name: TokenSource, ty: TypeID) {
        self.scopes.last_mut().unwrap().insert(name.clone(), ty);
        self.unowned_vars.remove(&name);
    }

    fn lookup_var(&self, name: &TokenSource) -> Option<TypeID> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(*ty);
            }
        }
        if let Some(var_sig) = self
            .type_db
            .lookup_var_in_module(&self.current_module, name)
        {
            return Some(var_sig.ty);
        }
        None
    }

    fn is_droppable_type(&self, ty: TypeID) -> bool {
        let canonical = self.type_db.resolve(ty);
        if self.type_db.destructor(canonical).is_some() {
            return true;
        }
        match self.type_db.get_type(canonical) {
            Type::Pointer(_) | Type::Slice(_) | Type::DynArray(_) | Type::String => true,
            Type::Struct { fields, .. } => {
                if let Some(fields) = fields {
                    fields.iter().any(|f| self.is_droppable_type(f.ty))
                } else {
                    false
                }
            }
            Type::Enum { variants, .. } => variants.iter().any(|v| {
                if let Some(payload_ty) = v.payload {
                    self.is_droppable_type(payload_ty)
                } else {
                    false
                }
            }),
            _ => false,
        }
    }

    fn is_movable_type(&self, ty: TypeID) -> bool {
        let canonical = self.type_db.resolve(ty);
        if self.type_db.destructor(canonical).is_some() {
            return true;
        }
        match self.type_db.get_type(canonical) {
            Type::Slice(_) | Type::DynArray(_) | Type::String => true,
            Type::Struct { fields, .. } => {
                if let Some(fields) = fields {
                    fields.iter().any(|f| self.is_droppable_type(f.ty))
                } else {
                    false
                }
            }
            Type::Enum { variants, .. } => variants.iter().any(|v| {
                if let Some(payload_ty) = v.payload {
                    self.is_droppable_type(payload_ty)
                } else {
                    false
                }
            }),
            _ => false,
        }
    }

    fn get_const_int(&self, expr: &s::Expr) -> Option<i64> {
        match &expr.kind {
            s::ExprKind::Integer(integer_literal) => Some(integer_literal.value as i64),
            s::ExprKind::Char(char_literal) => Some(char_literal.value as i64),
            s::ExprKind::Unary(unary_expr) if unary_expr.op == s::Op::Neg => {
                if let s::ExprKind::Integer(integer_literal) = &unary_expr.right.kind {
                    Some(-(integer_literal.value as i64))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn infer_global_var(&mut self, decl: &mut s::VarDeclStmt, is_thread_local: bool) {
        if decl.is_foreign {
            if let Some((loc, ref lib_name)) = decl.foreign_lib {
                if !self
                    .declared_libraries
                    .contains_key(lib_name.source.as_str())
                {
                    self.reporter.report(
                        loc,
                        format!(
                            "Library '{}' not declared/imported for foreign variable",
                            lib_name
                        ),
                    );
                }
            }
        }
        let mut expected = None;
        if let Some(ref ast_ty) = decl.var_type {
            let t = self.map_type(ast_ty);
            let res = self.resolve_recursive(t);
            self.check_type_defined(ast_ty.loc(), res);
            expected = Some(res);
        }
        let mut ty_id = if decl.is_foreign {
            match expected {
                Some(t) => {
                    decl.expr.resolved_type = Some(t);
                    t
                }
                None => {
                    self.reporter.report(
                        decl.names[0].loc,
                        "foreign variables must have an explicit type",
                    );
                    self.type_db.void()
                }
            }
        } else {
            self.expected_type = expected;
            let t = self.infer_expr(&mut decl.expr);
            self.expected_type = None;
            let actual = self.resolve_recursive(t);
            expected.unwrap_or(actual)
        };
        if ty_id == self.type_db.untyped_int() {
            ty_id = self.type_db.int();
        } else if ty_id == self.type_db.untyped_float() {
            ty_id = self.type_db.float();
        }
        for name_tok in &mut decl.names {
            let name = &name_tok.source;
            let sig = types::VariableSignature {
                name: name.clone(),
                ty: ty_id,
                is_private: decl.is_private,
                is_thread_local,
                is_foreign: decl.is_foreign,
            };
            self.type_db
                .modules_get_mut(&self.current_module)
                .variables
                .insert(name.clone(), sig);
        }
    }

    fn pass2_check_bodies(&mut self, items: &mut [s::FileItem]) {
        for item in items.iter_mut() {
            match item {
                s::FileItem::Function(decl) => {
                    if let Some(body) = &mut decl.body {
                        self.unowned_vars.clear();
                        self.expected_return_type = decl
                            .signature
                            .return_type
                            .as_ref()
                            .map(|t| {
                                let mut ret_ty = self.map_type(t);
                                ret_ty = self.resolve_recursive(ret_ty);
                                ret_ty
                            })
                            .unwrap_or_else(|| self.type_db.void());
                        self.push_scope();
                        for param in &decl.signature.parameters {
                            let mut ty = self.map_type(&param.typ);
                            ty = self.resolve_recursive(ty);
                            self.insert_var(param.name.source.clone(), ty);
                        }
                        self.check_block(body);
                        self.pop_scope();
                    }
                }
                s::FileItem::MethodImplementation(method) => {
                    if let Some(body) = &mut method.function.body {
                        self.unowned_vars.clear();
                        self.expected_return_type = method
                            .function
                            .signature
                            .return_type
                            .as_ref()
                            .map(|t| {
                                let mut ret_ty = self.map_type(t);
                                ret_ty = self.resolve_recursive(ret_ty);
                                ret_ty
                            })
                            .unwrap_or_else(|| self.type_db.void());
                        self.push_scope();

                        let mut self_ty = self.map_type(&method.self_parameter.typ);
                        self_ty = self.resolve_recursive(self_ty);
                        let self_name = method.self_parameter.name.source.clone();
                        self.insert_var(self_name, self_ty);

                        for param in &method.function.signature.parameters {
                            let mut ty = self.map_type(&param.typ);
                            ty = self.resolve_recursive(ty);
                            self.insert_var(param.name.source.clone(), ty);
                        }

                        self.check_block(body);
                        self.pop_scope();
                    }
                }
                s::FileItem::Operator(decl) => {
                    self.expected_return_type = {
                        let mut ret_ty = self.map_type(&decl.return_type);
                        ret_ty = self.resolve_recursive(ret_ty);
                        ret_ty
                    };
                    self.push_scope();
                    for param in &decl.parameters {
                        let mut ty = self.map_type(&param.typ);
                        ty = self.resolve_recursive(ty);
                        self.insert_var(param.name.source.clone(), ty);
                    }
                    self.check_block(&mut decl.body);
                    self.pop_scope();
                }
                s::FileItem::VarDecl(decl) => {
                    if decl.is_foreign {
                        continue;
                    }
                    let expected_ty = if let Some(ref ast_ty) = decl.var_type {
                        self.map_type(ast_ty)
                    } else {
                        self.type_db.void()
                    };
                    if decl.var_type.is_some() {
                        self.expected_type = Some(expected_ty);
                    }
                    let actual_ty = self.infer_expr(&mut decl.expr);
                    self.expected_type = None;
                    if decl.var_type.is_some()
                        && self.type_db.unify(expected_ty, actual_ty).is_err()
                    {
                        if let Some(coerced) = self.coerce(actual_ty, expected_ty) {
                            decl.expr.resolved_type = Some(coerced);
                        } else {
                            let expected_str = self.type_db.type_to_string(expected_ty);
                            let found_str = self.type_db.type_to_string(actual_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: decl.expr.loc(),
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn check_block(&mut self, block: &mut s::BlockStmt) {
        self.push_scope();
        for stmt in &mut block.stmts {
            self.check_stmt(stmt);
        }
        self.pop_scope();
    }

    fn check_stmt(&mut self, stmt: &mut s::Stmt) {
        match stmt {
            s::Stmt::ExprStmt(expr) => {
                self.infer_expr(expr);
            }
            s::Stmt::VarDecl(decl) => {
                let declared_ty = decl.var_type.as_ref().map(|t| {
                    let mut ty = self.map_type(t);
                    ty = self.resolve_recursive(ty);
                    self.check_type_defined(t.loc(), ty);
                    ty
                });
                if let Some(decl_ty) = declared_ty {
                    decl.expr.resolved_type = Some(decl_ty);
                    self.expected_type = Some(decl_ty);
                }
                let expr_ty = self.infer_expr(&mut decl.expr);
                self.expected_type = None;

                if decl.names.len() == 1 {
                    let name = &decl.names[0];
                    let final_ty = if let Some(decl_ty) = declared_ty {
                        if expr_ty != self.type_db.void()
                            && self.type_db.unify(decl_ty, expr_ty).is_err()
                        {
                            if let Some(decl_ty) = self.coerce(expr_ty, decl_ty) {
                                decl.expr.resolved_type = Some(decl_ty);
                                self.check_local_array_to_slice_coercion(
                                    decl_ty, expr_ty, &decl.expr, name.loc,
                                );
                            } else {
                                let expected_str = self.type_db.type_to_string(decl_ty);
                                let found_str = self.type_db.type_to_string(expr_ty);
                                let err = SemanticError::TypeMismatch {
                                    loc: name.loc,
                                    expected: expected_str,
                                    found: found_str,
                                };
                                self.reporter.report(err.loc(), err.to_string());
                            }
                        };
                        decl_ty
                    } else if expr_ty == self.type_db.void() {
                        self.reporter
                            .report(name.loc, "Cannot infer type from void expression");
                        self.type_db.void()
                    } else {
                        if let s::ExprKind::Default(loc) = &decl.expr.kind {
                            self.reporter.report(
                                *loc,
                                "Cannot infer type for variable initialized to #default"
                                    .to_string(),
                            );
                            self.type_db.void()
                        } else {
                            if expr_ty == self.type_db.untyped_int() {
                                decl.expr.resolved_type = Some(self.type_db.int());
                                self.type_db.int()
                            } else if expr_ty == self.type_db.untyped_float() {
                                decl.expr.resolved_type = Some(self.type_db.float());
                                self.type_db.float()
                            } else {
                                expr_ty
                            }
                        }
                    };
                    self.insert_var(name.source.clone(), final_ty);
                } else {
                    let target_ty = if let Some(decl_ty) = declared_ty {
                        if expr_ty != self.type_db.void()
                            && self.type_db.unify(decl_ty, expr_ty).is_err()
                        {
                            let expected_str = self.type_db.type_to_string(decl_ty);
                            let found_str = self.type_db.type_to_string(expr_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: decl.names[0].loc,
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                        decl_ty
                    } else {
                        expr_ty
                    };

                    let resolved_target = self.resolve(target_ty);
                    match self.type_db.get_type(resolved_target).clone() {
                        Type::Tuple(elements) => {
                            if elements.len() != decl.names.len() {
                                self.reporter.report(
                                    decl.names[0].loc,
                                    format!(
                                        "Cannot destructure tuple of size {} into {} variables",
                                        elements.len(),
                                        decl.names.len()
                                    ),
                                );
                            } else {
                                for (i, name) in decl.names.iter().enumerate() {
                                    let elem_ty = elements[i];
                                    self.insert_var(name.source.clone(), elem_ty);
                                }
                            }
                        }
                        _ => {
                            self.reporter.report(
                                decl.names[0].loc,
                                format!(
                                    "Cannot destructure non-tuple type `{}`",
                                    self.type_db.type_to_string(resolved_target)
                                ),
                            );
                        }
                    }
                }
            }
            s::Stmt::Const(decl) => {
                let expr_ty = self.infer_expr(&mut decl.expr);
                let decl_ty = {
                    let mut ty = self.map_type(&decl.typ);
                    ty = self.resolve_recursive(ty);
                    ty
                };

                if decl_ty != self.type_db.void() && self.type_db.unify(decl_ty, expr_ty).is_err() {
                    if let Some(coerced) = self.coerce(expr_ty, decl_ty) {
                        decl.expr.resolved_type = Some(coerced);
                    } else {
                        let expected_str = self.type_db.type_to_string(decl_ty);
                        let found_str = self.type_db.type_to_string(expr_ty);
                        let err = SemanticError::TypeMismatch {
                            loc: decl.name.loc,
                            expected: expected_str,
                            found: found_str,
                        };
                        self.reporter.report(err.loc(), err.to_string());
                    }
                }

                match self.eval_const_expr(&decl.expr) {
                    Ok(val) => {
                        self.constants
                            .last_mut()
                            .unwrap()
                            .insert(decl.name.source.to_string(), val);
                        self.insert_var(decl.name.source.clone(), decl_ty);
                    }
                    Err(err_msg) => {
                        self.reporter.report(decl.loc, err_msg);
                    }
                }
            }
            s::Stmt::Return(loc, expr_opt) => {
                let ty = if let Some(expr) = expr_opt {
                    self.expected_type = Some(self.expected_return_type);
                    let inferred = self.infer_expr(expr);
                    self.expected_type = None;
                    let canonical_ret = self.type_db.resolve(self.expected_return_type);
                    if matches!(
                        self.type_db.get_type(canonical_ret),
                        Type::Slice(..) | Type::Pointer(_)
                    ) && self.is_expr_stack_backed(expr)
                    {
                        self.reporter.report(
                            *loc,
                            "Returning a local stack-allocated value is unsafe".to_string(),
                        );
                    }
                    inferred
                } else {
                    self.type_db.void()
                };
                if self.type_db.unify(self.expected_return_type, ty).is_err() {
                    let mut coerced = false;
                    if let Some(expr) = expr_opt
                        && let Some(coerced_ty) = self.coerce(ty, self.expected_return_type)
                    {
                        expr.resolved_type = Some(coerced_ty);
                        coerced = true;
                    }
                    if !coerced {
                        let expected_str = self.type_db.type_to_string(self.expected_return_type);
                        let found_str = self.type_db.type_to_string(ty);
                        let err = SemanticError::TypeMismatch {
                            loc: *loc,
                            expected: expected_str,
                            found: found_str,
                        };
                        self.reporter.report(err.loc(), err.to_string());
                    }
                }
            }
            s::Stmt::Block(block) => self.check_block(block),
            s::Stmt::IfStmt(s::IfStmt::If { cond, true_body }) => {
                let cond_ty = self.infer_expr(cond);
                let bool_ty = self.type_db.bool();
                self.unify_or_report(bool_ty, cond_ty, cond.loc());
                self.check_block(true_body);
            }
            s::Stmt::IfStmt(s::IfStmt::IfElse {
                cond,
                true_body,
                false_body,
            }) => {
                let cond_ty = self.infer_expr(cond);
                let bool_ty = self.type_db.bool();
                self.unify_or_report(bool_ty, cond_ty, cond.loc());
                self.check_block(true_body);
                self.check_block(false_body);
            }
            s::Stmt::Call(call) => {
                let mut dummy = s::Expr::new(s::ExprKind::Call(call.clone()));
                self.infer_expr(&mut dummy);
                if let s::ExprKind::Call(resolved_call) = dummy.kind {
                    *call = resolved_call;
                }
            }
            s::Stmt::ForStmt(s::ForStmt::ForLoop(block)) => self.check_block(block),
            s::Stmt::ForStmt(s::ForStmt::ForCond { cond, body }) => {
                let cond_ty = self.infer_expr(cond);
                let bool_ty = self.type_db.bool();
                self.unify_or_report(bool_ty, cond_ty, cond.loc());
                self.check_block(body);
            }
            s::Stmt::Break(_) | s::Stmt::Continue(_) => {}
            s::Stmt::Defer(_, inner) => {
                let saved_unowned = self.unowned_vars.clone();
                self.check_stmt(inner);
                self.unowned_vars = saved_unowned;
            }
            s::Stmt::Switch(switch_stmt) => {
                self.infer_expr(&mut switch_stmt.cond);
                for branch in &mut switch_stmt.branches {
                    let loc = branch.pattern.loc();
                    let pattern_expr = std::mem::replace(
                        &mut branch.pattern,
                        s::Expr::new(s::ExprKind::Null(loc)),
                    );
                    let cmp_expr = s::Expr::new(s::ExprKind::Binary(s::BinaryExpr {
                        left: Box::new(switch_stmt.cond.clone()),
                        op: s::Op::Eq,
                        right: Box::new(pattern_expr),
                        use_operator_overload: None,
                    }));
                    branch.pattern = cmp_expr;

                    let pattern_ty = self.infer_expr(&mut branch.pattern);
                    self.unify_or_report(self.type_db.bool(), pattern_ty, branch.pattern.loc());

                    self.scopes.push(HashMap::new());
                    self.check_stmt(&mut branch.body);
                    self.scopes.pop();
                }
                if let Some(ref mut default_stmt) = switch_stmt.default {
                    self.check_stmt(default_stmt);
                }
            }
            s::Stmt::ForEach(fe) => {
                let iter_ty = self.infer_expr(&mut fe.iter_expr);
                let resolved_iter = self.resolve(iter_ty);
                let elem_ty = if resolved_iter == self.type_db.string() {
                    self.type_db.u8()
                } else {
                    match self.type_db.get_type(resolved_iter).clone() {
                        Type::Array(elem, _) => elem,
                        Type::Slice(elem) => elem,
                        Type::DynArray(elem) => elem,
                        Type::Pointer(inner) => {
                            let resolved_inner = self.resolve(inner);
                            if resolved_inner == self.type_db.string() {
                                self.type_db.u8()
                            } else {
                                match self.type_db.get_type(resolved_inner).clone() {
                                    Type::Array(elem, _) => elem,
                                    Type::Slice(elem) => elem,
                                    Type::DynArray(elem) => elem,
                                    _ => {
                                        let iter_str = self.type_db.type_to_string(iter_ty);
                                        let err = format!("Cannot iterate over type {}", iter_str);
                                        self.reporter.report(fe.iter_expr.loc(), err);
                                        self.type_db.void()
                                    }
                                }
                            }
                        }
                        _ => {
                            let iter_str = self.type_db.type_to_string(iter_ty);
                            let err = format!("Cannot iterate over type {}", iter_str);
                            self.reporter.report(fe.iter_expr.loc(), err);
                            self.type_db.void()
                        }
                    }
                };

                self.push_scope();
                self.insert_var(fe.var_name.source.clone(), elem_ty);
                self.check_block(&mut fe.body);
                self.pop_scope();
            }
        }
    }

    fn get_underlying_primitive(&mut self, id: TypeID) -> TypeID {
        let mut canon = self.resolve(id);
        loop {
            match self.type_db.get_type(canon).clone() {
                Type::Distinct { base, .. } => {
                    canon = self.resolve(base);
                }
                Type::Enum { repr, .. } => {
                    canon = self.resolve(repr);
                }
                _ => break,
            }
        }
        canon
    }

    fn coerce(&mut self, expr_ty: TypeID, decl_ty: TypeID) -> Option<TypeID> {
        let decl_resolved = self.resolve(decl_ty);
        let expr_resolved = self.resolve(expr_ty);

        if expr_resolved == self.type_db.noreturn() {
            return Some(decl_ty);
        }

        let decl_canon = self.get_underlying_primitive(decl_resolved);
        let expr_canon = self.get_underlying_primitive(expr_resolved);

        let int_primitive = self.resolve(self.type_db.int());
        let usize_primitive = self.resolve(self.type_db.usize());

        if expr_canon == self.type_db.untyped_int() && self.type_db.is_integer(decl_canon) {
            return Some(decl_resolved);
        }

        if expr_canon == self.type_db.untyped_float() && self.type_db.is_float(decl_canon) {
            return Some(decl_resolved);
        }

        if expr_canon == int_primitive && decl_canon == usize_primitive {
            return Some(decl_resolved);
        }

        let decl_type = self.type_db.get_type(decl_resolved).clone();
        let expr_type = self.type_db.get_type(expr_resolved).clone();

        match (decl_type, expr_type) {
            (Type::Pointer(expected_elem), Type::Pointer(actual_elem)) => {
                if let Some(coerced_elem) = self.coerce(actual_elem, expected_elem) {
                    Some(self.type_db.pointer(coerced_elem))
                } else if self.type_db.unify(expected_elem, actual_elem).is_ok() {
                    Some(decl_ty)
                } else {
                    None
                }
            }
            (Type::Array(expected_elem, expected_len), Type::Array(actual_elem, actual_len))
                if expected_len == actual_len =>
            {
                if let Some(coerced_elem) = self.coerce(actual_elem, expected_elem) {
                    Some(self.type_db.array(coerced_elem, expected_len))
                } else if self.type_db.unify(expected_elem, actual_elem).is_ok() {
                    Some(decl_ty)
                } else {
                    None
                }
            }
            (Type::Slice(expected_elem), Type::Slice(actual_elem)) => {
                if let Some(coerced_elem) = self.coerce(actual_elem, expected_elem) {
                    Some(self.type_db.slice(coerced_elem))
                } else if self.type_db.unify(expected_elem, actual_elem).is_ok() {
                    Some(decl_ty)
                } else {
                    None
                }
            }
            (Type::DynArray(expected_elem), Type::DynArray(actual_elem)) => {
                if let Some(coerced_elem) = self.coerce(actual_elem, expected_elem) {
                    Some(self.type_db.dyn_array(coerced_elem))
                } else if self.type_db.unify(expected_elem, actual_elem).is_ok() {
                    Some(decl_ty)
                } else {
                    None
                }
            }
            (Type::Slice(expected_elem), Type::Array(actual_elem, _)) => {
                if let Some(coerced_elem) = self.coerce(actual_elem, expected_elem) {
                    Some(self.type_db.slice(coerced_elem))
                } else if self.type_db.unify(expected_elem, actual_elem).is_ok() {
                    Some(decl_ty)
                } else {
                    None
                }
            }
            (Type::Slice(expected_elem), Type::DynArray(actual_elem)) => {
                if let Some(coerced_elem) = self.coerce(actual_elem, expected_elem) {
                    Some(self.type_db.slice(coerced_elem))
                } else if self.type_db.unify(expected_elem, actual_elem).is_ok() {
                    Some(decl_ty)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn infer_expr(&mut self, expr: &mut s::Expr) -> TypeID {
        let ty = match &mut expr.kind {
            s::ExprKind::Integer(_) | s::ExprKind::Char(_) => self.type_db.untyped_int(),
            s::ExprKind::Bool(_) => self.type_db.bool(),
            s::ExprKind::Float(_) => self.type_db.untyped_float(),
            s::ExprKind::StringLiteral(_) => self.type_db.string(),
            s::ExprKind::Null(_) => self.type_db.pointer(self.type_db.void()),
            s::ExprKind::Feature(loc, t) => {
                expr.kind = s::ExprKind::Bool(s::BoolLiteral {
                    loc: *loc,
                    value: self.features.contains(&t.source),
                });
                self.type_db.bool()
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Scalar(s), _) => {
                s.resolved_type = Some(self.infer_expr(s));
                self.type_db.any()
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Array(arr), _) => {
                for s in arr.iter_mut() {
                    s.resolved_type = Some(self.infer_expr(s));
                }
                self.type_db.array(self.type_db.any(), arr.len())
            }
            s::ExprKind::Tuple(exprs, _) => {
                let mut elem_tys = Vec::new();
                for expr in exprs {
                    elem_tys.push(self.infer_expr(expr));
                }
                self.type_db.tuple(elem_tys)
            }
            s::ExprKind::Identifier(ident) => {
                if let Some(ty) = self.lookup_var(&ident.source) {
                    ty
                } else if let Some(sig) = self
                    .type_db
                    .modules_get(&self.current_module)
                    .functions
                    .get(&ident.source)
                {
                    self.type_db
                        .fn_pointer(sig.params.iter().map(|p| p.typ).collect(), sig.return_type)
                } else if let Some(ty) = self
                    .type_db
                    .lookup_type_in_module(&self.current_module, &ident.source)
                {
                    ty
                } else if let Some(info) = self
                    .type_db
                    .modules_get(&self.current_module)
                    .imported_modules
                    .get(&ident.source)
                {
                    if let Some(original_name) = info.original_name() {
                        ident.source = original_name.clone();
                    }
                    self.type_db.module()
                } else {
                    let err = SemanticError::UndefinedVariable {
                        loc: ident.loc,
                        name: ident.source.to_string(),
                    };
                    self.reporter.report(err.loc(), err.to_string());
                    self.type_db.void()
                }
            }
            s::ExprKind::Binary(bin) => {
                let is_left_implicit = self.is_implicit_enum_variant(&bin.left);
                let is_right_implicit = self.is_implicit_enum_variant(&bin.right);

                let mut left_ty;
                let mut right_ty;

                if is_left_implicit && !is_right_implicit {
                    right_ty = self.infer_expr(&mut bin.right);
                    self.expected_type = Some(right_ty);
                    left_ty = self.infer_expr(&mut bin.left);
                    self.expected_type = None;
                } else if is_right_implicit && !is_left_implicit {
                    left_ty = self.infer_expr(&mut bin.left);
                    self.expected_type = Some(left_ty);
                    right_ty = self.infer_expr(&mut bin.right);
                    self.expected_type = None;
                } else {
                    left_ty = self.infer_expr(&mut bin.left);
                    right_ty = self.infer_expr(&mut bin.right);
                }

                let mut left_canon = self.resolve(left_ty);
                let mut right_canon = self.resolve(right_ty);
                let left_is_ptr = matches!(self.type_db.get_type(left_canon), Type::Pointer(_));
                let right_is_ptr = matches!(self.type_db.get_type(right_canon), Type::Pointer(_));

                if left_is_ptr && right_canon == self.type_db.untyped_int() {
                    let usize_ty = self.type_db.usize();
                    if let Some(coerced) = self.coerce(right_ty, usize_ty) {
                        bin.right.resolved_type = Some(coerced);
                        right_ty = coerced;
                        right_canon = self.resolve(coerced);
                    }
                } else if right_is_ptr && left_canon == self.type_db.untyped_int() {
                    let usize_ty = self.type_db.usize();
                    if let Some(coerced) = self.coerce(left_ty, usize_ty) {
                        bin.left.resolved_type = Some(coerced);
                        left_ty = coerced;
                        left_canon = self.resolve(coerced);
                    }
                }

                if left_canon != right_canon && !left_is_ptr && !right_is_ptr {
                    if let Some(coerced) = self.coerce(left_ty, right_ty) {
                        bin.left.resolved_type = Some(coerced);
                        left_ty = coerced;
                        left_canon = self.resolve(coerced);
                    } else if let Some(coerced) = self.coerce(right_ty, left_ty) {
                        bin.right.resolved_type = Some(coerced);
                        right_ty = coerced;
                        right_canon = self.resolve(coerced);
                    }
                }

                let left_is_int =
                    self.type_db.is_integer(left_canon) || left_canon == self.type_db.untyped_int();
                let right_is_int = self.type_db.is_integer(right_canon)
                    || right_canon == self.type_db.untyped_int();

                let is_ptr_arithmetic = match bin.op {
                    s::Op::Add => (left_is_ptr && right_is_int) || (left_is_int && right_is_ptr),
                    s::Op::Sub => (left_is_ptr && right_is_int) || (left_is_ptr && right_is_ptr),
                    _ => false,
                };

                if !is_ptr_arithmetic
                    && left_ty != self.type_db.void()
                    && right_ty != self.type_db.void()
                    && self.type_db.unify(left_ty, right_ty).is_err()
                {
                    let expected_str = self.type_db.type_to_string(left_ty);
                    let found_str = self.type_db.type_to_string(right_ty);
                    let err = SemanticError::TypeMismatch {
                        loc: bin.right.loc(),
                        expected: expected_str,
                        found: found_str,
                    };
                    self.reporter.report(err.loc(), err.to_string());
                }

                if is_ptr_arithmetic {
                    if bin.op == s::Op::Add && right_is_ptr {
                        right_ty
                    } else if bin.op == s::Op::Sub && left_is_ptr && right_is_ptr {
                        self.type_db.int()
                    } else {
                        left_ty
                    }
                } else {
                    match (
                        bin.op,
                        self.type_db.get_type(left_ty),
                        self.type_db.get_type(right_ty),
                    ) {
                        (
                            s::Op::Eq
                            | s::Op::NotEq
                            | s::Op::Lt
                            | s::Op::LtEq
                            | s::Op::Gt
                            | s::Op::GtEq,
                            Type::Float(..)
                            | Type::UntypedFloat
                            | Type::Bool
                            | Type::Integer(..)
                            | Type::UntypedInt
                            | Type::Enum { .. }
                            | Type::Pointer(_),
                            Type::Float(..)
                            | Type::UntypedFloat
                            | Type::Bool
                            | Type::Integer(..)
                            | Type::UntypedInt
                            | Type::Enum { .. }
                            | Type::Pointer(_),
                        ) => self.type_db.bool(),
                        (
                            _,
                            Type::Float(..)
                            | Type::UntypedFloat
                            | Type::Bool
                            | Type::Integer(..)
                            | Type::UntypedInt,
                            Type::Float(..)
                            | Type::UntypedFloat
                            | Type::Bool
                            | Type::Integer(..)
                            | Type::UntypedInt,
                        ) => left_ty,
                        _ => {
                            if let Some(sig) = self.operator_overloads.get(&(
                                bin.op.get_name(),
                                left_canon,
                                right_canon,
                            )) {
                                bin.use_operator_overload = Some(sig.name.clone());
                                sig.return_type
                            } else {
                                let left_str = self.type_db.type_to_string(left_ty);
                                let right_str = self.type_db.type_to_string(right_ty);
                                let err = SemanticError::FunctionNotFount {
                                    loc: bin.right.loc(),
                                    name: format!("{:?}", bin.op),
                                    expected: format!("({}, {})", left_str, right_str),
                                };
                                self.reporter.report(err.loc(), err.to_string());
                                self.type_db.void()
                            }
                        }
                    }
                }
            }
            s::ExprKind::Call(call) => self.infer_call_expression(call),
            s::ExprKind::Assign(assign) => {
                if let s::ExprKind::Tuple(targets, loc) = &mut assign.left.kind {
                    let left_loc = *loc;
                    let mut elem_types = Vec::new();
                    for target in targets.iter_mut() {
                        elem_types.push(self.infer_expr(target));
                    }
                    let left_ty = self.type_db.tuple(elem_types);
                    self.expected_type = Some(left_ty);
                    let right_ty = self.infer_expr(&mut assign.right);
                    self.expected_type = None;

                    let canonical_right = self.resolve(right_ty);
                    match self.type_db.get_type(canonical_right).clone() {
                        Type::Tuple(elements) => {
                            if elements.len() != targets.len() {
                                self.reporter.report(
                                    left_loc,
                                    format!(
                                        "Cannot assign tuple of size {} to {} variables",
                                        elements.len(),
                                        targets.len()
                                    ),
                                );
                            } else {
                                for (i, target) in targets.iter_mut().enumerate() {
                                    let is_lval = matches!(
                                        &target.kind,
                                        s::ExprKind::Identifier(_)
                                            | s::ExprKind::Member(_)
                                            | s::ExprKind::Index(_)
                                            | s::ExprKind::Unary(s::UnaryExpr {
                                                op: s::Op::Deref,
                                                ..
                                            })
                                    );
                                    if !is_lval {
                                        self.reporter.report(
                                            target.loc(),
                                            "Left-hand side of assignment must be a variable or member",
                                        );
                                    }

                                    let target_ty = self.infer_expr(target);
                                    if self.type_db.unify(target_ty, elements[i]).is_err() {
                                        let expected_str = self.type_db.type_to_string(target_ty);
                                        let found_str = self.type_db.type_to_string(elements[i]);
                                        let err = SemanticError::TypeMismatch {
                                            loc: target.loc(),
                                            expected: expected_str,
                                            found: found_str,
                                        };
                                        self.reporter.report(err.loc(), err.to_string());
                                    }
                                }
                            }
                        }
                        _ => {
                            self.reporter.report(
                                left_loc,
                                format!(
                                    "Cannot destructure non-tuple type `{}`",
                                    self.type_db.type_to_string(canonical_right)
                                ),
                            );
                        }
                    }
                    right_ty
                } else {
                    if let s::ExprKind::Identifier(ref l_ident) = assign.left.kind {
                        self.unowned_vars.remove(&l_ident.source);
                    }
                    let left_ty = self.infer_expr(&mut assign.left);

                    if let s::ExprKind::Member(mem) = &assign.left.kind {
                        if let Some(obj_ty) = mem.object.resolved_type {
                            let mut canonical_base = self.resolve(obj_ty);
                            while let Type::Pointer(inner) =
                                self.type_db.get_type(canonical_base).clone()
                            {
                                canonical_base = self.resolve(inner);
                            }
                            if let Type::Enum { .. } = self.type_db.get_type(canonical_base) {
                                self.reporter.report(
                                    assign.left.loc(),
                                    format!(
                                        "Cannot assign to enum variant '{}'",
                                        mem.property.source()
                                    ),
                                );
                            }
                        }
                    }

                    self.expected_type = Some(left_ty);
                    let right_ty = self.infer_expr(&mut assign.right);
                    self.expected_type = None;

                    if let s::ExprKind::Identifier(ref r_ident) = assign.right.kind {
                        if self.is_movable_type(right_ty) {
                            self.unowned_vars.insert(r_ident.source.clone());
                        }
                    }
                    if let s::ExprKind::Identifier(ref l_ident) = assign.left.kind {
                        self.unowned_vars.remove(&l_ident.source);
                    }

                    let mut is_valid_lhs = matches!(
                        &assign.left.kind,
                        s::ExprKind::Identifier(_) | s::ExprKind::Member(_) | s::ExprKind::Index(_)
                    );
                    if let s::ExprKind::Unary(unary) = &assign.left.kind {
                        if unary.op == s::Op::Deref {
                            is_valid_lhs = true;
                        }
                    }
                    if !is_valid_lhs {
                        self.reporter.report(
                            assign.left.loc(),
                            "Complex assignment left-hand side not supported",
                        );
                    }

                    if left_ty != self.type_db.void() && right_ty != self.type_db.void() {
                        if self.type_db.unify(left_ty, right_ty).is_err() {
                            if let Some(coerced) = self.coerce(right_ty, left_ty) {
                                assign.right.resolved_type = Some(coerced);
                                self.check_local_array_to_slice_coercion(
                                    left_ty,
                                    right_ty,
                                    &assign.right,
                                    assign.right.loc(),
                                );
                            } else {
                                let expected_str = self.type_db.type_to_string(left_ty);
                                let found_str = self.type_db.type_to_string(right_ty);
                                let err = SemanticError::TypeMismatch {
                                    loc: assign.right.loc(),
                                    expected: expected_str,
                                    found: found_str,
                                };
                                self.reporter.report(err.loc(), err.to_string());
                            }
                        }
                    }
                    right_ty
                }
            }
            s::ExprKind::Unary(unary) => {
                let right_ty = self.infer_expr(&mut unary.right);
                match unary.op {
                    s::Op::Neg => {
                        let canonical_right = self.resolve(right_ty);
                        if self.type_db.is_float(canonical_right)
                            || canonical_right == self.type_db.untyped_float()
                        {
                            canonical_right
                        } else if canonical_right == self.type_db.usize() {
                            canonical_right
                        } else if canonical_right == self.type_db.untyped_int() {
                            canonical_right
                        } else {
                            let int_ty = self.type_db.int();
                            if self.type_db.unify(int_ty, right_ty).is_err() {
                                let expected_str = self.type_db.type_to_string(int_ty);
                                let found_str = self.type_db.type_to_string(right_ty);
                                let err = SemanticError::TypeMismatch {
                                    loc: unary.right.loc(),
                                    expected: expected_str,
                                    found: found_str,
                                };
                                self.reporter.report(err.loc(), err.to_string());
                            }
                            int_ty
                        }
                    }
                    s::Op::Not => {
                        let bool_ty = self.type_db.bool();
                        if self.type_db.unify(bool_ty, right_ty).is_err() {
                            let expected_str = self.type_db.type_to_string(bool_ty);
                            let found_str = self.type_db.type_to_string(right_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: unary.right.loc(),
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                        bool_ty
                    }
                    s::Op::Refer => {
                        self.resolve(right_ty);
                        self.type_db.pointer(right_ty)
                    }
                    s::Op::Deref => {
                        if let s::ExprKind::Identifier(ref r_ident) = unary.right.kind {
                            if self.unowned_vars.contains(&r_ident.source) {
                                self.reporter.report(
                                    unary.right.loc(),
                                    format!("use of moved value '{}'", r_ident.source),
                                );
                            }
                        }
                        let canonical = self.resolve(right_ty);
                        match self.type_db.get_type(canonical).clone() {
                            Type::Pointer(inner) => inner,
                            _ => {
                                let found_str = self.type_db.type_to_string(right_ty);
                                self.reporter.report(
                                    unary.right.loc(),
                                    format!("Cannot dereference non-pointer type: {}", found_str),
                                );
                                self.type_db.void()
                            }
                        }
                    }
                    _ => right_ty,
                }
            }
            s::ExprKind::Cast(target_type, inner, loc) => {
                let mut dest_ty = self.map_type(target_type);
                dest_ty = self.resolve_recursive(dest_ty);
                self.check_type_defined(target_type.loc(), dest_ty);
                if inner.resolved_type.is_none() {
                    inner.resolved_type = Some(dest_ty);
                }
                let right_ty = self.infer_expr(inner);
                if right_ty != self.type_db.void() && dest_ty != self.type_db.void() {
                    let underlying_src = self.type_db.get_underlying_type(right_ty);
                    let underlying_dest = self.type_db.get_underlying_type(dest_ty);
                    if underlying_src != underlying_dest {
                        if !self.is_castable_type(right_ty) {
                            let src_str = self.type_db.type_to_string(right_ty);
                            self.reporter.report(
                                *loc,
                                format!("Cannot cast from non-castable type: {}", src_str),
                            );
                        }
                        if !self.is_castable_type(dest_ty) {
                            let dest_str = self.type_db.type_to_string(dest_ty);
                            self.reporter.report(
                                *loc,
                                format!("Cannot cast to non-castable type: {}", dest_str),
                            );
                        }
                    }
                }
                dest_ty
            }
            s::ExprKind::AutoCast(inner, loc) => {
                if inner.resolved_type.is_none() && expr.resolved_type.is_some() {
                    inner.resolved_type = expr.resolved_type;
                }
                let right_ty = self.infer_expr(inner);
                if right_ty != self.type_db.void() && !self.is_castable_type(right_ty) {
                    let src_str = self.type_db.type_to_string(right_ty);
                    self.reporter.report(
                        *loc,
                        format!("Cannot auto-cast from non-castable type: {}", src_str),
                    );
                }
                self.type_db.new_inference_var()
            }
            s::ExprKind::InitList(lit) => {
                let mut target_type_id = None;
                let mut struct_name_for_defaults = None;

                if let Some(hint) = &lit.type_hint {
                    if let s::Type::Scalar(name_tok) = hint {
                        let name = name_tok.source.clone();
                        if let Some(struct_id) = self
                            .type_db
                            .lookup_type_in_module(&self.current_module, &name)
                        {
                            target_type_id = Some(self.resolve(struct_id));
                            struct_name_for_defaults = Some(name);
                        } else {
                            self.reporter
                                .report(name_tok.loc, format!("Undefined struct '{}'", name));
                            return self.type_db.void();
                        }
                    } else {
                        let mut hint_ty = self.map_type(hint);
                        hint_ty = self.resolve_recursive(hint_ty);
                        target_type_id = Some(self.resolve(hint_ty));
                    }
                } else if let Some(resolved) = expr.resolved_type {
                    target_type_id = Some(self.resolve(resolved));
                } else if let Some(expected) = self.expected_type {
                    target_type_id = Some(self.resolve(expected));
                }

                if let Some(type_id) = target_type_id {
                    let canon = self.resolve(type_id);
                    match self.type_db.get_type(canon).clone() {
                        Type::Struct {
                            name: struct_name,
                            fields: Some(def_fields),
                            ..
                        } => {
                            expr.resolved_type = Some(type_id);
                            let struct_name = struct_name_for_defaults.unwrap_or(struct_name);
                            let mut initialized_fields: HashMap<String, Loc> = HashMap::new();
                            let mut pos_index = 0;

                            for elem in &mut lit.elements {
                                match elem {
                                    s::InitElement::Positional(elem_expr) => {
                                        if pos_index >= def_fields.len() {
                                            self.reporter.report(
                                                elem_expr.loc(),
                                                format!(
                                                    "Too many initializers for struct '{}'",
                                                    struct_name
                                                ),
                                            );
                                        } else {
                                            let def_field = &def_fields[pos_index];
                                            if let Some(&_prev_loc) =
                                                initialized_fields.get(def_field.name.as_str())
                                            {
                                                self.reporter.report(
                                                    elem_expr.loc(),
                                                    format!(
                                                        "Field '{}' initialized more than once",
                                                        def_field.name
                                                    ),
                                                );
                                            } else {
                                                initialized_fields.insert(
                                                    def_field.name.to_string(),
                                                    elem_expr.loc(),
                                                );
                                                let init_ty = self.infer_expr(elem_expr);
                                                if self
                                                    .type_db
                                                    .unify(def_field.ty, init_ty)
                                                    .is_err()
                                                {
                                                    if let Some(coerced) =
                                                        self.coerce(init_ty, def_field.ty)
                                                    {
                                                        elem_expr.resolved_type = Some(coerced);
                                                    } else {
                                                        let expected_str = self
                                                            .type_db
                                                            .type_to_string(def_field.ty);
                                                        let found_str =
                                                            self.type_db.type_to_string(init_ty);
                                                        let err = SemanticError::TypeMismatch {
                                                            loc: elem_expr.loc(),
                                                            expected: expected_str,
                                                            found: found_str,
                                                        };
                                                        self.reporter
                                                            .report(err.loc(), err.to_string());
                                                    }
                                                }
                                            }
                                        }
                                        pos_index += 1;
                                    }
                                    s::InitElement::Named { name, value } => {
                                        let field_name = name.source();
                                        if let Some(def_field) =
                                            def_fields.iter().find(|f| f.name == field_name)
                                        {
                                            if let Some(&_prev_loc) =
                                                initialized_fields.get(field_name)
                                            {
                                                self.reporter.report(
                                                    name.loc,
                                                    format!(
                                                        "Field '{}' initialized more than once",
                                                        field_name
                                                    ),
                                                );
                                            } else {
                                                initialized_fields
                                                    .insert(field_name.to_string(), name.loc);
                                                let init_ty = self.infer_expr(value);
                                                if self
                                                    .type_db
                                                    .unify(def_field.ty, init_ty)
                                                    .is_err()
                                                {
                                                    if let Some(coerced) =
                                                        self.coerce(init_ty, def_field.ty)
                                                    {
                                                        value.resolved_type = Some(coerced);
                                                    } else {
                                                        let expected_str = self
                                                            .type_db
                                                            .type_to_string(def_field.ty);
                                                        let found_str =
                                                            self.type_db.type_to_string(init_ty);
                                                        let err = SemanticError::TypeMismatch {
                                                            loc: name.loc,
                                                            expected: expected_str,
                                                            found: found_str,
                                                        };
                                                        self.reporter
                                                            .report(err.loc(), err.to_string());
                                                    }
                                                }
                                            }
                                        } else {
                                            self.reporter.report(
                                                name.loc,
                                                format!(
                                                    "Struct '{}' has no field named '{}'",
                                                    struct_name, field_name
                                                ),
                                            );
                                        }
                                    }
                                }
                            }

                            canon
                        }
                        Type::Slice(elem_ty) => {
                            for elem in &mut lit.elements {
                                match elem {
                                    s::InitElement::Positional(elem_expr) => {
                                        let elem_inferred = self.infer_expr(elem_expr);
                                        if self.type_db.unify(elem_ty, elem_inferred).is_err() {
                                            if let Some(coerced) =
                                                self.coerce(elem_inferred, elem_ty)
                                            {
                                                elem_expr.resolved_type = Some(coerced);
                                            } else {
                                                let expected_str =
                                                    self.type_db.type_to_string(elem_ty);
                                                let found_str =
                                                    self.type_db.type_to_string(elem_inferred);
                                                let err = SemanticError::TypeMismatch {
                                                    loc: elem_expr.loc(),
                                                    expected: expected_str,
                                                    found: found_str,
                                                };
                                                self.reporter.report(err.loc(), err.to_string());
                                            }
                                        }
                                    }
                                    s::InitElement::Named { name, .. } => {
                                        self.reporter.report(
                                            name.loc,
                                            "cannot use named fields in array/slice/dynarray literal".to_string(),
                                        );
                                    }
                                }
                            }
                            type_id
                        }
                        Type::DynArray(elem_ty) => {
                            if !lit.elements.is_empty() {
                                self.reporter.report(
                                    lit.loc,
                                    "Dynamic arrays cannot be initialized with non-empty static elements".to_string(),
                                );
                            }
                            for elem in &mut lit.elements {
                                match elem {
                                    s::InitElement::Positional(elem_expr) => {
                                        let elem_inferred = self.infer_expr(elem_expr);
                                        if self.type_db.unify(elem_ty, elem_inferred).is_err() {
                                            if let Some(coerced) =
                                                self.coerce(elem_inferred, elem_ty)
                                            {
                                                elem_expr.resolved_type = Some(coerced);
                                            } else {
                                                let expected_str =
                                                    self.type_db.type_to_string(elem_ty);
                                                let found_str =
                                                    self.type_db.type_to_string(elem_inferred);
                                                let err = SemanticError::TypeMismatch {
                                                    loc: elem_expr.loc(),
                                                    expected: expected_str,
                                                    found: found_str,
                                                };
                                                self.reporter.report(err.loc(), err.to_string());
                                            }
                                        }
                                    }
                                    s::InitElement::Named { name, .. } => {
                                        self.reporter.report(
                                            name.loc,
                                            "cannot use named fields in array/slice/dynarray literal".to_string(),
                                        );
                                    }
                                }
                            }
                            type_id
                        }
                        Type::Array(elem_ty, _) => {
                            for elem in &mut lit.elements {
                                match elem {
                                    s::InitElement::Positional(elem_expr) => {
                                        let elem_inferred = self.infer_expr(elem_expr);
                                        if self.type_db.unify(elem_ty, elem_inferred).is_err() {
                                            if let Some(coerced) =
                                                self.coerce(elem_inferred, elem_ty)
                                            {
                                                elem_expr.resolved_type = Some(coerced);
                                            } else {
                                                let expected_str =
                                                    self.type_db.type_to_string(elem_ty);
                                                let found_str =
                                                    self.type_db.type_to_string(elem_inferred);
                                                let err = SemanticError::TypeMismatch {
                                                    loc: elem_expr.loc(),
                                                    expected: expected_str,
                                                    found: found_str,
                                                };
                                                self.reporter.report(err.loc(), err.to_string());
                                            }
                                        }
                                    }
                                    s::InitElement::Named { name, .. } => {
                                        self.reporter.report(
                                            name.loc,
                                            "cannot use named fields in array/slice/dynarray literal".to_string(),
                                        );
                                    }
                                }
                            }
                            type_id
                        }
                        _ => self.infer_init_list_untyped(lit, expr.resolved_type),
                    }
                } else {
                    self.infer_init_list_untyped(lit, expr.resolved_type)
                }
            }
            s::ExprKind::Index(idx) => {
                let base_ty = self.infer_expr(&mut idx.array);
                let index_ty = self.infer_expr(&mut idx.index);

                let int_ty = self.type_db.int();
                let usize_ty = self.type_db.usize();
                let index_canon = self.resolve(index_ty);
                if index_canon != int_ty && index_canon != usize_ty {
                    if let Some(coerced) = self.coerce(index_ty, int_ty) {
                        idx.index.resolved_type = Some(coerced);
                    } else if let Some(coerced) = self.coerce(index_ty, usize_ty) {
                        idx.index.resolved_type = Some(coerced);
                    } else {
                        let expected_str = self.type_db.type_to_string(int_ty);
                        let found_str = self.type_db.type_to_string(index_ty);
                        let err = SemanticError::TypeMismatch {
                            loc: idx.index.loc(),
                            expected: expected_str,
                            found: found_str,
                        };
                        self.reporter.report(err.loc(), err.to_string());
                    }
                }

                let canonical_base = self.resolve(base_ty);
                match self.type_db.get_type(canonical_base) {
                    Type::Array(element_ty, len) => {
                        let len = *len;
                        if let Some(idx_val) = self.get_const_int(&idx.index)
                            && (idx_val < 0 || idx_val >= (len as i64))
                        {
                            self.reporter.report(
                                idx.loc,
                                format!(
                                    "Index {} is out of bounds for array of length {}",
                                    idx_val, len
                                ),
                            );
                        }
                        *element_ty
                    }
                    Type::Slice(element_ty) | Type::DynArray(element_ty) => *element_ty,
                    Type::Pointer(element_ty) => *element_ty,
                    _ => {
                        self.reporter.report(
                            idx.loc,
                            format!(
                                "Cannot index into non-array type '{}'",
                                self.type_db.type_to_string(canonical_base)
                            ),
                        );
                        self.type_db.void()
                    }
                }
            }
            s::ExprKind::Member(mem) => {
                if let s::ExprKind::Identifier(ident) = &mem.object.kind
                    && ident.source.as_str() == "."
                {
                    if let Some(expected) = self.expected_type {
                        let canonical_expected = self.resolve(expected);
                        let underlying = self.type_db.get_underlying_type(canonical_expected);
                        if let Type::Enum { name, variants, .. } =
                            self.type_db.get_type(underlying).clone()
                        {
                            let prop_name = mem.property.source();
                            if let Some(v) = variants.iter().find(|var| {
                                let vname: &str = var.name.as_ref();
                                vname == prop_name
                            }) {
                                if v.payload.is_some() {
                                    self.reporter.report(
                                        mem.property.loc,
                                        format!("Variant '{}' of enum '{}' requires a payload and must be constructed via call", prop_name, name),
                                    );
                                }
                                mem.object.resolved_type = Some(expected);
                                expr.resolved_type = Some(expected);
                                return expected;
                            } else {
                                self.reporter.report(
                                    mem.property.loc,
                                    format!("Variant '{}' not found in enum '{}'", prop_name, name),
                                );
                                let void_ty = self.type_db.void();
                                expr.resolved_type = Some(void_ty);
                                return void_ty;
                            }
                        }
                    }
                    self.reporter.report(
                        mem.property.loc,
                        format!(
                            "Cannot infer implicit enum variant '.{}' without context type",
                            mem.property.source()
                        ),
                    );
                    let void_ty = self.type_db.void();
                    expr.resolved_type = Some(void_ty);
                    return void_ty;
                }

                let base_ty = self.infer_expr(&mut mem.object);
                let mut canonical_base = self.resolve(base_ty);
                while let Type::Pointer(inner) = self.type_db.get_type(canonical_base).clone() {
                    canonical_base = self.resolve(inner);
                }
                let prop_name = mem.property.source();
                if canonical_base == self.type_db.module()
                    && let s::ExprKind::Identifier(ident) = &mem.object.kind
                {
                    let alias = ident.source.as_str();
                    let actual_module = if let Some(module) =
                        self.type_db.try_modules_get(&self.current_module)
                    {
                        if let Some(info) = module.imported_modules.get(&TokenSource::from(alias)) {
                            info.original_name().map(|n| n.as_str()).unwrap_or(alias)
                        } else {
                            alias
                        }
                    } else {
                        alias
                    };

                    if let Some(var_sig) = self
                        .type_db
                        .lookup_var_in_module(actual_module, &mem.property.source)
                    {
                        expr.resolved_type = Some(var_sig.ty);
                        return var_sig.ty;
                    } else if let Some(ty) = self
                        .type_db
                        .lookup_type_in_module(actual_module, &mem.property.source)
                    {
                        expr.resolved_type = Some(ty);
                        return ty;
                    } else {
                        self.reporter.report(
                            mem.property.loc,
                            format!(
                                "Symbol '{}' not found in module '{}'",
                                prop_name, ident.source
                            ),
                        );
                        let void_ty = self.type_db.void();
                        expr.resolved_type = Some(void_ty);
                        return void_ty;
                    }
                }

                let underlying_base = self.unwrap_pointer_type(canonical_base);
                match self.type_db.get_type(underlying_base) {
                    Type::Struct {
                        name,
                        fields: Some(fields),
                        ..
                    } => {
                        if let Some(f) = fields.iter().find(|field| {
                            let name: &str = field.name.as_ref();
                            name == prop_name
                        }) {
                            f.ty
                        } else {
                            if let Some((_def_module, sig)) =
                                self.lookup_method(underlying_base, &mem.property.source)
                            {
                                self.type_db.fn_pointer(
                                    sig.params.iter().map(|p| p.typ).collect(),
                                    sig.return_type,
                                )
                            } else {
                                self.reporter.report(
                                    mem.property.loc,
                                    format!(
                                        "Field or method '{}' not found in struct '{}'",
                                        prop_name, name
                                    ),
                                );
                                self.type_db.void()
                            }
                        }
                    }
                    Type::Enum { name, variants, .. } => {
                        if let Some(variant) = variants.iter().find(|v| {
                            let name: &str = v.name.as_ref();
                            name == prop_name
                        }) {
                            if variant.payload.is_some() {
                                self.reporter.report(
                                    mem.property.loc,
                                    format!("Variant '{}' of enum '{}' requires a payload and must be constructed via call", prop_name, name),
                                );
                            }
                            canonical_base
                        } else {
                            if let Some((_def_module, sig)) =
                                self.lookup_method(underlying_base, &mem.property.source)
                            {
                                self.type_db.fn_pointer(
                                    sig.params.iter().map(|p| p.typ).collect(),
                                    sig.return_type,
                                )
                            } else {
                                self.reporter.report(
                                    mem.property.loc,
                                    format!(
                                        "Variant or method '{}' not found in enum '{}'",
                                        prop_name, name
                                    ),
                                );
                                self.type_db.void()
                            }
                        }
                    }
                    Type::Array(element_ty, _)
                    | Type::Slice(element_ty)
                    | Type::DynArray(element_ty) => {
                        if prop_name == "len" {
                            self.type_db.usize()
                        } else if prop_name == "data" {
                            self.type_db.pointer(*element_ty)
                        } else if prop_name == "cap"
                            && matches!(self.type_db.get_type(underlying_base), Type::DynArray(..))
                        {
                            self.type_db.usize()
                        } else {
                            let type_kind = match self.type_db.get_type(underlying_base) {
                                Type::Array(..) => "Arrays",
                                Type::Slice(..) => "Slices",
                                Type::DynArray(..) => "Dynamic arrays",
                                _ => unreachable!(),
                            };
                            let expected_props = if type_kind == "Dynamic arrays" {
                                "fields 'len', 'data', and 'cap'"
                            } else {
                                "fields 'len' and 'data'"
                            };
                            self.reporter.report(
                                mem.property.loc,
                                format!(
                                    "{} only have {}, found '{}'",
                                    type_kind, expected_props, prop_name
                                ),
                            );
                            self.type_db.void()
                        }
                    }
                    Type::Pointer(_inner) => {
                        unreachable!("Pointer type should have been auto-dereferenced in the loop");
                    }
                    Type::String => match prop_name {
                        "data" => self.type_db.pointer(self.type_db.u8()),
                        "len" => self.type_db.usize(),
                        _ => {
                            self.reporter.report(
                                mem.property.loc,
                                format!("Field '{}' not found in string", prop_name),
                            );
                            self.type_db.void()
                        }
                    },
                    Type::Tuple(elements) => {
                        if let Ok(idx) = prop_name.parse::<usize>() {
                            if idx < elements.len() {
                                elements[idx]
                            } else {
                                self.reporter.report(
                                    mem.property.loc,
                                    format!(
                                        "Tuple index '{}' out of bounds (tuple size is {})",
                                        idx,
                                        elements.len()
                                    ),
                                );
                                self.type_db.void()
                            }
                        } else {
                            self.reporter.report(
                                mem.property.loc,
                                format!(
                                    "Tuple fields must be integer indices, found '{}'",
                                    prop_name
                                ),
                            );
                            self.type_db.void()
                        }
                    }
                    _ => {
                        self.reporter.report(
                            mem.property.loc,
                            format!(
                                "Cannot access member of non-struct/non-array type '{}'",
                                self.type_db.type_to_string(canonical_base)
                            ),
                        );
                        self.type_db.void()
                    }
                }
            }
            s::ExprKind::TypeInfo(ast_ty, _) => {
                let mut target_ty = self.map_type(ast_ty);
                target_ty = self.resolve_recursive(target_ty);
                let canonical_target = self.type_db.resolve(target_ty);
                self.type_db.queried_types_mut().insert(canonical_target);
                let type_info_id = self.type_db.type_info();
                self.type_db.pointer(type_info_id)
            }
            s::ExprKind::SizeOf(ast_ty, _) => {
                let target_ty = self.map_type(ast_ty);
                let _ = self.resolve_recursive(target_ty);
                self.type_db.usize()
            }
            s::ExprKind::AlignOf(ast_ty, _) => {
                let target_ty = self.map_type(ast_ty);
                let _ = self.resolve_recursive(target_ty);
                self.type_db.usize()
            }
            s::ExprKind::New(ast_ty, _) => {
                let mut target_ty = self.map_type(ast_ty);
                target_ty = self.resolve_recursive(target_ty);
                self.type_db.pointer(target_ty)
            }
            s::ExprKind::Drop(expr, _) => {
                self.builtin_drop(expr);
                self.type_db.void()
            }
            s::ExprKind::Make(ast_ty, args, loc) => {
                let make_loc = *loc;
                let raw_target_ty = self.map_type(ast_ty);
                let target_ty = self.resolve_recursive(raw_target_ty);
                self.check_type_defined(ast_ty.loc(), target_ty);

                let canonical = self.resolve(target_ty);
                let result_ty = match self.type_db.get_type(canonical).clone() {
                    Type::Pointer(elem_ty) => {
                        let elem_canon = self.resolve(elem_ty);
                        if self.type_db.constructor(elem_canon).is_some() {
                            self.typecheck_constructor_args(elem_canon, args, make_loc);
                            target_ty
                        } else if args.is_empty() {
                            target_ty
                        } else {
                            self.reporter.report(
                                make_loc,
                                format!(
                                    "type '{}' does not take constructor arguments",
                                    self.type_db.type_to_string(elem_canon)
                                ),
                            );
                            target_ty
                        }
                    }
                    Type::Slice(_) | Type::DynArray(_) => {
                        if args.len() == 1 {
                            let count_expr = &mut args[0];
                            let count_ty = self.infer_expr(count_expr);
                            let usize_ty = self.type_db.usize();
                            if self.type_db.unify(usize_ty, count_ty).is_err() {
                                if let Some(coerced) = self.coerce(count_ty, usize_ty) {
                                    count_expr.resolved_type = Some(coerced);
                                } else {
                                    let expected_str = self.type_db.type_to_string(usize_ty);
                                    let found_str = self.type_db.type_to_string(count_ty);
                                    let err = SemanticError::TypeMismatch {
                                        loc: count_expr.loc(),
                                        expected: expected_str,
                                        found: found_str,
                                    };
                                    self.reporter.report(err.loc(), err.to_string());
                                }
                            }
                            target_ty
                        } else if args.is_empty() {
                            target_ty
                        } else {
                            self.reporter.report(
                                make_loc,
                                format!(
                                    "make for slice or dynamic array expects 0 or 1 argument, found {}",
                                    args.len()
                                ),
                            );
                            target_ty
                        }
                    }
                    _ => {
                        if canonical.constructor(self.type_db).is_some() {
                            self.typecheck_constructor_args(canonical, args, make_loc);
                            target_ty
                        } else if args.is_empty() {
                            self.type_db.pointer(target_ty)
                        } else {
                            self.reporter.report(
                                make_loc,
                                format!(
                                    "type '{}' has no make constructor",
                                    self.type_db.type_to_string(canonical)
                                ),
                            );
                            target_ty
                        }
                    }
                };
                expr.resolved_type = Some(result_ty);
                result_ty
            }
            s::ExprKind::Default(_) => self.type_db.new_inference_var(),
        };
        expr.resolved_type = Some(ty);
        ty
    }

    fn infer_init_list_untyped(
        &mut self,
        lit: &mut s::InitListExpr,
        resolved_type: Option<TypeID>,
    ) -> TypeID {
        for elem in &lit.elements {
            if let s::InitElement::Named { name, .. } = elem {
                self.reporter.report(
                    name.loc,
                    "cannot use named fields in array/slice/dynarray literal".to_string(),
                );
            }
        }

        if lit.elements.is_empty() {
            if let Some(resolved) = resolved_type {
                self.resolve(resolved)
            } else {
                self.type_db.new_inference_var()
            }
        } else {
            let (first_ty, skip) = if let Some(resolved) = resolved_type {
                (resolved, 0)
            } else {
                let first_ty = self.infer_expr(lit.elements[0].expr_mut());
                (first_ty, 1)
            };
            for elem in lit.elements.iter_mut().skip(skip) {
                let elem_expr = elem.expr_mut();
                let elem_ty = self.infer_expr(elem_expr);
                if self.type_db.unify(first_ty, elem_ty).is_err() {
                    if let Some(coerced) = self.coerce(elem_ty, first_ty) {
                        elem_expr.resolved_type = Some(coerced);
                    } else {
                        let expected_str = self.type_db.type_to_string(first_ty);
                        let found_str = self.type_db.type_to_string(elem_ty);
                        let err = SemanticError::TypeMismatch {
                            loc: elem_expr.loc(),
                            expected: expected_str,
                            found: found_str,
                        };
                        self.reporter.report(err.loc(), err.to_string());
                    }
                }
            }
            self.type_db.array(first_ty, lit.elements.len())
        }
    }

    fn infer_call_expression(&mut self, call: &mut s::CallExpr) -> TypeID {
        // Intercept built-in operations
        match &mut call.callee.kind {
            s::ExprKind::Identifier(name) if name.source() == "push" => {
                if call.arguments.len() >= 1 {
                    self.infer_expr(call.arguments[0].expr_mut());
                }
                let mut elem_ty = None;
                if call.arguments.len() >= 1 {
                    let arg0 = call.arguments[0].expr();
                    let arg0_ty = arg0.resolved_type.unwrap_or_else(|| self.type_db.void());
                    let canon = self.resolve(arg0_ty);
                    if let Type::Pointer(inner) = self.type_db.get_type(canon).clone() {
                        let inner_canon = self.resolve(inner);
                        if let Type::DynArray(el) = self.type_db.get_type(inner_canon).clone() {
                            elem_ty = Some(el);
                        }
                    }
                }
                for arg in call.arguments.iter_mut().skip(1) {
                    self.expected_type = elem_ty;
                    self.infer_expr(arg.expr_mut());
                    self.expected_type = None;
                }
                self.builtin_push(call);
                return self.type_db.void();
            }
            s::ExprKind::Identifier(name) if name.source() == "pop" => {
                if call.arguments.len() >= 1 {
                    self.infer_expr(call.arguments[0].expr_mut());
                }
                return self.builtin_pop(call);
            }
            s::ExprKind::Identifier(name) if name.source() == "clear" => {
                if call.arguments.len() >= 1 {
                    self.infer_expr(call.arguments[0].expr_mut());
                }
                self.builtin_clear(call);
                return self.type_db.void();
            }
            _ => (),
        };

        let mut is_indirect = false;
        let mut sig: Option<FunctionSignature> = None;
        let mut fn_params = None;
        let mut is_instance_method_call = false;

        match &mut call.callee.kind {
            s::ExprKind::Member(s::MemberExpr { object, property }) => {
                let obj_ty = self.infer_expr(object.as_mut());
                if obj_ty == self.type_db.module() {
                    if let s::ExprKind::Identifier(ident) = &object.kind {
                        let alias = ident.source.as_str();
                        let actual_module = if let Some(module) =
                            self.type_db.try_modules_get(&self.current_module)
                        {
                            if let Some(info) =
                                module.imported_modules.get(&TokenSource::from(alias))
                            {
                                info.original_name().map(|n| n.as_str()).unwrap_or(alias)
                            } else {
                                alias
                            }
                        } else {
                            alias
                        };

                        call.resolved_name = Some(format!("{}.{}", actual_module, property.source));
                        call.module_name = Some(TokenSource::from(actual_module));
                        sig = self
                            .type_db
                            .try_modules_get(actual_module)
                            .and_then(|m| m.functions.get(&property.source))
                            .cloned();
                    } else {
                        is_indirect = true;
                    }
                } else {
                    let is_type_ident = match &object.kind {
                        s::ExprKind::Identifier(ident) => {
                            self.type_db
                                .lookup_type_in_module(&self.current_module, &ident.source)
                                .is_some()
                                && self.lookup_var(&ident.source).is_none()
                        }
                        s::ExprKind::Member(mem) => {
                            if let s::ExprKind::Identifier(mod_ident) = &mem.object.kind {
                                let mod_name = mod_ident.source.as_str();
                                self.type_db
                                    .lookup_type_in_module(mod_name, &mem.property.source)
                                    .is_some()
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };

                    let receiver_type_id = if is_type_ident {
                        match &object.kind {
                            s::ExprKind::Identifier(id) => self
                                .type_db
                                .lookup_type_in_module(&self.current_module, &id.source)
                                .unwrap(),
                            s::ExprKind::Member(mem) => {
                                let mod_name = match &mem.object.kind {
                                    s::ExprKind::Identifier(id) => id.source.as_str(),
                                    _ => unreachable!(),
                                };
                                self.type_db
                                    .lookup_type_in_module(mod_name, &mem.property.source)
                                    .unwrap()
                            }
                            _ => unreachable!(),
                        }
                    } else {
                        self.unwrap_pointer_type(obj_ty)
                    };

                    if let Some((def_module, found_sig)) =
                        self.lookup_method(receiver_type_id, &property.source)
                    {
                        call.resolved_name = Some(found_sig.full_name.clone());
                        call.module_name = Some(TokenSource::from(def_module));
                        sig = Some(found_sig);
                        if !is_type_ident {
                            is_instance_method_call = true;
                        }
                    } else {
                        is_indirect = true;
                    }
                }
            }
            s::ExprKind::Identifier(ident) => {
                if self.lookup_var(&ident.source).is_some() {
                    is_indirect = true;
                } else {
                    let module = self.type_db.modules_get(&self.current_module);
                    let local_sig = module.functions.get(&ident.source);
                    if let Some(lsig) = local_sig {
                        call.resolved_name = Some(lsig.full_name.clone());
                        call.module_name = Some(self.current_module.clone());
                        sig = Some(lsig.clone());
                    } else {
                        call.resolved_name = Some(ident.source.to_string());
                        sig = module
                            .imported_modules
                            .iter()
                            .filter(|(_, info)| info.is_all())
                            .find_map(|(name, info)| {
                                let orig_name = info
                                    .original_name()
                                    .cloned()
                                    .unwrap_or_else(|| name.clone());
                                if let Some(found_sig) = self
                                    .type_db
                                    .try_modules_get(&orig_name)
                                    .and_then(|m| m.functions.get(&ident.source))
                                {
                                    call.resolved_name = Some(found_sig.full_name.clone());
                                    call.module_name = Some(orig_name);
                                    Some(found_sig)
                                } else {
                                    None
                                }
                            })
                            .cloned();
                    }
                }
            }
            _ => {
                is_indirect = true;
            }
        }

        if is_indirect {
            let callee_ty = self.infer_expr(&mut call.callee);
            let canonical = self.resolve(callee_ty);
            if let Type::FnPointer { params, .. } = self.type_db.get_type(canonical).clone() {
                fn_params = Some(params);
            }
        }

        let mut arg_types = Vec::with_capacity(call.arguments.len());
        let param_offset = if is_instance_method_call { 1 } else { 0 };

        for (i, arg) in call.arguments.iter_mut().enumerate() {
            let mut expected = None;
            if let Some(ref sig) = sig {
                let target_param_idx = i + param_offset;
                match arg {
                    s::Argument::Positional(_) => {
                        if target_param_idx < sig.params.len() {
                            let param = &sig.params[target_param_idx];
                            if param.is_variadic {
                                let canon = self.resolve(param.typ);
                                if let Type::Slice(elem_ty) = self.type_db.get_type(canon).clone() {
                                    expected = Some(elem_ty);
                                }
                            } else {
                                expected = Some(param.typ);
                            }
                        }
                    }
                    s::Argument::Named { name, .. } => {
                        if let Some(param) = sig.params.iter().find(|p| p.name == name.source) {
                            expected = Some(param.typ);
                        }
                    }
                }
            } else if let Some(ref params) = fn_params {
                if i < params.len() {
                    expected = Some(params[i]);
                }
            }

            self.expected_type = expected;
            let arg_ty = self.infer_expr(arg.expr_mut());
            self.expected_type = None;
            arg_types.push(arg_ty);
        }

        if is_indirect {
            let callee_ty = self.infer_expr(&mut call.callee);
            let canonical = self.resolve(callee_ty);
            if let Type::FnPointer {
                params,
                return_type,
            } = self.type_db.get_type(canonical).clone()
            {
                let num_args = call.arguments.len();
                if num_args != params.len() {
                    self.reporter.report(
                        call.loc,
                        format!(
                            "Arity mismatch: function pointer expects {} arguments, but {} were provided",
                            params.len(),
                            num_args
                        ),
                    );
                }
                for (i, arg) in call.arguments.iter_mut().enumerate() {
                    if i >= params.len() {
                        break;
                    }
                    match arg {
                        s::Argument::Named { .. } => {
                            self.reporter.report(
                                arg.expr().loc(),
                                "Named parameters are not supported in indirect calls".to_string(),
                            );
                        }
                        s::Argument::Positional(arg_expr) => {
                            if self.check_arg_type(
                                params[i],
                                arg_types[i],
                                Some(arg_expr),
                                arg_expr.loc(),
                            ) && let Some(coerced) = self.coerce(arg_types[i], params[i])
                            {
                                arg_expr.resolved_type = Some(coerced);
                            }
                        }
                    }
                }
                return return_type;
            } else {
                self.reporter
                    .report(call.loc, "Expected function pointer type for indirect call");
                return self.type_db.void();
            }
        }

        if let Some(sig) = sig {
            // sig.is_foreign
            let num_args = call.arguments.len();
            let has_typed_variadic = sig.params.last().is_some_and(|p| p.is_variadic);
            let is_direct_slice = if has_typed_variadic && num_args == sig.params.len() {
                let last_arg_ty = self.resolve(arg_types[num_args - 1]);
                let var_param_ty = self.resolve(sig.params.last().unwrap().typ);
                if last_arg_ty == var_param_ty {
                    true
                } else {
                    match (
                        self.type_db.get_type(last_arg_ty).clone(),
                        self.type_db.get_type(var_param_ty).clone(),
                    ) {
                        (Type::Slice(a), Type::Slice(b)) => self.type_db.unify(a, b).is_ok(),
                        (Type::Array(a, _), Type::Slice(b)) => self.type_db.unify(a, b).is_ok(),
                        _ => false,
                    }
                }
            } else {
                false
            };

            if has_typed_variadic && !is_direct_slice {
                let fixed_count = sig.params.len() - 1;
                let mut param_to_arg: Vec<Option<usize>> = vec![None; fixed_count];
                let mut variadic_arg_indices: Vec<usize> = Vec::new();
                let mut mapping_error = false;

                for i in 0..num_args {
                    let arg = &call.arguments[i];
                    match arg {
                        s::Argument::Positional(_) => {
                            if i < fixed_count {
                                param_to_arg[i] = Some(i);
                            } else {
                                variadic_arg_indices.push(i);
                            }
                        }
                        s::Argument::Named { name, .. } => {
                            if let Some(param_idx) =
                                sig.params.iter().position(|p| p.name == name.source)
                            {
                                if param_idx < fixed_count {
                                    if let Some(existing_arg_idx) = param_to_arg[param_idx] {
                                        let existing_arg = &call.arguments[existing_arg_idx];
                                        if existing_arg.is_positional() {
                                            self.reporter.report(
                                                name.loc,
                                                format!(
                                                    "Parameter '{}' is already supplied as a positional argument",
                                                    name.source
                                                ),
                                            );
                                        } else {
                                            self.reporter.report(
                                                name.loc,
                                                format!(
                                                    "Duplicate argument for parameter '{}'",
                                                    name.source
                                                ),
                                            );
                                        }
                                        mapping_error = true;
                                    } else {
                                        param_to_arg[param_idx] = Some(i);
                                    }
                                } else {
                                    variadic_arg_indices.push(i);
                                }
                            } else {
                                self.reporter.report(
                                    name.loc,
                                    format!("Named parameter not found: {}", name.source),
                                );
                                mapping_error = true;
                            }
                        }
                    }
                }

                for j in 0..fixed_count {
                    if param_to_arg[j].is_none() {
                        self.reporter.report(
                            call.loc,
                            format!("Missing argument for parameter '{}'", sig.params[j].name),
                        );
                        mapping_error = true;
                    }
                }

                if !mapping_error {
                    // Type check fixed parameters
                    for j in 0..fixed_count {
                        if let Some(arg_idx) = param_to_arg[j] {
                            let arg_expr = call.arguments[arg_idx].expr_mut();
                            if self.check_arg_type(
                                sig.params[j].typ,
                                arg_types[arg_idx],
                                Some(arg_expr),
                                arg_expr.loc(),
                            ) && let Some(coerced) =
                                self.coerce(arg_types[arg_idx], sig.params[j].typ)
                            {
                                arg_expr.resolved_type = Some(coerced);
                            }
                        }
                    }

                    // Handle variadic parameter packing
                    let var_param = sig.params.last().unwrap();
                    let var_param_ty = self.resolve(var_param.typ);
                    let elem_ty = match self.type_db.get_type(var_param_ty).clone() {
                        Type::Slice(elem) => elem,
                        _ => unreachable!(),
                    };
                    let is_any_elem = self.resolve(elem_ty) == self.type_db.any();

                    for &arg_idx in &variadic_arg_indices {
                        let arg_expr = call.arguments[arg_idx].expr_mut();
                        if is_any_elem {
                            self.infer_expr(arg_expr);
                        } else {
                            if self.check_arg_type(
                                elem_ty,
                                arg_types[arg_idx],
                                Some(arg_expr),
                                arg_expr.loc(),
                            ) && let Some(coerced) = self.coerce(arg_types[arg_idx], elem_ty)
                            {
                                arg_expr.resolved_type = Some(coerced);
                            }
                        }
                    }

                    let original_args = std::mem::take(&mut call.arguments);
                    let mut original_args: Vec<Option<s::Argument>> =
                        original_args.into_iter().map(Some).collect();

                    let mut fixed_exprs = Vec::with_capacity(fixed_count);
                    for j in 0..fixed_count {
                        let arg_idx = param_to_arg[j].unwrap();
                        let arg = original_args[arg_idx].take().unwrap();
                        let expr = match arg {
                            s::Argument::Named { value, .. } => value,
                            s::Argument::Positional(expr) => expr,
                        };
                        fixed_exprs.push(expr);
                    }

                    let mut var_exprs = Vec::with_capacity(variadic_arg_indices.len());
                    for arg_idx in variadic_arg_indices {
                        let arg = original_args[arg_idx].take().unwrap();
                        let expr = match arg {
                            s::Argument::Named { value, .. } => value,
                            s::Argument::Positional(expr) => expr,
                        };
                        var_exprs.push(expr);
                    }

                    let mut packed_expr = if is_any_elem {
                        s::Expr::new(s::ExprKind::AnyCast(
                            s::AnyCastExpr::Array(var_exprs),
                            call.loc,
                        ))
                    } else {
                        s::Expr::new(s::ExprKind::InitList(s::InitListExpr {
                            elements: var_exprs
                                .into_iter()
                                .map(s::InitElement::Positional)
                                .collect(),
                            type_hint: None,
                            loc: call.loc,
                        }))
                    };

                    packed_expr.resolved_type = Some(var_param.typ);
                    let packed_ty = self.infer_expr(&mut packed_expr);
                    if let Some(coerced) = self.coerce(packed_ty, var_param.typ) {
                        packed_expr.resolved_type = Some(coerced);
                    }

                    let mut new_arguments = Vec::with_capacity(sig.params.len());
                    for expr in fixed_exprs {
                        new_arguments.push(s::Argument::Positional(expr));
                    }
                    new_arguments.push(s::Argument::Positional(packed_expr));

                    call.arguments = new_arguments;
                }
            } else {
                let mut param_to_arg: Vec<Option<usize>> = vec![None; sig.params.len()];
                let mut variadic_arg_indices: Vec<usize> = Vec::new();
                let mut mapping_error = false;

                for i in 0..num_args {
                    let arg = &call.arguments[i];
                    match arg {
                        s::Argument::Positional(_) => {
                            let target_param_idx = i + param_offset;
                            if target_param_idx < sig.params.len() {
                                param_to_arg[target_param_idx] = Some(i);
                            } else if sig.has_va_args {
                                variadic_arg_indices.push(i);
                            } else {
                                self.reporter.report(
                                    call.loc,
                                    format!(
                                        "Arity mismatch: function expects {} arguments, but {} were provided",
                                        sig.params.len() - param_offset,
                                        num_args
                                    ),
                                );
                                mapping_error = true;
                            }
                        }
                        s::Argument::Named { name, .. } => {
                            if let Some(param_idx) =
                                sig.params.iter().position(|p| p.name == name.source)
                            {
                                if let Some(existing_arg_idx) = param_to_arg[param_idx] {
                                    let existing_arg = &call.arguments[existing_arg_idx];
                                    if existing_arg.is_positional() {
                                        self.reporter.report(
                                            name.loc,
                                            format!(
                                                "Parameter '{}' is already supplied as a positional argument",
                                                name.source
                                            ),
                                        );
                                    } else {
                                        self.reporter.report(
                                            name.loc,
                                            format!(
                                                "Duplicate argument for parameter '{}'",
                                                name.source
                                            ),
                                        );
                                    }
                                    mapping_error = true;
                                } else {
                                    param_to_arg[param_idx] = Some(i);
                                }
                            } else {
                                self.reporter.report(
                                    name.loc,
                                    format!("Named parameter not found: {}", name.source),
                                );
                                mapping_error = true;
                            }
                        }
                    }
                }

                if is_instance_method_call {
                    if let s::ExprKind::Member(mem) = &mut call.callee.kind {
                        let self_expected_ty = sig.params[0].typ;
                        let mut self_actual_ty = self.infer_expr(&mut mem.object);

                        let canon_expected = self.resolve(self_expected_ty);
                        let canon_actual = self.resolve(self_actual_ty);

                        if let Type::Pointer(inner) = self.type_db.get_type(canon_expected).clone()
                        {
                            if self.resolve(inner) == canon_actual {
                                self_actual_ty = self_expected_ty;
                            }
                        }
                        if let Type::Pointer(inner) = self.type_db.get_type(canon_actual).clone() {
                            if self.resolve(inner) == canon_expected {
                                self_actual_ty = self_expected_ty;
                            }
                        }

                        let loc = mem.object.loc();
                        let _ = self.check_arg_type(
                            self_expected_ty,
                            self_actual_ty,
                            Some(&mut mem.object),
                            loc,
                        );
                    }
                }

                for j in param_offset..sig.params.len() {
                    if param_to_arg[j].is_none() {
                        self.reporter.report(
                            call.loc,
                            format!("Missing argument for parameter '{}'", sig.params[j].name),
                        );
                        mapping_error = true;
                    }
                }

                if !sig.has_va_args
                    && num_args > (sig.params.len() - param_offset)
                    && !mapping_error
                {
                    self.reporter.report(
                        call.loc,
                        format!(
                            "Arity mismatch: function expects {} arguments, but {} were provided",
                            sig.params.len() - param_offset,
                            num_args
                        ),
                    );
                    mapping_error = true;
                }

                // Now perform type-checking for mapped parameters
                for j in param_offset..sig.params.len() {
                    if let Some(arg_idx) = param_to_arg[j] {
                        let arg_expr = call.arguments[arg_idx].expr_mut();
                        if self.check_arg_type(
                            sig.params[j].typ,
                            arg_types[arg_idx],
                            Some(arg_expr),
                            arg_expr.loc(),
                        ) && let Some(coerced) =
                            self.coerce(arg_types[arg_idx], sig.params[j].typ)
                        {
                            arg_expr.resolved_type = Some(coerced);
                        }
                    }
                }

                // Turn named parameters into positional parameters if there were no errors
                if !mapping_error {
                    let original_args = std::mem::take(&mut call.arguments);
                    let mut original_args: Vec<Option<s::Argument>> =
                        original_args.into_iter().map(Some).collect();
                    let mut new_arguments = Vec::with_capacity(
                        sig.params.len() - param_offset + variadic_arg_indices.len(),
                    );

                    for j in param_offset..sig.params.len() {
                        if let Some(arg_idx) = param_to_arg[j] {
                            if let Some(arg) = original_args[arg_idx].take() {
                                let expr = match arg {
                                    s::Argument::Named { value, .. } => value,
                                    s::Argument::Positional(expr) => expr,
                                };
                                new_arguments.push(s::Argument::Positional(expr));
                            }
                        }
                    }

                    for arg_idx in variadic_arg_indices {
                        if let Some(arg) = original_args[arg_idx].take() {
                            let expr = match arg {
                                s::Argument::Named { value, .. } => value,
                                s::Argument::Positional(expr) => expr,
                            };
                            new_arguments.push(s::Argument::Positional(expr));
                        }
                    }

                    call.arguments = new_arguments;
                }
            }

            if sig.is_private
                && !call
                    .module_name
                    .as_ref()
                    .is_some_and(|m| *m == self.current_module)
            {
                self.reporter.report(
                    call.loc,
                    format!("Can not call private funtion {}", sig.name),
                );
            }
            sig.return_type
        } else {
            self.reporter.report(
                call.loc,
                format!(
                    "Function not found: {}",
                    call.resolved_name.as_deref().unwrap_or("<unknown>")
                ),
            );
            self.type_db.void()
        }
    }

    fn typecheck_constructor_args(
        &mut self,
        target_type_id: TypeID,
        args: &mut [s::Expr],
        loc: Loc,
    ) {
        if let Some((_, sig)) = self.lookup_method(target_type_id, &TokenSource::from("make")) {
            if self.unwrap_pointer_type(sig.params[0].typ)
                != self.unwrap_pointer_type(target_type_id)
            {
                todo!()
            }
            if sig.return_type != self.unwrap_pointer_type(target_type_id) {
                todo!()
            }
            let expected_params = &sig.params[1..];
            if args.len() != expected_params.len() && !sig.has_va_args {
                self.reporter.report(
                    loc,
                    format!(
                        "Constructor for '{}' expects {} arguments, but {} were provided",
                        self.type_db.type_to_string(target_type_id),
                        expected_params.len(),
                        args.len()
                    ),
                );
            }
            for (i, arg) in args.iter_mut().enumerate() {
                let expected = expected_params.get(i).map(|p| p.typ);
                self.expected_type = expected;
                let arg_ty = self.infer_expr(arg);
                self.expected_type = None;
                if let Some(expected_ty) = expected {
                    if self.check_arg_type(expected_ty, arg_ty, Some(arg), arg.loc()) {
                        if let Some(coerced) = self.coerce(arg_ty, expected_ty) {
                            arg.resolved_type = Some(coerced);
                        }
                    }
                }
            }
        } else {
            for arg in args {
                self.infer_expr(arg);
            }
        }
    }

    fn builtin_drop(&mut self, arg0: &mut s::Expr) {
        let arg0_ty = self.infer_expr(arg0);
        let canon0 = self.resolve(arg0_ty);

        let is_droppable = self.is_droppable_type(canon0);

        if !is_droppable {
            self.reporter.report(
                arg0.loc(),
                format!(
                    "drop expects a dynamic array, slice, string, pointer, or droppable struct/enum as its argument, found {}",
                    self.type_db.type_to_string(canon0)
                ),
            );
            return;
        }

        if let s::ExprKind::Identifier(ident) = &arg0.kind {
            if self.unowned_vars.contains(&ident.source) {
                self.reporter
                    .report(arg0.loc(), format!("use of moved value '{}'", ident.source));
                return;
            }
            self.unowned_vars.insert(ident.source.clone());
        }
    }

    fn builtin_push(&mut self, call: &mut s::CallExpr) {
        if !self.ensure_arity_min(call, 1) {
            return;
        }
        let arg0_expr = call.arguments[0].expr();
        if let Some(elem_ty) = self.ensure_arg_dyn_array_ptr(arg0_expr) {
            // Check if exactly one argument is passed after the pointer,
            // and if it is a slice, array, or dynamic array of the same element type.
            let mut is_slice_push = false;
            if call.arguments.len() == 2 {
                let arg1_expr = call.arguments[1].expr_mut();
                let arg1_ty = arg1_expr
                    .resolved_type
                    .unwrap_or_else(|| self.type_db.void());
                let canon1 = self.resolve(arg1_ty);
                match self.type_db.get_type(canon1).clone() {
                    Type::Slice(sub) | Type::DynArray(sub) | Type::Array(sub, _) => {
                        let sub_canon = self.resolve(sub);
                        let elem_canon = self.resolve(elem_ty);
                        if self.type_db.unify(sub_canon, elem_canon).is_ok() {
                            is_slice_push = true;
                            if let Some(coerced) = self.coerce(arg1_ty, canon1) {
                                arg1_expr.resolved_type = Some(coerced);
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !is_slice_push {
                for arg_arg in call.arguments[1..].iter_mut() {
                    let arg_expr = arg_arg.expr_mut();
                    self.ensure_arg_type(arg_expr, elem_ty);
                }
            }
        }
        call.resolved_name = Some("builtin.push".to_string());
    }

    fn builtin_pop(&mut self, call: &mut s::CallExpr) -> TypeID {
        if !self.ensure_arity(call, 1) {
            return self.type_db.void();
        }
        let arg0_expr = call.arguments[0].expr();
        if let Some(elem_ty) = self.ensure_arg_dyn_array_ptr(arg0_expr) {
            call.resolved_name = Some("builtin.pop".to_string());
            elem_ty
        } else {
            self.type_db.void()
        }
    }

    fn builtin_clear(&mut self, call: &mut s::CallExpr) {
        if !self.ensure_arity(call, 1) {
            return;
        }
        let arg0_expr = call.arguments[0].expr();
        if self.ensure_arg_dyn_array_ptr(arg0_expr).is_some() {
            call.resolved_name = Some("builtin.clear".to_string());
        }
    }

    fn ensure_arity(&mut self, call: &s::CallExpr, expected: usize) -> bool {
        if call.arguments.len() != expected {
            let name = match &call.callee.kind {
                s::ExprKind::Identifier(ident) => ident.source(),
                _ => "function",
            };
            self.reporter.report(
                call.loc,
                format!(
                    "{name} expects {expected} arguments, but {} were provided",
                    call.arguments.len()
                ),
            );
            false
        } else {
            true
        }
    }

    fn ensure_arity_min(&mut self, call: &s::CallExpr, min: usize) -> bool {
        if call.arguments.len() < min {
            let name = match &call.callee.kind {
                s::ExprKind::Identifier(ident) => ident.source(),
                _ => "function",
            };
            self.reporter.report(
                call.loc,
                format!(
                    "{name} expects at least {min} arguments, but {} were provided",
                    call.arguments.len()
                ),
            );
            false
        } else {
            true
        }
    }

    fn ensure_arg_type(&mut self, arg: &mut s::Expr, expected_ty: TypeID) -> bool {
        let arg_ty = self.infer_expr(arg);
        if self.check_arg_type(expected_ty, arg_ty, Some(arg), arg.loc()) {
            if let Some(coerced) = self.coerce(arg_ty, expected_ty) {
                arg.resolved_type = Some(coerced);
            }
            true
        } else {
            false
        }
    }

    fn ensure_arg_dyn_array_ptr(&mut self, arg: &s::Expr) -> Option<TypeID> {
        let arg_ty = arg.resolved_type.unwrap_or_else(|| self.type_db.void());
        let canon = self.resolve(arg_ty);
        match self.type_db.get_type(canon).clone() {
            Type::Pointer(inner) => {
                let inner_canon = self.resolve(inner);
                match self.type_db.get_type(inner_canon).clone() {
                    Type::DynArray(elem_ty) => Some(elem_ty),
                    _ => {
                        self.reporter.report(
                            arg.loc(),
                            format!(
                                "Expected pointer to a dynamic array, but found pointer to '{}'",
                                self.type_db.type_to_string(inner_canon)
                            ),
                        );
                        None
                    }
                }
            }
            _ => {
                self.reporter.report(
                    arg.loc(),
                    format!(
                        "Expected pointer to a dynamic array, but found '{}'",
                        self.type_db.type_to_string(canon)
                    ),
                );
                None
            }
        }
    }

    fn is_local_array_var(&self, expr: &s::Expr) -> bool {
        match &expr.kind {
            s::ExprKind::Identifier(ident) => {
                if let Some(ty) = self.lookup_var(&ident.source) {
                    let canonical = self.type_db.resolve(ty);
                    matches!(self.type_db.get_type(canonical), Type::Array(..))
                } else {
                    false
                }
            }
            s::ExprKind::Cast(_, inner, _) | s::ExprKind::AutoCast(inner, _) => {
                self.is_local_array_var(inner)
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Scalar(inner), _) => {
                self.is_local_array_var(inner)
            }
            _ => false,
        }
    }

    fn is_expr_stack_backed(&self, expr: &s::Expr) -> bool {
        match &expr.kind {
            s::ExprKind::InitList(..) => true,
            s::ExprKind::Cast(_, inner, _) | s::ExprKind::AutoCast(inner, _) => {
                self.is_expr_stack_backed(inner)
            }
            s::ExprKind::Unary(s::UnaryExpr { op, right }) if op.is_refer() => {
                self.is_expr_stack_backed(right.as_ref())
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Scalar(inner), _) => {
                self.is_expr_stack_backed(inner)
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Array(arr), _) => {
                arr.iter().any(|item| self.is_expr_stack_backed(item))
            }
            s::ExprKind::Tuple(exprs, _) => {
                exprs.iter().any(|item| self.is_expr_stack_backed(item))
            }
            _ => false,
        }
    }

    fn is_castable_type(&self, ty: TypeID) -> bool {
        let underlying = self.type_db.get_underlying_type(ty);
        if self.type_db.is_primitive_castable(underlying) {
            return true;
        }
        matches!(
            self.type_db.get_type(underlying),
            Type::Pointer(_) | Type::Enum { .. } | Type::TypeVar(_) | Type::FnPointer { .. }
        )
    }

    fn resolve(&mut self, id: TypeID) -> TypeID {
        self.type_db.resolve(id)
    }

    fn resolve_recursive(&mut self, id: TypeID) -> TypeID {
        self.resolve_recursive_visited.clear();
        self.resolve_recursive_impl(id)
    }

    fn resolve_recursive_impl(&mut self, id: TypeID) -> TypeID {
        let canonical = self.type_db.resolve(id);
        if !self.resolve_recursive_visited.insert(canonical) {
            return canonical;
        }
        let resolved = self.resolve(canonical);

        match self.type_db.get_type(resolved).clone() {
            Type::Pointer(inner) => {
                self.resolve_recursive_impl(inner);
            }
            Type::Array(inner, _) => {
                self.resolve_recursive_impl(inner);
            }
            Type::Slice(inner) => {
                self.resolve_recursive_impl(inner);
            }
            Type::Tuple(elements) => {
                for elem in elements {
                    self.resolve_recursive_impl(elem);
                }
            }
            Type::FnPointer {
                params,
                return_type,
            } => {
                for param in params {
                    self.resolve_recursive_impl(param);
                }
                self.resolve_recursive_impl(return_type);
            }
            Type::Distinct { base, .. } => {
                self.resolve_recursive_impl(base);
            }
            Type::Struct {
                fields: Some(fields),
                ..
            } => {
                for field in fields {
                    self.resolve_recursive_impl(field.ty);
                }
            }
            Type::Enum { repr, variants, .. } => {
                self.resolve_recursive_impl(repr);
                for variant in variants {
                    if let Some(payload_ty) = variant.payload {
                        self.resolve_recursive_impl(payload_ty);
                    }
                }
            }
            _ => {}
        }
        self.resolve_recursive_visited.remove(&canonical);
        resolved
    }

    fn check_type_ext(
        &mut self,
        expected: TypeID,
        actual: TypeID,
        expr: Option<&s::Expr>,
        loc: Loc,
        is_arg: bool,
    ) -> bool {
        if actual == self.type_db.void() && expected != self.type_db.void() {
            let expected_str = self.type_db.type_to_string(expected);
            let found_str = self.type_db.type_to_string(actual);
            let err = SemanticError::TypeMismatch {
                loc,
                expected: expected_str,
                found: found_str,
            };
            self.reporter.report(err.loc(), err.to_string());
            return false;
        }

        if self.type_db.unify(expected, actual).is_ok() {
            if let Some(e) = expr
                && !is_arg
            {
                self.check_local_array_to_slice_coercion(expected, actual, e, loc);
            }
            return true;
        }

        if let Some(coerced) = self.coerce(actual, expected) {
            if let Some(e) = expr
                && !is_arg
            {
                self.check_local_array_to_slice_coercion(coerced, actual, e, loc);
            }
            return true;
        }

        let expected_str = self.type_db.type_to_string(expected);
        let found_str = self.type_db.type_to_string(actual);
        let err = SemanticError::TypeMismatch {
            loc,
            expected: expected_str,
            found: found_str,
        };
        self.reporter.report(err.loc(), err.to_string());
        false
    }

    fn check_type(
        &mut self,
        expected: TypeID,
        actual: TypeID,
        expr: Option<&s::Expr>,
        loc: Loc,
    ) -> bool {
        self.check_type_ext(expected, actual, expr, loc, false)
    }

    fn check_arg_type(
        &mut self,
        expected: TypeID,
        actual: TypeID,
        expr: Option<&s::Expr>,
        loc: Loc,
    ) -> bool {
        self.check_type_ext(expected, actual, expr, loc, true)
    }

    fn unify_or_report(&mut self, expected: TypeID, actual: TypeID, loc: Loc) -> bool {
        self.check_type(expected, actual, None, loc)
    }

    fn check_local_array_to_slice_coercion(
        &mut self,
        expected: TypeID,
        actual: TypeID,
        expr: &s::Expr,
        loc: Loc,
    ) {
        let actual_canon = self.type_db.resolve(actual);
        let expected_canon = self.type_db.resolve(expected);
        if matches!(self.type_db.get_type(expected_canon), Type::Slice(..))
            && matches!(self.type_db.get_type(actual_canon), Type::Array(..))
            && self.is_local_array_var(expr)
        {
            self.reporter.report(
                    loc,
                    "Implicit coercion of standard local arrays to slices is only allowed for function arguments".to_string()
                );
        }
    }

    fn eval_const_expr(&self, expr: &s::Expr) -> Result<ConstValue, String> {
        match &expr.kind {
            s::ExprKind::Integer(integer_literal) => {
                Ok(ConstValue::Int(integer_literal.value as i64))
            }
            s::ExprKind::Char(char_literal) => Ok(ConstValue::Int(char_literal.value as i64)),
            s::ExprKind::Float(float_literal) => Ok(ConstValue::Float(float_literal.value)),
            s::ExprKind::Bool(bool_literal) => Ok(ConstValue::Bool(bool_literal.value)),
            s::ExprKind::StringLiteral(token) => Ok(ConstValue::String(token.source.to_string())),
            s::ExprKind::Identifier(ident) => {
                for scope in self.constants.iter().rev() {
                    if let Some(val) = scope.get(ident.source.as_str()) {
                        return Ok(val.clone());
                    }
                }
                Err(format!(
                    "Identifier '{}' is not a defined constant",
                    ident.source
                ))
            }
            s::ExprKind::Unary(unary_expr) => {
                let right_val = self.eval_const_expr(&unary_expr.right)?;
                match (unary_expr.op, right_val) {
                    (s::Op::Neg, ConstValue::Int(v)) => Ok(ConstValue::Int(-v)),
                    (s::Op::Neg, ConstValue::Float(v)) => Ok(ConstValue::Float(-v)),
                    (s::Op::Not, ConstValue::Bool(b)) => Ok(ConstValue::Bool(!b)),
                    _ => Err("Invalid unary operator for constant expression".to_string()),
                }
            }
            s::ExprKind::Binary(binary_expr) => {
                let left_val = self.eval_const_expr(&binary_expr.left)?;
                let right_val = self.eval_const_expr(&binary_expr.right)?;
                match (binary_expr.op, left_val, right_val) {
                    (s::Op::Add, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Int(l + r))
                    }
                    (s::Op::Add, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Float(l + r))
                    }
                    (s::Op::Sub, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Int(l - r))
                    }
                    (s::Op::Sub, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Float(l - r))
                    }
                    (s::Op::Mul, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Int(l * r))
                    }
                    (s::Op::Mul, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Float(l * r))
                    }
                    (s::Op::Div, ConstValue::Int(l), ConstValue::Int(r)) => {
                        if r == 0 {
                            Err("Division by zero in constant expression".to_string())
                        } else {
                            Ok(ConstValue::Int(l / r))
                        }
                    }
                    (s::Op::Div, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Float(l / r))
                    }
                    (s::Op::Mod, ConstValue::Int(l), ConstValue::Int(r)) => {
                        if r == 0 {
                            Err("Modulo by zero in constant expression".to_string())
                        } else {
                            Ok(ConstValue::Int(l % r))
                        }
                    }
                    (s::Op::Eq, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Bool(l == r))
                    }
                    (s::Op::Eq, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Bool(l == r))
                    }
                    (s::Op::NotEq, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Bool(l != r))
                    }
                    (s::Op::NotEq, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Bool(l != r))
                    }
                    (s::Op::Lt, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Bool(l < r))
                    }
                    (s::Op::Lt, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Bool(l < r))
                    }
                    (s::Op::LtEq, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Bool(l <= r))
                    }
                    (s::Op::LtEq, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Bool(l <= r))
                    }
                    (s::Op::Gt, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Bool(l > r))
                    }
                    (s::Op::Gt, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Bool(l > r))
                    }
                    (s::Op::GtEq, ConstValue::Int(l), ConstValue::Int(r)) => {
                        Ok(ConstValue::Bool(l >= r))
                    }
                    (s::Op::GtEq, ConstValue::Float(l), ConstValue::Float(r)) => {
                        Ok(ConstValue::Bool(l >= r))
                    }
                    _ => Err("Invalid binary operator/types for constant expression".to_string()),
                }
            }
            s::ExprKind::Member(mem) => {
                if let s::ExprKind::Identifier(ident) = &mem.object.kind {
                    let key = format!("{}.{}", ident.source, mem.property.source);
                    for scope in self.constants.iter().rev() {
                        if let Some(val) = scope.get(&key) {
                            return Ok(val.clone());
                        }
                        if let Some(val) = scope.get(mem.property.source.as_str()) {
                            return Ok(val.clone());
                        }
                    }
                }
                Err("Expression is not a valid compile-time constant".to_string())
            }
            _ => Err("Expression is not a valid compile-time constant".to_string()),
        }
    }

    fn is_implicit_enum_variant(&self, expr: &s::Expr) -> bool {
        if let s::ExprKind::Member(mem) = &expr.kind {
            if let s::ExprKind::Identifier(ident) = &mem.object.kind {
                return ident.source.as_str() == ".";
            }
        }
        false
    }
}
