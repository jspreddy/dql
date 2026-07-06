use std::io::{self, Write};
use std::process::{Command, Stdio};

pub trait DisplayBackend {
    fn writer(&mut self) -> Box<dyn Write + '_>;
    fn write_line(&mut self, line: &str) -> io::Result<()> {
        let mut writer = self.writer();
        writeln!(writer, "{line}")?;
        writer.flush()
    }
    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct StdoutDisplay;

impl DisplayBackend for StdoutDisplay {
    fn writer(&mut self) -> Box<dyn Write + '_> {
        Box::new(io::stdout())
    }
}

pub fn stdout_display() -> StdoutDisplay {
    StdoutDisplay
}

pub struct LessDisplay {
    buffer: Vec<u8>,
}

impl LessDisplay {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
        }
    }
}

impl Default for LessDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayBackend for LessDisplay {
    fn writer(&mut self) -> Box<dyn Write + '_> {
        Box::new(&mut self.buffer)
    }

    fn finish(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let mut child = Command::new("less")
            .args(["-FXR"])
            .stdin(Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&self.buffer)?;
        }
        child.wait()?;
        Ok(())
    }
}

pub fn less_display() -> LessDisplay {
    LessDisplay::new()
}

pub enum DisplayMode {
    Stdout,
    Less,
}

impl DisplayMode {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "stdout" => Some(Self::Stdout),
            "less" => Some(Self::Less),
            _ => None,
        }
    }

    pub fn backend(&self) -> Box<dyn DisplayBackend> {
        match self {
            Self::Stdout => Box::new(stdout_display()),
            Self::Less => Box::new(less_display()),
        }
    }
}
