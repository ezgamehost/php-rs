use php_rs::phpt::{Case, Outcome};
use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    thread,
    time::{Duration, Instant},
};

fn main() -> ExitCode {
    if env::args_os().nth(1).as_deref() == Some(OsStr::new("--worker")) {
        return worker();
    }
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
    let mut timeout = Duration::from_secs(10);
    while let Some(arg) = args.next() {
        if arg == "--reference" {
            reference = Some(args.next().ok_or("--reference requires a PHP 8.5 binary")?);
        } else if arg == "--timeout" {
            let seconds: u64 = args
                .next()
                .ok_or("--timeout requires seconds")?
                .to_str()
                .ok_or("Invalid timeout")?
                .parse()
                .map_err(|_| "Invalid timeout")?;
            if seconds == 0 {
                return Err("Timeout must be positive".into());
            }
            timeout = Duration::from_secs(seconds);
        } else if arg == "--cwd" {
            let directory = args.next().ok_or("--cwd requires a directory")?;
            working_directory = fs::canonicalize(directory)
                .map_err(|error| format!("Working directory: {error}"))?;
            if !working_directory.is_dir() {
                return Err("Working directory is not a directory".into());
            }
        } else if arg == "--help" {
            println!(
                "Usage: php-rs-phpt [--reference PHP_8_5_BINARY] [--cwd SUITE_ROOT] [--timeout SECONDS] [FILE_OR_DIRECTORY ...]\nDefault directory: tests/phpt. Default per-process timeout: 10 seconds. EXPECTF/EXPECTREGEX require --reference. Unsupported tests make the run unsuccessful."
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
        let outcome = match Case::parse_with_reference(&bytes, reference.as_ref().map(Path::new)) {
            Err(error) => Outcome::Failed(format!("Malformed PHPT: {error}")),
            Ok(case) if !case.unsupported_sections().is_empty() => {
                Outcome::Unsupported(case.unsupported_sections().to_vec())
            }
            Ok(case) => match case.prepare(&path) {
                Err(error) => Outcome::Failed(error),
                Ok(prepared) => {
                    let (outcome, status) =
                        run_worker(&path, &working_directory, reference.as_deref(), timeout);
                    if outcome == Outcome::Passed {
                        if let Some(binary) = &reference {
                            reference_check(
                                binary,
                                &case,
                                prepared.path(),
                                &working_directory,
                                status,
                                timeout,
                            )?
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
    expected_status: u8,
    timeout: Duration,
) -> Result<Outcome, String> {
    let mut command = Command::new(binary);
    command
        .args(["-n", "-d", "display_errors=1", "-d", "log_errors=0"])
        .arg(path)
        .current_dir(working_directory)
        .stdin(Stdio::null());
    let output = match timed_output(&mut command, timeout) {
        Ok(output) => output,
        Err(error) => return Ok(Outcome::Failed(format!("Reference PHP: {error}"))),
    };
    if output.status.code() != Some(i32::from(expected_status)) || !output.stderr.is_empty() {
        return Ok(Outcome::Failed(format!(
            "Reference PHP failed (expected exit status {expected_status}, got {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(match case.check_output(&output.stdout) {
        Outcome::Failed(reason) => Outcome::Failed(format!("Reference PHP mismatch: {reason}")),
        other => other,
    })
}

fn worker() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(2).collect();
    if !(2..=3).contains(&args.len()) {
        return ExitCode::from(2);
    }
    let result = (|| -> Result<(), String> {
        let fixture = Path::new(&args[0]);
        let bytes = fs::read(fixture).map_err(|error| error.to_string())?;
        let case = Case::parse_with_reference(&bytes, args.get(2).map(Path::new))?;
        let path = fs::canonicalize(fixture)
            .map_err(|error| error.to_string())?
            .with_extension("php");
        let (outcome, status) = case.run_file_with_status(&path, Path::new(&args[1]));
        match outcome {
            Outcome::Passed => println!("PASS {status}"),
            Outcome::Failed(reason) => print!("FAIL\n{reason}"),
            Outcome::Unsupported(_) => return Err("Unsupported worker fixture".into()),
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

fn run_worker(
    fixture: &Path,
    cwd: &Path,
    reference: Option<&OsStr>,
    timeout: Duration,
) -> (Outcome, u8) {
    let result = (|| -> Result<(Outcome, u8), String> {
        let mut command = Command::new(env::current_exe().map_err(|error| error.to_string())?);
        command.arg("--worker").arg(fixture).arg(cwd);
        if let Some(reference) = reference {
            command.arg(reference);
        }
        let output = timed_output(&mut command, timeout)?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(format!(
                "Worker {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let response = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
        if let Some(status) = response.strip_prefix("PASS ") {
            return Ok((
                Outcome::Passed,
                status.trim().parse().map_err(|_| "Invalid worker status")?,
            ));
        }
        if let Some(reason) = response.strip_prefix("FAIL\n") {
            return Ok((Outcome::Failed(reason.into()), 255));
        }
        Err("Invalid worker response".into())
    })();
    result.unwrap_or_else(|error| (Outcome::Failed(error), 255))
}

fn timed_output(command: &mut Command, timeout: Duration) -> Result<std::process::Output, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let start = Instant::now();
    let mut expired = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(1)),
            pending => {
                expired = matches!(pending, Ok(None));
                let _ = child.kill();
                break child.wait();
            }
        }
    };
    let stdout = out
        .join()
        .map_err(|_| "Output reader panicked")?
        .map_err(|error| error.to_string())?;
    let stderr = err
        .join()
        .map_err(|_| "Error reader panicked")?
        .map_err(|error| error.to_string())?;
    if expired {
        return Err(format!("Timed out after {} seconds", timeout.as_secs()));
    }
    Ok(std::process::Output {
        status: status.map_err(|error| error.to_string())?,
        stdout,
        stderr,
    })
}
