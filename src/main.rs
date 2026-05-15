mod lexer;
mod parser;
mod ast;
mod types;
mod error;
mod checker;
mod codegen;
mod builtins;
mod borrowck;
mod repl;

use std::env;
use std::fs;
use std::process::Command;
use codegen::EmitMode;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        repl::run_repl();
        return;
    }

    let cmd = &args[1];
    match cmd.as_str() {
        "repl" => repl::run_repl(),
        "help" | "--help" | "-h" => print_usage(),
        "version" | "--version" | "-v" => {
            println!("ln v1.0.0 — Luno");
            println!("target: {}-unknown-{}-gnu",
                std::env::consts::ARCH,
                std::env::consts::OS
            );
            if let Ok(cc) = which_cc() {
                println!("cc: {}", cc);
            }
        }
        "init" => {
            cmd_init();
        }
        "new" => {
            if args.len() < 3 { print_usage(); return; }
            cmd_new(&args[2]);
        }
        "setup" => {
            cmd_setup();
        }
        "uninstall" => {
            cmd_uninstall();
        }
        "install" => {
            if args.len() < 3 { print_usage(); return; }
            cmd_install(&args[2]);
        }
        "build" => {
            let manifest = find_manifest();
            if let Some(path) = manifest {
                let emit = parse_emit(&args, 2);
                let main_path = format!("{}/src/main.luno", path);
                if fs::metadata(&main_path).is_ok() {
                    build_file(&main_path, emit);
                } else {
                    eprintln!("error: no src/main.luno found in package");
                    std::process::exit(1);
                }
            } else if args.len() >= 3 {
                let file = &args[2];
                let emit = parse_emit(&args, 3);
                build_file(file, emit);
            } else {
                eprintln!("error: specify a file or run from a package directory");
                std::process::exit(1);
            }
        }
        "run" => {
            let manifest = find_manifest();
            if let Some(path) = manifest {
                let emit = parse_emit(&args, 2);
                let main_path = format!("{}/src/main.luno", path);
                if fs::metadata(&main_path).is_ok() {
                    run_file(&main_path, emit);
                } else {
                    eprintln!("error: no src/main.luno found in package");
                    std::process::exit(1);
                }
            } else if args.len() >= 3 {
                let file = &args[2];
                let emit = parse_emit(&args, 3);
                run_file(file, emit);
            } else {
                eprintln!("error: specify a file or run from a package directory");
                std::process::exit(1);
            }
        }
        "check" => {
            if args.len() < 3 { print_usage(); return; }
            check_file(&args[2]);
        }
        _ => {
            if cmd.ends_with(".luno") {
                run_file(cmd, EmitMode::Exe);
            } else {
                print_usage();
            }
        }
    }
}

fn parse_emit(args: &[String], start: usize) -> EmitMode {
    for i in start..args.len() {
        if args[i] == "--emit" {
            if i + 1 < args.len() {
                return match args[i + 1].as_str() {
                    "c" => EmitMode::CSource,
                    "asm" | "s" => EmitMode::Asm,
                    "obj" | "o" => EmitMode::Obj,
                    "exe" => EmitMode::Exe,
                    _ => EmitMode::Exe,
                };
            }
        }
    }
    EmitMode::Exe
}

fn which_cc() -> Result<String, String> {
    for cc in &["gcc-14", "gcc-13", "gcc-12", "gcc", "clang-18", "clang-17", "clang", "cc"] {
        if Command::new(cc).arg("--version").output().is_ok() {
            return Ok(cc.to_string());
        }
    }
    Err("no C compiler found".into())
}

fn find_manifest() -> Option<String> {
    let candidates = vec![".", "..", "../.."];
    for dir in &candidates {
        let path = format!("{}/luno.json", dir);
        if fs::metadata(&path).is_ok() {
            return Some(dir.to_string());
        }
    }
    None
}

fn run_file(path: &str, emit: EmitMode) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: could not read '{}': {}", path, e); std::process::exit(1); }
    };

    let should_run = matches!(emit, EmitMode::Exe);
    let result = compile_to_c(&source, path, emit);
    match result {
        Ok(out_path) => {
            if should_run {
                let status = if cfg!(target_os = "windows") {
                    Command::new("cmd").args(&["/C", &out_path]).status()
                } else {
                    Command::new(&out_path).status()
                };
                match status {
                    Ok(_) => {}
                    Err(e) => eprintln!("error: could not execute: {}", e),
                }
            }
        }
        Err(diags) => {
            diags.emit(&source);
            std::process::exit(1);
        }
    }
}

fn build_file(path: &str, emit: EmitMode) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: could not read '{}': {}", path, e); std::process::exit(1); }
    };

    match compile_to_c(&source, path, emit) {
        Ok(out_path) => println!("output: {}", out_path),
        Err(diags) => {
            diags.emit(&source);
            std::process::exit(1);
        }
    }
}

fn check_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: could not read '{}': {}", path, e); std::process::exit(1); }
    };

    match type_check(&source) {
        Ok(()) => println!("type check passed: {}", path),
        Err(diags) => {
            diags.emit(&source);
            std::process::exit(1);
        }
    }
}

fn cmd_init() {
    if fs::metadata("luno.json").is_ok() {
        eprintln!("error: luno.json already exists");
        std::process::exit(1);
    }
    let name = env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "project".into());

    let manifest = format!(
        r#"{{"name":"{}","version":"0.1.0","edition":"2026","dependencies":{{}}}}"#,
        name
    );
    fs::write("luno.json", manifest).unwrap_or_else(|e| {
        eprintln!("error: could not write luno.json: {}", e);
        std::process::exit(1);
    });
    fs::create_dir_all("src").unwrap_or_else(|e| {
        eprintln!("error: could not create src/: {}", e);
        std::process::exit(1);
    });
    fs::create_dir_all("tests").unwrap_or_else(|e| {
        eprintln!("error: could not create tests/: {}", e);
        std::process::exit(1);
    });
    let main_luno = r#"fn main() {
    print("Hello, Luno!")
}
"#;
    fs::write("src/main.luno", main_luno).unwrap_or_else(|e| {
        eprintln!("error: could not write src/main.luno: {}", e);
        std::process::exit(1);
    });
    println!("initialized package: {}", name);
}

fn cmd_new(name: &str) {
    fs::create_dir_all(format!("{}/src", name)).unwrap_or_else(|e| {
        eprintln!("error: could not create '{}': {}", name, e);
        std::process::exit(1);
    });
    fs::create_dir_all(format!("{}/tests", name)).unwrap_or_else(|e| {
        eprintln!("error: could not create tests/: {}", e);
        std::process::exit(1);
    });
    let manifest = format!(
        r#"{{"name":"{}","version":"0.1.0","edition":"2026","dependencies":{{}}}}"#,
        name
    );
    let main_luno = r#"fn main() {
    print("Hello, Luno!")
}
"#;
    fs::write(format!("{}/luno.json", name), manifest).unwrap_or_else(|e| {
        eprintln!("error: could not write luno.json: {}", e);
        std::process::exit(1);
    });
    fs::write(format!("{}/src/main.luno", name), main_luno).unwrap_or_else(|e| {
        eprintln!("error: could not write main.luno: {}", e);
        std::process::exit(1);
    });
    println!("created project: {}", name);
}

fn cmd_setup() {
    let home = env::var("HOME").unwrap_or_else(|_| {
        eprintln!("error: $HOME not set");
        std::process::exit(1);
    });

    let install_dir = format!("{}/.luno/bin", home);
    let install_path = format!("{}/ln", install_dir);

    // Create install directory
    fs::create_dir_all(&install_dir).unwrap_or_else(|e| {
        eprintln!("error: could not create {}: {}", install_dir, e);
        std::process::exit(1);
    });

    // Copy the running binary
    let self_path = env::current_exe().unwrap_or_else(|e| {
        eprintln!("error: could not determine binary path: {}", e);
        std::process::exit(1);
    });

    fs::copy(&self_path, &install_path).unwrap_or_else(|e| {
        eprintln!("error: could not copy to {}: {}", install_path, e);
        std::process::exit(1);
    });

    println!("installed ln to {}", install_path);

    // Add to PATH in shell configs
    let path_line = "export PATH=\"$HOME/.luno/bin:$PATH\"";
    let configs = vec![".bashrc", ".zshrc", ".profile", ".bash_profile", ".zshenv"];
    let mut updated = Vec::new();

    for rc in &configs {
        let rc_path = format!("{}/{}", home, rc);
        if fs::metadata(&rc_path).is_ok() {
            let content = fs::read_to_string(&rc_path).unwrap_or_default();
            if content.contains(".luno/bin") {
                continue;
            }
            let mut new_content = content;
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
            new_content.push_str("\n# Luno\n");
            new_content.push_str(path_line);
            new_content.push('\n');
            fs::write(&rc_path, &new_content).unwrap_or_else(|e| {
                eprintln!("warning: could not write {}: {}", rc_path, e);
            });
            updated.push(rc.to_string());
        }
    }

    // Also update .profile if nothing else was found
    if updated.is_empty() {
        let profile_path = format!("{}/.profile", home);
        let mut content = fs::read_to_string(&profile_path).unwrap_or_default();
        if !content.contains(".luno/bin") {
            if !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str("\n# Luno\n");
            content.push_str(path_line);
            content.push('\n');
            fs::write(&profile_path, &content).unwrap_or_else(|e| {
                eprintln!("warning: could not write {}: {}", profile_path, e);
            });
            updated.push(".profile".to_string());
        }
    }

    if !updated.is_empty() {
        println!("added to PATH in: {}", updated.join(", "));
    }

    println!();
    println!("Restart your shell or run:");
    println!("  export PATH=\"$HOME/.luno/bin:$PATH\"");
    println!();
    println!("Then verify with: ln version");
}

fn cmd_uninstall() {
    let home = env::var("HOME").unwrap_or_else(|_| {
        eprintln!("error: $HOME not set");
        std::process::exit(1);
    });

    let install_dir = format!("{}/.luno", home);
    let install_path = format!("{}/.luno/bin/ln", home);

    // Remove binary
    if fs::metadata(&install_path).is_ok() {
        fs::remove_file(&install_path).unwrap_or_else(|e| {
            eprintln!("warning: could not remove {}: {}", install_path, e);
        });
        println!("removed {}", install_path);
    }

    // Remove .luno directory if empty
    if fs::metadata(&install_dir).is_ok() {
        fs::remove_dir_all(&install_dir).unwrap_or_else(|_| {});
        println!("removed {}", install_dir);
    }

    // Remove PATH lines from shell configs
    let configs = vec![".bashrc", ".zshrc", ".profile", ".bash_profile", ".zshenv"];
    for rc in &configs {
        let rc_path = format!("{}/{}", home, rc);
        if fs::metadata(&rc_path).is_ok() {
            let content = fs::read_to_string(&rc_path).unwrap_or_default();
            let new_content: Vec<&str> = content
                .lines()
                .filter(|l| !l.contains(".luno/bin") && l.trim() != "# Luno")
                .collect();
            let result = new_content.join("\n");
            if result != content.trim() {
                fs::write(&rc_path, &result).unwrap_or_else(|e| {
                    eprintln!("warning: could not write {}: {}", rc_path, e);
                });
                println!("cleaned PATH from {}", rc_path);
            }
        }
    }

    println!("ln uninstalled");
}

fn cmd_install(pkg: &str) {
    let manifest_path = "luno.json";
    let mut content = fs::read_to_string(manifest_path).unwrap_or_else(|e| {
        eprintln!("error: could not read luno.json: {} (run 'ln init' first)", e);
        std::process::exit(1);
    });

    let parts: Vec<&str> = pkg.split('@').collect();
    let name = parts[0];
    let version = if parts.len() > 1 { parts[1] } else { "latest" };
    let dep_entry = format!("\"{}\": \"{}\"", name, version);

    if content.contains("\"dependencies\":{}") {
        content = content.replace("\"dependencies\":{}", &format!("\"dependencies\":{{{}}}", dep_entry));
    } else if let Some(deps_start) = content.find("\"dependencies\":{") {
        let brace_start = deps_start + "\"dependencies\":{".len() - 1;
        // Find matching closing brace
        let rest = &content[brace_start + 1..];
        if let Some(end) = rest.find('}') {
            let inner = &rest[..end];
            let new_inner = if inner.trim().is_empty() {
                format!("\n        {}", dep_entry)
            } else {
                format!("{},\n        {}", inner.trim_end(), dep_entry)
            };
            let before = &content[..brace_start + 1];
            let after = &rest[end..];
            content = format!("{}{}{}", before, new_inner, after);
        }
    } else {
        eprintln!("error: could not parse luno.json dependencies");
        std::process::exit(1);
    }

    fs::write(manifest_path, content).unwrap_or_else(|e| {
        eprintln!("error: could not write luno.json: {}", e);
        std::process::exit(1);
    });
    println!("installed {}@{}", name, version);
}

fn compile_to_c(source: &str, path: &str, emit: EmitMode) -> std::result::Result<String, crate::error::Diagnostics> {
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;

    let mut parser = parser::Parser::new(tokens);
    let program = parser.parse()?;

    let mut checker = checker::Checker::new();
    checker.check_program(&program);
    if checker.diags.has_errors() {
        return Err(checker.diags);
    }

    let mut bck = borrowck::BorrowChecker::new(&checker, &program);
    let _ = bck.check_program(&program);
    if bck.has_errors() {
        return Err(std::mem::take(bck.diags_mut()));
    }

    let mut codegen = codegen::Codegen::new(checker.types);

    let stem = path.trim_end_matches(".luno");
    let out_path = match emit {
        EmitMode::Exe => {
            if cfg!(target_os = "windows") { format!("{}.exe", stem) }
            else { stem.to_string() }
        }
        EmitMode::Asm => format!("{}.{}", stem, codegen::Codegen::target_asm_suffix()),
        EmitMode::Obj => {
            if cfg!(target_os = "windows") { format!("{}.obj", stem) }
            else { format!("{}.o", stem) }
        }
        EmitMode::CSource => format!("{}.c", stem),
    };

    codegen.compile_to_native(&program, path, emit)
        .map_err(|e| {
            let mut d = crate::error::Diagnostics::new();
            d.push(crate::error::Diag::error(e, crate::error::Span::dummy()));
            d
        })?;

    Ok(out_path)
}

fn type_check(source: &str) -> std::result::Result<(), crate::error::Diagnostics> {
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;

    let mut parser = parser::Parser::new(tokens);
    let program = parser.parse()?;

    let mut checker = checker::Checker::new();
    checker.check_program(&program);
    if checker.diags.has_errors() {
        return Err(checker.diags);
    }

    let mut bck = borrowck::BorrowChecker::new(&checker, &program);
    let _ = bck.check_program(&program);
    if bck.has_errors() {
        return Err(std::mem::take(bck.diags_mut()));
    }

    Ok(())
}

fn print_usage() {
    println!("ln v1.0.0 — Luno: compiled systems language\n");
    println!("Usage:");
    println!("  ln <file.luno>              Compile and run");
    println!("  ln init                     Initialize package in current directory");
    println!("  ln new <project>            Create new project");
    println!("  ln install <package>        Add dependency (@version optional)");
    println!("  ln build [file]             Compile to binary (package or file)");
    println!("  ln run [file]               Compile and run (package or file)");
    println!("  ln check <file>             Type-check only");
    println!("  ln setup                    Install ln to PATH (~/.luno/bin)");
    println!("  ln uninstall                Remove ln from system");
    println!("  ln repl                     Start REPL");
    println!("  ln help                     Show help");
    println!();
    println!("Build options:");
    println!("  --emit c      Emit C source only");
    println!("  --emit asm    Emit assembly");
    println!("  --emit obj    Emit object file");
    println!("  --emit exe    Emit executable (default)");
    println!();
    println!("Examples:");
    println!("  ln run hello.luno");
    println!("  ln build hello.luno --emit asm");
    println!("  ln check hello.luno");
    println!("  ln init");
    println!("  ln install http");
    println!("  ln setup");
}
