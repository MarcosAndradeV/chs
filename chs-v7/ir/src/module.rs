use std::collections::HashMap;

use lex_just_parse::lexer::TokenSource;

use super::function::Function;

#[derive(Debug, Clone, PartialEq)]
pub enum ConstVal {
    Int(i64),
    Float(f64),
    Bool(bool),
    Zero,
}

#[derive(Debug, Clone)]
pub struct Global {
    pub name: TokenSource,
    pub ty: types::TypeID,
    pub is_thread_local: bool,
    pub is_export: bool,
    pub is_foreign: bool,
    pub init_val: ConstVal,
}

#[derive(Debug)]
pub struct Module {
    functions: HashMap<String, Function>,
    globals: Vec<Global>,
}

impl Default for Module {
    fn default() -> Self {
        Self::new()
    }
}

impl Module {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            globals: Vec::new(),
        }
    }

    pub fn add_function(&mut self, function: Function) {
        self.functions.insert(function.name().to_string(), function);
    }

    pub fn add_global(&mut self, global: Global) {
        self.globals.push(global);
    }

    pub fn globals(&self) -> &[Global] {
        &self.globals
    }

    pub fn functions(&self) -> &HashMap<String, Function> {
        &self.functions
    }

    pub fn functions_mut(&mut self) -> &mut HashMap<String, Function> {
        &mut self.functions
    }

    pub fn globals_mut(&mut self) -> &mut Vec<Global> {
        &mut self.globals
    }
}
