mod common;
use common::*;
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    process::{Child, Stdio},
    time::{Duration, Instant},
};

struct Terminal {
    master: File,
    child: Child,
    buffer: Vec<u8>,
    initial_flags: libc::tcflag_t,
}

impl Terminal {
    fn start(f: &Fixture) -> Self {
        let (mut master, mut slave) = (-1, -1);
        let size = libc::winsize {
            ws_row: 12,
            ws_col: 90,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: valid output pointers; openpty initializes the owned descriptors.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &size,
                )
            },
            0
        );
        // SAFETY: these fresh descriptors each receive exactly one owner.
        let master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        let initial_flags = Self::flags(&master);
        let child = {
            let mut command = f.command(env!("CARGO_BIN_EXE_loom"));
            command
                .arg("init")
                .env("TERM", "xterm-256color")
                .stdin(Stdio::from(slave.try_clone().unwrap()))
                .stdout(Stdio::from(slave.try_clone().unwrap()))
                .stderr(Stdio::from(slave));
            // SAFETY: the callback only invokes async-signal-safe OS functions.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            command.spawn().unwrap()
        };
        Self {
            master,
            child,
            buffer: Vec::new(),
            initial_flags,
        }
    }
    fn flags(file: &File) -> libc::tcflag_t {
        let mut attributes = std::mem::MaybeUninit::<libc::termios>::uninit();
        // SAFETY: tcgetattr writes the termios structure on success.
        assert_eq!(
            unsafe { libc::tcgetattr(file.as_raw_fd(), attributes.as_mut_ptr()) },
            0
        );
        unsafe { attributes.assume_init() }.c_lflag
    }
    fn send(&mut self, keys: &[u8]) {
        // Wait for the menu reader to enter raw mode so Ctrl+C is delivered as
        // a key rather than as a terminal-generated signal during a redraw.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Self::flags(&self.master) & libc::ICANON != 0 {
            assert!(Instant::now() < deadline, "menu did not start reading keys");
            std::thread::sleep(Duration::from_millis(2));
        }
        self.master.write_all(keys).unwrap();
    }
    fn expect(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(position) = self
                .buffer
                .windows(text.len())
                .position(|window| window == text.as_bytes())
            {
                self.buffer.drain(..position + text.len());
                return;
            }
            assert!(
                Instant::now() < deadline,
                "missing {text:?}; terminal output: {}",
                String::from_utf8_lossy(&self.buffer)
            );
            let mut poll = libc::pollfd {
                fd: self.master.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll receives one valid pollfd structure.
            if unsafe { libc::poll(&mut poll, 1, 50) } > 0 {
                let mut data = [0; 4096];
                match self.master.read(&mut data) {
                    Ok(count) if count > 0 => self.buffer.extend_from_slice(&data[..count]),
                    other => panic!(
                        "terminal closed while waiting for {text:?}: {other:?}; {}",
                        String::from_utf8_lossy(&self.buffer)
                    ),
                }
            }
        }
    }
    fn finish(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "{status}");
                break;
            }
            assert!(Instant::now() < deadline, "initializer did not exit");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            Self::flags(&self.master),
            self.initial_flags,
            "terminal flags were not restored"
        );
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn arrow_space_selection_and_version_choice_write_the_expected_project() {
    let f = Fixture::new();
    f.tool("node", "22.23.3", "[default]\npaths=['bin']");
    f.tool("node", "22.1.0", "[default]\npaths=['bin']");
    f.tool("other", "1", "[default]\npaths=['bin']");
    let mut terminal = Terminal::start(&f);
    terminal.expect("Select tools & versions");
    // Enter node's children, select its older version, then select other.
    terminal.send(b"\x1b[C\x1b[B \x1b[B \r");
    terminal.expect("Review project configuration");
    terminal.send(b"\r");
    terminal.expect("Wrote");
    terminal.finish();
    let parsed: toml::Table =
        toml::from_str(&fs::read_to_string(f.work.join(".loom")).unwrap()).unwrap();
    assert_eq!(parsed["node"].as_str(), Some("22.1.0"));
    assert_eq!(parsed["other"].as_str(), Some("1"));
}

#[test]
fn escape_and_ctrl_c_cancel_without_writing_and_restore_terminal() {
    let f = Fixture::new();
    f.tool("node", "22", "[default]\npaths=['bin']");
    for key in [b"\x1b".as_slice(), b"\x03".as_slice()] {
        let mut terminal = Terminal::start(&f);
        terminal.expect("Select tools & versions");
        terminal.send(key);
        terminal.expect("Initialization cancelled.");
        terminal.finish();
        assert!(!f.work.join(".loom").exists());
    }
}

#[test]
fn empty_selection_is_rejected_and_existing_file_defaults_to_cancel() {
    let f = Fixture::new();
    f.tool("node", "22", "[default]\npaths=['bin']");
    fs::write(f.work.join(".loom"), "# original\nnode='22'\n").unwrap();
    let mut terminal = Terminal::start(&f);
    terminal.expect("Select tools & versions");
    terminal.send(b"\r");
    terminal.expect("Select a version with Space before continuing.");
    terminal.send(b" \r");
    terminal.expect("Review project configuration");
    terminal.send(b"\r");
    terminal.expect("Initialization cancelled.");
    terminal.finish();
    assert_eq!(
        fs::read_to_string(f.work.join(".loom")).unwrap(),
        "# original\nnode='22'\n"
    );
}

#[test]
fn long_tree_scrolls_and_space_unchecks_a_tool() {
    let f = Fixture::new();
    for index in 0..20 {
        f.tool(&format!("tool{index:02}"), "1", "[default]\npaths=['bin']");
    }
    let mut terminal = Terminal::start(&f);
    terminal.expect("Select tools & versions");
    // Toggle the first tool on/off, collapse it, then select the last tool.
    terminal.send(b"  \x1b[D\x1b[F \r");
    terminal.expect("Review project configuration");
    terminal.send(b"\r");
    terminal.expect("Wrote");
    terminal.finish();
    let parsed: toml::Table =
        toml::from_str(&fs::read_to_string(f.work.join(".loom")).unwrap()).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed["tool19"].as_str(), Some("1"));
}

#[test]
fn replacing_a_selected_version_and_collapsing_preserves_one_version_per_tool() {
    let f = Fixture::new();
    f.tool("node", "22.23.3", "[default]\npaths=['bin']");
    f.tool("node", "22.1.0", "[default]\npaths=['bin']");
    let mut terminal = Terminal::start(&f);
    terminal.expect("Select tools & versions");
    // Opening a folder does not select it. Choose the first version, switch to
    // its sibling, then move to the parent and collapse it before reviewing.
    terminal.send(b"\x1b[C \x1b[B \x1b[D\x1b[D\r");
    terminal.expect("Review project configuration");
    terminal.send(b"\r");
    terminal.expect("Wrote");
    terminal.finish();
    let parsed: toml::Table =
        toml::from_str(&fs::read_to_string(f.work.join(".loom")).unwrap()).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed["node"].as_str(), Some("22.1.0"));
}
