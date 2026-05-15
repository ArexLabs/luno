use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::checker::Checker;
use crate::codegen::Codegen;
use std::io::{self, Write};

pub fn run_repl() {
    println!("Luno v1.0.0 — Interactive REPL");
    println!("Type 'exit' or Ctrl+C to quit.\n");

    loop {
        print!("luno> ");
        io::stdout().flush().ok();

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => break,
            Err(_) => break,
            _ => {}
        }

        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        if trimmed == "exit" || trimmed == "quit" { break; }

        // Try to compile and run
        let mut lexer = Lexer::new(trimmed);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(diags) => {
                diags.emit(trimmed);
                continue;
            }
        };

        let mut parser = Parser::new(tokens);
        let program = match parser.parse() {
            Ok(p) => p,
            Err(diags) => {
                diags.emit(trimmed);
                continue;
            }
        };

        let mut checker = Checker::new();
        checker.check_program(&program);
        if checker.diags.has_errors() {
            checker.diags.emit(trimmed);
            continue;
        }

        let mut codegen = Codegen::new(checker.types);
        let c_code = codegen.generate(&program);

        // Write and compile
        let c_path = format!("/tmp/luno_repl_{}.c", std::process::id());
        if let Err(e) = std::fs::write(&c_path, &c_code) {
            eprintln!("error: could not write temp file: {}", e);
            continue;
        }

        let binary = format!("/tmp/luno_repl_{}", std::process::id());
        let status = std::process::Command::new("gcc")
            .args(&["-O2", "-Wall", "-Wno-unused-variable", "-o", &binary, &c_path, "-lm"])
            .status();

        match status {
            Ok(s) if s.success() => {
                let _ = std::process::Command::new(&binary).status();
            }
            _ => {
                eprintln!("(compilation skipped — gcc not available)");
            }
        }

        let _ = std::fs::remove_file(&c_path);
        let _ = std::fs::remove_file(&binary);
    }

    println!();
}
