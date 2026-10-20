//! Local web UI served by `dqlrs --web`.
//!
//! Browser assets live in `dqlrs-web/static` and are compiled into the binary.
//! A checkout of that directory is preferred at runtime so UI edits show up
//! without a rebuild. The installed binary falls back to the embedded copy.

mod environment;
mod files;
mod query_cache;
mod server;
pub(crate) mod shape;

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::session::Session;

pub fn run(session: Session, root: PathBuf, listen: SocketAddr) -> color_eyre::Result<()> {
    server::serve(session, root, listen).map_err(|err| color_eyre::eyre::eyre!("{err}"))
}
