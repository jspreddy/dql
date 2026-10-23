//! Local web UI served by `dqlrs --web`.
//!
//! Browser assets live in `dqlrs-web/static` and are compiled into the binary.
//! A checkout of that directory is preferred at runtime so UI edits show up
//! without a rebuild. The installed binary falls back to the embedded copy.

mod environment;
pub(crate) mod files;
pub(crate) mod lint;
mod presence;
mod query_cache;
pub(crate) mod search;
mod server;
pub(crate) mod shape;

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::session::Session;

pub fn run(
    session: Session,
    root: PathBuf,
    listen: SocketAddr,
    mcp: bool,
) -> color_eyre::Result<()> {
    server::serve(session, root, listen, mcp).map_err(|err| color_eyre::eyre::eyre!("{err}"))
}
