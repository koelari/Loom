#![allow(dead_code)]
use std::{
    fs,
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use tempfile::TempDir;

pub struct Fixture {
    pub temp: TempDir,
    pub data: PathBuf,
    pub home: PathBuf,
    pub work: PathBuf,
    pub bin: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let temp = tempfile::Builder::new()
            .prefix("loom tests ' ")
            .tempdir()
            .unwrap();
        let home = temp.path().join("home");
        let data = home.join(".loom");
        let work = temp.path().join("project");
        let bin = temp.path().join("bin");
        for path in [&data, &home, &work, &bin] {
            fs::create_dir_all(path).unwrap();
        }
        symlink(env!("CARGO_BIN_EXE_loom"), bin.join("loom")).unwrap();
        Self {
            temp,
            data,
            home,
            work,
            bin,
        }
    }
    pub fn tool(&self, tool: &str, version: &str, declaration: &str) -> PathBuf {
        let root = self.data.join("tools").join(tool);
        let version_dir = root.join(version);
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(root.join("loom.toml"), declaration).unwrap();
        version_dir
    }
    pub fn command(&self, binary: impl AsRef<Path>) -> Command {
        let mut command = Command::new(binary.as_ref());
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .env("SHELL", "/bin/bash")
            .env("TERM", "dumb")
            .env(
                "INIT",
                Path::new(env!("CARGO_MANIFEST_DIR")).join("shell/init.sh"),
            )
            .env("DATA", &self.data)
            .current_dir(&self.work);
        command
    }
    pub fn loom(&self, args: &[&str]) -> Output {
        self.command(env!("CARGO_BIN_EXE_loom"))
            .args(args)
            .output()
            .unwrap()
    }
    pub fn shell(&self, shell: &str, script: &str) -> Output {
        let mut command = self.command(shell);
        if shell.ends_with("bash") {
            command.args(["--noprofile", "--norc"]);
        } else {
            command.arg("-f");
        }
        command.args(["-c", script]).output().unwrap()
    }
    pub fn session(&self, shell: &str, args: &[&str], input: &str) -> Output {
        let mut command = self.command(env!("CARGO_BIN_EXE_loom"));
        command
            .env("SHELL", shell)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}
pub fn executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
pub fn success(output: &Output) -> String {
    assert!(
        output.status.success(),
        "status: {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}
