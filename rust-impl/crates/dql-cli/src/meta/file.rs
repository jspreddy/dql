use crate::session::Session;
use std::collections::HashMap;
use std::fs;

pub fn handle(
    session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
) -> Result<(), String> {
    let filename = args
        .first()
        .ok_or_else(|| "file requires a path".to_string())?;
    let contents = fs::read_to_string(filename).map_err(|err| err.to_string())?;
    let output_config = session.config.output_config();
    let mut backend = dql_output::DisplayMode::from_name(&session.config.display)
        .unwrap_or(dql_output::DisplayMode::Stdout)
        .backend();
    for line in contents.lines() {
        if let Some(result) = session
            .engine
            .execute_fragment(line)
            .map_err(|err| err.to_string())?
        {
            dql_output::render_result(&result, &output_config, backend.as_mut())
                .map_err(|err| err.to_string())?;
        }
    }
    backend.finish().map_err(|err| err.to_string())?;
    Ok(())
}
