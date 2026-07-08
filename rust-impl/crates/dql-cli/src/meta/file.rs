use crate::session::Session;
use std::collections::HashMap;
use std::fs;
use std::io::Write;

pub fn handle(
    session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
    out: &mut dyn Write,
    repl: bool,
) -> Result<(), String> {
    let filename = args
        .first()
        .ok_or_else(|| "file requires a path".to_string())?;
    let contents = fs::read_to_string(filename).map_err(|err| err.to_string())?;
    let output_config = session.config.output_config();
    if repl {
        let mut buffer = Vec::new();
        for line in contents.lines() {
            if let Some(result) = session
                .engine
                .execute_fragment(line)
                .map_err(|err| err.to_string())?
            {
                render_to_buffer(&result, &output_config, &mut buffer)?;
            }
        }
        out.write_all(&buffer).map_err(|err| err.to_string())?;
        return Ok(());
    }
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

fn render_to_buffer(
    result: &dql_engine::StatementResult,
    output_config: &dql_output::OutputConfig,
    buffer: &mut Vec<u8>,
) -> Result<(), String> {
    struct BufferBackend<'a>(&'a mut Vec<u8>);

    impl Write for BufferBackend<'_> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl dql_output::DisplayBackend for BufferBackend<'_> {
        fn writer(&mut self) -> Box<dyn Write + '_> {
            Box::new(self)
        }
    }

    let mut backend = BufferBackend(buffer);
    dql_output::render_result(result, output_config, &mut backend).map_err(|err| err.to_string())
}
