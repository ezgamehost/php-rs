//! PHPT parsing and output conformance checks. Unsupported sections are
//! reported explicitly, never ignored or treated as successful tests.
use crate::Runtime;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug)]
pub struct Case {
    pub name: String,
    pub source: Vec<u8>,
    expected: Vec<u8>,
    unsupported: Vec<String>,
    expectation: String,
    reference: Option<PathBuf>,
}
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Passed,
    Failed(String),
    Unsupported(Vec<String>),
}

/// A fixture's temporary .php sibling. Existing files are never replaced.
/// The generated file is removed when this handle is dropped.
#[derive(Debug)]
pub struct PreparedCase<'a> {
    case: &'a Case,
    path: PathBuf,
}

impl PreparedCase<'_> {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn run(&self, working_directory: &Path) -> Outcome {
        self.run_with_status(working_directory).0
    }

    /// Run the fixture and return its CLI-compatible process status as well.
    pub fn run_with_status(&self, working_directory: &Path) -> (Outcome, u8) {
        self.case
            .run_file_with_status(&self.path, working_directory)
    }
}

impl Case {
    /// Execute a file already materialized by the parent harness. Keeping its
    /// lifetime in the parent allows cleanup even if a worker aborts.
    pub fn run_file_with_status(&self, path: &Path, working_directory: &Path) -> (Outcome, u8) {
        let mut runtime = Runtime::default();
        if let Err(error) = runtime.set_working_directory(working_directory) {
            return (Outcome::Failed(format!("Working directory: {error}")), 255);
        }
        let result = runtime.execute_file(path);
        let status = if result.is_err() {
            255
        } else {
            runtime.exit_status().unwrap_or(0) as u8
        };
        (self.runtime_outcome(&runtime, result), status)
    }
}

impl Drop for PreparedCase<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl Case {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        Self::parse_with_reference(bytes, None)
    }

    /// PHP supplies PCRE matching for EXPECTF/EXPECTREGEX only; fixture programs
    /// still execute in the Rust runtime. Without it these formats are unsupported.
    pub fn parse_with_reference(bytes: &[u8], reference: Option<&Path>) -> Result<Self, String> {
        let mut sections: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut current = None;
        for line in bytes.split_inclusive(|c| *c == b'\n') {
            let header = line.strip_suffix(b"\n").unwrap_or(line);
            let header = header.strip_suffix(b"\r").unwrap_or(header);
            if let Some(name) = header
                .strip_prefix(b"--")
                .and_then(|h| h.strip_suffix(b"--"))
                && !name.is_empty()
                && name.iter().all(|c| c.is_ascii_uppercase() || *c == b'_')
            {
                let name = String::from_utf8(name.to_vec()).unwrap();
                if sections.insert(name.clone(), Vec::new()).is_some() {
                    return Err(format!("Duplicate section: {name}"));
                }
                current = Some(name);
            } else if let Some(name) = &current {
                sections.get_mut(name).unwrap().extend_from_slice(line);
            } else if !line.iter().all(u8::is_ascii_whitespace) {
                return Err("Content before first PHPT section".into());
            }
        }
        let name = sections.get("TEST").ok_or("Missing TEST section")?;
        let name = String::from_utf8_lossy(name).trim().to_owned();
        if name.is_empty() {
            return Err("Empty TEST section".into());
        }
        let files: Vec<_> = ["FILE", "FILEEOF", "FILE_EXTERNAL"]
            .into_iter()
            .filter(|s| sections.contains_key(*s))
            .collect();
        if files.len() != 1 {
            return Err("Expected exactly one FILE section variant".into());
        }
        let expected: Vec<_> = [
            "EXPECT",
            "EXPECTF",
            "EXPECTREGEX",
            "EXPECT_EXTERNAL",
            "EXPECTF_EXTERNAL",
            "EXPECTREGEX_EXTERNAL",
        ]
        .into_iter()
        .filter(|s| sections.contains_key(*s))
        .collect();
        if expected.len() != 1 {
            return Err("Expected exactly one EXPECT section variant".into());
        }
        let unsupported = sections
            .keys()
            .filter(|key| {
                !(["TEST", "FILE", "EXPECT", "DESCRIPTION", "CREDITS"].contains(&key.as_str())
                    || reference.is_some() && ["EXPECTF", "EXPECTREGEX"].contains(&key.as_str()))
            })
            .cloned()
            .collect();
        Ok(Self {
            name,
            source: sections[files[0]].clone(),
            expected: sections[expected[0]].clone(),
            unsupported,
            expectation: expected[0].to_owned(),
            reference: reference.map(Path::to_owned),
        })
    }
    pub fn unsupported_sections(&self) -> &[String] {
        &self.unsupported
    }

    pub fn prepare(&self, fixture: &Path) -> Result<PreparedCase<'_>, String> {
        if !self.unsupported.is_empty() {
            return Err("Cannot prepare an unsupported fixture".into());
        }
        let path = fs::canonicalize(fixture)
            .map_err(|error| error.to_string())?
            .with_extension("php");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    format!(
                        "Generated file already exists; refusing to overwrite {}",
                        path.display()
                    )
                } else {
                    format!("Cannot create {}: {error}", path.display())
                }
            })?;
        let prepared = PreparedCase { case: self, path };
        let written = file.write_all(&self.source);
        drop(file);
        written.map_err(|error| error.to_string())?;
        Ok(prepared)
    }

    pub fn run(&self) -> Outcome {
        if !self.unsupported.is_empty() {
            return Outcome::Unsupported(self.unsupported.clone());
        }
        let mut runtime = Runtime::default();
        let result = runtime.execute(&self.source);
        self.runtime_outcome(&runtime, result)
    }

    fn runtime_outcome(
        &self,
        runtime: &Runtime,
        result: Result<crate::Value, crate::Error>,
    ) -> Outcome {
        if let Err(error) = result {
            return Outcome::Failed(error.to_string());
        }
        self.check_output(&runtime.output_with_diagnostics())
    }
    pub fn check_output(&self, actual: &[u8]) -> Outcome {
        if !self.unsupported.is_empty() {
            return Outcome::Unsupported(self.unsupported.clone());
        }
        let actual_normalized = normalize(actual);
        let expected_normalized = normalize(&self.expected);
        let matches = if self.expectation == "EXPECT" {
            actual_normalized == expected_normalized
        } else {
            match self.match_pattern(&expected_normalized, &actual_normalized) {
                Ok(matches) => matches,
                Err(error) => return Outcome::Failed(format!("PHPT matcher: {error}")),
            }
        };
        if matches {
            Outcome::Passed
        } else {
            Outcome::Failed(format!(
                "Expected {:?}, got {:?}",
                String::from_utf8_lossy(&self.expected),
                String::from_utf8_lossy(actual)
            ))
        }
    }

    fn match_pattern(&self, expected: &[u8], actual: &[u8]) -> Result<bool, String> {
        let binary = self.reference.as_ref().ok_or("Missing reference PHP")?;
        let mut child = Command::new(binary)
            .args([
                "-n",
                "-r",
                include_str!("phpt_match.php"),
                &self.expectation,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        // Length framing preserves arbitrary bytes, including NULs and newlines.
        // Patterns are input data, never interpolated into PHP source or shell code.
        let written = (|| -> std::io::Result<()> {
            let mut stdin = child.stdin.take().unwrap();
            writeln!(stdin, "{}", expected.len())?;
            stdin.write_all(expected)?;
            stdin.write_all(actual)
        })();
        let output = child
            .wait_with_output()
            .map_err(|error| error.to_string())?;
        written.map_err(|error| error.to_string())?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(format!(
                "{}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        match output.stdout.as_slice() {
            b"1" => Ok(true),
            b"0" => Ok(false),
            _ => Err("Invalid matcher response".into()),
        }
    }
}
fn normalize(bytes: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos..].starts_with(b"\r\n") {
            pos += 1;
        }
        normalized.push(bytes[pos]);
        pos += 1;
    }
    // Match run-tests.php's trim() character set, including NUL but not form feed.
    let trim = |b: &u8| b" \t\n\r\0\x0b".contains(b);
    let start = normalized
        .iter()
        .position(|b| !trim(b))
        .unwrap_or(normalized.len());
    let end = normalized
        .iter()
        .rposition(|b| !trim(b))
        .map(|p| p + 1)
        .unwrap_or(start);
    normalized[start..end].to_vec()
}
