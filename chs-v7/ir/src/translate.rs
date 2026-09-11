use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use diagnostic::{ChsResult, DiagnosticReporter, bail};
use lex_just_parse::lexer::{Loc, TokenSource};
use syntax::ast as s;
use types as t;

use crate::{ConstVal, StructLayout};

use super::block::BasicBlock;
use super::builder::IrBuilder;
use super::function::{Function, Signature};
use super::inst::{BlockId, InstData, Instruction, Operand};
use super::module::Module;
use super::types::Type;

pub fn translate_ast_items(
    asts: &[s::FileAst],
    type_db: &mut t::TypeDatabase,
    reporter: &mut DiagnosticReporter,
) -> ChsResult<Module> {
    let mut module = Module::new();

    // Pass 0: Gather and evaluate global constants
    let mut global_constants = HashMap::new();
    for ast in asts {
        let mod_prefix = &ast.module.source;
        for item in &ast.items {
            if let s::FileItem::Const(decl) = item {
                let val = if let Some(val) = eval_const_expr(&decl.expr, &global_constants) {
                    val
                } else {
                    reporter.report(
                        decl.expr.loc(),
                        format!("{} are not allowed in constants", decl.expr.name()),
                    );
                    continue;
                };
                global_constants.insert(decl.name.source().to_string(), val.clone());
                global_constants.insert(format!("{}.{}", mod_prefix, decl.name.source()), val);
            }
        }
    }

    // Pass 1: Gather global declarations
    for ast in asts {
        for item in &ast.items {
            match item {
                s::FileItem::Function(decl) => {
                    let t::FunctionSignature {
                        params,
                        return_type,
                        is_private,
                        ..
                    } = type_db
                        .modules_get(&ast.module.source)
                        .functions
                        .get(&decl.signature.name.source)
                        .unwrap();

                    match module
                        .functions_mut()
                        .entry(decl.resolved_name.clone().unwrap())
                    {
                        std::collections::hash_map::Entry::Occupied(_) => {
                            reporter.report(
                                decl.signature.name.loc,
                                format!(
                                    "Function '{}' is already defined",
                                    decl.resolved_name.as_ref().unwrap()
                                ),
                            );
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            let is_foreign = decl
                                .directives
                                .iter()
                                .any(|d| matches!(d, s::FunctionDirective::Foreign(..)));

                            let is_export = decl
                                .directives
                                .iter()
                                .any(|d| matches!(d, s::FunctionDirective::Export));

                            let signature = Signature {
                                name: decl.signature.name.source.clone(),
                                has_va_args: decl.signature.va_args,
                                params: params.iter().map(|p| p.typ).collect(),
                                return_type: *return_type,
                                is_private: *is_private,
                            };

                            let (has_link_name, link_name) = decl
                                .directives
                                .iter()
                                .find_map(|d| {
                                    if let s::FunctionDirective::LinkName(..) = d {
                                        d.as_link_name()
                                    } else {
                                        None
                                    }
                                })
                                .map(|n| (true, n.source.clone()))
                                .unwrap_or_else(|| (false, signature.name.clone()));

                            let function = if is_foreign {
                                let name = TokenSource::from(decl.resolved_name.clone().unwrap());
                                Function::foreign(name.clone(), link_name, signature)
                            } else {
                                let name = TokenSource::from(decl.resolved_name.clone().unwrap());
                                let link_name = if has_link_name { Some(link_name) } else { None };
                                Function::new(name, signature, is_export, link_name)
                            };
                            entry.insert(function);
                        }
                    }
                }
                s::FileItem::Operator(decl) => {
                    let mut params = Vec::new();
                    for param in &decl.parameters {
                        params.push(s::map_type(&param.typ, type_db, ast.module.source.as_str()));
                    }
                    let return_type =
                        s::map_type(&decl.return_type, type_db, ast.module.source.as_str());
                    let signature = Signature {
                        name: decl.resolved_name.clone().unwrap(),
                        params,
                        return_type,

                        has_va_args: false,
                        is_private: false,
                    };

                    match module.functions_mut().entry(signature.name.to_string()) {
                        std::collections::hash_map::Entry::Occupied(_) => {
                            reporter.report(
                                decl.loc,
                                format!(
                                    "Function '{}' is already defined",
                                    decl.resolved_name.as_ref().unwrap()
                                ),
                            );
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            let name = decl.resolved_name.clone().unwrap();
                            entry.insert(Function::new(name.clone(), signature, false, None));
                        }
                    }
                }
                s::FileItem::MethodImplementation(method) => {
                    let self_ty = s::map_type(
                        &method.self_parameter.typ,
                        type_db,
                        ast.module.source.as_str(),
                    );
                    let receiver_type_id = self_ty.unwrap_pointer_type(type_db);
                    let receiver_name = type_db.type_to_string(receiver_type_id);
                    let method_key = TokenSource::from(format!(
                        "{}.{}",
                        receiver_name, method.function.signature.name.source
                    ));

                    if let Some(sig_info) = type_db
                        .modules_get(&ast.module.source)
                        .functions
                        .get(&method_key)
                    {
                        let signature = Signature {
                            name: method.function.signature.name.source.clone(),
                            has_va_args: method.function.signature.va_args,
                            params: sig_info.params.iter().map(|p| p.typ).collect(),
                            return_type: sig_info.return_type,
                            is_private: sig_info.is_private,
                        };
                        let name =
                            TokenSource::from(method.function.resolved_name.clone().unwrap());
                        module.functions_mut().insert(
                            name.to_string(),
                            Function::new(name.clone(), signature, false, None),
                        );
                    }
                }
                _ => (),
            }
        }
    }

    // Pass 1.5: Gather and translate global/thread-local variables
    for ast in asts {
        for item in &ast.items {
            match item {
                s::FileItem::VarDecl(decl) => {
                    if decl.is_foreign {
                        if decl.is_thread_local {
                            reporter.report(
                                decl.names[0].loc,
                                "foreign variables cannot be thread local",
                            );
                            continue;
                        }
                        let var_ty = decl
                            .expr
                            .resolved_type
                            .expect("Foreign variable type resolved");
                        for name_tok in &decl.names {
                            module.add_global(crate::Global {
                                name: name_tok.source.clone(),
                                ty: var_ty,
                                is_thread_local: false,
                                is_foreign: true,
                                is_export: false,
                                init_val: ConstVal::Zero,
                            });
                        }
                        continue;
                    }

                    let init_val = match eval_global_var(reporter, &global_constants, decl) {
                        Some(value) => value,
                        None => continue,
                    };
                    let var_ty = if let Some(ref ast_ty) = decl.var_type {
                        s::map_type(ast_ty, type_db, ast.module.source.as_str())
                    } else {
                        match init_val {
                            ConstVal::Int(_) => type_db.int(),
                            ConstVal::Float(_) => type_db.float(),
                            ConstVal::Bool(_) => type_db.bool(),
                            ConstVal::Zero => decl.expr.resolved_type.unwrap(),
                        }
                    };
                    for name_tok in &decl.names {
                        module.add_global(crate::Global {
                            name: name_tok.source.clone(),
                            ty: var_ty,
                            is_thread_local: decl.is_thread_local,
                            is_foreign: false,
                            is_export: decl.is_export,
                            init_val: init_val.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    // Pass 2: Translate bodies
    for ast in asts {
        for item in &ast.items {
            match item {
                s::FileItem::Function(decl) => {
                    translate_function(
                        decl,
                        &mut module,
                        type_db,
                        reporter,
                        &global_constants,
                        ast.module.source.clone(),
                    );
                }
                s::FileItem::Operator(decl) => {
                    translate_operator(
                        decl,
                        &mut module,
                        type_db,
                        reporter,
                        &global_constants,
                        ast.module.source.clone(),
                    );
                }
                s::FileItem::MethodImplementation(method) => {
                    translate_method_implementation(
                        method,
                        &mut module,
                        type_db,
                        reporter,
                        &global_constants,
                        ast.module.source.clone(),
                    );
                }
                _ => {}
            }
        }
    }

    if reporter.has_errors() {
        bail!("fail to translate")
    } else {
        Ok(module)
    }
}

fn eval_global_var(
    reporter: &mut DiagnosticReporter,
    global_constants: &HashMap<String, ConstVal>,
    decl: &s::VarDeclStmt,
) -> Option<ConstVal> {
    let init_val = if let Some(init_val) = eval_const_expr(&decl.expr, global_constants) {
        init_val
    } else {
        reporter.report(
            decl.expr.loc(),
            format!(
                "expressions of kind '{}' are not allowed in global variable initialization",
                decl.expr.name()
            ),
        );
        return None;
    };
    Some(init_val)
}

fn eval_const_expr(
    expr: &s::Expr,
    globals: &std::collections::HashMap<String, ConstVal>,
) -> Option<ConstVal> {
    let val = match &expr.kind {
        s::ExprKind::Integer(lit) => ConstVal::Int(lit.value as i64),
        s::ExprKind::Char(lit) => ConstVal::Int(lit.value as i64),
        s::ExprKind::Float(lit) => ConstVal::Float(lit.value),
        s::ExprKind::Bool(b) => ConstVal::Bool(b.value),
        s::ExprKind::Identifier(ident) => {
            if let Some(val) = globals.get(ident.source()) {
                val.clone()
            } else {
                ConstVal::Zero
            }
        }
        s::ExprKind::Member(mem) => {
            if let s::ExprKind::Identifier(ident) = &mem.object.kind {
                let key = format!("{}.{}", ident.source(), mem.property.source());
                if let Some(val) = globals.get(&key) {
                    val.clone()
                } else {
                    ConstVal::Zero
                }
            } else {
                ConstVal::Zero
            }
        }
        s::ExprKind::Unary(unary_expr) => {
            let right_val = eval_const_expr(&unary_expr.right, globals)?;
            match (unary_expr.op, right_val) {
                (s::Op::Neg, ConstVal::Int(v)) => ConstVal::Int(-v),
                (s::Op::Neg, ConstVal::Float(v)) => ConstVal::Float(-v),
                (s::Op::Not, ConstVal::Bool(b)) => ConstVal::Bool(!b),
                _ => ConstVal::Zero,
            }
        }
        s::ExprKind::Binary(binary_expr) => {
            let left_val = eval_const_expr(&binary_expr.left, globals)?;
            let right_val = eval_const_expr(&binary_expr.right, globals)?;
            match (binary_expr.op, left_val, right_val) {
                (s::Op::Add, ConstVal::Int(l), ConstVal::Int(r)) => ConstVal::Int(l + r),
                (s::Op::Add, ConstVal::Float(l), ConstVal::Float(r)) => ConstVal::Float(l + r),
                (s::Op::Sub, ConstVal::Int(l), ConstVal::Int(r)) => ConstVal::Int(l - r),
                (s::Op::Sub, ConstVal::Float(l), ConstVal::Float(r)) => ConstVal::Float(l - r),
                (s::Op::Mul, ConstVal::Int(l), ConstVal::Int(r)) => ConstVal::Int(l * r),
                (s::Op::Mul, ConstVal::Float(l), ConstVal::Float(r)) => ConstVal::Float(l * r),
                (s::Op::Div, ConstVal::Int(l), ConstVal::Int(r)) => {
                    if r == 0 {
                        ConstVal::Zero
                    } else {
                        ConstVal::Int(l / r)
                    }
                }
                (s::Op::Div, ConstVal::Float(l), ConstVal::Float(r)) => ConstVal::Float(l / r),
                (s::Op::Mod, ConstVal::Int(l), ConstVal::Int(r)) => {
                    if r == 0 {
                        ConstVal::Zero
                    } else {
                        ConstVal::Int(l % r)
                    }
                }
                _ => ConstVal::Zero,
            }
        }
        s::ExprKind::Null(_) => ConstVal::Zero,
        s::ExprKind::Cast(_, expr, _) => eval_const_expr(expr, globals)?,
        s::ExprKind::AutoCast(expr, _) => eval_const_expr(expr, globals)?,
        s::ExprKind::InitList(_) => ConstVal::Zero,
        s::ExprKind::Default(_) | s::ExprKind::StringLiteral(_) => ConstVal::Zero,
        _ => return None,
    };
    Some(val)
}

#[derive(Clone, Copy)]
struct LoopTarget {
    break_block: BlockId,
    continue_block: BlockId,
    scope_depth: usize,
}

struct Translator<'a, 'b> {
    builder: IrBuilder<'a>,
    scopes: Vec<HashMap<String, (Operand, Type)>>,
    defers: Vec<Vec<s::Stmt>>,
    module: &'b Module,
    reporter: &'b mut DiagnosticReporter,
    loop_targets: Vec<LoopTarget>,
    current_file: Option<PathBuf>,
    constants: Vec<HashMap<String, ConstVal>>,
    current_module: TokenSource,
    owned_vars: Vec<HashSet<String>>,
    dropped_derived: Vec<HashSet<String>>,
}

impl<'a, 'b> Translator<'a, 'b> {
    fn new(
        blocks: &'a mut Vec<BasicBlock>,
        instructions: &'a mut Vec<InstData>,
        module: &'b Module,
        type_db: &'a mut t::TypeDatabase,
        reporter: &'b mut DiagnosticReporter,
        global_constants: &HashMap<String, ConstVal>,
        current_module: TokenSource,
    ) -> Self {
        Self {
            builder: IrBuilder::new(blocks, instructions, type_db),
            scopes: vec![HashMap::new()],
            defers: vec![Vec::new()],
            module,
            reporter,
            loop_targets: Vec::new(),
            current_file: None,
            constants: vec![global_constants.clone()],
            current_module,
            owned_vars: vec![HashSet::new()],
            dropped_derived: vec![HashSet::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.defers.push(Vec::new());
        self.constants.push(HashMap::new());
        self.owned_vars.push(HashSet::new());
        self.dropped_derived.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
        self.defers.pop();
        self.constants.pop();
        self.owned_vars.pop();
        self.dropped_derived.pop();
    }

    fn lookup_const(&self, name: &str) -> Option<&ConstVal> {
        for scope in self.constants.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val);
            }
        }
        None
    }

    fn eval_const_expr(&self, expr: &s::Expr) -> ConstVal {
        match &expr.kind {
            s::ExprKind::Integer(lit) => ConstVal::Int(lit.value as i64),
            s::ExprKind::Char(lit) => ConstVal::Int(lit.value as i64),
            s::ExprKind::Float(lit) => ConstVal::Float(lit.value),
            s::ExprKind::Bool(b) => ConstVal::Bool(b.value),
            s::ExprKind::Identifier(ident) => {
                if let Some(val) = self.lookup_const(ident.source()) {
                    val.clone()
                } else {
                    ConstVal::Zero
                }
            }
            s::ExprKind::Member(mem) => {
                if let s::ExprKind::Identifier(ident) = &mem.object.kind {
                    let key = format!("{}.{}", ident.source(), mem.property.source());
                    if let Some(val) = self.lookup_const(&key) {
                        val.clone()
                    } else {
                        ConstVal::Zero
                    }
                } else {
                    ConstVal::Zero
                }
            }
            s::ExprKind::Unary(unary_expr) => {
                let right_val = self.eval_const_expr(&unary_expr.right);
                match (unary_expr.op, right_val) {
                    (s::Op::Neg, ConstVal::Int(v)) => ConstVal::Int(-v),
                    (s::Op::Neg, ConstVal::Float(v)) => ConstVal::Float(-v),
                    (s::Op::Not, ConstVal::Bool(b)) => ConstVal::Bool(!b),
                    _ => ConstVal::Zero,
                }
            }
            s::ExprKind::Binary(binary_expr) => {
                let left_val = self.eval_const_expr(&binary_expr.left);
                let right_val = self.eval_const_expr(&binary_expr.right);
                match (binary_expr.op, left_val, right_val) {
                    (s::Op::Add, ConstVal::Int(l), ConstVal::Int(r)) => ConstVal::Int(l + r),
                    (s::Op::Add, ConstVal::Float(l), ConstVal::Float(r)) => ConstVal::Float(l + r),
                    (s::Op::Sub, ConstVal::Int(l), ConstVal::Int(r)) => ConstVal::Int(l - r),
                    (s::Op::Sub, ConstVal::Float(l), ConstVal::Float(r)) => ConstVal::Float(l - r),
                    (s::Op::Mul, ConstVal::Int(l), ConstVal::Int(r)) => ConstVal::Int(l * r),
                    (s::Op::Mul, ConstVal::Float(l), ConstVal::Float(r)) => ConstVal::Float(l * r),
                    (s::Op::Div, ConstVal::Int(l), ConstVal::Int(r)) => {
                        if r == 0 {
                            ConstVal::Zero
                        } else {
                            ConstVal::Int(l / r)
                        }
                    }
                    (s::Op::Div, ConstVal::Float(l), ConstVal::Float(r)) => ConstVal::Float(l / r),
                    (s::Op::Mod, ConstVal::Int(l), ConstVal::Int(r)) => {
                        if r == 0 {
                            ConstVal::Zero
                        } else {
                            ConstVal::Int(l % r)
                        }
                    }
                    _ => ConstVal::Zero,
                }
            }
            s::ExprKind::Null(_) => ConstVal::Zero,
            s::ExprKind::Cast(_, expr, _) => self.eval_const_expr(expr),
            s::ExprKind::AutoCast(expr, _) => self.eval_const_expr(expr),
            s::ExprKind::InitList(_) => ConstVal::Zero,
            _ => ConstVal::Zero,
        }
    }

    fn map_ast_type(&mut self, ast_ty: &s::Type) -> Type {
        let constants = &self.constants;
        let current_mod = self.current_module.clone();
        s::map_type_ext(
            ast_ty,
            self.builder.type_db,
            current_mod.as_str(),
            &|name| {
                for scope in constants.iter().rev() {
                    if let Some(val) = scope.get(name)
                        && let ConstVal::Int(v) = val
                    {
                        return Some(*v as usize);
                    }
                }
                None
            },
        )
    }

    fn run_defers(&mut self, up_to_depth: usize) {
        let current_depth = self.defers.len();
        if current_depth == 0 {
            return;
        }
        for depth in (up_to_depth..current_depth).rev() {
            let defers = self.defers[depth].clone();
            for stmt in defers.iter().rev() {
                self.translate_stmt(stmt);
            }
        }
    }

    fn insert_var(&mut self, name: String, ptr: Operand, ty: Type) {
        self.scopes.last_mut().unwrap().insert(name, (ptr, ty));
    }

    fn is_droppable_type(&self, ty: t::TypeID) -> bool {
        let canonical = self.builder.type_db.resolve(ty);
        if self.builder.type_db.destructor(canonical).is_some() {
            return true;
        }
        match self.builder.type_db.get_type(canonical).clone() {
            t::Type::Pointer(_) | t::Type::Slice(_) | t::Type::DynArray(_) | t::Type::String => {
                true
            }
            t::Type::Struct {
                fields: Some(fields),
                ..
            } => fields.iter().any(|f| self.is_droppable_type(f.ty)),
            t::Type::Enum { variants, .. } => variants
                .iter()
                .any(|v| v.payload.map_or(false, |p| self.is_droppable_type(p))),
            _ => false,
        }
    }

    fn drop_ownership_root(&self, expr: &s::Expr) -> Option<TokenSource> {
        match &expr.kind {
            s::ExprKind::Identifier(ident) => Some(ident.source.clone()),
            s::ExprKind::Member(member) => self.drop_ownership_root(&member.object),
            s::ExprKind::Index(index) => self.drop_ownership_root(&index.array),
            _ => None,
        }
    }

    fn owns_var(&self, name: &TokenSource) -> bool {
        self.owned_vars
            .iter()
            .rev()
            .any(|scope| scope.contains(name.as_str()))
    }

    fn mark_owned(&mut self, name: TokenSource) {
        self.owned_vars
            .last_mut()
            .unwrap()
            .insert(name.as_str().to_string());
    }

    fn consume_owned_var(&mut self, name: &TokenSource) -> bool {
        if let Some(scope) = self.owned_vars.last_mut() {
            if scope.remove(name.as_str()) {
                return true;
            }
        }
        // for scope in self.owned_vars.iter_mut().rev() {
        //     if scope.remove(name.as_str()) {
        //         return true;
        //     }
        // }
        false
    }

    fn consume_derived_drop(&mut self, name: &TokenSource) -> bool {
        if !self.owns_var(name) {
            return false;
        }
        self.dropped_derived
            .last_mut()
            .unwrap()
            .insert(name.as_str().to_string());
        true
    }

    fn is_derived_drop_target(expr: &s::Expr) -> bool {
        matches!(expr.kind, s::ExprKind::Member(_) | s::ExprKind::Index(_))
    }

    fn fresh_owned_initializer(&self, expr: &s::Expr) -> bool {
        matches!(
            expr.kind,
            s::ExprKind::New(..)
                | s::ExprKind::Make(..)
                | s::ExprKind::Default(..)
                | s::ExprKind::InitList(..)
        )
    }

    fn owned_return_initializer(&self, expr: &s::Expr) -> bool {
        let s::ExprKind::Call(call) = &expr.kind else {
            return false;
        };
        let function_name = match &call.callee.kind {
            s::ExprKind::Identifier(ident) => &ident.source,
            s::ExprKind::Member(member) => &member.property.source,
            _ => return false,
        };
        let module_name = call.module_name.as_ref().unwrap_or(&self.current_module);
        self.builder
            .type_db
            .try_modules_get(module_name)
            .and_then(|module| module.functions.get(function_name))
            .is_some_and(|signature| signature.has_owned_return)
    }

    fn ownership_transfer_root(&self, expr: &s::Expr) -> Option<TokenSource> {
        let s::ExprKind::InitList(literal) = &expr.kind else {
            return None;
        };
        literal
            .elements
            .iter()
            .enumerate()
            .find_map(|(idx, elem)| match elem {
                s::InitElement::Named { name, value } if name.source() == "data" => Some(value),
                s::InitElement::Positional(value) if idx == 0 => Some(value),
                _ => None,
            })
            .and_then(|val| self.drop_ownership_root(val))
            .filter(|root| self.owns_var(root))
    }

    fn lookup_var(&self, name: &str) -> Option<(Operand, Type)> {
        for scope in self.scopes.iter().rev() {
            if let Some((ptr, ty)) = scope.get(name) {
                return Some((ptr.clone(), *ty));
            }
        }

        for g in self.module.globals() {
            if g.name.as_str() == name {
                let ptr = if g.is_thread_local {
                    Operand::ThreadLocalGlobal(std::rc::Rc::from(g.name.as_str()))
                } else {
                    Operand::Global(std::rc::Rc::from(g.name.as_str()))
                };
                return Some((ptr, g.ty));
            }
        }

        None
    }

    fn is_lvalue(&self, expr: &s::Expr) -> bool {
        match &expr.kind {
            s::ExprKind::Identifier(ident) => self.lookup_var(ident.source()).is_some(),
            s::ExprKind::Unary(unary) => unary.op == s::Op::Deref,
            s::ExprKind::Member(_) => true,
            s::ExprKind::Index(_) => true,
            _ => false,
        }
    }

    fn get_slice_like_data_and_len(&mut self, val: Operand, ty: Type) -> (Operand, Operand) {
        let canonical = self.builder.type_db.resolve(ty);
        let usize_ty = self.builder.type_db.usize();
        let len_ptr_ty = self.builder.type_db.pointer(usize_ty);

        match self.builder.type_db.get_type(canonical).clone() {
            t::Type::Slice(elem) | t::Type::DynArray(elem) => {
                let temp_ptr = self.builder.build_alloca(canonical);
                self.builder.build_store(canonical, temp_ptr.clone(), val);

                let elem_ptr_ty = self.builder.type_db.pointer(elem);
                let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                let data_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(temp_ptr.clone(), 0), data_ptr_ty);
                let src_data = self.builder.build_load(elem_ptr_ty, Operand::Reg(data_ptr));

                let len_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(temp_ptr, 8), len_ptr_ty);
                let src_len = self.builder.build_load(usize_ty, Operand::Reg(len_ptr));

                (src_data, src_len)
            }
            t::Type::Array(elem, size) => {
                let temp_ptr = self.builder.build_alloca(canonical);
                self.builder.build_store(canonical, temp_ptr.clone(), val);

                let elem_ptr_ty = self.builder.type_db.pointer(elem);
                let first_elem_ptr = self.builder.build_inst(
                    Instruction::GetIndexPtr(temp_ptr, Operand::Int(0)),
                    elem_ptr_ty,
                );
                (Operand::Reg(first_elem_ptr), Operand::Int(size as u64))
            }
            _ => unreachable!(),
        }
    }

    fn lookup_function(&self, name: &str) -> Option<(String, Signature)> {
        if let Some(func) = self.module.functions().get(name) {
            return Some((name.to_string(), func.signature().clone()));
        }
        for (key, func) in self.module.functions() {
            if func.signature().name.as_str() == name || func.symbol_name() == name {
                return Some((key.clone(), func.signature().clone()));
            }
        }
        None
    }

    fn emit_bounds_check(&mut self, loc: Loc, idx_op: Operand, len_op: Operand) {
        let message_op = Operand::String(std::rc::Rc::from(
            format!("{}: Index out of bounds", loc).as_str(),
        ));
        self.builder.build_inst(
            Instruction::OOBCheck(message_op, idx_op, len_op),
            self.builder.type_db.void(),
        );
    }

    fn get_member_offset(&mut self, base_ty: Type, prop_name: &str) -> (u32, Type) {
        let canonical = self.builder.type_db.resolve(base_ty);
        match self.builder.type_db.get_type(canonical) {
            t::Type::Slice(elem) | t::Type::DynArray(elem) => {
                let elem = *elem;
                match prop_name {
                    "data" => (0, self.builder.type_db.pointer(elem)),
                    "len" => (8, self.builder.type_db.usize()),
                    "cap"
                        if matches!(
                            self.builder.type_db.get_type(canonical),
                            t::Type::DynArray(..)
                        ) =>
                    {
                        (16, self.builder.type_db.usize())
                    }
                    _ => {
                        panic!("Unknown slice/dynamic array property: {}", prop_name);
                    }
                }
            }
            t::Type::Struct {
                fields: Some(fields),
                ..
            } => {
                let layout = StructLayout::compute(canonical, self.builder.type_db);
                let idx = fields.iter().position(|f| f.name == prop_name).unwrap();
                (layout.fields[idx].offset, fields[idx].ty)
            }
            t::Type::Tuple(elements) => {
                let idx: usize = prop_name.parse().unwrap();
                let layout = StructLayout::compute(canonical, self.builder.type_db);
                (layout.fields[idx].offset, elements[idx])
            }
            t::Type::Any(_) => self.get_member_offset(canonical, prop_name),
            t::Type::String => match prop_name {
                "data" => (0, self.builder.type_db.pointer(self.builder.type_db.u8())),
                "len" => (8, self.builder.type_db.usize()),
                _ => {
                    panic!("Unknown string property: {}", prop_name);
                }
            },
            t::Type::Distinct { base, .. } => self.get_member_offset(*base, prop_name),
            _ => panic!(
                "Expected structural type, got {:?}",
                self.builder.type_db.get_type(canonical)
            ),
        }
    }

    fn translate_default_value(&mut self, ty: Type) -> (Operand, Type) {
        let canonical = self.builder.type_db.resolve(ty);
        match self.builder.type_db.get_type(canonical).clone() {
            t::Type::Integer(..)
            | t::Type::Bool
            | t::Type::Float(..)
            | t::Type::UntypedFloat
            | t::Type::Void
            | t::Type::NoReturn
            | t::Type::UntypedInt => {
                if self.builder.type_db.is_float(canonical)
                    || canonical == self.builder.type_db.untyped_float()
                {
                    (Operand::Float(0.0), ty)
                } else if canonical == self.builder.type_db.bool() {
                    (Operand::Bool(false), ty)
                } else {
                    (Operand::Int(0), ty)
                }
            }
            t::Type::Pointer(_) | t::Type::FnPointer { .. } => (Operand::Int(0), ty),
            t::Type::String => {
                let ptr = self.builder.build_alloca(ty);
                let (data_offset, data_ty) = self.get_member_offset(ty, "data");
                let (len_offset, len_ty) = self.get_member_offset(ty, "len");

                let data_ptr_ty = self.builder.type_db.pointer(data_ty);
                let data_ptr = self.builder.build_inst(
                    Instruction::GetMemberPtr(ptr.clone(), data_offset),
                    data_ptr_ty,
                );
                self.builder
                    .build_store(data_ty, Operand::Reg(data_ptr), Operand::Int(0));

                let len_ptr_ty = self.builder.type_db.pointer(len_ty);
                let len_ptr = self.builder.build_inst(
                    Instruction::GetMemberPtr(ptr.clone(), len_offset),
                    len_ptr_ty,
                );
                self.builder
                    .build_store(len_ty, Operand::Reg(len_ptr), Operand::Int(0));

                (self.builder.build_load(ty, ptr), ty)
            }
            t::Type::Struct {
                fields: Some(fields),
                ..
            } => {
                let ptr = self.builder.build_alloca(ty);
                for f in &fields {
                    let (val, _) = self.translate_default_value(f.ty);
                    let (offset, _) = self.get_member_offset(ty, &f.name);
                    let field_ptr_ty = self.builder.type_db.pointer(f.ty);
                    let field_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr.clone(), offset), field_ptr_ty);
                    self.builder.build_store(f.ty, Operand::Reg(field_ptr), val);
                }
                (self.builder.build_load(ty, ptr), ty)
            }
            t::Type::Array(elem_ty, size) => {
                let ptr = self.builder.build_alloca(ty);
                let elem_size = crate::type_layout(elem_ty, self.builder.type_db).0;
                for i in 0..size {
                    let (val, _) = self.translate_default_value(elem_ty);
                    let offset = i as u32 * elem_size;
                    let elem_ptr_ty = self.builder.type_db.pointer(elem_ty);
                    let elem_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr.clone(), offset), elem_ptr_ty);
                    self.builder
                        .build_store(elem_ty, Operand::Reg(elem_ptr), val);
                }
                (self.builder.build_load(ty, ptr), ty)
            }
            t::Type::Slice(_elem_ty) | t::Type::DynArray(_elem_ty) => {
                let ptr = self.builder.build_alloca(ty);
                let (data_offset, data_ty) = self.get_member_offset(ty, "data");
                let (len_offset, len_ty) = self.get_member_offset(ty, "len");

                let data_ptr_ty = self.builder.type_db.pointer(data_ty);
                let data_ptr = self.builder.build_inst(
                    Instruction::GetMemberPtr(ptr.clone(), data_offset),
                    data_ptr_ty,
                );
                self.builder
                    .build_store(data_ty, Operand::Reg(data_ptr), Operand::Int(0));

                let len_ptr_ty = self.builder.type_db.pointer(len_ty);
                let len_ptr = self.builder.build_inst(
                    Instruction::GetMemberPtr(ptr.clone(), len_offset),
                    len_ptr_ty,
                );
                self.builder
                    .build_store(len_ty, Operand::Reg(len_ptr), Operand::Int(0));

                if matches!(
                    self.builder.type_db.get_type(canonical),
                    t::Type::DynArray(..)
                ) {
                    let (cap_offset, cap_ty) = self.get_member_offset(ty, "cap");
                    let cap_ptr_ty = self.builder.type_db.pointer(cap_ty);
                    let cap_ptr = self.builder.build_inst(
                        Instruction::GetMemberPtr(ptr.clone(), cap_offset),
                        cap_ptr_ty,
                    );
                    self.builder
                        .build_store(cap_ty, Operand::Reg(cap_ptr), Operand::Int(0));
                }

                (self.builder.build_load(ty, ptr), ty)
            }
            t::Type::Tuple(elements) => {
                let ptr = self.builder.build_alloca(ty);
                for (i, elem_ty) in elements.iter().enumerate() {
                    let (val, _) = self.translate_default_value(*elem_ty);
                    let (offset, _) = self.get_member_offset(ty, &i.to_string());
                    let field_ptr_ty = self.builder.type_db.pointer(*elem_ty);
                    let field_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr.clone(), offset), field_ptr_ty);
                    self.builder
                        .build_store(*elem_ty, Operand::Reg(field_ptr), val);
                }
                (self.builder.build_load(ty, ptr), ty)
            }
            t::Type::Distinct { base, .. } => {
                let (val, _) = self.translate_default_value(base);
                (val, ty)
            }
            _ => (Operand::Int(0), ty),
        }
    }

    fn translate_lvalue(&mut self, expr: &s::Expr) -> (Operand, Type) {
        match &expr.kind {
            s::ExprKind::Identifier(ident) => {
                let name = ident.source();
                if let Some((ptr, ty)) = self.lookup_var(name) {
                    (ptr, ty)
                }
                // else if let Some(ty) = self.builder.type_db.lookup_by_name(name) {
                //     (Operand::Int(0), ty)
                // }
                else {
                    self.reporter
                        .report(ident.loc, format!("Undefined variable '{}'", name));
                    (Operand::Int(0), self.builder.type_db.void())
                }
            }
            s::ExprKind::Unary(unary) if unary.op == s::Op::Deref => {
                let (ptr, ptr_ty) = self.translate_expr(&unary.right);
                let canonical = self.builder.type_db.resolve(ptr_ty);
                let inner_ty = match self.builder.type_db.get_type(canonical) {
                    t::Type::Pointer(inner) if *inner == self.builder.type_db.void() => {
                        self.reporter
                            .report(expr.loc(), "Cannot read from void pointer");
                        self.builder.type_db.void()
                    }
                    t::Type::Pointer(inner) => *inner,
                    _ => {
                        self.reporter
                            .report(expr.loc(), "Expected pointer type for dereference");
                        self.builder.type_db.void()
                    }
                };
                (ptr, inner_ty)
            }
            s::ExprKind::Member(mem) => {
                let obj_ty = mem
                    .object
                    .resolved_type
                    .expect("object type must be resolved");
                let canonical_obj = self.builder.type_db.resolve(obj_ty);
                let prop_name = mem.property.source();
                if let t::Type::Enum { repr, variants, .. } =
                    self.builder.type_db.get_type(canonical_obj).clone()
                {
                    if let Some(v) = variants.iter().find(|f| f.name == prop_name) {
                        return (Operand::Int(v.default_value), repr);
                    } else {
                        self.reporter.report(
                            mem.property.loc,
                            format!("variant '{}' not found in enum", prop_name),
                        );
                        return (Operand::Int(0), self.builder.type_db.void());
                    }
                }

                let obj_resolved = mem
                    .object
                    .resolved_type
                    .map(|t| self.builder.type_db.resolve(t));
                let is_module = obj_resolved == Some(self.builder.type_db.module());
                if is_module {
                    if let s::ExprKind::Identifier(ident) = &mem.object.kind {
                        let full_name = format!("{}.{}", ident.source(), mem.property.source());
                        if let Some((ptr, ty)) = self.lookup_var(&full_name) {
                            return (ptr, ty);
                        }
                    }
                    if let Some((ptr, ty)) = self.lookup_var(mem.property.source()) {
                        return (ptr, ty);
                    }
                }
                let is_lval = !is_module
                    && matches!(
                        &mem.object.kind,
                        s::ExprKind::Identifier(_)
                            | s::ExprKind::Unary(s::UnaryExpr {
                                op: s::Op::Deref,
                                ..
                            })
                            | s::ExprKind::Member(_)
                            | s::ExprKind::Index(_)
                    );
                let (mut obj_ptr, mut obj_ty) = if is_lval {
                    self.translate_lvalue(&mem.object)
                } else {
                    let (val, ty) = self.translate_expr(&mem.object);
                    let canonical = self.builder.type_db.resolve(ty);
                    if let t::Type::Pointer(inner) = self.builder.type_db.get_type(canonical) {
                        (val, *inner)
                    } else {
                        let ptr = self.builder.build_alloca(ty);
                        self.builder.build_store(ty, ptr.clone(), val);
                        (ptr, ty)
                    }
                };
                let prop_name = mem.property.source();

                let mut canonical = self.builder.type_db.resolve(obj_ty);
                while let t::Type::Pointer(inner) = self.builder.type_db.get_type(canonical).clone()
                {
                    obj_ptr = self.builder.build_load(obj_ty, obj_ptr);
                    obj_ty = inner;
                    canonical = self.builder.type_db.resolve(obj_ty);
                }

                if let t::Type::Enum { repr, variants, .. } =
                    self.builder.type_db.get_type(canonical).clone()
                {
                    if let Some(v) = variants.iter().find(|f| f.name == prop_name) {
                        (Operand::Int(v.default_value), repr)
                    } else {
                        self.reporter.report(
                            mem.property.loc,
                            format!("variant '{}' not found", prop_name),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                } else if let t::Type::Array(elem_ty, sz) =
                    self.builder.type_db.get_type(canonical).clone()
                {
                    match prop_name {
                        "data" => {
                            let ptr_ty = self.builder.type_db.pointer(elem_ty);
                            let first_elem_ptr = self.builder.build_inst(
                                Instruction::GetIndexPtr(obj_ptr, Operand::Int(0)),
                                ptr_ty,
                            );
                            let temp_ptr = self.builder.build_alloca(ptr_ty);
                            self.builder.build_store(
                                ptr_ty,
                                temp_ptr.clone(),
                                Operand::Reg(first_elem_ptr),
                            );
                            (temp_ptr, ptr_ty)
                        }
                        "len" => {
                            let usz_ty = self.builder.type_db.usize();
                            (Operand::Int(sz as _), usz_ty)
                        }
                        _ => {
                            self.reporter.report(
                                mem.property.loc,
                                format!("Array field '{}' not found", prop_name),
                            );
                            (Operand::Int(0), self.builder.type_db.void())
                        }
                    }
                } else {
                    let (offset, field_ty) = self.get_member_offset(obj_ty, prop_name);
                    let ptr_ty = self.builder.type_db.pointer(field_ty);
                    let field_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(obj_ptr, offset), ptr_ty);
                    (Operand::Reg(field_ptr), field_ty)
                }
            }
            s::ExprKind::Index(idx) => {
                let loc = expr.loc();
                let (arr_ptr, arr_ty) = self.translate_lvalue(&idx.array);
                let (idx_op, _) = self.translate_expr(&idx.index);

                let canonical = self.builder.type_db.resolve(arr_ty);
                match self.builder.type_db.get_type(canonical) {
                    t::Type::Pointer(elem_ty) => {
                        let elem_ty = *elem_ty;
                        let ptr_ty = self.builder.type_db.pointer(elem_ty);
                        let loaded_ptr = self.builder.build_load(arr_ty, arr_ptr);
                        let elem_ptr = self
                            .builder
                            .build_inst(Instruction::GetIndexPtr(loaded_ptr, idx_op), ptr_ty);
                        (Operand::Reg(elem_ptr), elem_ty)
                    }
                    t::Type::Array(elem_ty, size) => {
                        let elem_ty = *elem_ty;
                        let size = *size;
                        self.emit_bounds_check(loc, idx_op.clone(), Operand::Int(size as u64));
                        let ptr_ty = self.builder.type_db.pointer(elem_ty);
                        let elem_ptr = self
                            .builder
                            .build_inst(Instruction::GetIndexPtr(arr_ptr, idx_op), ptr_ty);
                        (Operand::Reg(elem_ptr), elem_ty)
                    }
                    t::Type::Slice(elem_ty) | t::Type::DynArray(elem_ty) => {
                        let elem_ty = *elem_ty;
                        let ptr_ty = self.builder.type_db.pointer(elem_ty);

                        // Load slice length (at offset 8)
                        let usize_ty = self.builder.type_db.usize();
                        let len_ptr_ty = self.builder.type_db.pointer(usize_ty);
                        let len_field_ptr = self
                            .builder
                            .build_inst(Instruction::GetMemberPtr(arr_ptr.clone(), 8), len_ptr_ty);
                        let len_op = self
                            .builder
                            .build_load(usize_ty, Operand::Reg(len_field_ptr));

                        self.emit_bounds_check(loc, idx_op.clone(), len_op);

                        let data_ptr_ty = self.builder.type_db.pointer(ptr_ty);
                        let data_field_ptr = self
                            .builder
                            .build_inst(Instruction::GetMemberPtr(arr_ptr, 0), data_ptr_ty);
                        let data_ptr = self
                            .builder
                            .build_load(ptr_ty, Operand::Reg(data_field_ptr));
                        let elem_ptr = self
                            .builder
                            .build_inst(Instruction::GetIndexPtr(data_ptr, idx_op), ptr_ty);
                        (Operand::Reg(elem_ptr), elem_ty)
                    }
                    _ => {
                        self.reporter
                            .report(idx.loc, "Cannot index into non-pointer/non-array type");
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                }
            }
            _ => {
                self.reporter
                    .report(expr.loc(), "Expression is not an LValue");
                (Operand::Int(0), self.builder.type_db.void())
            }
        }
    }

    fn is_block_terminated(&self) -> bool {
        if let Some(block_id) = self.builder.current_block {
            let block = &self.builder.blocks[block_id.0 as usize];
            if let Some(&last_inst_id) = block.instructions.last() {
                let inst = &self.builder.instructions[last_inst_id.0 as usize].inst;
                return matches!(
                    inst,
                    Instruction::Br(_) | Instruction::CondBr(_, _, _) | Instruction::Return(_)
                );
            }
            false
        } else {
            true // No current block means we are terminated/unreachable!
        }
    }

    fn translate_block(&mut self, block: &s::BlockStmt) {
        self.push_scope();
        for stmt in &block.stmts {
            if self.is_block_terminated() {
                break;
            }
            self.translate_stmt(stmt);
        }
        if !self.is_block_terminated() {
            let current_depth = self.defers.len();
            if current_depth > 0 {
                self.run_defers(current_depth - 1);
            }
        }
        self.pop_scope();
    }

    fn translate_stmt(&mut self, stmt: &s::Stmt) {
        match stmt {
            s::Stmt::ExprStmt(expr) => {
                self.translate_expr(expr);
            }
            s::Stmt::VarDecl(decl) => {
                let (val, expr_ty) = self.translate_expr(&decl.expr);

                let declared_ty = decl.var_type.as_ref().map(|t| self.map_ast_type(t));

                let final_ty = if let Some(decl_ty) = declared_ty {
                    decl_ty
                } else {
                    expr_ty
                };

                if decl.names.len() == 1 {
                    let ptr = self.builder.build_alloca(final_ty);
                    self.builder.build_store(final_ty, ptr.clone(), val);
                    self.insert_var(decl.names[0].source().to_string(), ptr, final_ty);

                    let transfer_root = self.ownership_transfer_root(&decl.expr);
                    if self.is_droppable_type(final_ty)
                        && (self.fresh_owned_initializer(&decl.expr)
                            || self.owned_return_initializer(&decl.expr)
                            || transfer_root.is_some())
                    {
                        if let Some(root) = transfer_root {
                            self.consume_owned_var(&root);
                        }
                        self.mark_owned(decl.names[0].source.clone());
                    }
                } else {
                    let tuple_ptr = self.builder.build_alloca(final_ty);
                    self.builder.build_store(final_ty, tuple_ptr.clone(), val);

                    let canonical_final = self.builder.type_db.resolve(final_ty);
                    if let t::Type::Tuple(elements) =
                        self.builder.type_db.get_type(canonical_final).clone()
                    {
                        for (i, name) in decl.names.iter().enumerate() {
                            let elem_ty = elements[i];
                            let ptr_ty = self.builder.type_db.pointer(elem_ty);
                            let (offset, _) = self.get_member_offset(final_ty, &i.to_string());
                            let member_ptr = self.builder.build_inst(
                                Instruction::GetMemberPtr(tuple_ptr.clone(), offset),
                                ptr_ty,
                            );
                            let elem_val =
                                self.builder.build_load(elem_ty, Operand::Reg(member_ptr));
                            let var_ptr = self.builder.build_alloca(elem_ty);
                            self.builder.build_store(elem_ty, var_ptr.clone(), elem_val);
                            self.insert_var(name.source().to_string(), var_ptr, elem_ty);
                            if self.is_droppable_type(elem_ty)
                                && self.owned_return_initializer(&decl.expr)
                            {
                                self.mark_owned(name.source.clone());
                            }
                        }
                    } else {
                        panic!("Expected tuple type for destructuring");
                    }
                }
            }
            s::Stmt::Const(decl) => {
                let val = self.eval_const_expr(&decl.expr);
                self.constants
                    .last_mut()
                    .unwrap()
                    .insert(decl.name.source().to_string(), val);
            }
            s::Stmt::Return(_, expr_opt) => {
                let (val, ty) = if let Some(expr) = expr_opt {
                    self.translate_expr(expr)
                } else {
                    (Operand::Int(0), self.builder.type_db.void())
                };

                self.run_defers(0);

                let is_void = ty == self.builder.type_db.void();
                self.builder
                    .build_return(if is_void { None } else { Some(val) });
            }
            s::Stmt::Block(block) => {
                self.translate_block(block);
            }
            s::Stmt::IfStmt(s::IfStmt::If { cond, true_body }) => {
                let (cond_val, _) = self.translate_expr(cond);

                let true_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder
                    .build_cond_br(cond_val, true_block, merge_block);

                self.builder.set_block(true_block);
                self.translate_block(true_body);
                if !self.is_block_terminated() {
                    self.builder.build_br(merge_block);
                }

                self.builder.set_block(merge_block);
            }
            s::Stmt::IfStmt(s::IfStmt::IfElse {
                cond,
                true_body,
                false_body,
            }) => {
                let (cond_val, _) = self.translate_expr(cond);

                let true_block = self.builder.create_block();
                let false_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder
                    .build_cond_br(cond_val, true_block, false_block);

                self.builder.set_block(true_block);
                self.translate_block(true_body);
                let true_terminated = self.is_block_terminated();
                if !true_terminated {
                    self.builder.build_br(merge_block);
                }

                self.builder.set_block(false_block);
                self.translate_block(false_body);
                let false_terminated = self.is_block_terminated();
                if !false_terminated {
                    self.builder.build_br(merge_block);
                }

                if true_terminated && false_terminated {
                    self.builder.current_block = None;
                } else {
                    self.builder.set_block(merge_block);
                }
            }
            s::Stmt::ForStmt(s::ForStmt::ForCond { cond, body }) => {
                let cond_block = self.builder.create_block();
                let body_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder.build_br(cond_block);

                self.builder.set_block(cond_block);
                let (cond_val, _) = self.translate_expr(cond);
                self.builder
                    .build_cond_br(cond_val, body_block, merge_block);

                self.builder.set_block(body_block);
                self.loop_targets.push(LoopTarget {
                    break_block: merge_block,
                    continue_block: cond_block,
                    scope_depth: self.scopes.len(),
                });
                self.translate_block(body);
                self.loop_targets.pop();

                if !self.is_block_terminated() {
                    self.builder.build_br(cond_block);
                }

                self.builder.set_block(merge_block);
            }
            s::Stmt::ForStmt(s::ForStmt::ForLoop(body)) => {
                let body_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder.build_br(body_block);

                self.builder.set_block(body_block);
                self.loop_targets.push(LoopTarget {
                    break_block: merge_block,
                    continue_block: body_block,
                    scope_depth: self.scopes.len(),
                });
                self.translate_block(body);
                self.loop_targets.pop();

                if !self.is_block_terminated() {
                    self.builder.build_br(body_block);
                }

                self.builder.set_block(merge_block);
            }
            s::Stmt::Break(loc) => {
                if let Some(target) = self.loop_targets.last().cloned() {
                    self.run_defers(target.scope_depth);
                    self.builder.build_br(target.break_block);
                } else {
                    self.reporter
                        .report(*loc, "break statement outside of loop");
                }
            }
            s::Stmt::Continue(loc) => {
                if let Some(target) = self.loop_targets.last().cloned() {
                    self.run_defers(target.scope_depth);
                    self.builder.build_br(target.continue_block);
                } else {
                    self.reporter
                        .report(*loc, "continue statement outside of loop");
                }
            }
            s::Stmt::ForEach(fe) => {
                let (iter_val, iter_ty) = self.translate_expr(&fe.iter_expr);
                let iter_canon = self.builder.type_db.resolve(iter_ty);
                let (elem_ty, len_val, data_val) = if iter_canon == self.builder.type_db.string() {
                    let ptr = self.builder.build_alloca(iter_canon);
                    self.builder.build_store(iter_canon, ptr.clone(), iter_val);

                    let len_ptr_ty = self.builder.type_db.pointer(self.builder.type_db.int());
                    let len_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr.clone(), 8), len_ptr_ty);
                    let len_val = self
                        .builder
                        .build_load(self.builder.type_db.int(), Operand::Reg(len_ptr));

                    let u8_ty = self.builder.type_db.u8();
                    let elem_ptr_ty = self.builder.type_db.pointer(u8_ty);
                    let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                    let data_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr, 0), data_ptr_ty);
                    let data_val = self.builder.build_load(elem_ptr_ty, Operand::Reg(data_ptr));
                    (u8_ty, len_val, data_val)
                } else {
                    match self.builder.type_db.get_type(iter_canon).clone() {
                        t::Type::Array(elem, size) => {
                            let ptr = self.builder.build_alloca(iter_canon);
                            self.builder.build_store(iter_canon, ptr.clone(), iter_val);
                            let len_val = Operand::Int(size as u64);
                            let data_val = ptr;
                            (elem, len_val, data_val)
                        }
                        t::Type::Slice(elem) | t::Type::DynArray(elem) => {
                            let ptr = self.builder.build_alloca(iter_canon);
                            self.builder.build_store(iter_canon, ptr.clone(), iter_val);

                            let len_ptr_ty =
                                self.builder.type_db.pointer(self.builder.type_db.int());
                            let len_ptr = self
                                .builder
                                .build_inst(Instruction::GetMemberPtr(ptr.clone(), 8), len_ptr_ty);
                            let len_val = self
                                .builder
                                .build_load(self.builder.type_db.int(), Operand::Reg(len_ptr));

                            let elem_ptr_ty = self.builder.type_db.pointer(elem);
                            let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                            let data_ptr = self
                                .builder
                                .build_inst(Instruction::GetMemberPtr(ptr, 0), data_ptr_ty);
                            let data_val =
                                self.builder.build_load(elem_ptr_ty, Operand::Reg(data_ptr));
                            (elem, len_val, data_val)
                        }
                        t::Type::Pointer(inner) => {
                            let inner_canon = self.builder.type_db.resolve(inner);
                            if inner_canon == self.builder.type_db.string() {
                                let ptr = iter_val;
                                let len_ptr_ty =
                                    self.builder.type_db.pointer(self.builder.type_db.int());
                                let len_ptr = self.builder.build_inst(
                                    Instruction::GetMemberPtr(ptr.clone(), 8),
                                    len_ptr_ty,
                                );
                                let len_val = self
                                    .builder
                                    .build_load(self.builder.type_db.int(), Operand::Reg(len_ptr));

                                let u8_ty = self.builder.type_db.u8();
                                let elem_ptr_ty = self.builder.type_db.pointer(u8_ty);
                                let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                                let data_ptr = self
                                    .builder
                                    .build_inst(Instruction::GetMemberPtr(ptr, 0), data_ptr_ty);
                                let data_val =
                                    self.builder.build_load(elem_ptr_ty, Operand::Reg(data_ptr));
                                (u8_ty, len_val, data_val)
                            } else {
                                match self.builder.type_db.get_type(inner_canon).clone() {
                                    t::Type::Array(elem, size) => {
                                        let len_val = Operand::Int(size as u64);
                                        let data_val = iter_val;
                                        (elem, len_val, data_val)
                                    }
                                    t::Type::Slice(elem) | t::Type::DynArray(elem) => {
                                        let ptr = iter_val;
                                        let len_ptr_ty = self
                                            .builder
                                            .type_db
                                            .pointer(self.builder.type_db.int());
                                        let len_ptr = self.builder.build_inst(
                                            Instruction::GetMemberPtr(ptr.clone(), 8),
                                            len_ptr_ty,
                                        );
                                        let len_val = self.builder.build_load(
                                            self.builder.type_db.int(),
                                            Operand::Reg(len_ptr),
                                        );

                                        let elem_ptr_ty = self.builder.type_db.pointer(elem);
                                        let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                                        let data_ptr = self.builder.build_inst(
                                            Instruction::GetMemberPtr(ptr, 0),
                                            data_ptr_ty,
                                        );
                                        let data_val = self
                                            .builder
                                            .build_load(elem_ptr_ty, Operand::Reg(data_ptr));
                                        (elem, len_val, data_val)
                                    }
                                    _ => panic!(
                                        "Expected array, slice or pointer to array/slice for foreach iteration"
                                    ),
                                }
                            }
                        }
                        _ => panic!(
                            "Expected array, slice or pointer to array/slice for foreach iteration"
                        ),
                    }
                };

                let int_ty = self.builder.type_db.int();
                let idx_ptr = self.builder.build_alloca(int_ty);
                self.builder
                    .build_store(int_ty, idx_ptr.clone(), Operand::Int(0));

                let cond_block = self.builder.create_block();
                let body_block = self.builder.create_block();
                let inc_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder.build_br(cond_block);

                // Condition block: idx < len
                self.builder.set_block(cond_block);
                let idx_val = self.builder.build_load(int_ty, idx_ptr.clone());
                let cmp_val = self.builder.build_inst(
                    Instruction::Lt(int_ty, idx_val.clone(), len_val),
                    self.builder.type_db.bool(),
                );
                self.builder
                    .build_cond_br(Operand::Reg(cmp_val), body_block, merge_block);

                // Body block
                self.builder.set_block(body_block);

                self.push_scope();

                // Get element: data_val[idx_val]
                let elem_ptr_ty = self.builder.type_db.pointer(elem_ty);
                let elem_ptr = self
                    .builder
                    .build_inst(Instruction::GetIndexPtr(data_val, idx_val), elem_ptr_ty);
                let elem_val = self.builder.build_load(elem_ty, Operand::Reg(elem_ptr));

                let var_ptr = self.builder.build_alloca(elem_ty);
                self.builder.build_store(elem_ty, var_ptr.clone(), elem_val);

                self.insert_var(fe.var_name.source().to_string(), var_ptr, elem_ty);

                self.loop_targets.push(LoopTarget {
                    break_block: merge_block,
                    continue_block: inc_block,
                    scope_depth: self.scopes.len(),
                });

                self.translate_block(&fe.body);

                self.loop_targets.pop();
                self.pop_scope();

                if !self.is_block_terminated() {
                    self.builder.build_br(inc_block);
                }

                // Increment block: idx = idx + 1
                self.builder.set_block(inc_block);
                let current_idx = self.builder.build_load(int_ty, idx_ptr.clone());
                let next_idx = self
                    .builder
                    .build_inst(Instruction::Add(current_idx, Operand::Int(1)), int_ty);
                self.builder
                    .build_store(int_ty, idx_ptr.clone(), Operand::Reg(next_idx));
                self.builder.build_br(cond_block);

                self.builder.set_block(merge_block);
            }
            s::Stmt::Defer(_, inner_stmt) => {
                self.defers.last_mut().unwrap().push(*inner_stmt.clone());
            }
            s::Stmt::Switch(switch_stmt) => {
                let merge_block = self.builder.create_block();
                let mut all_branches_terminate = true;

                let mut check_blocks = Vec::new();
                for _ in 0..switch_stmt.branches.len() {
                    check_blocks.push(self.builder.create_block());
                }

                let default_block = if switch_stmt.default.is_some() {
                    Some(self.builder.create_block())
                } else {
                    None
                };

                let (cond_val, cond_ty) = self.translate_expr(&switch_stmt.cond);

                let first_dest = if !check_blocks.is_empty() {
                    check_blocks[0]
                } else if let Some(def_b) = default_block {
                    def_b
                } else {
                    merge_block
                };
                self.builder.build_br(first_dest);

                for (i, branch) in switch_stmt.branches.iter().enumerate() {
                    self.builder.set_block(check_blocks[i]);

                    let bin = match &branch.pattern.kind {
                        s::ExprKind::Binary(bin) => bin,
                        _ => unreachable!(),
                    };
                    let (right, _right_ty) = self.translate_expr(&bin.right);

                    let cmp_val = if let Some(callee_name) = &bin.use_operator_overload {
                        let mut args = Vec::new();
                        args.push(cond_val.clone());
                        args.push(right);

                        if let Some(func) = self.module.functions().get(callee_name.as_str()) {
                            let ret_ty = func.signature().return_type;
                            let callee = Operand::String(std::rc::Rc::from(callee_name.as_str()));
                            Operand::Reg(self.builder.build_inst(
                                Instruction::Call(callee, args.into_boxed_slice()),
                                ret_ty,
                            ))
                        } else {
                            self.reporter.report(
                                bin.op_loc(),
                                format!("Call to undefined function '{}'", callee_name),
                            );
                            Operand::Int(0)
                        }
                    } else {
                        let inst = match bin.op {
                            s::Op::Eq => Instruction::Eq(cond_ty, cond_val.clone(), right),
                            s::Op::NotEq => Instruction::NotEq(cond_ty, cond_val.clone(), right),
                            _ => unreachable!(),
                        };
                        Operand::Reg(self.builder.build_inst(inst, self.builder.type_db.bool()))
                    };

                    let body_block = self.builder.create_block();
                    let false_dest = if i + 1 < check_blocks.len() {
                        check_blocks[i + 1]
                    } else if let Some(def_b) = default_block {
                        def_b
                    } else {
                        merge_block
                    };

                    self.builder.build_cond_br(cmp_val, body_block, false_dest);

                    self.builder.set_block(body_block);
                    self.translate_stmt(&branch.body);

                    if !self.is_block_terminated() {
                        all_branches_terminate = false;
                        self.builder.build_br(merge_block);
                    }
                }

                if let Some(ref default_stmt) = switch_stmt.default {
                    let def_b = default_block.unwrap();
                    self.builder.set_block(def_b);
                    self.translate_stmt(default_stmt);
                    if !self.is_block_terminated() {
                        all_branches_terminate = false;
                        self.builder.build_br(merge_block);
                    }
                } else {
                    all_branches_terminate = false;
                }

                if all_branches_terminate {
                    self.builder.current_block = None;
                } else {
                    self.builder.set_block(merge_block);
                }
            }
            s::Stmt::Call(call) => {
                let dummy_expr = s::Expr::new(s::ExprKind::Call(call.clone()));
                self.translate_expr(&dummy_expr);
            }
        }
    }

    fn translate_expr(&mut self, expr: &s::Expr) -> (Operand, Type) {
        let (val, ty) = match &expr.kind {
            s::ExprKind::Null(_) => (Operand::Null, self.builder.type_db.rawptr()),
            s::ExprKind::Integer(lit) => {
                let resolved = if let Some(r_ty) = expr.resolved_type {
                    let canon = self.builder.type_db.resolve(r_ty);
                    if canon == self.builder.type_db.untyped_int() {
                        self.builder.type_db.int()
                    } else {
                        canon
                    }
                } else {
                    self.builder.type_db.int()
                };
                (Operand::Int(lit.value), resolved)
            }
            s::ExprKind::Char(lit) => {
                let resolved = if let Some(r_ty) = expr.resolved_type {
                    let canon = self.builder.type_db.resolve(r_ty);
                    if canon == self.builder.type_db.untyped_int() {
                        self.builder.type_db.int()
                    } else {
                        canon
                    }
                } else {
                    self.builder.type_db.int()
                };
                (Operand::Int(lit.value as u64), resolved)
            }
            s::ExprKind::Bool(lit) => (Operand::Bool(lit.value), self.builder.type_db.bool()),
            s::ExprKind::Float(lit) => {
                let resolved = if let Some(t) = expr.resolved_type {
                    let canon = self.builder.type_db.resolve(t);
                    if canon == self.builder.type_db.untyped_float() {
                        self.builder.type_db.float()
                    } else {
                        canon
                    }
                } else {
                    self.builder.type_db.float()
                };
                (Operand::Float(lit.value), resolved)
            }
            s::ExprKind::StringLiteral(lit) => {
                let s = lit.unescape();
                (
                    Operand::String(std::rc::Rc::from(s)),
                    self.builder.type_db.string(),
                )
            }
            s::ExprKind::Identifier(ident) => {
                let name = &ident.source;

                if let Some(const_val) = self.lookup_const(name) {
                    let operand = match const_val {
                        ConstVal::Int(v) => Operand::Int(*v as u64),
                        ConstVal::Float(v) => Operand::Float(*v),
                        ConstVal::Bool(b) => Operand::Bool(*b),
                        ConstVal::Zero => Operand::Int(0),
                    };
                    let ty = expr.resolved_type.unwrap_or_else(|| match const_val {
                        ConstVal::Int(_) => self.builder.type_db.int(),
                        ConstVal::Float(_) => self.builder.type_db.float(),
                        ConstVal::Bool(_) => self.builder.type_db.bool(),
                        ConstVal::Zero => self.builder.type_db.void(),
                    });
                    (operand, ty)
                } else if let Some((ptr, ty)) = self.lookup_var(name) {
                    (self.builder.build_load(ty, ptr), ty)
                } else if let Some((fn_key, sig)) = self.lookup_function(name) {
                    let fn_pointer_ty =
                        self.builder.type_db.fn_pointer(sig.params, sig.return_type);
                    (
                        Operand::Global(std::rc::Rc::from(fn_key.as_str())),
                        fn_pointer_ty,
                    )
                } else {
                    self.reporter
                        .report(ident.loc, format!("Undefined variable '{}'", name));
                    (Operand::Int(0), self.builder.type_db.void())
                }
            }
            s::ExprKind::Member(_)
            | s::ExprKind::Index(_)
            | s::ExprKind::Unary(s::UnaryExpr {
                op: s::Op::Deref, ..
            }) => {
                if let s::ExprKind::Member(mem) = &expr.kind {
                    if let s::ExprKind::Identifier(ident) = &mem.object.kind {
                        let full_name = format!("{}.{}", ident.source(), mem.property.source());
                        if let Some(const_val) = self.lookup_const(&full_name) {
                            let operand = match const_val {
                                ConstVal::Int(v) => Operand::Int(*v as u64),
                                ConstVal::Float(v) => Operand::Float(*v),
                                ConstVal::Bool(b) => Operand::Bool(*b),
                                ConstVal::Zero => Operand::Int(0),
                            };
                            let ty = expr.resolved_type.unwrap_or_else(|| match const_val {
                                ConstVal::Int(_) => self.builder.type_db.int(),
                                ConstVal::Float(_) => self.builder.type_db.float(),
                                ConstVal::Bool(_) => self.builder.type_db.bool(),
                                ConstVal::Zero => self.builder.type_db.void(),
                            });
                            return (operand, ty);
                        }
                    }
                    if let Some(obj_ty) = mem.object.resolved_type {
                        let mut canonical = self.builder.type_db.resolve(obj_ty);
                        while let t::Type::Pointer(inner) =
                            self.builder.type_db.get_type(canonical).clone()
                        {
                            canonical = self.builder.type_db.resolve(inner);
                        }
                        let prop_name = mem.property.source();
                        let ty_val = self.builder.type_db.get_type(canonical).clone();
                        match ty_val {
                            t::Type::Enum { repr, variants, .. } => {
                                if let Some(v) = variants.iter().find(|f| f.name == prop_name) {
                                    let ptr = self.builder.build_alloca(canonical);
                                    self.builder.build_store(
                                        repr,
                                        ptr.clone(),
                                        Operand::Int(v.default_value),
                                    );
                                    return (self.builder.build_load(canonical, ptr), canonical);
                                }
                            }
                            t::Type::Array(_, size) if prop_name == "len" => {
                                return (Operand::Int(size as u64), self.builder.type_db.int());
                            }
                            _ => {}
                        }
                    }
                }
                let (ptr, ty) = self.translate_lvalue(expr);
                (self.builder.build_load(ty, ptr), ty)
            }
            s::ExprKind::Binary(bin) => {
                let (left, left_ty) = self.translate_expr(&bin.left);
                let (right, right_ty) = self.translate_expr(&bin.right);

                if let Some(callee_name) = &bin.use_operator_overload {
                    let mut args = Vec::new();

                    args.push(left);
                    args.push(right);

                    if let Some(func) = self.module.functions().get(callee_name.as_str()) {
                        let ret_ty = func.signature().return_type;
                        let callee = Operand::String(std::rc::Rc::from(callee_name.as_str()));
                        (
                            Operand::Reg(self.builder.build_inst(
                                Instruction::Call(callee, args.into_boxed_slice()),
                                ret_ty,
                            )),
                            ret_ty,
                        )
                    } else {
                        self.reporter.report(
                            bin.op_loc(),
                            format!("Call to undefined function '{}'", callee_name),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                } else {
                    let is_comp = matches!(
                        bin.op,
                        s::Op::Eq
                            | s::Op::NotEq
                            | s::Op::Lt
                            | s::Op::LtEq
                            | s::Op::Gt
                            | s::Op::GtEq
                    );
                    let ret_ty = if is_comp {
                        self.builder.type_db.bool()
                    } else {
                        let left_canon = self.builder.type_db.resolve(left_ty);
                        let right_canon = self.builder.type_db.resolve(right_ty);
                        let left_is_ptr = matches!(
                            self.builder.type_db.get_type(left_canon),
                            t::Type::Pointer(_)
                        );
                        let right_is_ptr = matches!(
                            self.builder.type_db.get_type(right_canon),
                            t::Type::Pointer(_)
                        );

                        if bin.op == s::Op::Add && right_is_ptr {
                            right_ty
                        } else if bin.op == s::Op::Sub && left_is_ptr && right_is_ptr {
                            self.builder.type_db.int()
                        } else {
                            left_ty
                        }
                    };

                    let inst = match bin.op {
                        s::Op::Add => Instruction::Add(left, right),
                        s::Op::Sub => Instruction::Sub(left, right),
                        s::Op::Mul => Instruction::Mul(left, right),
                        s::Op::Div => Instruction::Div(left, right),
                        s::Op::Mod => Instruction::Mod(left, right),
                        s::Op::Eq => Instruction::Eq(left_ty, left, right),
                        s::Op::NotEq => Instruction::NotEq(left_ty, left, right),
                        s::Op::Lt => Instruction::Lt(left_ty, left, right),
                        s::Op::LtEq => Instruction::LtEq(left_ty, left, right),
                        s::Op::Gt => Instruction::Gt(left_ty, left, right),
                        s::Op::GtEq => Instruction::GtEq(left_ty, left, right),
                        s::Op::And => Instruction::And(left, right),
                        s::Op::Or => Instruction::Or(left, right),
                        s::Op::BitAnd => Instruction::BitAnd(left, right),
                        s::Op::BitOr => Instruction::BitOr(left, right),
                        s::Op::BitXor => Instruction::BitXor(left, right),
                        _ => {
                            self.reporter
                                .report(bin.op_loc(), "Unsupported binary operator");
                            Instruction::Add(left, right)
                        }
                    };
                    (Operand::Reg(self.builder.build_inst(inst, ret_ty)), ret_ty)
                }
            }
            s::ExprKind::Unary(unary) => {
                if unary.op == s::Op::Refer {
                    let (ptr, ty) = self.translate_lvalue(&unary.right);
                    let ptr_ty = self.builder.type_db.pointer(ty);
                    (ptr, ptr_ty)
                } else {
                    let (right, right_ty) = self.translate_expr(&unary.right);
                    let inst = match unary.op {
                        s::Op::Neg => Instruction::Neg(right),
                        s::Op::Not => Instruction::Not(right),
                        _ => {
                            self.reporter
                                .report(expr.loc(), "Unsupported unary operator");
                            Instruction::Neg(right)
                        }
                    };
                    (
                        Operand::Reg(self.builder.build_inst(inst, right_ty)),
                        right_ty,
                    )
                }
            }
            s::ExprKind::Cast(_, inner, loc) | s::ExprKind::AutoCast(inner, loc) => {
                let (src_val, _src_ty) = self.translate_expr(inner);
                let dest_ty = self
                    .builder
                    .type_db
                    .resolve(expr.resolved_type.unwrap_or(_src_ty));

                // If it is a TypeVar, it means the type is unconstrained/ambiguous
                let canonical_dest = self.builder.type_db.resolve(dest_ty);
                if let t::Type::TypeVar(_) = self.builder.type_db.get_type(canonical_dest) {
                    self.reporter.report(
                        *loc,
                        "Ambiguous auto-cast: cannot infer target type from context",
                    );
                    (Operand::Int(0), self.builder.type_db.void())
                } else {
                    // Check if it's actually castable
                    let underlying_src = self.builder.type_db.get_underlying_type(_src_ty);
                    let underlying_dest = self.builder.type_db.get_underlying_type(dest_ty);
                    let is_cast_valid = if underlying_src == underlying_dest {
                        true
                    } else {
                        let mut is_src_castable =
                            self.builder.type_db.is_primitive_castable(underlying_src);
                        if !is_src_castable
                            && let t::Type::Pointer(_) | t::Type::Enum { .. } =
                                self.builder.type_db.get_type(underlying_src)
                        {
                            is_src_castable = true;
                        }

                        let mut is_dest_castable =
                            self.builder.type_db.is_primitive_castable(underlying_dest);
                        if !is_dest_castable
                            && let t::Type::Pointer(_) | t::Type::Enum { .. } =
                                self.builder.type_db.get_type(underlying_dest)
                        {
                            is_dest_castable = true;
                        }

                        is_src_castable && is_dest_castable
                    };
                    if !is_cast_valid {
                        let dest_str = self.builder.type_db.type_to_string(dest_ty);
                        self.reporter.report(
                            *loc,
                            format!("Cannot cast to non-castable type: {}", dest_str),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    } else {
                        // Generate the cast instruction
                        let inst = Instruction::Cast(src_val);
                        (
                            Operand::Reg(self.builder.build_inst(inst, dest_ty)),
                            dest_ty,
                        )
                    }
                }
            }
            s::ExprKind::Call(call) => {
                let mut args = Vec::new();

                if let s::ExprKind::Member(mem) = &call.callee.kind {
                    let obj_ty = mem
                        .object
                        .resolved_type
                        .map(|t| self.builder.type_db.resolve(t));
                    let is_module = obj_ty == Some(self.builder.type_db.module());
                    let is_type = match &mem.object.kind {
                        s::ExprKind::Identifier(ident) => self
                            .builder
                            .type_db
                            .lookup_type_in_module(&self.current_module, &ident.source)
                            .is_some(),
                        s::ExprKind::Member(inner_mem) => {
                            if let s::ExprKind::Identifier(mod_ident) = &inner_mem.object.kind {
                                self.builder
                                    .type_db
                                    .lookup_type_in_module(
                                        &mod_ident.source,
                                        &inner_mem.property.source,
                                    )
                                    .is_some()
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                    let is_module_or_type = is_module || is_type;

                    if !is_module_or_type {
                        let self_val = if let Some(callee_name) = &call.resolved_name
                            && let Some(func) = self.module.functions().get(callee_name.as_str())
                            && !func.signature().params.is_empty()
                        {
                            let expected_self_ty = func.signature().params[0];
                            let obj_ty = mem
                                .object
                                .resolved_type
                                .unwrap_or_else(|| self.builder.type_db.void());
                            let canon_obj = self.builder.type_db.resolve(obj_ty);
                            if matches!(
                                self.builder.type_db.get_type(expected_self_ty),
                                t::Type::Pointer(_)
                            ) && !matches!(
                                self.builder.type_db.get_type(canon_obj),
                                t::Type::Pointer(_)
                            ) {
                                let (lval_ptr, _) = self.translate_lvalue(&mem.object);
                                lval_ptr
                            } else {
                                let (val, _) = self.translate_expr(&mem.object);
                                val
                            }
                        } else {
                            let (val, _) = self.translate_expr(&mem.object);
                            val
                        };
                        args.push(self_val);
                    }
                }

                for arg in &call.arguments {
                    let (arg_val, _) = self.translate_expr(arg.expr());
                    args.push(arg_val);
                }

                if let Some(callee_name) = &call.resolved_name {
                    let elem_ty = if !call.arguments.is_empty() {
                        let arg0_expr = &call.arguments[0];
                        if let Some(arr_ptr_ty) = arg0_expr.expr().resolved_type {
                            let canon_arr_ptr = self.builder.type_db.resolve(arr_ptr_ty);
                            match self.builder.type_db.get_type(canon_arr_ptr) {
                                t::Type::Pointer(inner) => {
                                    let canon_inner = self.builder.type_db.resolve(*inner);
                                    match self.builder.type_db.get_type(canon_inner) {
                                        t::Type::DynArray(elem) => *elem,
                                        _ => self.builder.type_db.void(),
                                    }
                                }
                                _ => self.builder.type_db.void(),
                            }
                        } else {
                            self.builder.type_db.void()
                        }
                    } else {
                        self.builder.type_db.void()
                    };

                    if callee_name == "builtin.push" {
                        let arr_ptr = args[0].clone();
                        let void_ty = self.builder.type_db.void();
                        let (elem_size, _) =
                            crate::types::type_layout(elem_ty, self.builder.type_db);

                        let mut is_slice_push = false;
                        if args.len() == 2 {
                            let arg1_ty = call.arguments[1].expr().resolved_type.unwrap();
                            let canon1 = self.builder.type_db.resolve(arg1_ty);
                            if matches!(
                                self.builder.type_db.get_type(canon1),
                                t::Type::Slice(_) | t::Type::DynArray(_) | t::Type::Array(_, _)
                            ) {
                                is_slice_push = true;
                            }
                        }

                        if is_slice_push {
                            let val_op = args[1].clone();
                            let val_ty = call.arguments[1].expr().resolved_type.unwrap();
                            let (src_data, src_len) =
                                self.get_slice_like_data_and_len(val_op, val_ty);
                            let push_slice_callee = Operand::String(std::rc::Rc::from(
                                "runtime.chs_dyn_array_push_slice",
                            ));
                            self.builder.build_inst(
                                Instruction::Call(
                                    push_slice_callee,
                                    Box::new([
                                        arr_ptr,
                                        src_data,
                                        src_len,
                                        Operand::Int(elem_size as u64),
                                    ]),
                                ),
                                void_ty,
                            );
                        } else {
                            let push_callee =
                                Operand::String(std::rc::Rc::from("runtime.chs_dyn_array_push"));
                            for val_op in args[1..].iter().cloned() {
                                let val_slot = self.builder.build_alloca(elem_ty);
                                self.builder.build_store(elem_ty, val_slot.clone(), val_op);
                                self.builder.build_inst(
                                    Instruction::Call(
                                        push_callee.clone(),
                                        Box::new([
                                            arr_ptr.clone(),
                                            val_slot,
                                            Operand::Int(elem_size as u64),
                                        ]),
                                    ),
                                    void_ty,
                                );
                            }
                        }

                        return (Operand::Int(0), self.builder.type_db.void());
                    } else if callee_name == "builtin.pop" {
                        let arr_ptr = args[0].clone();
                        let (elem_size, _) =
                            crate::types::type_layout(elem_ty, self.builder.type_db);
                        let out_slot = self.builder.build_alloca(elem_ty);
                        let pop_callee =
                            Operand::String(std::rc::Rc::from("runtime.chs_dyn_array_pop"));
                        self.builder.build_inst(
                            Instruction::Call(
                                pop_callee,
                                Box::new([
                                    arr_ptr,
                                    out_slot.clone(),
                                    Operand::Int(elem_size as u64),
                                ]),
                            ),
                            self.builder.type_db.void(),
                        );
                        let val = self.builder.build_load(elem_ty, out_slot);
                        return (val, elem_ty);
                    } else if callee_name == "builtin.clear" {
                        let arr_ptr = args[0].clone();
                        let clear_callee =
                            Operand::String(std::rc::Rc::from("runtime.chs_dyn_array_clear"));
                        self.builder.build_inst(
                            Instruction::Call(clear_callee, Box::new([arr_ptr])),
                            self.builder.type_db.void(),
                        );
                        return (Operand::Int(0), self.builder.type_db.void());
                    } else if let Some(func) = self.module.functions().get(callee_name.as_str()) {
                        let ret_ty = func.signature().return_type;
                        let callee = Operand::String(std::rc::Rc::from(callee_name.as_str()));
                        let reg = self
                            .builder
                            .build_inst(Instruction::Call(callee, args.into_boxed_slice()), ret_ty);
                        if ret_ty == self.builder.type_db.noreturn() {
                            self.builder.build_return(None);
                        }
                        (Operand::Reg(reg), ret_ty)
                    } else {
                        self.reporter.report(
                            call.loc,
                            format!("Call to undefined function '{}'", callee_name),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                } else {
                    let (callee_val, callee_ty) = self.translate_expr(&call.callee);
                    let canonical = self.builder.type_db.resolve(callee_ty);
                    if let t::Type::FnPointer { return_type, .. } =
                        self.builder.type_db.get_type(canonical).clone()
                    {
                        let reg = self.builder.build_inst(
                            Instruction::Call(callee_val, args.into_boxed_slice()),
                            return_type,
                        );
                        if return_type == self.builder.type_db.noreturn() {
                            self.builder.build_return(None);
                        }
                        (Operand::Reg(reg), return_type)
                    } else {
                        self.reporter.report(
                            call.loc,
                            "Expected function pointer type for indirect call".to_string(),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                }
            }
            s::ExprKind::Assign(assign) => {
                let (right_val, right_ty) = self.translate_expr(&assign.right);

                if let s::ExprKind::Tuple(targets, _) = &assign.left.kind {
                    let tuple_ptr = self.builder.build_alloca(right_ty);
                    self.builder
                        .build_store(right_ty, tuple_ptr.clone(), right_val.clone());

                    let canonical_right = self.builder.type_db.resolve(right_ty);
                    if let t::Type::Tuple(elements) =
                        self.builder.type_db.get_type(canonical_right).clone()
                    {
                        for (i, target) in targets.iter().enumerate() {
                            let elem_ty = elements[i];
                            let ptr_ty = self.builder.type_db.pointer(elem_ty);
                            let (offset, _) = self.get_member_offset(right_ty, &i.to_string());
                            let member_ptr = self.builder.build_inst(
                                Instruction::GetMemberPtr(tuple_ptr.clone(), offset),
                                ptr_ty,
                            );
                            let elem_val =
                                self.builder.build_load(elem_ty, Operand::Reg(member_ptr));
                            let (target_ptr, target_ty) = self.translate_lvalue(target);
                            self.builder.build_store(target_ty, target_ptr, elem_val);
                        }
                    } else {
                        panic!("Expected tuple type for destructuring assignment");
                    }
                    (right_val, right_ty)
                } else {
                    let (ptr, left_ty) = self.translate_lvalue(&assign.left);
                    let val_to_store = match assign.assign_kind {
                        s::AssignKind::Default => right_val,
                        s::AssignKind::Add => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Add(current_val, right_val), left_ty),
                            )
                        }
                        s::AssignKind::Sub => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Sub(current_val, right_val), left_ty),
                            )
                        }
                        s::AssignKind::Mul => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Mul(current_val, right_val), left_ty),
                            )
                        }
                        s::AssignKind::Div => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Div(current_val, right_val), left_ty),
                            )
                        }
                        s::AssignKind::Mod => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Mod(current_val, right_val), left_ty),
                            )
                        }
                    };

                    self.builder.build_store(left_ty, ptr, val_to_store.clone());
                    (val_to_store, left_ty)
                }
            }
            s::ExprKind::Tuple(elements, _) => {
                let tuple_ty = expr.resolved_type.expect("Tuple literal must be resolved");
                let ptr = self.builder.build_alloca(tuple_ty);

                let canonical = self.builder.type_db.resolve(tuple_ty);
                let element_tys = match self.builder.type_db.get_type(canonical) {
                    t::Type::Tuple(elements) => elements.clone(),
                    _ => unreachable!("Must be tuple type"),
                };

                for (i, elem_expr) in elements.iter().enumerate() {
                    let elem_ty = element_tys[i];
                    let (val, _) = self.translate_expr(elem_expr);
                    let ptr_ty = self.builder.type_db.pointer(elem_ty);
                    let (offset, _) = self.get_member_offset(tuple_ty, &i.to_string());
                    let field_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr.clone(), offset), ptr_ty);
                    self.builder
                        .build_store(elem_ty, Operand::Reg(field_ptr), val);
                }

                (self.builder.build_load(tuple_ty, ptr), tuple_ty)
            }
            s::ExprKind::InitList(lit) => {
                let resolved_ty = expr.resolved_type.expect("InitList must be resolved");
                let canonical_ty = self.builder.type_db.resolve(resolved_ty);

                match self.builder.type_db.get_type(canonical_ty).clone() {
                    t::Type::Struct {
                        fields: Some(fields),
                        ..
                    } => {
                        let ptr = self.builder.build_alloca(resolved_ty);
                        for f in &fields {
                            let (val, _) = self.translate_default_value(f.ty);
                            let (offset, _) = self.get_member_offset(resolved_ty, &f.name);
                            let field_ptr_ty = self.builder.type_db.pointer(f.ty);
                            let field_ptr = self.builder.build_inst(
                                Instruction::GetMemberPtr(ptr.clone(), offset),
                                field_ptr_ty,
                            );
                            self.builder.build_store(f.ty, Operand::Reg(field_ptr), val);
                        }
                        let mut pos_index = 0;
                        for elem in &lit.elements {
                            match elem {
                                s::InitElement::Positional(val_expr) => {
                                    let field_def = &fields[pos_index];
                                    let field_name = &field_def.name;
                                    let field_ty = field_def.ty;
                                    let (val, _) = self.translate_expr(val_expr);
                                    let ptr_ty = self.builder.type_db.pointer(field_ty);
                                    let (offset, _) =
                                        self.get_member_offset(resolved_ty, field_name);
                                    let field_ptr = self.builder.build_inst(
                                        Instruction::GetMemberPtr(ptr.clone(), offset),
                                        ptr_ty,
                                    );
                                    self.builder.build_store(
                                        field_ty,
                                        Operand::Reg(field_ptr),
                                        val,
                                    );
                                    pos_index += 1;
                                }
                                s::InitElement::Named { name, value } => {
                                    let field_name = name.source();
                                    let field_def =
                                        fields.iter().find(|fd| fd.name == field_name).unwrap();
                                    let field_ty = field_def.ty;
                                    let (val, _) = self.translate_expr(value);
                                    let ptr_ty = self.builder.type_db.pointer(field_ty);
                                    let (offset, _) =
                                        self.get_member_offset(resolved_ty, field_name);
                                    let field_ptr = self.builder.build_inst(
                                        Instruction::GetMemberPtr(ptr.clone(), offset),
                                        ptr_ty,
                                    );
                                    self.builder.build_store(
                                        field_ty,
                                        Operand::Reg(field_ptr),
                                        val,
                                    );
                                }
                            }
                        }
                        (self.builder.build_load(resolved_ty, ptr), resolved_ty)
                    }
                    _ => {
                        if let t::Type::DynArray(..) = self.builder.type_db.get_type(canonical_ty)
                            && lit.elements.is_empty()
                        {
                            return self.translate_default_value(resolved_ty);
                        }
                        let (element_ty, size) =
                            match self.builder.type_db.get_type(canonical_ty).clone() {
                                t::Type::Array(elem, sz) => (elem, sz),
                                t::Type::Slice(elem) | t::Type::DynArray(elem) => {
                                    (elem, lit.elements.len())
                                }
                                _ => (
                                    self.builder.type_db.get_inner_type_id(canonical_ty),
                                    lit.elements.len(),
                                ),
                            };

                        let array_ty = self.builder.type_db.array(element_ty, size);
                        let ptr = self.builder.build_alloca(array_ty);
                        let elem_ptr_ty = self.builder.type_db.pointer(element_ty);

                        for i in 0..size {
                            let val = if i < lit.elements.len() {
                                let (v, _) = self.translate_expr(lit.elements[i].expr());
                                v
                            } else {
                                let mut default_expr =
                                    s::Expr::new(s::ExprKind::Default(expr.loc()));
                                default_expr.resolved_type = Some(element_ty);
                                let (v, _) = self.translate_expr(&default_expr);
                                v
                            };
                            let idx_op = Operand::Int(i as u64);
                            let elem_ptr = self.builder.build_inst(
                                Instruction::GetIndexPtr(ptr.clone(), idx_op),
                                elem_ptr_ty,
                            );
                            self.builder
                                .build_store(element_ty, Operand::Reg(elem_ptr), val);
                        }
                        (self.builder.build_load(array_ty, ptr), array_ty)
                    }
                }
            }
            s::ExprKind::TypeInfo(ast_ty, _) => {
                let target_ty = self.map_ast_type(ast_ty);
                let canonical_target = self.builder.type_db.resolve(target_ty);
                self.builder
                    .type_db
                    .queried_types_mut()
                    .insert(canonical_target);
                let type_info_id = self.builder.type_db.type_info();
                let ptr_ty = self.builder.type_db.pointer(type_info_id);
                (
                    Operand::Global(std::rc::Rc::from(format!(
                        "chs_type_info_{}",
                        canonical_target.0
                    ))),
                    ptr_ty,
                )
            }
            s::ExprKind::SizeOf(ast_ty, _) => {
                let target_ty = self.map_ast_type(ast_ty);
                let (size, _) = crate::types::type_layout(target_ty, self.builder.type_db);
                (Operand::Int(size as u64), self.builder.type_db.usize())
            }
            s::ExprKind::AlignOf(ast_ty, _) => {
                let target_ty = self.map_ast_type(ast_ty);
                let (_, align) = crate::types::type_layout(target_ty, self.builder.type_db);
                (Operand::Int(align as u64), self.builder.type_db.usize())
            }
            s::ExprKind::New(ast_ty, _) => {
                let target_ty = self.map_ast_type(ast_ty);
                let (elem_size, _) = crate::types::type_layout(target_ty, self.builder.type_db);
                let void_ptr_ty = self.builder.type_db.rawptr();
                let callee = Operand::String(std::rc::Rc::from("runtime.chs_alloc"));
                let ptr = self.builder.build_inst(
                    Instruction::Call(callee, Box::new([Operand::Int(elem_size as u64)])),
                    void_ptr_ty,
                );

                // Zero-initialize using memset
                let memset_callee = Operand::String(std::rc::Rc::from("runtime.chs_memset"));
                self.builder.build_inst(
                    Instruction::Call(
                        memset_callee,
                        Box::new([
                            Operand::Reg(ptr),
                            Operand::Int(0),
                            Operand::Int(elem_size as u64),
                        ]),
                    ),
                    void_ptr_ty,
                );

                (Operand::Reg(ptr), self.builder.type_db.pointer(target_ty))
            }
            s::ExprKind::Drop(arg_expr, _) => {
                let void_ty = self.builder.type_db.void();
                let Some(root) = self.drop_ownership_root(arg_expr) else {
                    self.reporter.report(
                        arg_expr.loc(),
                        "drop requires an owned lvalue; literals, borrowed values, and temporaries cannot be dropped",
                    );
                    return (Operand::Int(0), void_ty);
                };

                let consumed = if Self::is_derived_drop_target(arg_expr) {
                    self.consume_derived_drop(&root)
                } else {
                    self.consume_owned_var(&root)
                };
                if !consumed {
                    self.reporter.report(
                        arg_expr.loc(),
                        "value has already been dropped or is not owned by this scope",
                    );
                }

                // Acquire the lvalue address once. Re-translating a member/index here
                // would evaluate its receiver/index twice and duplicate side effects.
                let (storage, val_ty, val_op) = if self.is_lvalue(arg_expr) {
                    let (storage, ty) = self.translate_lvalue(arg_expr);
                    let val = self.builder.build_load(ty, storage.clone());
                    (storage, ty, val)
                } else {
                    let (val, ty) = self.translate_expr(arg_expr);
                    let storage = self.builder.build_alloca(ty);
                    self.builder.build_store(ty, storage.clone(), val.clone());
                    (storage, ty, val)
                };
                let canonical_ty = self.builder.type_db.resolve(val_ty);

                let is_string = val_ty == self.builder.type_db.string()
                    || matches!(self.builder.type_db.get_type(val_ty), t::Type::String);

                if is_string {
                    let drop_callee = Operand::String(std::rc::Rc::from("runtime.chs_string_drop"));
                    self.builder
                        .build_inst(Instruction::Call(drop_callee, Box::new([storage])), void_ty);
                    return (Operand::Int(0), void_ty);
                }

                match self.builder.type_db.get_type(canonical_ty).clone() {
                    t::Type::DynArray(_) => {
                        let drop_callee =
                            Operand::String(std::rc::Rc::from("runtime.chs_dyn_array_drop"));
                        self.builder.build_inst(
                            Instruction::Call(drop_callee, Box::new([storage])),
                            void_ty,
                        );
                    }
                    t::Type::Slice(_) => {
                        let drop_callee =
                            Operand::String(std::rc::Rc::from("runtime.chs_slice_drop"));
                        self.builder.build_inst(
                            Instruction::Call(drop_callee, Box::new([storage])),
                            void_ty,
                        );
                    }
                    t::Type::String => {
                        let drop_callee =
                            Operand::String(std::rc::Rc::from("runtime.chs_string_drop"));
                        self.builder.build_inst(
                            Instruction::Call(drop_callee, Box::new([storage])),
                            void_ty,
                        );
                    }
                    t::Type::Pointer(inner) => {
                        let inner_canon = self.builder.type_db.resolve(inner);
                        match self.builder.type_db.get_type(inner_canon) {
                            t::Type::DynArray(_) => {
                                let drop_callee = Operand::String(std::rc::Rc::from(
                                    "runtime.chs_dyn_array_drop",
                                ));
                                self.builder.build_inst(
                                    Instruction::Call(drop_callee, Box::new([val_op])),
                                    void_ty,
                                );
                            }
                            t::Type::Slice(_) => {
                                let drop_callee =
                                    Operand::String(std::rc::Rc::from("runtime.chs_slice_drop"));
                                self.builder.build_inst(
                                    Instruction::Call(drop_callee, Box::new([val_op])),
                                    void_ty,
                                );
                            }
                            t::Type::String => {
                                let drop_callee =
                                    Operand::String(std::rc::Rc::from("runtime.chs_string_drop"));
                                self.builder.build_inst(
                                    Instruction::Call(drop_callee, Box::new([val_op])),
                                    void_ty,
                                );
                            }
                            _ => {
                                let drop_callee =
                                    Operand::String(std::rc::Rc::from("runtime.chs_ptr_drop"));
                                self.builder.build_inst(
                                    Instruction::Call(drop_callee, Box::new([storage])),
                                    void_ty,
                                );
                            }
                        }
                    }
                    _ => {
                        let receiver_type_id =
                            canonical_ty.unwrap_pointer_type(self.builder.type_db);
                        let method_key = if let Some(dtor_fn_id) =
                            self.builder.type_db.destructor(receiver_type_id)
                        {
                            self.builder
                                .type_db
                                .get_function(dtor_fn_id)
                                .full_name
                                .clone()
                        } else {
                            let receiver_name =
                                self.builder.type_db.type_to_string(receiver_type_id);
                            format!("{}.drop", receiver_name)
                        };
                        let callee = Operand::String(std::rc::Rc::from(method_key));
                        self.builder
                            .build_inst(Instruction::Call(callee, Box::new([storage])), void_ty);
                    }
                }
                return (Operand::Int(0), void_ty);
            }
            s::ExprKind::Make(_, args, _) => {
                let target_ty = expr.resolved_type.unwrap();
                let canonical = self.builder.type_db.resolve(target_ty);
                match canonical.get_type(self.builder.type_db).clone() {
                    t::Type::Pointer(elem_ty) => {
                        let (elem_size, _) =
                            crate::types::type_layout(elem_ty, self.builder.type_db);
                        let void_ptr_ty = self.builder.type_db.rawptr();
                        let callee = Operand::String(std::rc::Rc::from("runtime.chs_alloc"));
                        let ptr = self.builder.build_inst(
                            Instruction::Call(callee, Box::new([Operand::Int(elem_size as u64)])),
                            void_ptr_ty,
                        );

                        if let Some(ctor_fn_id) = self.builder.type_db.constructor(elem_ty) {
                            let callee_name = self
                                .builder
                                .type_db
                                .get_function(ctor_fn_id)
                                .full_name
                                .clone();
                            let mut op_args = Vec::new();
                            for arg in args {
                                let (arg_val, _) = self.translate_expr(arg);
                                op_args.push(arg_val);
                            }
                            let ctor_callee = Operand::String(std::rc::Rc::from(callee_name));
                            let ctor_val = self.builder.build_inst(
                                Instruction::Call(ctor_callee, op_args.into_boxed_slice()),
                                elem_ty,
                            );
                            self.builder.build_store(
                                elem_ty,
                                Operand::Reg(ptr.clone()),
                                Operand::Reg(ctor_val),
                            );
                        } else {
                            let memset_callee =
                                Operand::String(std::rc::Rc::from("runtime.chs_memset"));
                            self.builder.build_inst(
                                Instruction::Call(
                                    memset_callee,
                                    Box::new([
                                        Operand::Reg(ptr.clone()),
                                        Operand::Int(0),
                                        Operand::Int(elem_size as u64),
                                    ]),
                                ),
                                void_ptr_ty,
                            );
                        }

                        (Operand::Reg(ptr), target_ty)
                    }
                    t::Type::DynArray(_) | t::Type::Slice(_) => {
                        let is_dyn_array = matches!(
                            self.builder.type_db.get_type(canonical),
                            t::Type::DynArray(_)
                        );
                        let inner_ty = self.builder.type_db.get_inner_type_id(canonical);
                        let count_val = if let Some(count_expr) = args.first() {
                            let (val, _) = self.translate_expr(count_expr);
                            val
                        } else {
                            Operand::Int(0)
                        };
                        let (elem_size, _) =
                            crate::types::type_layout(inner_ty, self.builder.type_db);
                        let usize_ty = self.builder.type_db.usize();
                        let total_size = self.builder.build_inst(
                            Instruction::Mul(count_val.clone(), Operand::Int(elem_size as u64)),
                            usize_ty,
                        );

                        let void_ptr_ty = self.builder.type_db.rawptr();
                        let callee = Operand::String(std::rc::Rc::from("runtime.chs_alloc"));
                        let ptr = self.builder.build_inst(
                            Instruction::Call(callee, Box::new([Operand::Reg(total_size)])),
                            void_ptr_ty,
                        );

                        let memset_callee =
                            Operand::String(std::rc::Rc::from("runtime.chs_memset"));
                        self.builder.build_inst(
                            Instruction::Call(
                                memset_callee,
                                Box::new([
                                    Operand::Reg(ptr),
                                    Operand::Int(0),
                                    Operand::Reg(total_size),
                                ]),
                            ),
                            void_ptr_ty,
                        );

                        let slice_ty = target_ty;
                        let slice_ptr = self.builder.build_alloca(slice_ty);

                        let (data_offset, _) = self.get_member_offset(slice_ty, "data");
                        let (len_offset, _) = self.get_member_offset(slice_ty, "len");

                        let elem_ptr_ty = self.builder.type_db.pointer(inner_ty);
                        let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                        let dest_data_ptr = self.builder.build_inst(
                            Instruction::GetMemberPtr(slice_ptr.clone(), data_offset),
                            data_ptr_ty,
                        );
                        self.builder.build_store(
                            elem_ptr_ty,
                            Operand::Reg(dest_data_ptr),
                            Operand::Reg(ptr),
                        );

                        let len_ptr_ty = self.builder.type_db.pointer(usize_ty);
                        let dest_len_ptr = self.builder.build_inst(
                            Instruction::GetMemberPtr(slice_ptr.clone(), len_offset),
                            len_ptr_ty,
                        );

                        if is_dyn_array {
                            let (cap_offset, _) = self.get_member_offset(slice_ty, "cap");
                            self.builder.build_store(
                                usize_ty,
                                Operand::Reg(dest_len_ptr),
                                Operand::Int(0),
                            );

                            let cap_ptr_ty = self.builder.type_db.pointer(usize_ty);
                            let dest_cap_ptr = self.builder.build_inst(
                                Instruction::GetMemberPtr(slice_ptr.clone(), cap_offset),
                                cap_ptr_ty,
                            );
                            self.builder.build_store(
                                usize_ty,
                                Operand::Reg(dest_cap_ptr),
                                count_val,
                            );
                        } else {
                            self.builder.build_store(
                                usize_ty,
                                Operand::Reg(dest_len_ptr),
                                count_val,
                            );
                        }

                        (self.builder.build_load(slice_ty, slice_ptr), slice_ty)
                    }
                    _ => {
                        let receiver_type_id = target_ty.unwrap_pointer_type(self.builder.type_db);
                        let method_key = if let Some(ctor_fn_id) =
                            self.builder.type_db.constructor(receiver_type_id)
                        {
                            self.builder
                                .type_db
                                .get_function(ctor_fn_id)
                                .full_name
                                .clone()
                        } else {
                            let receiver_name =
                                self.builder.type_db.type_to_string(receiver_type_id);
                            format!("{}.make", receiver_name)
                        };
                        let mut op_args = Vec::new();
                        for arg in args {
                            let (arg_val, _) = self.translate_expr(arg);
                            op_args.push(arg_val);
                        }
                        let callee = Operand::String(std::rc::Rc::from(method_key));
                        let target_val = self.builder.build_inst(
                            Instruction::Call(callee, op_args.into_boxed_slice()),
                            target_ty,
                        );
                        (Operand::Reg(target_val), target_ty)
                    }
                }
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Scalar(scalar), _) => {
                let (val, scalar_ty) = self.translate_expr(scalar);
                let canonical_scalar = self.builder.type_db.resolve(scalar_ty);
                let val_ptr = self.builder.build_alloca(scalar_ty);
                self.builder.build_store(scalar_ty, val_ptr.clone(), val);

                self.builder
                    .type_db
                    .queried_types_mut()
                    .insert(canonical_scalar);

                let any_ty = self.builder.type_db.any();
                let any_ptr = self.builder.build_alloca(any_ty);
                let (type_info_offset, _) = self.get_member_offset(any_ty, "type_info");
                let (value_offset, _) = self.get_member_offset(any_ty, "value");
                {
                    let type_info_ptr_ty = self
                        .builder
                        .type_db
                        .pointer(self.builder.type_db.type_info());
                    let field_ptr = self.builder.build_inst(
                        Instruction::GetMemberPtr(any_ptr.clone(), type_info_offset),
                        type_info_ptr_ty,
                    );
                    self.builder.build_store(
                        type_info_ptr_ty,
                        Operand::Reg(field_ptr),
                        Operand::Global(std::rc::Rc::from(format!(
                            "chs_type_info_{}",
                            canonical_scalar.0
                        ))),
                    );
                }
                {
                    let void_ptr_ty = self.builder.type_db.rawptr();
                    let field_ptr = self.builder.build_inst(
                        Instruction::GetMemberPtr(any_ptr.clone(), value_offset),
                        void_ptr_ty,
                    );
                    self.builder
                        .build_store(void_ptr_ty, Operand::Reg(field_ptr), val_ptr);
                }

                (self.builder.build_load(any_ty, any_ptr), any_ty)
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Array(arr), _) => {
                let any_ty = self.builder.type_db.any();
                let element_ty = any_ty;
                let elem_ptr_ty = self.builder.type_db.pointer(any_ty);
                let (type_info_offset, _) = self.get_member_offset(any_ty, "type_info");
                let (value_offset, _) = self.get_member_offset(any_ty, "value");

                let array_ty = self.builder.type_db.array(element_ty, arr.len());
                let ptr = self.builder.build_alloca(array_ty);

                for (i, elem) in arr.iter().enumerate() {
                    let (val, scalar_ty) = self.translate_expr(elem);
                    let canonical_scalar = self.builder.type_db.resolve(scalar_ty);
                    let val_ptr = self.builder.build_alloca(scalar_ty);
                    self.builder.build_store(scalar_ty, val_ptr.clone(), val);
                    self.builder
                        .type_db
                        .queried_types_mut()
                        .insert(canonical_scalar);

                    let any_ptr = self.builder.build_alloca(any_ty);
                    {
                        let type_info_ptr_ty = self
                            .builder
                            .type_db
                            .pointer(self.builder.type_db.type_info());
                        let field_ptr = self.builder.build_inst(
                            Instruction::GetMemberPtr(any_ptr.clone(), type_info_offset),
                            type_info_ptr_ty,
                        );
                        self.builder.build_store(
                            type_info_ptr_ty,
                            Operand::Reg(field_ptr),
                            Operand::Global(std::rc::Rc::from(format!(
                                "chs_type_info_{}",
                                canonical_scalar.0
                            ))),
                        );
                    }
                    {
                        let void_ptr_ty = self.builder.type_db.rawptr();
                        let field_ptr = self.builder.build_inst(
                            Instruction::GetMemberPtr(any_ptr.clone(), value_offset),
                            void_ptr_ty,
                        );
                        self.builder
                            .build_store(void_ptr_ty, Operand::Reg(field_ptr), val_ptr);
                    }

                    let idx_op = Operand::Int(i as u64);
                    let elem_ptr = self
                        .builder
                        .build_inst(Instruction::GetIndexPtr(ptr.clone(), idx_op), elem_ptr_ty);
                    self.builder
                        .build_store(element_ty, Operand::Reg(elem_ptr), any_ptr);
                }
                (self.builder.build_load(array_ty, ptr), array_ty)
            }
            s::ExprKind::Default(_) => {
                let resolved_ty = expr
                    .resolved_type
                    .expect("Default expression must have resolved type");
                self.translate_default_value(resolved_ty)
            }
            #[allow(unreachable_patterns)]
            _ => {
                self.reporter.report(
                    expr.loc(),
                    format!(
                        "Expression `{}` not supported in IR translation yet",
                        expr.name()
                    ),
                );
                (Operand::Int(0), self.builder.type_db.void())
            }
        };

        // Coercion check!
        let canonical_actual = self.builder.type_db.resolve(ty);
        if let Some(expected_ty) = expr.resolved_type {
            let canonical_expected = self.builder.type_db.resolve(expected_ty);
            if canonical_actual != canonical_expected
                && let (t::Type::Array(elem_ty, size), t::Type::Slice(slice_elem)) = (
                    self.builder.type_db.get_type(canonical_actual).clone(),
                    self.builder.type_db.get_type(canonical_expected).clone(),
                )
                && self.builder.type_db.unify(elem_ty, slice_elem).is_ok()
            {
                let slice_ptr = self.builder.build_alloca(canonical_expected);
                let ptr_ty = self.builder.type_db.pointer(elem_ty);
                let data_ptr_ty = self.builder.type_db.pointer(ptr_ty);
                let data_field_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(slice_ptr.clone(), 0), data_ptr_ty);

                let array_ptr = if matches!(
                    expr.kind,
                    s::ExprKind::InitList(_) | s::ExprKind::AnyCast(s::AnyCastExpr::Array(_), _)
                ) {
                    let array_ty = ty;
                    let ptr = self.builder.build_alloca(array_ty);
                    self.builder.build_store(ty, ptr.clone(), val.clone());
                    ptr
                } else {
                    let (lval_ptr, _) = self.translate_lvalue(expr);
                    lval_ptr
                };

                let first_elem_ptr = self
                    .builder
                    .build_inst(Instruction::GetIndexPtr(array_ptr, Operand::Int(0)), ptr_ty);
                self.builder.build_store(
                    ptr_ty,
                    Operand::Reg(data_field_ptr),
                    Operand::Reg(first_elem_ptr),
                );

                let usize_ty = self.builder.type_db.usize();
                let len_ptr_ty = self.builder.type_db.pointer(usize_ty);
                let len_field_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(slice_ptr.clone(), 8), len_ptr_ty);
                self.builder.build_store(
                    usize_ty,
                    Operand::Reg(len_field_ptr),
                    Operand::Int(size as u64),
                );

                return (
                    self.builder.build_load(canonical_expected, slice_ptr),
                    canonical_expected,
                );
            }

            if canonical_actual != canonical_expected
                && let (t::Type::DynArray(elem_ty), t::Type::Slice(slice_elem)) = (
                    self.builder.type_db.get_type(canonical_actual).clone(),
                    self.builder.type_db.get_type(canonical_expected).clone(),
                )
                && self.builder.type_db.unify(elem_ty, slice_elem).is_ok()
            {
                let slice_ptr = self.builder.build_alloca(canonical_expected);
                let dyn_ptr = self.builder.build_alloca(canonical_actual);
                self.builder
                    .build_store(canonical_actual, dyn_ptr.clone(), val);

                let ptr_ty = self.builder.type_db.pointer(elem_ty);
                let data_ptr_ty = self.builder.type_db.pointer(ptr_ty);

                // Copy data pointer (offset 0)
                let src_data_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(dyn_ptr.clone(), 0), data_ptr_ty);
                let data_val = self.builder.build_load(ptr_ty, Operand::Reg(src_data_ptr));
                let dest_data_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(slice_ptr.clone(), 0), data_ptr_ty);
                self.builder
                    .build_store(ptr_ty, Operand::Reg(dest_data_ptr), data_val);

                // Copy length (offset 8)
                let usize_ty = self.builder.type_db.usize();
                let len_ptr_ty = self.builder.type_db.pointer(usize_ty);
                let src_len_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(dyn_ptr, 8), len_ptr_ty);
                let len_val = self.builder.build_load(usize_ty, Operand::Reg(src_len_ptr));
                let dest_len_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(slice_ptr.clone(), 8), len_ptr_ty);
                self.builder
                    .build_store(usize_ty, Operand::Reg(dest_len_ptr), len_val);

                return (
                    self.builder.build_load(canonical_expected, slice_ptr),
                    canonical_expected,
                );
            }

            if canonical_actual != canonical_expected {
                let is_primitive_or_ptr_actual = matches!(
                    self.builder.type_db.get_type(canonical_actual),
                    t::Type::Void
                        | t::Type::Bool
                        | t::Type::Float(..)
                        | t::Type::UntypedFloat
                        | t::Type::NoReturn
                        | t::Type::UntypedInt
                        | t::Type::Integer(..)
                        | t::Type::Pointer(_)
                        | t::Type::FnPointer { .. }
                        | t::Type::Enum { .. }
                );
                let is_primitive_or_ptr_expected = matches!(
                    self.builder.type_db.get_type(canonical_expected),
                    t::Type::Void
                        | t::Type::Bool
                        | t::Type::Float(..)
                        | t::Type::UntypedFloat
                        | t::Type::NoReturn
                        | t::Type::UntypedInt
                        | t::Type::Integer(..)
                        | t::Type::Pointer(_)
                        | t::Type::FnPointer { .. }
                        | t::Type::Enum { .. }
                );
                if is_primitive_or_ptr_actual && is_primitive_or_ptr_expected {
                    let cast_inst = Instruction::Cast(val);
                    return (
                        Operand::Reg(self.builder.build_inst(cast_inst, canonical_expected)),
                        canonical_expected,
                    );
                }
            }
        }

        (val, ty)
    }
}

fn translate_function(
    decl: &s::Function,
    module: &mut Module,
    type_db: &mut t::TypeDatabase,
    reporter: &mut DiagnosticReporter,
    global_constants: &HashMap<String, ConstVal>,
    current_module: TokenSource,
) {
    let entry_block = BasicBlock::new(BlockId(0));
    let mut blocks = vec![entry_block];
    let mut instructions = Vec::new();
    let entry_block = BlockId(0);

    let (params, return_type) = {
        if let Some(Function::Default { signature, .. }) = module
            .functions()
            .get(decl.resolved_name.as_ref().unwrap().as_str())
        {
            (signature.params.clone(), signature.return_type)
        } else {
            return;
        }
    };

    let mut translator = Translator::new(
        &mut blocks,
        &mut instructions,
        module,
        type_db,
        reporter,
        global_constants,
        current_module,
    );
    let file_path = Path::new(*decl.signature.name.loc.file_path()).to_path_buf();
    translator.current_file = Some(file_path);
    translator.builder.set_block(entry_block);

    translator.push_scope();
    for (i, param) in decl.signature.parameters.iter().enumerate() {
        let ty = params[i];
        let ptr = translator.builder.build_alloca(ty);
        translator
            .builder
            .build_store(ty, ptr.clone(), Operand::Param(i as u32));
        translator.insert_var(param.name.source().to_string(), ptr, ty);
        if translator.is_droppable_type(ty) {
            translator.mark_owned(param.name.source.clone());
        }
    }

    if let Some(body) = &decl.body {
        translator.translate_block(body);
    }

    if !translator.is_block_terminated() {
        if return_type != translator.builder.type_db.void()
            && return_type != translator.builder.type_db.noreturn()
        {
            translator
                .reporter
                .report(decl.signature.name.loc, "Function must return a value");
        }
        translator.builder.build_return(None);
    }
    translator.pop_scope();

    if let Some(Function::Default {
        blocks: f_blocks,
        instructions: f_instructions,
        entry_block: f_entry_block,
        ..
    }) = module
        .functions_mut()
        .get_mut(decl.resolved_name.as_ref().unwrap().as_str())
    {
        *f_entry_block = entry_block;
        *f_blocks = blocks;
        *f_instructions = instructions;
    }
}

fn translate_operator(
    decl: &s::OperatorOverload,
    module: &mut Module,
    type_db: &mut t::TypeDatabase,
    reporter: &mut DiagnosticReporter,
    global_constants: &HashMap<String, ConstVal>,
    current_module: TokenSource,
) {
    let entry_block = BasicBlock::new(BlockId(0));
    let mut blocks = vec![entry_block];
    let mut instructions = Vec::new();
    let entry_block = BlockId(0);

    let (params, return_type) = {
        if let Some(Function::Default { signature, .. }) = module
            .functions()
            .get(decl.resolved_name.as_ref().unwrap().as_str())
        {
            (signature.params.clone(), signature.return_type)
        } else {
            (vec![], type_db.void())
        }
    };

    let mut translator = Translator::new(
        &mut blocks,
        &mut instructions,
        module,
        type_db,
        reporter,
        global_constants,
        current_module,
    );
    let file_path = Path::new(*decl.loc.file_path()).to_path_buf();
    translator.current_file = Some(file_path);
    translator.builder.set_block(entry_block);

    translator.push_scope();
    for (i, param) in decl.parameters.iter().enumerate() {
        let ty = params[i];
        let ptr = translator.builder.build_alloca(ty);
        translator
            .builder
            .build_store(ty, ptr.clone(), Operand::Param(i as u32));
        translator.insert_var(param.name.source().to_string(), ptr, ty);
    }

    translator.translate_block(&decl.body);

    if !translator.is_block_terminated() {
        if return_type != translator.builder.type_db.void()
            && return_type != translator.builder.type_db.noreturn()
        {
            translator
                .reporter
                .report(decl.loc, "Function must return a value");
        }
        translator.builder.build_return(None);
    }
    translator.pop_scope();

    if let Some(Function::Default {
        blocks: f_blocks,
        instructions: f_instructions,
        entry_block: f_entry_block,
        ..
    }) = module
        .functions_mut()
        .get_mut(decl.resolved_name.as_ref().unwrap().as_str())
    {
        *f_entry_block = entry_block;
        *f_blocks = blocks;
        *f_instructions = instructions;
    }
}

fn translate_method_implementation(
    method: &s::MethodImplementation,
    module: &mut Module,
    type_db: &mut t::TypeDatabase,
    reporter: &mut DiagnosticReporter,
    global_constants: &HashMap<String, ConstVal>,
    current_module: TokenSource,
) {
    let entry_block = BasicBlock::new(BlockId(0));
    let mut blocks = vec![entry_block];
    let mut instructions = Vec::new();
    let entry_block = BlockId(0);

    let (params, return_type) = {
        if let Some(Function::Default { signature, .. }) = module
            .functions()
            .get(method.function.resolved_name.as_ref().unwrap().as_str())
        {
            (signature.params.clone(), signature.return_type)
        } else {
            return;
        }
    };

    let mut translator = Translator::new(
        &mut blocks,
        &mut instructions,
        module,
        type_db,
        reporter,
        global_constants,
        current_module,
    );
    let file_path = Path::new(*method.function.signature.name.loc.file_path()).to_path_buf();
    translator.current_file = Some(file_path);
    translator.builder.set_block(entry_block);

    translator.push_scope();

    let self_ty = params[0];
    let self_ptr = translator.builder.build_alloca(self_ty);
    translator
        .builder
        .build_store(self_ty, self_ptr.clone(), Operand::Param(0));
    let self_name = method.self_parameter.name.source().to_string();
    translator.insert_var(self_name, self_ptr, self_ty);
    if translator.is_droppable_type(self_ty) {
        translator.mark_owned(method.self_parameter.name.source.clone());
    }

    for (i, param) in method.function.signature.parameters.iter().enumerate() {
        let ty = params[i + 1];
        let ptr = translator.builder.build_alloca(ty);
        translator
            .builder
            .build_store(ty, ptr.clone(), Operand::Param((i + 1) as u32));
        translator.insert_var(param.name.source().to_string(), ptr, ty);
        if translator.is_droppable_type(ty) {
            translator.mark_owned(param.name.source.clone());
        }
    }

    if let Some(body) = &method.function.body {
        translator.translate_block(body);
    }

    if !translator.is_block_terminated() {
        if return_type != translator.builder.type_db.void()
            && return_type != translator.builder.type_db.noreturn()
        {
            translator.reporter.report(
                method.function.signature.name.loc,
                "Method must return a value",
            );
        }
        translator.builder.build_return(None);
    }
    translator.pop_scope();

    if let Some(Function::Default {
        blocks: f_blocks,
        instructions: f_instructions,
        entry_block: f_entry_block,
        ..
    }) = module
        .functions_mut()
        .get_mut(method.function.resolved_name.as_ref().unwrap().as_str())
    {
        *f_entry_block = entry_block;
        *f_blocks = blocks;
        *f_instructions = instructions;
    }
}
