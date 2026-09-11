use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::exit,
};

use diagnostic::{ChsResult, DiagnosticReporter, bail};
use indexmap::IndexMap;
use semantics::TypeChecker;
use syntax::{ast, parse_file};
use types::{TokenSource, TypeDatabase};

pub fn get_std_path() -> PathBuf {
    if cfg!(feature = "development") {
        return PathBuf::from("./std");
    }
    if let Ok(path) = std::env::var("CHS_HOME") {
        return PathBuf::from(path).join("std");
    }
    eprintln!("$CHS_HOME is not defined");
    exit(1);
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CodegenBackend {
    #[default]
    Qbe,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    #[default]
    Default,
    O0,
    O1,
    O2,
    O3,
}

pub struct CompilerProcess {
    foreign_libraries: HashMap<String, ast::Library>,
    compiled_paths: HashSet<PathBuf>,

    sources: Vec<PathBuf>,
    search_paths: HashSet<PathBuf>,
    target_name: PathBuf,
    backend: CodegenBackend,
    verbose: bool,
    is_main: bool,
    optimization_level: OptimizationLevel,
    features: HashSet<TokenSource>,
}

impl Default for CompilerProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerProcess {
    pub fn new() -> Self {
        Self {
            is_main: true,
            target_name: PathBuf::from("out"),
            foreign_libraries: HashMap::new(),
            compiled_paths: HashSet::new(),
            features: HashSet::new(),
            sources: Vec::new(),
            search_paths: HashSet::new(),
            backend: CodegenBackend::Qbe,
            verbose: false,
            optimization_level: OptimizationLevel::Default,
        }
    }

    pub fn add_default_search_paths(&mut self) -> ChsResult<()> {
        self.add_search_path(env::current_dir()?)?;
        self.add_search_path(get_std_path())?;
        let runtime_path = get_std_path().join("runtime");
        self.add_search_path(&runtime_path)?;
        self.add_search_path(&runtime_path.join("lib"))?;
        if let Ok(entries) = fs::read_dir(runtime_path) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("chs") {
                    self.add_source(entry.path())?;
                }
            }
        }
        Ok(())
    }

    pub fn add_source<P: AsRef<Path>>(&mut self, source: P) -> ChsResult<()> {
        // NOTE: On Windows, use `dunce::canonicalize` to avoid UNC path (`\\?\`) issues with GCC.
        let source = fs::canonicalize(source.as_ref()).unwrap_or(source.as_ref().to_path_buf());
        self.sources.push(source);
        Ok(())
    }

    pub fn add_search_path<P: AsRef<Path>>(&mut self, file_path: P) -> ChsResult<()> {
        let file_path =
            fs::canonicalize(file_path.as_ref()).unwrap_or(file_path.as_ref().to_path_buf());
        self.search_paths.insert(file_path);
        Ok(())
    }

    pub fn add_feature(&mut self, feature: String) {
        self.features.insert(TokenSource::from(feature));
    }

    pub fn compile(&mut self) -> ChsResult<()> {
        let mut type_db = TypeDatabase::default();
        let mut reporter = DiagnosticReporter::default();
        if let Ok(mut module) = self.process(&mut type_db, &mut reporter) {
            ir::optimize(&mut module);
            let s_file = self.codegen_qbe(&mut module, &mut type_db)?;
            self.link_binary(s_file)?;
            Ok(())
        } else {
            if reporter.has_errors() {
                reporter.print_all();
            }
            bail!("Could not translate source code to ir");
        }
    }

    fn process(
        &mut self,
        type_db: &mut TypeDatabase,
        reporter: &mut DiagnosticReporter,
    ) -> ChsResult<ir::Module> {
        let mut modules: IndexMap<String, Vec<ast::FileAst>> = IndexMap::new();
        let mut file_queue = self.sources.clone();
        while let Some(fp) = file_queue.pop() {
            let fp = fs::canonicalize(&fp).unwrap_or(fp);
            if self.compiled_paths.insert(fp.clone()) {
                let source = fs::read_to_string(&fp)?;
                let ast = parse_file(&fp, &source, reporter)?;
                self.resolve_import(reporter, &mut file_queue, fp, &ast);
                self.resolve_foreign_library(reporter, &ast);
                modules
                    .entry(ast.module.source.to_string())
                    .or_default()
                    .push(ast);
            }
        }
        modules.reverse();
        let mut merged_asts = Vec::new();
        let mut tc = TypeChecker::new(
            type_db,
            reporter,
            &mut self.foreign_libraries,
            &self.features,
        );
        // Pass 1.0: Gather imports & libraries
        for (_, asts) in modules.iter_mut() {
            for ast in asts.iter_mut() {
                tc.gather_imports(ast);
            }
        }

        // Pass 1.1: Register placeholders for all structs and enums
        for (_, asts) in modules.iter_mut() {
            for ast in asts.iter_mut() {
                tc.gather_placeholders(ast);
            }
        }

        // Pass 1.2: Resolve type declarations (aliases and distinct types)
        for (_, asts) in modules.iter_mut() {
            for ast in asts.iter_mut() {
                tc.resolve_type_decls(ast);
            }
        }

        // Pass 1.3: Gather and typecheck all global constants
        for (_, asts) in modules.iter_mut() {
            for ast in asts.iter_mut() {
                tc.resolve_constants(ast);
            }
        }

        // Pass 1.4: Resolve field/variant details for structs and enums
        for (_, asts) in modules.iter_mut() {
            for ast in asts.iter_mut() {
                tc.resolve_struct_enum_fields(ast);
            }
        }

        // Pass 1.5: Gather function signatures and operator overloads
        for (_, asts) in modules.iter_mut() {
            for ast in asts.iter_mut() {
                tc.resolve_fn_signatures(ast);
            }
        }

        // Pass 1.6: Gather global and thread-local variables
        for (_, asts) in modules.iter_mut() {
            for ast in asts.iter_mut() {
                tc.resolve_global_variables(ast);
            }
        }

        // Pass 2: Check bodies for all modules
        for (_, asts) in modules {
            for mut ast in asts {
                if !tc.check_bodies(&mut ast) {
                    continue;
                }
                merged_asts.push(ast);
            }
        }
        let mut module = ir::translate::translate_ast_items(&merged_asts, type_db, reporter)?;
        if self
            .features
            .contains(&TokenSource::from("CHS_DEBUG_ALLOC"))
        {
            module.add_global(ir::Global {
                name: TokenSource::from("chs_debug_alloc_feature"),
                ty: type_db.bool(),
                is_thread_local: false,
                is_export: true,
                is_foreign: false,
                init_val: ir::ConstVal::Bool(true),
            });
        }
        if self.features.contains(&TokenSource::from("CHS_USE_GC")) {
            module.add_global(ir::Global {
                name: TokenSource::from("chs_use_gc_feature"),
                ty: type_db.bool(),
                is_thread_local: false,
                is_export: true,
                is_foreign: false,
                init_val: ir::ConstVal::Bool(true),
            });
        }
        Ok(module)
    }

    fn resolve_foreign_library(&mut self, reporter: &mut DiagnosticReporter, ast: &ast::FileAst) {
        for item in &ast.items {
            if let ast::FileItem::Library(lib) = item
                && let Some(l) = self
                    .foreign_libraries
                    .insert(lib.name.source().to_string(), lib.clone())
            {
                reporter.report(
                    l.name.loc,
                    format!("Redefinition of foreign library: {}", l.name),
                );
            }
        }
    }

    fn resolve_import(
        &mut self,
        reporter: &mut DiagnosticReporter,
        file_queue: &mut Vec<PathBuf>,
        fp: PathBuf,
        ast: &ast::FileAst,
    ) {
        for item in &ast.items {
            if let ast::FileItem::Import(import_decl) = item {
                let path_str = import_decl.path.source();
                let mut resolved = None;

                // 1. Try relative to current file
                let mut relative = fp.parent().unwrap_or(Path::new(".")).join(path_str);
                if !relative.exists() {
                    relative = relative.with_extension("chs");
                }

                if relative.exists() {
                    resolved = Some(relative);
                } else {
                    // 2. Try search paths
                    for sp in &self.search_paths {
                        let mut candidate = sp.join(path_str);
                        if !candidate.exists() {
                            candidate = candidate.with_extension("chs");
                        }
                        if candidate.exists() {
                            resolved = Some(candidate);
                            break;
                        }
                    }
                }

                if let Some(r) = resolved {
                    let r = fs::canonicalize(&r).unwrap_or(r);

                    if r.is_dir() {
                        if let Ok(entries) = fs::read_dir(&r) {
                            for entry in entries.flatten() {
                                if entry.path().extension().and_then(|s| s.to_str()) == Some("chs")
                                {
                                    file_queue.push(entry.path());
                                }
                            }
                        }
                    } else if r.is_file() {
                        file_queue.push(r);
                    }
                } else {
                    reporter.report(
                        import_decl.path.loc,
                        format!("Could not resolve import: {}", path_str),
                    );
                    continue;
                }
            }
        }
    }

    fn codegen_qbe(
        &self,
        module: &mut ir::Module,
        type_db: &mut TypeDatabase,
    ) -> ChsResult<PathBuf> {
        let out_dir = env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".build");

        fs::create_dir_all(&out_dir)?;
        fs::write(out_dir.join(".gitignore"), "*")?;

        if self.backend != CodegenBackend::Qbe {
            bail!("Unsupported backend.");
        }

        let transpiler = codegen::qbe::QbeTranspiler::new(module, type_db);
        let qbe_code = transpiler.transpile();

        let ssa_file = out_dir.join(self.target_name.with_extension("ssa"));
        fs::write(&ssa_file, qbe_code)?;

        if self.verbose {
            println!("Transpiled QBE SSA code written to {}", ssa_file.display());
        }

        let s_file = out_dir.join(self.target_name.with_extension("s"));
        let qbe_exe = std::env::var("QBE").unwrap_or_else(|_| "qbe".to_string());

        let mut qbe_cmd = std::process::Command::new(qbe_exe);
        qbe_cmd.arg("-o").arg(&s_file).arg(&ssa_file);

        if !qbe_cmd.status()?.success() {
            bail!("Failed to compile QBE SSA code with qbe.");
        }

        Ok(s_file)
    }

    fn link_binary(&self, s_file: PathBuf) -> ChsResult<()> {
        // let out_dir = env::current_dir()
        //     .unwrap_or_else(|_| PathBuf::from("."))
        //     .join(".build");

        // Allow overriding the C compiler via standard environment variables
        let cc = std::env::var("CC").unwrap_or_else(|_| "gcc".to_string());
        let mut cmd = std::process::Command::new(cc);

        cmd.arg(&s_file);
        if self.is_main {
            cmd.arg("-nostartfiles");
        }
        let final_output = env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&self.target_name);

        cmd.arg("-o").arg(&final_output);

        match self.optimization_level {
            OptimizationLevel::O0 => {
                cmd.arg("-O0");
            }
            OptimizationLevel::O1 => {
                cmd.arg("-O1");
            }
            OptimizationLevel::O2 => {
                cmd.arg("-O2");
            }
            OptimizationLevel::O3 => {
                cmd.arg("-O3");
            }
            OptimizationLevel::Default => {}
        }

        for library in self.foreign_libraries.values() {
            let arg = if library.link_name.starts_with("lib") {
                format!("-l:{}", library.link_name)
            } else if library.kind.is_static() {
                format!("-l:lib{}.a", library.link_name)
            } else {
                format!("-l{}", library.link_name)
            };
            cmd.arg(arg);
        }

        for path in &self.search_paths {
            cmd.arg("-I").arg(path);
            cmd.arg("-L").arg(path);
        }
        if self.verbose {
            println!("CMD: {:?}", cmd);
        }
        let status = cmd.status()?;

        if status.success() {
            if self.verbose {
                let msg = if self.is_main {
                    format!(
                        "Successfully compiled executable to .build/{}",
                        self.target_name.display()
                    )
                } else {
                    format!(
                        "Successfully compiled object to .build/{}.o",
                        self.target_name.display()
                    )
                };
                println!("{}", msg);
            }
            Ok(())
        } else {
            bail!("Failed to link with C compiler.");
        }
    }

    pub fn set_target_name(&mut self, target_name: PathBuf) {
        self.target_name = target_name;
    }

    pub fn target_name(&self) -> &PathBuf {
        &self.target_name
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    pub fn set_optimization_level(&mut self, optimization_level: OptimizationLevel) {
        self.optimization_level = optimization_level;
    }
}
