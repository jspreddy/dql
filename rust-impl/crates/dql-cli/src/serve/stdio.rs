use super::{run_framed, Control};
use crate::session::Session;
use std::io::{self, BufReader};

pub fn run(session: &mut Session) -> io::Result<()> {
    let stdin = io::stdin();
    match run_framed(BufReader::new(stdin.lock()), io::stdout(), session)? {
        Control::Shutdown | Control::Disconnect => Ok(()),
    }
}
