use php_rs::phpt::{Case, Outcome};
use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
};

fn main() -> ExitCode {
    match run() {
        Ok(success) => ExitCode::from(if success { 0 } else { 1 }),
        Err(error) => {
            eprintln!("php-rs-phpt: {error}");
            ExitCode::from(1)
        }
    }
}
fn run() -> Result<bool, String> {
    let mut args = env::args_os().skip(1);
    let mut reference = None;
    let mut working_directory = env::current_dir().map_err(|error| error.to_string())?;
    let mut paths = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--reference" {
            reference = Some(args.next().ok_or("--reference requires a PHP 8.5 binary")?);
        } else if arg == "--cwd" {
            let directory = args.next().ok_or("--cwd requires a directory")?;
            working_directory = fs::canonicalize(directory)
                .map_err(|error| format!("Working directory: {error}"))?;
            if !working_directory.is_dir() {
                return Err("Working directory is not a directory".into());
            }
        } else if arg == "--help" {
            println!(
                "Usage: php-rs-phpt [--reference PHP_8_5_BINARY] [--cwd SUITE_ROOT] [FILE_OR_DIRECTORY ...]\nDefault directory: tests/phpt. Unsupported tests make the run unsuccessful."
            );
            return Ok(true);
        } else if arg.as_encoded_bytes().starts_with(b"-") {
            return Err(format!("Unknown option: {}", arg.to_string_lossy()));
        } else {
            paths.push(PathBuf::from(arg));
        }
    }
    if paths.is_empty() {
        paths.push(PathBuf::from("tests/phpt"));
    }
    if let Some(binary) = &mut reference {
        if Path::new(binary).components().count() > 1 {
            *binary = fs::canonicalize(&*binary)
                .map_err(|error| format!("Reference PHP: {error}"))?
                .into_os_string();
        }
        verify_reference(binary)?;
    }
    let mut files = Vec::new();
    for path in paths {
        collect(&path, &mut files)?;
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        return Err("No PHPT files found".into());
    }
    let (mut passed, mut failed, mut unsupported) = (0, 0, 0);
    for path in files {
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let outcome = match Case::parse(&bytes) {
            Err(error) => Outcome::Failed(format!("Malformed PHPT: {error}")),
            Ok(case) if !case.unsupported_sections().is_empty() => {
                Outcome::Unsupported(case.unsupported_sections().to_vec())
            }
            Ok(case) => match case.prepare(&path) {
                Err(error) => Outcome::Failed(error),
                Ok(prepared) => {
                    let outcome = prepared.run(&working_directory);
                    if outcome == Outcome::Passed {
                        if let Some(binary) = &reference {
                            reference_check(binary, &case, prepared.path(), &working_directory)?
                        } else {
                            outcome
                        }
                    } else {
                        outcome
                    }
                }
            },
        };
        let label = match outcome {
            Outcome::Passed => {
                passed += 1;
                "PASS".to_owned()
            }
            Outcome::Failed(reason) => {
                failed += 1;
                format!("FAIL: {reason}")
            }
            Outcome::Unsupported(sections) => {
                unsupported += 1;
                format!("UNSUPPORTED: {}", sections.join(", "))
            }
        };
        println!("{} {label}", path.display());
    }
    println!("{passed} passed, {failed} failed, {unsupported} unsupported");
    Ok(failed == 0 && unsupported == 0)
}
fn collect(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            // Avoid symlink cycles when recursively scanning an upstream checkout.
            if !entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
                collect(&entry.path(), files)?;
            }
        }
    } else if metadata.is_file() && path.extension() == Some(OsStr::new("phpt")) {
        files.push(path.to_owned());
    }
    Ok(())
}
fn verify_reference(binary: &OsString) -> Result<(), String> {
    let version = Command::new(binary)
        .args([
            "-n",
            "-r",
            "echo PHP_MAJOR_VERSION, '.', PHP_MINOR_VERSION;",
        ])
        .output()
        .map_err(|e| format!("Reference PHP: {e}"))?;
    if !version.status.success() || version.stdout != b"8.5" {
        return Err("Reference binary must be PHP 8.5".into());
    }
    println!(
        "Reference: PHP {} ({})",
        String::from_utf8_lossy(&version.stdout),
        binary.to_string_lossy()
    );
    Ok(())
}
fn reference_check(
    binary: &OsString,
    case: &Case,
    path: &Path,
    working_directory: &Path,
) -> Result<Outcome, String> {
    let output = Command::new(binary)
        .args(["-n", "-d", "display_errors=stderr", "-d", "log_errors=0"])
        .arg(path)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Ok(Outcome::Failed(format!(
            "Reference PHP failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(match case.check_output(&output.stdout) {
        Outcome::Failed(reason) => Outcome::Failed(format!("Reference PHP mismatch: {reason}")),
        other => other,
    })
}
