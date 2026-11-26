//! Clients for the installed runner's fixed Git reader and bounded command transport.
#![allow(unexpected_cfgs)]
use vstd::prelude::*;

verus! {
/// A complete response contains exactly the declared stdout and stderr bytes.
pub fn complete_frame(stdout: usize, stderr: usize, bytes: usize) -> (complete: bool)
    ensures complete == (stdout as int + stderr == bytes),
{
    stdout <= bytes && stderr == bytes - stdout
}
}

#[cfg(not(verus_keep_ghost))]
use crate::Delta;
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde::de::DeserializeOwned;
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
pub(crate) struct Snapshot {
    pub values: BTreeMap<String, Value>,
    pub originals: BTreeMap<String, String>,
    pub effective_path: String,
}

#[cfg(not(verus_keep_ghost))]
pub(crate) struct Reader {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    pub bound: bool,
}
#[cfg(not(verus_keep_ghost))]
impl Reader {
    pub fn open(root: &Path, header: Value) -> Result<Self, String> {
        let mut child = Command::new("chrono-harness")
            .arg("git-facts-v1")
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("chrono-harness git-facts-v1: {e}"))?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut reader = Self {
            child,
            input,
            output,
            bound: false,
        };
        #[derive(Deserialize)]
        struct Opened {
            bound: bool,
        }
        reader.bound = reader.call::<Opened>(header)?.bound;
        Ok(reader)
    }
    fn call<T: DeserializeOwned>(&mut self, command: Value) -> Result<T, String> {
        let stdin = self.input.as_mut().unwrap();
        serde_json::to_writer(&mut *stdin, &command).map_err(|e| e.to_string())?;
        stdin
            .write_all(b"\n")
            .and_then(|_| stdin.flush())
            .map_err(|e| e.to_string())?;
        let mut line = Vec::new();
        self.output
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if line.last() != Some(&b'\n') {
            return Err("E_PROTOCOL: incomplete Git facts response".into());
        }
        serde_json::from_slice::<Result<T, String>>(&line)
            .map_err(|e| format!("E_PROTOCOL: {e}"))?
    }
    pub fn snapshot(&mut self, endpoint: &str) -> Result<Snapshot, String> {
        self.call(json!({"op":"registry-snapshot","endpoint":endpoint}))
    }
    pub fn verify_tree(&mut self, endpoint: &str) -> Result<String, String> {
        self.call(json!({"op":"verify-tree","endpoint":endpoint}))
    }
    pub fn delta(&mut self) -> Result<Vec<Delta>, String> {
        self.call(json!({"op":"delta"}))
    }
    pub fn document(&mut self, endpoint: &str, path: &str) -> Result<Value, String> {
        self.call(json!({"op":"json-blob","endpoint":endpoint,"path":path}))
    }
    pub fn observation(&mut self) -> Result<Value, String> {
        self.call(json!({"op":"observation"}))
    }
    pub fn close(mut self) -> Result<(), String> {
        drop(self.input.take());
        let status = self.child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("git-facts-v1 exited {status}"))
        }
    }
}
#[cfg(not(verus_keep_ghost))]
impl Drop for Reader {
    fn drop(&mut self) {
        drop(self.input.take());
        let _ = self.child.wait();
    }
}

#[cfg(not(verus_keep_ghost))]
pub(crate) struct Program {
    path: PathBuf,
    digest: String,
}
#[cfg(not(verus_keep_ghost))]
impl Program {
    pub fn resolve(root: &Path, name: &str, search: Option<&str>) -> Result<Self, String> {
        use sha2::{Digest, Sha256};
        let env = search
            .map(std::ffi::OsString::from)
            .or_else(|| std::env::var_os("PATH"))
            .ok_or("PATH is missing")?;
        let root = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(root);
        let path = std::env::split_paths(&env)
            .map(|dir| root.join(dir).join(name))
            .find(|p| p.is_file())
            .ok_or_else(|| format!("executable not found: {name}"))?;
        let mut file = fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut hasher = digest_io::IoWrapper(Sha256::new());
        std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            digest: hex::encode(hasher.0.finalize()),
        })
    }
    pub fn run(
        &self,
        root: &Path,
        operation: &str,
        env: &BTreeMap<String, String>,
        policy: &Value,
        input: &[u8],
    ) -> Result<Vec<u8>, String> {
        let timeout = policy["timeout_seconds"]
            .as_u64()
            .ok_or("invalid protocol timeout")?;
        let limit = policy["stdout_limit_bytes"]
            .as_u64()
            .ok_or("invalid protocol output bound")?;
        let header = json!({"program":self.path,"args":[operation],"env":env,"timeout_seconds":timeout,"output_limit_bytes":limit,"expected_sha256":self.digest});
        let mut child = Command::new("chrono-harness")
            .arg("run-command-v1")
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("chrono-harness run-command-v1: {e}"))?;
        let mut send = child.stdin.take().unwrap();
        // The transport reads the complete frame before producing either output stream.
        let written = serde_json::to_writer(&mut send, &header)
            .map_err(|e| e.to_string())
            .and_then(|_| {
                send.write_all(b"\n")
                    .and_then(|_| send.write_all(input))
                    .map_err(|e| e.to_string())
            });
        drop(send);
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim_end().to_owned());
        }
        written?;
        #[derive(Deserialize)]
        struct Lengths {
            exit_code: i32,
            stdout_length: usize,
            stderr_length: usize,
        }
        let mut bytes = out.stdout.as_slice();
        let mut line = Vec::new();
        bytes
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        let lengths: Lengths = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
        if !complete_frame(lengths.stdout_length, lengths.stderr_length, bytes.len()) {
            return Err("E_PROTOCOL: command stream lengths differ".into());
        }
        let (stdout, stderr) = bytes.split_at(lengths.stdout_length);
        if lengths.exit_code != 0 {
            let message = std::str::from_utf8(stderr)
                .map_err(|e| e.to_string())?
                .trim_end();
            return Err(if message.is_empty() {
                format!("{operation} exited {}", lengths.exit_code)
            } else {
                message.into()
            });
        }
        Ok(stdout.to_vec())
    }
}
