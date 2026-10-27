use php_rs::{Runtime, lint};
use std::{
    env, fs,
    io::{self, Read, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    match execute() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("php-rs: {error}");
            ExitCode::from(1)
        }
    }
}

fn execute() -> Result<u8, Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let mut source = Vec::new();
    let mut lint_only = false;
    let mut code_mode = false;
    let mut file = None;
    match args.first().and_then(|a| a.to_str()) {
        Some("-r") if args.len() == 2 => {
            code_mode = true;
            source.extend_from_slice(args[1].as_encoded_bytes());
        }
        Some("-l") if args.len() == 2 => {
            source = fs::read(&args[1])?;
            lint_only = true;
        }
        Some("-h" | "--help") => {
            println!(
                "Usage: php-rs [file.php | -r 'code' | -l file.php]\nWithout arguments, reads a PHP document from stdin."
            );
            return Ok(0);
        }
        Some("-v" | "--version") => {
            println!(
                "php-rs {} (PHP 8.5 target, incomplete)",
                env!("CARGO_PKG_VERSION")
            );
            return Ok(0);
        }
        Some(arg) if arg.starts_with('-') => {
            return Err(format!("Unknown option or missing argument: {arg}").into());
        }
        Some(_) if args.len() == 1 => file = Some(&args[0]),
        None if args.is_empty() => {
            io::stdin().read_to_end(&mut source)?;
        }
        None if args.len() == 1 => file = Some(&args[0]),
        _ => return Err("Unexpected command-line arguments".into()),
    }
    if lint_only {
        return match lint(&source) {
            Ok(()) => {
                println!("No syntax errors detected in {}", args[1].to_string_lossy());
                Ok(0)
            }
            Err(error) => {
                eprintln!("{error}");
                Ok(255)
            }
        };
    }
    let mut runtime = Runtime::default();
    let result = if let Some(file) = file {
        runtime.execute_file(file)
    } else if code_mode {
        runtime.execute_code(&source)
    } else {
        runtime.execute(&source)
    };
    io::stdout().write_all(&runtime.output)?;
    for warning in &runtime.warnings {
        eprintln!("Warning: {warning}");
    }
    match result {
        Ok(_) => Ok(0),
        Err(error) => {
            eprintln!("{error}");
            Ok(255)
        }
    }
}
