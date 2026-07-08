use crate::session::{Session, REGIONS};
use std::collections::HashMap;
use std::io::Write;

pub fn handle_use(
    session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
    _out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    let region = args
        .first()
        .ok_or_else(|| "use requires a region".to_string())?;
    if !REGIONS.contains(&region.as_str()) && session.local_endpoint.is_none() {
        // Allow unknown regions for parity with flexible connect
    }
    session.region = region.clone();
    session
        .engine
        .reconnect(
            &session.region,
            session.local_endpoint.clone(),
            session.config.allow_select_scan,
        )
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn handle_local(
    session: &mut Session,
    args: &[String],
    kwargs: &HashMap<String, String>,
    out: &mut dyn Write,
    repl: bool,
) -> Result<(), String> {
    let host = kwargs
        .get("host")
        .cloned()
        .or_else(|| args.first().cloned())
        .unwrap_or_else(|| "localhost".to_string());
    let port = kwargs
        .get("port")
        .or_else(|| args.get(1))
        .map(|value| {
            value
                .parse::<u16>()
                .map_err(|_| format!("invalid port {value}"))
        })
        .transpose()?
        .unwrap_or(8000);
    if host == "off" {
        session.local_endpoint = None;
    } else {
        session.local_endpoint = Some((host, port));
    }
    let region = session.region.clone();
    handle_use(
        session,
        std::slice::from_ref(&region),
        &HashMap::new(),
        out,
        repl,
    )
}
