use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use expectrl::Expect;
use expectrl::session::OsSession;
use tempfile::TempDir;

const ARROW_DOWN: &str = "\x1b[B";
const ENTER: &str = "\r";
const ESC: &str = "\x1b";
const TIMEOUT: Duration = Duration::from_secs(30);
const KEY_DELAY: Duration = Duration::from_millis(10);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct TerminalSessionBuilder<'a> {
    bin_path: &'a Path,
    workspace: &'a TempDir,
    args: &'a [&'a str],
    env: Vec<(OsString, OsString)>,
}

impl TerminalSessionBuilder<'_> {
    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, val: impl Into<OsString>) -> Self {
        self.env.push((key.into(), val.into()));
        self
    }

    pub fn spawn(self) -> TerminalSession {
        let mut cmd = Command::new(self.bin_path);
        cmd.args(self.args);
        cmd.current_dir(self.workspace.path());
        cmd.env("CARGO_CHANGESET_FORCE_TTY", "1");
        for (key, val) in self.env {
            cmd.env(key, val);
        }
        let pty = OsSession::spawn(cmd).expect("failed to spawn session");
        TerminalSession {
            pty,
            vt: vt100::Parser::new(24, 120, 100),
        }
    }
}

pub struct TerminalSession {
    pty: OsSession,
    vt: vt100::Parser,
}

impl TerminalSession {
    pub fn builder<'a>(
        bin_path: &'a Path,
        workspace: &'a TempDir,
        args: &'a [&'a str],
    ) -> TerminalSessionBuilder<'a> {
        TerminalSessionBuilder {
            bin_path,
            workspace,
            args,
            env: Vec::new(),
        }
    }

    pub fn spawn(bin_path: &Path, workspace: &TempDir, args: &[&str]) -> Self {
        Self::builder(bin_path, workspace, args).spawn()
    }

    fn poll(&mut self) {
        let mut buf = [0u8; 4096];
        loop {
            match self.pty.try_read(&mut buf) {
                Ok(n) if n > 0 => self.vt.process(&buf[..n]),
                _ => break,
            }
        }
    }

    fn screen(&mut self) -> String {
        self.poll();
        let raw = self.vt.screen().contents();
        raw.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end_matches('\n')
            .to_owned()
    }

    pub fn debug_print_screen(&mut self) {
        eprintln!("=== PTY screen ===\n{}\n==================", self.screen());
    }

    pub fn wait_for(&mut self, needle: &str) -> &mut Self {
        let start = Instant::now();
        loop {
            self.poll();
            if self.vt.screen().contents().contains(needle) {
                return self;
            }
            if start.elapsed() > TIMEOUT {
                self.debug_print_screen();
                panic!("Timed out waiting for {needle:?}");
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    pub fn wait_for_exit(&mut self) {
        let start = Instant::now();
        let mut buf = [0u8; 4096];
        loop {
            match self.pty.try_read(&mut buf) {
                Ok(0) => return,
                Ok(n) => self.vt.process(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Other => return,
                Err(_) => {
                    if start.elapsed() > TIMEOUT {
                        return;
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
            }
        }
    }

    pub fn assert_screen(&mut self, message: &str, expected: &str) {
        let actual = self.screen();
        let expected_trimmed = expected
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end_matches('\n')
            .to_owned();
        assert_eq!(actual, expected_trimmed, "{message}");
    }

    pub fn assert_screen_starts_with(&mut self, message: &str, prefix: &str) {
        let actual = self.screen();
        let prefix_trimmed = prefix
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end_matches('\n')
            .to_owned();
        assert!(
            actual.starts_with(&prefix_trimmed),
            "{message}\nexpected screen to start with:\n{prefix_trimmed}\n\nbut got:\n{actual}"
        );
    }

    pub fn assert_screen_ends_with(&mut self, message: &str, suffix: &str) {
        let actual = self.screen();
        let suffix_trimmed = suffix
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end_matches('\n')
            .to_owned();
        assert!(
            actual.ends_with(&suffix_trimmed),
            "{message}\nexpected screen to end with:\n{suffix_trimmed}\n\nbut got:\n{actual}"
        );
    }

    pub fn select_item(&mut self, index: usize) -> &mut Self {
        for _ in 0..=index {
            self.pty.send(ARROW_DOWN).expect("send arrow-down key");
            std::thread::sleep(KEY_DELAY);
        }
        self.pty.send(ENTER).expect("send enter key");
        self
    }

    pub fn toggle_item(&mut self, index: usize) -> &mut Self {
        for _ in 0..index {
            self.pty.send(ARROW_DOWN).expect("send arrow-down key");
            std::thread::sleep(KEY_DELAY);
        }
        self.pty.send(" ").expect("send space to toggle item");
        self
    }

    pub fn cancel(&mut self) -> &mut Self {
        self.pty.send(ESC).expect("send escape key");
        self
    }

    pub fn ctrl_c(&mut self) -> &mut Self {
        self.pty.send("\x03").expect("send Ctrl+C");
        self
    }

    pub fn send_raw(&mut self, bytes: &str) -> &mut Self {
        self.pty.send(bytes).expect("send raw bytes to PTY");
        self
    }

    pub fn send_line(&mut self, text: &str) -> &mut Self {
        self.pty.send(text).expect("send text to PTY");
        self.pty.send("\n").expect("send newline to PTY");
        self
    }

    pub fn type_line(&mut self, text: &str) -> &mut Self {
        self.pty.send(text).expect("send text to PTY");
        self.pty.send(ENTER).expect("send enter key");
        self
    }

    pub fn confirm(&mut self) -> &mut Self {
        self.pty.send(ENTER).expect("send enter key");
        self
    }
}
