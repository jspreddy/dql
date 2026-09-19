use super::{run_framed, Control};
use crate::session::Session;
use std::io::{self, BufReader};

pub fn run(session: &mut Session) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    match run_framed(BufReader::new(stdin.lock()), stdout.lock(), session)? {
        Control::Shutdown | Control::Disconnect => Ok(()),
    }
}
