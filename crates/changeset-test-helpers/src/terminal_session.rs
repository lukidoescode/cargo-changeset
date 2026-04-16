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
const KEY_DELAY: Duration = Duration::from_millis(50);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct TerminalSession {
    pty: OsSession,
    vt: vt100::Parser,
}

impl TerminalSession {
    pub fn spawn(bin_path: &Path, workspace: &TempDir, args: &[&str]) -> Self {
        let mut cmd = Command::new(bin_path);
        cmd.args(args);
        cmd.current_dir(workspace.path());
        cmd.env("CARGO_CHANGESET_FORCE_TTY", "1");
        let pty = OsSession::spawn(cmd).expect("failed to spawn session");
        Self {
            pty,
            vt: vt100::Parser::new(24, 120, 100),
        }
    }

    pub fn poll(&mut self) {
        let mut buf = [0u8; 4096];
        loop {
            match self.pty.try_read(&mut buf) {
                Ok(n) if n > 0 => self.vt.process(&buf[..n]),
                _ => break,
            }
        }
    }

    pub fn screen(&mut self) -> String {
        self.poll();
        let raw = self.vt.screen().contents();
        raw.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end_matches('\n')
            .to_owned()
    }

    pub fn wait_for(&mut self, needle: &str) -> &mut Self {
        let start = Instant::now();
        loop {
            self.poll();
            if self.vt.screen().contents().contains(needle) {
                return self;
            }
            assert!(
                start.elapsed() <= TIMEOUT,
                "Timed out waiting for {needle:?}\nScreen:\n{}",
                self.screen()
            );
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

    pub fn select_item(&mut self, index: usize) -> &mut Self {
        for _ in 0..=index {
            self.pty.send(ARROW_DOWN).expect("send arrow-down key");
            std::thread::sleep(KEY_DELAY);
        }
        self.pty.send(ENTER).expect("send enter key");
        self
    }

    pub fn cancel(&mut self) -> &mut Self {
        self.pty.send(ESC).expect("send escape key");
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
