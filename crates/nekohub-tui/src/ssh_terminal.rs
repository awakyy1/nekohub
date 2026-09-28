use std::{
    fmt,
    io::{Read, Write},
    thread,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::mpsc;

const SCROLLBACK_LINES: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalPhase {
    Idle,
    Password,
    Booting,
    Connected,
    Closed(String),
    Error(String),
}

pub struct TerminalPanel {
    parser: vt100::Parser,
    pub phase: TerminalPhase,
    pub password: String,
    pub target: String,
    generation: u64,
    boot_started_at: u64,
}

impl fmt::Debug for TerminalPanel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TerminalPanel")
            .field("phase", &self.phase)
            .field("target", &self.target)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Default for TerminalPanel {
    fn default() -> Self {
        Self {
            parser: vt100::Parser::new(24, 80, SCROLLBACK_LINES),
            phase: TerminalPhase::Idle,
            password: String::new(),
            target: String::new(),
            generation: 0,
            boot_started_at: 0,
        }
    }
}

impl TerminalPanel {
    pub fn begin_password(&mut self, target: &str) {
        self.password.clear();
        target.clone_into(&mut self.target);
        self.phase = TerminalPhase::Password;
    }

    pub fn push_password_character(&mut self, character: char) {
        if self.password.chars().count() < 256 && !character.is_control() {
            self.password.push(character);
        }
    }

    pub fn pop_password_character(&mut self) {
        self.password.pop();
    }

    pub fn start(&mut self, rows: u16, cols: u16, animation_tick: u64) -> TerminalRequest {
        self.generation = self.generation.wrapping_add(1);
        self.parser = vt100::Parser::new(rows, cols, SCROLLBACK_LINES);
        self.phase = TerminalPhase::Booting;
        self.boot_started_at = animation_tick;
        TerminalRequest {
            target: self.target.clone(),
            password: (!self.password.is_empty()).then(|| std::mem::take(&mut self.password)),
            generation: self.generation,
        }
    }

    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.password.clear();
        self.phase = TerminalPhase::Idle;
    }

    pub fn advance_boot(&mut self, animation_tick: u64) {
        if self.phase == TerminalPhase::Booting
            && animation_tick.wrapping_sub(self.boot_started_at) >= 42
        {
            self.phase = TerminalPhase::Connected;
        }
    }

    pub fn process_output(&mut self, generation: u64, bytes: &[u8]) {
        if generation == self.generation {
            self.parser.process(bytes);
        }
    }

    pub fn finish(&mut self, generation: u64, status: String) {
        if generation == self.generation && !matches!(self.phase, TerminalPhase::Error(_)) {
            self.phase = TerminalPhase::Closed(status);
        }
    }

    pub fn fail(&mut self, generation: u64, message: String) {
        if generation == self.generation {
            self.phase = TerminalPhase::Error(message);
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if self.parser.screen().size() != (rows, cols) {
            self.parser.screen_mut().set_size(rows, cols);
        }
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn boot_step(&self, animation_tick: u64) -> usize {
        usize::try_from(animation_tick.wrapping_sub(self.boot_started_at) / 10)
            .unwrap_or_default()
            .min(3)
    }
}

pub struct TerminalRequest {
    pub target: String,
    pub password: Option<String>,
    pub generation: u64,
}

#[derive(Debug)]
pub enum TerminalEvent {
    Output { generation: u64, bytes: Vec<u8> },
    Exit { generation: u64, status: String },
    Error { generation: u64, message: String },
}

pub struct SshTerminalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

impl SshTerminalSession {
    pub fn spawn(
        request: TerminalRequest,
        rows: u16,
        cols: u16,
        event_tx: mpsc::UnboundedSender<TerminalEvent>,
    ) -> Result<Self, String> {
        let TerminalRequest {
            target,
            password,
            generation,
        } = request;
        let pair = native_pty_system()
            .openpty(pty_size(rows, cols))
            .map_err(|error| format!("Could not create terminal: {error}"))?;
        let mut command = if let Some(password) = password.as_deref() {
            let mut command = CommandBuilder::new("sshpass");
            command.args(["-e", "ssh"]);
            command.env("SSHPASS", password);
            command
        } else {
            CommandBuilder::new("ssh")
        };
        command.args([
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=3",
            target.as_str(),
        ]);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");

        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("Could not start SSH: {error}"))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("Could not read terminal: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("Could not open terminal input: {error}"))?;
        let killer = child.clone_killer();
        let reader_tx = event_tx.clone();
        thread::spawn(move || {
            let mut buffer = [0_u8; 8_192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        if reader_tx
                            .send(TerminalEvent::Output {
                                generation,
                                bytes: buffer[..read].to_vec(),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(error) => {
                        if error.raw_os_error() == Some(5) {
                            break;
                        }
                        let _ = reader_tx.send(TerminalEvent::Error {
                            generation,
                            message: format!("Terminal read failed: {error}"),
                        });
                        break;
                    }
                }
            }
        });
        thread::spawn(move || match child.wait() {
            Ok(status) => {
                let _ = event_tx.send(TerminalEvent::Exit {
                    generation,
                    status: status.to_string(),
                });
            }
            Err(error) => {
                let _ = event_tx.send(TerminalEvent::Error {
                    generation,
                    message: format!("Could not wait for SSH: {error}"),
                });
            }
        });

        Ok(Self {
            master: pair.master,
            writer,
            killer,
        })
    }

    pub fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.writer
            .write_all(bytes)
            .and_then(|()| self.writer.flush())
            .map_err(|error| format!("Could not write to terminal: {error}"))
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        self.master
            .resize(pty_size(rows, cols))
            .map_err(|error| format!("Could not resize terminal: {error}"))
    }
}

impl Drop for SshTerminalSession {
    fn drop(&mut self) {
        let _ = self.killer.kill();
    }
}

pub fn embedded_size(outer_cols: u16, outer_rows: u16) -> (u16, u16) {
    (
        outer_rows.saturating_sub(16).max(2),
        outer_cols.saturating_sub(35).max(10),
    )
}

pub fn encode_key(key: KeyEvent) -> Vec<u8> {
    let mut encoded = Vec::new();
    if key.modifiers.contains(KeyModifiers::ALT) {
        encoded.push(0x1b);
    }
    match key.code {
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let uppercase = character.to_ascii_uppercase();
            if uppercase.is_ascii() {
                encoded.push((uppercase as u8) & 0x1f);
            }
        }
        KeyCode::Char(character) => {
            let mut buffer = [0_u8; 4];
            encoded.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
        }
        KeyCode::Enter => encoded.push(b'\r'),
        KeyCode::Backspace => encoded.push(0x7f),
        KeyCode::Tab => encoded.push(b'\t'),
        KeyCode::BackTab => encoded.extend_from_slice(b"\x1b[Z"),
        KeyCode::Esc => encoded.push(0x1b),
        KeyCode::Up => encoded.extend_from_slice(b"\x1b[A"),
        KeyCode::Down => encoded.extend_from_slice(b"\x1b[B"),
        KeyCode::Right => encoded.extend_from_slice(b"\x1b[C"),
        KeyCode::Left => encoded.extend_from_slice(b"\x1b[D"),
        KeyCode::Home => encoded.extend_from_slice(b"\x1b[H"),
        KeyCode::End => encoded.extend_from_slice(b"\x1b[F"),
        KeyCode::Insert => encoded.extend_from_slice(b"\x1b[2~"),
        KeyCode::Delete => encoded.extend_from_slice(b"\x1b[3~"),
        KeyCode::PageUp => encoded.extend_from_slice(b"\x1b[5~"),
        KeyCode::PageDown => encoded.extend_from_slice(b"\x1b[6~"),
        KeyCode::F(number @ 1..=4) => {
            encoded.extend_from_slice([0x1b, b'O', b'P' + number - 1].as_slice());
        }
        KeyCode::F(number @ 5..=12) => {
            let code = [15, 17, 18, 19, 20, 21, 23, 24][usize::from(number - 5)];
            encoded.extend_from_slice(format!("\x1b[{code}~").as_bytes());
        }
        _ => {}
    }
    encoded
}

pub fn is_terminal_close_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::F(10))
        || matches!(key.code, KeyCode::Char(']')) && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_terminal_keys() {
        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            vec![3]
        );
        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            b"\x1b[A"
        );
        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::Char('ç'), KeyModifiers::NONE)),
            "ç".as_bytes()
        );
    }

    #[test]
    fn recognizes_reliable_terminal_close_shortcuts() {
        assert!(is_terminal_close_key(KeyEvent::new(
            KeyCode::F(10),
            KeyModifiers::NONE
        )));
        assert!(is_terminal_close_key(KeyEvent::new(
            KeyCode::Char(']'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_terminal_close_key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn password_is_removed_when_connection_starts() {
        let mut panel = TerminalPanel::default();
        panel.begin_password("server");
        for character in "secret".chars() {
            panel.push_password_character(character);
        }
        let request = panel.start(20, 60, 1);
        assert_eq!(request.password.as_deref(), Some("secret"));
        assert!(panel.password.is_empty());
        assert_eq!(panel.phase, TerminalPhase::Booting);
        panel.cancel();
        panel.finish(request.generation, "stale session".into());
        assert_eq!(panel.phase, TerminalPhase::Idle);
    }
}
