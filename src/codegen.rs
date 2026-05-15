use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use crate::ast::*;
use crate::types::TypeTable;

pub enum EmitMode {
    CSource,
    Asm,
    Obj,
    Exe,
}

pub struct Codegen {
    output: String,
    indent: usize,
    var_map: HashMap<String, String>,
    var_count: usize,
    spawn_count: usize,
    spawn_wrappers: String,
}

impl Codegen {
    pub fn new(_types: TypeTable) -> Self {
        Codegen {
            output: String::new(),
            indent: 0,
            var_map: HashMap::new(),
            var_count: 0,
            spawn_count: 0,
            spawn_wrappers: String::new(),
        }
    }

    pub fn os_name() -> &'static str {
        if cfg!(target_os = "linux") { "linux" }
        else if cfg!(target_os = "macos") { "macos" }
        else if cfg!(target_os = "windows") { "windows" }
        else { "unknown" }
    }

    pub fn target_asm_suffix() -> &'static str {
        if cfg!(target_os = "windows") { "asm" } else { "s" }
    }

    fn default_cc() -> &'static str {
        for cc in &["gcc", "clang", "cc"] {
            if Command::new(cc).arg("--version").output().is_ok() {
                return cc;
            }
        }
        "gcc"
    }

    pub fn compile_to_native(
        &mut self,
        program: &Program,
        source_path: &str,
        emit: EmitMode,
    ) -> Result<(), String> {
        let c_code = self.generate(program);

        let src = Path::new(source_path);
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("output");

        let c_path = format!("{}.c", stem);
        fs::write(&c_path, &c_code).map_err(|e| format!("cannot write {}: {}", c_path, e))?;

        let cc = Self::default_cc();
        let os = Self::os_name();

        let mut base_args = vec!["-O2", "-Wall", "-o"];
        let link_libs = if cfg!(target_os = "windows") {
            vec!["-lm"]
        } else {
            vec!["-lm", "-lpthread"]
        };

        match emit {
            EmitMode::CSource => {
                println!("emitted: {}", c_path);
            }
            EmitMode::Asm => {
                let asm_path = format!("{}.{}", stem, Self::target_asm_suffix());
                let status = Command::new(cc)
                    .args(&["-S", "-O2", "-Wall", "-o", &asm_path, &c_path])
                    .args(&link_libs)
                    .status()
                    .map_err(|e| format!("cannot run {}: {}", cc, e))?;
                if !status.success() {
                    return Err("assembly generation failed".into());
                }
                println!("emitted: {}", asm_path);
            }
            EmitMode::Obj => {
                let obj_path = if cfg!(target_os = "windows") {
                    format!("{}.obj", stem)
                } else {
                    format!("{}.o", stem)
                };
                let status = Command::new(cc)
                    .args(&["-c", "-O2", "-Wall", "-o", &obj_path, &c_path])
                    .args(&link_libs)
                    .status()
                    .map_err(|e| format!("cannot run {}: {}", cc, e))?;
                if !status.success() {
                    return Err("object file generation failed".into());
                }
                println!("emitted: {}", obj_path);
            }
            EmitMode::Exe => {
                let exe_path = if cfg!(target_os = "windows") {
                    format!("{}.exe", stem)
                } else {
                    stem.to_string()
                };
                let status = Command::new(cc)
                    .args(&["-O2", "-Wall", "-o", &exe_path, &c_path])
                    .args(&link_libs)
                    .status()
                    .map_err(|e| format!("cannot run {}: {}", cc, e))?;
                if !status.success() {
                    return Err("compilation failed".into());
                }
                println!("built: {} [{}]", exe_path, os);
            }
        }

        Ok(())
    }

    fn emit_runtime_header(&mut self) {
        self.emit("/* Luno compiled output */\n");
        self.emit("#include <stdio.h>\n");
        self.emit("#include <stdlib.h>\n");
        self.emit("#include <string.h>\n");
        self.emit("#include <stdint.h>\n");
        self.emit("#include <stdbool.h>\n");
        self.emit("#include <math.h>\n");

        if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
            self.emit("#include <unistd.h>\n");
        }

        // Cross-platform threading abstraction
        self.emit("\n");
        self.emit("#ifdef _WIN32\n");
        self.emit("#include <windows.h>\n");
        self.emit("typedef CRITICAL_SECTION luno_mutex_t;\n");
        self.emit("typedef CONDITION_VARIABLE luno_cond_t;\n");
        self.emit("typedef HANDLE luno_thread_t;\n");
        self.emit("#define luno_mutex_init(m) InitializeCriticalSection(m)\n");
        self.emit("#define luno_mutex_lock(m) EnterCriticalSection(m)\n");
        self.emit("#define luno_mutex_unlock(m) LeaveCriticalSection(m)\n");
        self.emit("#define luno_mutex_destroy(m) DeleteCriticalSection(m)\n");
        self.emit("#define luno_cond_init(c) InitializeConditionVariable(c)\n");
        self.emit("#define luno_cond_wait(c,m) SleepConditionVariableCS((c),(m),INFINITE)\n");
        self.emit("#define luno_cond_signal(c) WakeConditionVariable(c)\n");
        self.emit("#define luno_cond_broadcast(c) WakeAllConditionVariable(c)\n");
        self.emit("#define luno_cond_destroy(c) ((void)0)\n");
        self.emit("#define luno_thread_create(t,f,a) (*(t)=CreateThread(NULL,0,(LPTHREAD_START_ROUTINE)(f),(a),0,NULL))\n");
        self.emit("#define luno_thread_detach(t) CloseHandle(t)\n");
        self.emit("#else\n");
        self.emit("#include <pthread.h>\n");
        self.emit("typedef pthread_mutex_t luno_mutex_t;\n");
        self.emit("typedef pthread_cond_t luno_cond_t;\n");
        self.emit("typedef pthread_t luno_thread_t;\n");
        self.emit("#define luno_mutex_init(m) pthread_mutex_init(m,NULL)\n");
        self.emit("#define luno_mutex_lock(m) pthread_mutex_lock(m)\n");
        self.emit("#define luno_mutex_unlock(m) pthread_mutex_unlock(m)\n");
        self.emit("#define luno_mutex_destroy(m) pthread_mutex_destroy(m)\n");
        self.emit("#define luno_cond_init(c) pthread_cond_init(c,NULL)\n");
        self.emit("#define luno_cond_wait(c,m) pthread_cond_wait((c),(m))\n");
        self.emit("#define luno_cond_signal(c) pthread_cond_signal(c)\n");
        self.emit("#define luno_cond_broadcast(c) pthread_cond_broadcast(c)\n");
        self.emit("#define luno_cond_destroy(c) pthread_cond_destroy(c)\n");
        self.emit("#define luno_thread_create(t,f,a) pthread_create((t),NULL,(f),(a))\n");
        self.emit("#define luno_thread_detach(t) pthread_detach(t)\n");
        self.emit("#endif\n\n");

        // Portable strdup
        self.emit("#ifdef _WIN32\n");
        self.emit("#define luno_strdup(s) _strdup(s)\n");
        self.emit("#else\n");
        self.emit("#define luno_strdup(s) strdup(s)\n");
        self.emit("#endif\n\n");
    }

    fn emit_string_runtime(&mut self) {
        self.emit("typedef struct {\n");
        self.emit("    char* data;\n");
        self.emit("    int64_t len;\n");
        self.emit("} LunoString;\n\n");

        self.emit("LunoString luno_string_new(const char* s) {\n");
        self.emit("    LunoString r;\n");
        self.emit("    r.data = luno_strdup(s);\n");
        self.emit("    r.len = (int64_t)strlen(s);\n");
        self.emit("    return r;\n");
        self.emit("}\n\n");

        self.emit("void luno_string_free(LunoString* s) {\n");
        self.emit("    free(s->data);\n");
        self.emit("    s->data = NULL;\n");
        self.emit("}\n\n");

        self.emit("int64_t luno_print_string(LunoString s) {\n");
        self.emit("    printf(\"%s\\n\", s.data);\n");
        self.emit("    return 0;\n");
        self.emit("}\n\n");

        self.emit("int64_t luno_print_int(int64_t v) {\n");
        self.emit("    printf(\"%lld\\n\", (long long)v);\n");
        self.emit("    return 0;\n");
        self.emit("}\n\n");

        self.emit("int64_t luno_print_float(double v) {\n");
        self.emit("    printf(\"%g\\n\", v);\n");
        self.emit("    return 0;\n");
        self.emit("}\n\n");

        self.emit("int64_t luno_print_bool(bool v) {\n");
        self.emit("    printf(\"%s\\n\", v ? \"true\" : \"false\");\n");
        self.emit("    return 0;\n");
        self.emit("}\n\n");
    }

    fn emit_future_runtime(&mut self) {
        self.emit("typedef struct LunoFuture {\n");
        self.emit("    void* result;\n");
        self.emit("    int ready;\n");
        self.emit("    luno_mutex_t mtx;\n");
        self.emit("    luno_cond_t cv;\n");
        self.emit("} LunoFuture;\n\n");

        self.emit("LunoFuture* luno_future_new(void) {\n");
        self.emit("    LunoFuture* f = (LunoFuture*)malloc(sizeof(LunoFuture));\n");
        self.emit("    f->result = NULL;\n");
        self.emit("    f->ready = 0;\n");
        self.emit("    luno_mutex_init(&f->mtx);\n");
        self.emit("    luno_cond_init(&f->cv);\n");
        self.emit("    return f;\n");
        self.emit("}\n\n");

        self.emit("void luno_future_set(LunoFuture* f, void* val) {\n");
        self.emit("    luno_mutex_lock(&f->mtx);\n");
        self.emit("    f->result = val;\n");
        self.emit("    f->ready = 1;\n");
        self.emit("    luno_cond_broadcast(&f->cv);\n");
        self.emit("    luno_mutex_unlock(&f->mtx);\n");
        self.emit("}\n\n");

        self.emit("void* luno_future_await(LunoFuture* f) {\n");
        self.emit("    luno_mutex_lock(&f->mtx);\n");
        self.emit("    while (!f->ready) {\n");
        self.emit("        luno_cond_wait(&f->cv, &f->mtx);\n");
        self.emit("    }\n");
        self.emit("    luno_mutex_unlock(&f->mtx);\n");
        self.emit("    void* result = f->result;\n");
        self.emit("    luno_mutex_destroy(&f->mtx);\n");
        self.emit("    luno_cond_destroy(&f->cv);\n");
        self.emit("    free(f);\n");
        self.emit("    return result;\n");
        self.emit("}\n\n");

        // Task runner
        self.emit("typedef struct {\n");
        self.emit("    void* (*fn)(void*);\n");
        self.emit("    void* args;\n");
        self.emit("    LunoFuture* future;\n");
        self.emit("} LunoTask;\n\n");

        self.emit("void* luno_task_run(void* arg) {\n");
        self.emit("    LunoTask* task = (LunoTask*)arg;\n");
        self.emit("    void* result = task->fn(task->args);\n");
        self.emit("    luno_future_set(task->future, result);\n");
        self.emit("    free(task);\n");
        self.emit("    return NULL;\n");
        self.emit("}\n\n");

        self.emit("LunoFuture* luno_spawn(void* (*fn)(void*), void* args) {\n");
        self.emit("    LunoFuture* fut = luno_future_new();\n");
        self.emit("    LunoTask* task = (LunoTask*)malloc(sizeof(LunoTask));\n");
        self.emit("    task->fn = fn;\n");
        self.emit("    task->args = args;\n");
        self.emit("    task->future = fut;\n");
        self.emit("    luno_thread_t thread;\n");
        self.emit("    luno_thread_create(&thread, luno_task_run, task);\n");
        self.emit("    luno_thread_detach(thread);\n");
        self.emit("    return fut;\n");
        self.emit("}\n\n");
    }

    fn emit_chan_runtime(&mut self) {
        self.emit("typedef struct {\n");
        self.emit("    void** buffer;\n");
        self.emit("    int capacity;\n");
        self.emit("    int count;\n");
        self.emit("    int head;\n");
        self.emit("    int tail;\n");
        self.emit("    luno_mutex_t mtx;\n");
        self.emit("    luno_cond_t not_full;\n");
        self.emit("    luno_cond_t not_empty;\n");
        self.emit("} LunoChan;\n\n");

        self.emit("LunoChan* luno_chan_new(int capacity) {\n");
        self.emit("    LunoChan* ch = (LunoChan*)malloc(sizeof(LunoChan));\n");
        self.emit("    ch->buffer = (void**)malloc(sizeof(void*) * (size_t)capacity);\n");
        self.emit("    ch->capacity = capacity;\n");
        self.emit("    ch->count = 0;\n");
        self.emit("    ch->head = 0;\n");
        self.emit("    ch->tail = 0;\n");
        self.emit("    luno_mutex_init(&ch->mtx);\n");
        self.emit("    luno_cond_init(&ch->not_full);\n");
        self.emit("    luno_cond_init(&ch->not_empty);\n");
        self.emit("    return ch;\n");
        self.emit("}\n\n");

        self.emit("void luno_chan_send(LunoChan* ch, void* val) {\n");
        self.emit("    luno_mutex_lock(&ch->mtx);\n");
        self.emit("    while (ch->count >= ch->capacity) {\n");
        self.emit("        luno_cond_wait(&ch->not_full, &ch->mtx);\n");
        self.emit("    }\n");
        self.emit("    ch->buffer[ch->tail] = val;\n");
        self.emit("    ch->tail = (ch->tail + 1) % ch->capacity;\n");
        self.emit("    ch->count++;\n");
        self.emit("    luno_cond_signal(&ch->not_empty);\n");
        self.emit("    luno_mutex_unlock(&ch->mtx);\n");
        self.emit("}\n\n");

        self.emit("void* luno_chan_recv(LunoChan* ch) {\n");
        self.emit("    luno_mutex_lock(&ch->mtx);\n");
        self.emit("    while (ch->count <= 0) {\n");
        self.emit("        luno_cond_wait(&ch->not_empty, &ch->mtx);\n");
        self.emit("    }\n");
        self.emit("    void* val = ch->buffer[ch->head];\n");
        self.emit("    ch->head = (ch->head + 1) % ch->capacity;\n");
        self.emit("    ch->count--;\n");
        self.emit("    luno_cond_signal(&ch->not_full);\n");
        self.emit("    luno_mutex_unlock(&ch->mtx);\n");
        self.emit("    return val;\n");
        self.emit("}\n\n");
    }

    pub fn generate(&mut self, program: &Program) -> String {
        self.emit_runtime_header();

        // User-defined struct definitions
        for stmt in &program.stmts {
            if let Stmt::TypeDef { name, fields, .. } = stmt {
                self.emit(&format!("typedef struct {{\n"));
                self.indent += 1;
                for f in fields {
                    self.emit_indent();
                    self.emit(&format!("{} {};\n", self.type_to_c(&f.type_expr), f.name));
                }
                self.indent -= 1;
                self.emit(&format!("}} {};\n\n", name));
            }
        }

        self.emit_string_runtime();
        self.emit_future_runtime();
        self.emit_chan_runtime();

        // Forward declarations
        for stmt in &program.stmts {
            if let Stmt::FnDef { name, params, return_type, .. } = stmt {
                let ret = return_type.as_ref()
                    .map(|t| self.type_expr_to_c_type(t))
                    .unwrap_or_else(|| "void".to_string());
                let c_name = if name == "main" { "_luno_main" } else { name };
                let param_strs: Vec<String> = params.iter()
                    .map(|p| {
                        let ct = p.type_hint.as_ref()
                            .map(|t| self.type_expr_to_c_type(t))
                            .unwrap_or_else(|| "void".to_string());
                        format!("{} {}", ct, p.name)
                    })
                    .collect();
                self.emit(&format!("{} {}({});\n", ret, c_name, param_strs.join(", ")));
            }
        }
        self.emit("\n");

        // Pre-generate spawn wrappers
        self.scan_and_gen_spawn_wrappers(program);
        let wrappers = std::mem::take(&mut self.spawn_wrappers);
        self.emit(&wrappers);

        // Function definitions
        for stmt in &program.stmts {
            if let Stmt::FnDef { name, params, return_type, body, .. } = stmt {
                let ret_type_str = return_type.as_ref()
                    .map(|t| self.type_expr_to_c_type(t))
                    .unwrap_or_else(|| "void".to_string());

                let (c_name, c_ret) = if name == "main" {
                    ("_luno_main".to_string(), "void".to_string())
                } else {
                    (name.clone(), ret_type_str)
                };

                let param_strs: Vec<String> = params.iter()
                    .map(|p| {
                        let ct = p.type_hint.as_ref()
                            .map(|t| self.type_expr_to_c_type(t))
                            .unwrap_or_else(|| "void".to_string());
                        format!("{} {}", ct, p.name)
                    })
                    .collect();

                self.emit(&format!("{} {}({}) {{\n", c_ret, c_name, param_strs.join(", ")));
                self.indent += 1;

                for s in body {
                    self.gen_stmt(s);
                }

                if c_ret != "void" && name != "main" {
                    self.emit_indent();
                    self.emit(&format!("return ({})0;\n", c_ret));
                }

                self.indent -= 1;
                self.emit("}\n\n");
            }
        }

        // Main wrapper
        let has_user_main = program.stmts.iter().any(|s| {
            matches!(s, Stmt::FnDef { name, .. } if name == "main")
        });

        self.emit("int main(int argc, char** argv) {\n");
        self.emit("    (void)argc; (void)argv;\n");
        if has_user_main {
            self.emit("    _luno_main();\n");
        }
        self.emit("    return 0;\n");
        self.emit("}\n");

        self.output.clone()
    }

    // --- Spawn wrapper pre-generation ---

    fn scan_and_gen_spawn_wrappers(&mut self, program: &Program) {
        for stmt in &program.stmts {
            self.scan_stmt_spawn(stmt);
        }
    }

    fn scan_stmt_spawn(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::FnDef { body, .. } => {
                for s in body {
                    self.scan_stmt_spawn(s);
                }
            }
            Stmt::Let { value, .. } => self.scan_expr_spawn(value),
            Stmt::Expr(expr) => self.scan_expr_spawn(expr),
            Stmt::Return(expr, _) => {
                if let Some(e) = expr {
                    self.scan_expr_spawn(e);
                }
            }
            _ => {}
        }
    }

    fn scan_expr_spawn(&mut self, expr: &Expr) {
        match expr {
            Expr::Spawn(inner, _) => {
                self.build_spawn_wrapper(inner);
                self.scan_expr_spawn(inner);
            }
            Expr::Block(stmts, _) => {
                for s in stmts { self.scan_stmt_spawn(s); }
            }
            Expr::If(cond, body, elifs, else_body, _) => {
                self.scan_expr_spawn(cond);
                for s in body { self.scan_stmt_spawn(s); }
                for (_, eb) in elifs { for s in eb { self.scan_stmt_spawn(s); } }
                if let Some(eb) = else_body { for s in eb { self.scan_stmt_spawn(s); } }
            }
            Expr::Match(_, arms, _) => {
                for (_, body) in arms { for s in body { self.scan_stmt_spawn(s); } }
            }
            Expr::ForLoop(_, iterable, body, _) => {
                self.scan_expr_spawn(iterable);
                for s in body { self.scan_stmt_spawn(s); }
            }
            Expr::WhileLoop(cond, body, _) => {
                self.scan_expr_spawn(cond);
                for s in body { self.scan_stmt_spawn(s); }
            }
            Expr::BinOp(l, _, r, _) => { self.scan_expr_spawn(l); self.scan_expr_spawn(r); }
            Expr::UnaryOp(_, o, _) => { self.scan_expr_spawn(o); }
            Expr::Cmp(l, _, r, _) => { self.scan_expr_spawn(l); self.scan_expr_spawn(r); }
            Expr::Logical(l, _, r, _) => { self.scan_expr_spawn(l); self.scan_expr_spawn(r); }
            Expr::Call(callee, args, _) => {
                self.scan_expr_spawn(callee);
                for a in args { self.scan_expr_spawn(a); }
            }
            Expr::MethodCall(obj, _, args, _) => {
                self.scan_expr_spawn(obj);
                for a in args { self.scan_expr_spawn(a); }
            }
            Expr::Await(inner, _) => { self.scan_expr_spawn(inner); }
            Expr::Index(obj, idx, _) => { self.scan_expr_spawn(obj); self.scan_expr_spawn(idx); }
            Expr::Attribute(obj, _, _) => { self.scan_expr_spawn(obj); }
            Expr::Lambda(_, body, _) => { self.scan_expr_spawn(body); }
            Expr::StructLit(_, fields, _) => { for (_, f) in fields { self.scan_expr_spawn(f); } }
            Expr::EnumVariant(_, _, args, _) => { for a in args { self.scan_expr_spawn(a); } }
            Expr::List(items, _) => { for i in items { self.scan_expr_spawn(i); } }
            Expr::Tuple(items, _) => { for i in items { self.scan_expr_spawn(i); } }
            Expr::Range(l, r, _) => { self.scan_expr_spawn(l); self.scan_expr_spawn(r); }
            Expr::Assign(target, _, value, _) => { self.scan_expr_spawn(target); self.scan_expr_spawn(value); }
            Expr::TryOp(inner, _) => { self.scan_expr_spawn(inner); }
            Expr::Cast(inner, _, _) => { self.scan_expr_spawn(inner); }
            Expr::Make(_, size, _) => { self.scan_expr_spawn(size); }
            _ => {}
        }
    }

    fn build_spawn_wrapper(&mut self, inner: &Expr) {
        let (callee_name, args) = match inner {
            Expr::Call(callee, args, _) => match callee.as_ref() {
                Expr::Ident(name, _) => (name.clone(), args),
                _ => return,
            },
            _ => return,
        };

        let id = self.spawn_count;
        self.spawn_count += 1;

        // Args struct
        self.spawn_wrappers.push_str("typedef struct {\n");
        for (i, arg) in args.iter().enumerate() {
            let arg_type = self.expr_to_c_type(arg);
            self.spawn_wrappers.push_str(&format!("    {} _a{};\n", arg_type, i));
        }
        self.spawn_wrappers.push_str(&format!("}} _spawn_args_{};\n\n", id));

        // Wrapper function
        let ret_type = self.expr_to_c_type(inner);
        self.spawn_wrappers.push_str(&format!("void* _spawn_wrapper_{}(void* _arg) {{\n", id));
        self.spawn_wrappers.push_str(&format!("    _spawn_args_{}* _a = (_spawn_args_{}*)_arg;\n", id, id));

        let args_refs: Vec<String> = (0..args.len()).map(|i| format!("_a->_a{}", i)).collect();
        let is_scalar = matches!(ret_type.as_str(), "int64_t" | "double" | "bool" | "char" | "uint8_t");

        if is_scalar {
            self.spawn_wrappers.push_str(&format!("    {} _r = {}({});\n", ret_type, callee_name, args_refs.join(", ")));
            if ret_type == "int64_t" {
                self.spawn_wrappers.push_str("    return (void*)(intptr_t)_r;\n");
            } else if ret_type == "double" {
                self.spawn_wrappers.push_str("    double* _boxed = (double*)malloc(sizeof(double)); *_boxed = _r; return (void*)_boxed;\n");
            } else {
                self.spawn_wrappers.push_str("    return NULL;\n");
            }
        } else {
            self.spawn_wrappers.push_str(&format!("    {}* _r = ({})malloc(sizeof({}));\n", ret_type, ret_type, ret_type));
            self.spawn_wrappers.push_str(&format!("    *_r = {}({});\n", callee_name, args_refs.join(", ")));
            self.spawn_wrappers.push_str("    return (void*)_r;\n");
        }

        self.spawn_wrappers.push_str("}\n\n");
    }

    // --- Statement code generation ---

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let cname = self.fresh_name(name);
                self.var_map.insert(name.clone(), cname.clone());
                let ctype = self.expr_to_c_type(value);
                let val = self.gen_expr(value);
                self.emit_indent();
                self.emit(&format!("{} {} = {};\n", ctype, cname, val));
            }
            Stmt::Const { name, value, .. } => {
                let cname = self.fresh_name(name);
                self.var_map.insert(name.clone(), cname.clone());
                let ctype = self.expr_to_c_type(value);
                let val = self.gen_expr(value);
                self.emit_indent();
                self.emit(&format!("const {} {} = {};\n", ctype, cname, val));
            }
            Stmt::Assign { target, value, .. } => {
                let val = self.gen_expr(value);
                let tgt = self.gen_expr(target);
                self.emit_indent();
                self.emit(&format!("{} = {};\n", tgt, val));
            }
            Stmt::Expr(expr) => {
                let val = self.gen_expr(expr);
                if !val.is_empty() && val != "NULL" {
                    self.emit_indent();
                    if !val.ends_with(';') {
                        self.emit(&format!("{};\n", val));
                    } else {
                        self.emit(&format!("{}\n", val));
                    }
                }
            }
            Stmt::Return(expr, _) => {
                let val = expr.as_ref().map(|e| self.gen_expr(e)).unwrap_or_default();
                self.emit_indent();
                self.emit(&format!("return {};\n", val));
            }
            Stmt::Break(_) => { self.emit_indent(); self.emit("break;\n"); }
            Stmt::Continue(_) => { self.emit_indent(); self.emit("continue;\n"); }
            Stmt::FnDef { .. } | Stmt::TypeDef { .. } | Stmt::EnumDef { .. }
            | Stmt::ImplBlock { .. } | Stmt::ImplTrait { .. } | Stmt::TraitDef { .. }
            | Stmt::Import { .. } | Stmt::FromImport { .. } => {}
        }
    }

    // --- Expression code generation ---

    fn gen_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(lit, _) => self.gen_literal(lit),
            Expr::Ident(name, _) => self.gen_ident(name),
            Expr::BinOp(left, op, right, _) => self.gen_binop(left, op, right),
            Expr::UnaryOp(op, operand, _) => self.gen_unary(op, operand),
            Expr::Cmp(left, op, right, _) => self.gen_cmp(left, op, right),
            Expr::Logical(left, op, right, _) => self.gen_logical(left, op, right),
            Expr::Call(callee, args, _) => self.gen_call(callee, args),
            Expr::MethodCall(obj, method, args, _) => self.gen_method_call(obj, method, args),
            Expr::Index(obj, idx, _) => {
                let o = self.gen_expr(obj);
                let i = self.gen_expr(idx);
                format!("{}[{}]", o, i)
            }
            Expr::Attribute(obj, name, _) => {
                let o = self.gen_expr(obj);
                format!("{}.{}", o, name)
            }
            Expr::If(cond, body, elifs, else_body, _) => self.gen_if(cond, body, elifs, else_body),
            Expr::Match(value, arms, _) => self.gen_match(value, arms),
            Expr::ForLoop(var, iterable, body, _) => self.gen_for_loop(var, iterable, body),
            Expr::WhileLoop(cond, body, _) => self.gen_while_loop(cond, body),
            Expr::Block(stmts, _) => self.gen_block(stmts),
            Expr::Lambda(_, _, _) => "NULL".into(),
            Expr::StructLit(name, fields, _) => self.gen_struct_lit(name, fields),
            Expr::EnumVariant(_, _, args, _) => {
                if args.is_empty() { "0".into() }
                else { self.gen_expr(&args[0]) }
            }
            Expr::List(_, _) => "NULL".into(),
            Expr::Tuple(items, _) => {
                if items.is_empty() { "NULL".into() }
                else { self.gen_expr(&items[0]) }
            }
            Expr::Await(inner, _) => {
                format!("luno_future_await({})", self.gen_expr(inner))
            }
            Expr::Spawn(inner, _) => self.gen_spawn(inner),
            Expr::Make(_chan_type, size, _) => {
                format!("(void*)luno_chan_new((int)({}))", self.gen_expr(size))
            }
            Expr::Range(_, _, _) => "0".into(),
            Expr::Assign(_, _, value, _) => self.gen_expr(value),
            Expr::TryOp(inner, _) => self.gen_expr(inner),
            Expr::Cast(inner, _, _) => self.gen_expr(inner),
        }
    }

    fn gen_literal(&self, lit: &Literal) -> String {
        match lit {
            Literal::Int(n) => format!("{}", n),
            Literal::Float(n) => format!("{}", n),
            Literal::Bool(true) => "true".into(),
            Literal::Bool(false) => "false".into(),
            Literal::Char(c) => format!("'{}'", *c as char),
            Literal::String(s) => format!("\"{}\"", s),
            Literal::Null => "NULL".into(),
        }
    }

    fn gen_ident(&self, name: &str) -> String {
        if name == "true" { return "true".into(); }
        if name == "false" { return "false".into(); }
        self.var_map.get(name).cloned().unwrap_or_else(|| name.to_string())
    }

    fn gen_binop(&mut self, left: &Expr, op: &BinOp, right: &Expr) -> String {
        let l = self.gen_expr(left);
        let r = self.gen_expr(right);
        let c_op = match op {
            BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
            BinOp::Div => "/", BinOp::Mod => "%",
            BinOp::And => "&&", BinOp::Or => "||",
            _ => "+",
        };
        if matches!(op, BinOp::Concat) {
            return format!("luno_string_new({})", l);
        }
        format!("({} {} {})", l, c_op, r)
    }

    fn gen_unary(&mut self, op: &UnaryOp, operand: &Expr) -> String {
        let o = self.gen_expr(operand);
        match op {
            UnaryOp::Neg => format!("(-{})", o),
            UnaryOp::Not => format!("(!{})", o),
            UnaryOp::Ref => format!("(&{})", o),
            UnaryOp::MutRef => format!("(&{})", o),
            UnaryOp::Deref => format!("(*{})", o),
        }
    }

    fn gen_cmp(&mut self, left: &Expr, op: &CmpOp, right: &Expr) -> String {
        let l = self.gen_expr(left);
        let r = self.gen_expr(right);
        let c_op = match op {
            CmpOp::Eq => "==", CmpOp::NotEq => "!=",
            CmpOp::Lt => "<", CmpOp::Gt => ">",
            CmpOp::LtEq => "<=", CmpOp::GtEq => ">=",
        };
        format!("({} {} {})", l, c_op, r)
    }

    fn gen_logical(&mut self, left: &Expr, op: &BinOp, right: &Expr) -> String {
        let l = self.gen_expr(left);
        let r = self.gen_expr(right);
        let c_op = match op {
            BinOp::And => "&&",
            BinOp::Or => "||",
            _ => "&&",
        };
        format!("({} {} {})", l, c_op, r)
    }

    fn gen_call(&mut self, callee: &Expr, args: &[Expr]) -> String {
        let callee_name = match callee {
            Expr::Ident(n, _) => n.clone(),
            _ => String::new(),
        };
        let args_str: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();

        if callee_name == "print" {
            let inner = if args_str.is_empty() {
                "luno_string_new(\"\")".to_string()
            } else {
                format!("luno_string_new({})", args_str.join(", "))
            };
            return format!("luno_print_string({})", inner);
        }

        if callee_name == "make" && !args_str.is_empty() {
            return format!("luno_chan_new((int)({}))", args_str[args_str.len() - 1]);
        }

        let callee_str = self.gen_ident(&callee_name);
        format!("{}({})", callee_str, args_str.join(", "))
    }

    fn gen_method_call(&mut self, obj: &Expr, method: &str, args: &[Expr]) -> String {
        let obj_str = self.gen_expr(obj);
        let args_str: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();

        match method {
            "print" => {
                let all = if args_str.is_empty() { obj_str }
                          else { format!("{}{}", obj_str, args_str.join(", ")) };
                format!("luno_print_string(luno_string_new({}))", all)
            }
            "send" => {
                format!("luno_chan_send({}, (void*)&{})", obj_str, args_str[0])
            }
            "recv" => format!("luno_chan_recv({})", obj_str),
            "sqrt" => format!("sqrt({})", obj_str),
            "abs" => format!("abs({})", obj_str),
            "length" => format!("(int64_t)strlen({}.data)", obj_str),
            "upper" | "lower" => obj_str,
            "sin" => format!("sin({})", obj_str),
            "cos" => format!("cos({})", obj_str),
            "to_string" => obj_str,
            _ => format!("{}_{}({})", obj_str, method, args_str.join(", ")),
        }
    }

    fn gen_if(&mut self, cond: &Expr, body: &[Stmt], elifs: &[(Expr, Vec<Stmt>)], else_body: &Option<Vec<Stmt>>) -> String {
        let c = self.gen_expr(cond);
        let mut result = format!("if ({}) {{\n", c);
        self.indent += 1;
        for s in body { self.gen_stmt(s); }
        self.indent -= 1;
        for (ec, eb) in elifs {
            let ec_str = self.gen_expr(ec);
            result.push_str(&format!("}} else if ({}) {{\n", ec_str));
            self.indent += 1;
            for s in eb { self.gen_stmt(s); }
            self.indent -= 1;
        }
        if let Some(eb) = else_body {
            result.push_str("} else {\n");
            self.indent += 1;
            for s in eb { self.gen_stmt(s); }
            self.indent -= 1;
        }
        result.push('}');
        result
    }

    fn gen_match(&mut self, value: &Expr, arms: &[(Pattern, Vec<Stmt>)]) -> String {
        let mut result = String::new();
        let val_str = self.gen_expr(value);
        for (i, (pattern, body)) in arms.iter().enumerate() {
            match pattern {
                Pattern::Literal(lit, _) => {
                    let lit_str = match lit {
                        Literal::Int(n) => format!("{}", n),
                        Literal::String(s) => format!("\"{}\"", s),
                        _ => "0".into(),
                    };
                    result.push_str(&if i == 0 {
                        format!("if ({} == {}) {{\n", val_str, lit_str)
                    } else {
                        format!("}} else if ({} == {}) {{\n", val_str, lit_str)
                    });
                }
                Pattern::Wildcard(_) => result.push_str("} else {\n"),
                _ => {}
            }
            self.indent += 1;
            for s in body { self.gen_stmt(s); }
            self.indent -= 1;
        }
        result.push('}');
        result
    }

    fn gen_for_loop(&mut self, var: &str, iterable: &Expr, body: &[Stmt]) -> String {
        let it = self.gen_expr(iterable);
        let mut result = format!("for (int64_t {}_i = 0; {}_i < (int64_t){}; {}_i++) {{\n", var, var, it, var);
        self.indent += 1;
        for s in body { self.gen_stmt(s); }
        self.indent -= 1;
        result.push('}');
        result
    }

    fn gen_while_loop(&mut self, cond: &Expr, body: &[Stmt]) -> String {
        let c = self.gen_expr(cond);
        let mut result = format!("while ({}) {{\n", c);
        self.indent += 1;
        for s in body { self.gen_stmt(s); }
        self.indent -= 1;
        result.push('}');
        result
    }

    fn gen_block(&mut self, stmts: &[Stmt]) -> String {
        let mut result = "{\n".to_string();
        self.indent += 1;
        for s in stmts { self.gen_stmt(s); }
        self.indent -= 1;
        result.push('}');
        result
    }

    fn gen_struct_lit(&mut self, name: &str, fields: &[(String, Expr)]) -> String {
        let mut result = format!("({}){{", name);
        for (i, (fname, fval)) in fields.iter().enumerate() {
            if i > 0 { result.push_str(", "); }
            result.push_str(&format!(".{} = {}", fname, self.gen_expr(fval)));
        }
        result.push('}');
        result
    }

    fn gen_spawn(&mut self, inner: &Expr) -> String {
        match inner {
            Expr::Call(_callee, args, _) => {
                let id = self.spawn_count - 1;
                let mut result = String::new();
                result.push_str(&format!("_spawn_args_{} _sa_{} = {{ ", id, id));
                let args_str: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                result.push_str(&args_str.join(", "));
                result.push_str(" };\n");
                self.emit_indent();
                self.emit(&result);

                let fut_name = format!("_fut_{}", id);
                self.emit_indent();
                self.emit(&format!(
                    "LunoFuture* {} = luno_spawn(_spawn_wrapper_{}, &_sa_{});\n",
                    fut_name, id, id
                ));
                fut_name
            }
            _ => self.gen_expr(inner),
        }
    }

    // --- Helpers ---

    fn fresh_name(&mut self, base: &str) -> String {
        let name = format!("{}_{}", base, self.var_count);
        self.var_count += 1;
        name
    }

    fn type_to_c(&self, texpr: &crate::ast::TypeExpr) -> String {
        match texpr {
            crate::ast::TypeExpr::Named(name, _) => match name.as_str() {
                "Int" => "int64_t".into(),
                "Float" => "double".into(),
                "Bool" => "bool".into(),
                "Char" => "char".into(),
                "Byte" => "uint8_t".into(),
                "String" => "LunoString".into(),
                "Future" => "LunoFuture*".into(),
                "Chan" => "LunoChan*".into(),
                _ => name.clone(),
            }
            crate::ast::TypeExpr::Generic(name, _, _) => match name.as_str() {
                "Future" => "LunoFuture*".into(),
                "Chan" => "LunoChan*".into(),
                _ => name.clone(),
            }
            crate::ast::TypeExpr::Ref(_, _) => "void*".into(),
            crate::ast::TypeExpr::MutRef(_, _) => "void*".into(),
            _ => "void*".into(),
        }
    }

    fn type_expr_to_c_type(&self, texpr: &crate::ast::TypeExpr) -> String {
        self.type_to_c(texpr)
    }

    fn expr_to_c_type(&self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(lit, _) => match lit {
                Literal::Int(_) => "int64_t".into(),
                Literal::Float(_) => "double".into(),
                Literal::Bool(_) => "bool".into(),
                Literal::Char(_) => "char".into(),
                Literal::String(_) => "LunoString".into(),
                Literal::Null => "void*".into(),
            }
            Expr::Spawn(_, _) => "LunoFuture*".into(),
            Expr::Await(_, _) => "void*".into(),
            Expr::Make(_, _, _) => "LunoChan*".into(),
            Expr::List(_, _) => "void*".into(),
            Expr::Call(callee, _, _) => {
                if let Expr::Ident(name, _) = callee.as_ref() {
                    if name == "main" { return "void".into(); }
                }
                "int64_t".into()
            }
            _ => "int64_t".into(),
        }
    }

    fn emit(&mut self, s: &str) { self.output.push_str(s); }

    fn emit_indent(&mut self) {
        for _ in 0..self.indent { self.output.push_str("    "); }
    }
}
