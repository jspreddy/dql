use dql_engine::{Engine, EngineError, SdkBackend, SdkConfig, StatementResult};

pub fn local_config() -> SdkConfig {
    SdkConfig::local(
        std::env::var("AWS_REGION").unwrap_or_else(|_| "us-west-1".to_string()),
        std::env::var("DQL_LOCAL_HOST").unwrap_or_else(|_| "localhost".to_string()),
        std::env::var("DQL_LOCAL_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(8000),
    )
}

pub fn local_available() -> bool {
    SdkBackend::connect(local_config()).is_ok()
}

pub fn skip_if_no_local() -> bool {
    if local_available() {
        false
    } else {
        eprintln!("skipping: DynamoDB Local not available");
        true
    }
}

pub fn local_engine() -> Result<Engine<SdkBackend>, EngineError> {
    let backend = SdkBackend::connect(local_config()).map_err(|err| err)?;
    Ok(Engine::new(backend))
}

pub struct LocalHarness {
    pub engine: Engine<SdkBackend>,
}

impl LocalHarness {
    pub fn try_new() -> Option<Self> {
        let mut engine = local_engine().ok()?;
        cleanup_all_tables(&mut engine);
        Some(Self { engine })
    }

    pub fn query(&mut self, command: &str) -> Result<StatementResult, EngineError> {
        self.engine.execute(command)
    }

    pub fn unique_table_name(prefix: &str) -> String {
        format!(
            "{prefix}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }
}

impl Drop for LocalHarness {
    fn drop(&mut self) {
        cleanup_all_tables(&mut self.engine);
    }
}

fn cleanup_all_tables(engine: &mut Engine<SdkBackend>) {
    if let Ok(names) = engine.table_names() {
        for name in names {
            let _ = engine.execute(&format!("DROP TABLE {name} IF EXISTS"));
        }
    }
}
