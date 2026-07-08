use crate::config::CliConfig;
use dql_engine::{Engine, EngineError, FragmentEngine, SdkBackend, SdkConfig, StatementResult};
use dql_output::{render_result, DisplayMode, OutputConfig};
use std::io::{self, Write};

pub const REGIONS: &[&str] = &[
    "us-east-1",
    "us-west-2",
    "us-west-1",
    "eu-west-1",
    "eu-central-1",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-northeast-1",
    "sa-east-1",
];

#[allow(clippy::large_enum_variant)]
pub enum RuntimeEngine {
    Memory(FragmentEngine<dql_engine::MemoryBackend>),
    Remote(FragmentEngine<SdkBackend>),
}

impl RuntimeEngine {
    pub fn build(
        region: &str,
        host: Option<&str>,
        port: u16,
        allow_select_scan: bool,
    ) -> Result<Self, EngineError> {
        if let Some(host) = host {
            let backend = SdkBackend::connect(SdkConfig::local(region, host, port))?;
            Ok(Self::Remote(FragmentEngine::new(
                Engine::new(backend).with_allow_select_scan(allow_select_scan),
            )))
        } else {
            Ok(Self::Memory(FragmentEngine::new(
                Engine::new(dql_engine::MemoryBackend::new())
                    .with_allow_select_scan(allow_select_scan),
            )))
        }
    }

    pub fn partial(&self) -> bool {
        match self {
            Self::Memory(engine) => engine.partial(),
            Self::Remote(engine) => engine.partial(),
        }
    }

    pub fn reset_fragment(&mut self) {
        match self {
            Self::Memory(engine) => engine.reset(),
            Self::Remote(engine) => engine.reset(),
        }
    }

    pub fn execute_fragment(
        &mut self,
        fragment: &str,
    ) -> Result<Option<StatementResult>, EngineError> {
        match self {
            Self::Memory(engine) => engine.execute(fragment),
            Self::Remote(engine) => engine.execute(fragment),
        }
    }

    pub fn execute_script(&mut self, input: &str) -> Result<StatementResult, EngineError> {
        match self {
            Self::Memory(engine) => engine.inner_mut().execute(input),
            Self::Remote(engine) => engine.inner_mut().execute(input),
        }
    }

    pub fn region(&self) -> &str {
        match self {
            Self::Memory(_) => "memory",
            Self::Remote(engine) => engine.inner().backend().region(),
        }
    }

    pub fn reconnect(
        &mut self,
        region: &str,
        local: Option<(String, u16)>,
        allow_select_scan: bool,
    ) -> Result<(), EngineError> {
        let _ = allow_select_scan;
        match self {
            Self::Memory(_) => Ok(()),
            Self::Remote(engine) => {
                let config = if let Some((host, port)) = local {
                    SdkConfig::local(region, host, port)
                } else {
                    SdkConfig {
                        region: region.to_string(),
                        host: None,
                        port: None,
                        access_key: None,
                        secret_key: None,
                    }
                };
                engine.inner_mut().backend_mut().reconnect(config)?;
                engine.inner_mut().cached_descriptions.clear();
                Ok(())
            }
        }
    }

    pub fn session_identity(&self) -> String {
        match self {
            Self::Memory(_) => "memory".to_string(),
            Self::Remote(engine) => engine
                .inner()
                .backend()
                .session_identity()
                .unwrap_or_else(|_| "unknown".to_string()),
        }
    }

    pub fn with_memory_mut<R>(
        &mut self,
        f: impl FnOnce(&mut FragmentEngine<dql_engine::MemoryBackend>) -> R,
    ) -> Option<R> {
        match self {
            Self::Memory(engine) => Some(f(engine)),
            Self::Remote(_) => None,
        }
    }

    pub fn with_memory<R>(
        &self,
        f: impl FnOnce(&FragmentEngine<dql_engine::MemoryBackend>) -> R,
    ) -> Option<R> {
        match self {
            Self::Memory(engine) => Some(f(engine)),
            Self::Remote(_) => None,
        }
    }

    pub fn with_remote_mut<R>(
        &mut self,
        f: impl FnOnce(&mut FragmentEngine<SdkBackend>) -> R,
    ) -> Option<R> {
        match self {
            Self::Remote(engine) => Some(f(engine)),
            Self::Memory(_) => None,
        }
    }

    pub fn with_remote<R>(&self, f: impl FnOnce(&FragmentEngine<SdkBackend>) -> R) -> Option<R> {
        match self {
            Self::Remote(engine) => Some(f(engine)),
            Self::Memory(_) => None,
        }
    }

    pub fn list_tables(&self) -> Result<Vec<String>, EngineError> {
        match self {
            Self::Memory(engine) => engine.inner().list_tables(),
            Self::Remote(engine) => engine.inner().list_tables(),
        }
    }

    pub fn describe_all(
        &mut self,
        refresh: bool,
    ) -> Result<Vec<dql_models::TableMeta>, EngineError> {
        match self {
            Self::Memory(engine) => engine.inner_mut().describe_all(refresh),
            Self::Remote(engine) => engine.inner_mut().describe_all(refresh),
        }
    }

    pub fn describe(
        &mut self,
        table: &str,
        refresh: bool,
    ) -> Result<Option<dql_models::TableMeta>, EngineError> {
        match self {
            Self::Memory(engine) => engine.inner_mut().describe(table, refresh),
            Self::Remote(engine) => engine.inner_mut().describe(table, refresh),
        }
    }

    pub fn table_item_count(&self, table: &str) -> usize {
        match self {
            Self::Memory(engine) => engine.inner().table_item_count(table),
            Self::Remote(engine) => engine.inner().table_item_count(table),
        }
    }

    pub fn set_rate_limit_option(&mut self, limit: Option<dql_engine::RateLimit>) {
        match self {
            Self::Memory(engine) => engine.inner_mut().set_rate_limit_option(limit),
            Self::Remote(engine) => engine.inner_mut().set_rate_limit_option(limit),
        }
    }

    pub fn apply_allow_select_scan(&mut self, allow: bool) {
        match self {
            Self::Memory(engine) => engine.inner_mut().set_allow_select_scan(allow),
            Self::Remote(engine) => engine.inner_mut().set_allow_select_scan(allow),
        }
    }
}

pub struct Session {
    pub config: CliConfig,
    pub engine: RuntimeEngine,
    pub local_endpoint: Option<(String, u16)>,
    pub region: String,
    pub history: crate::history::HistoryManager,
    pub throttle: crate::throttle::TableLimits,
}

impl Session {
    pub fn new(args: &crate::args::CliArgs) -> Result<Self, EngineError> {
        let config = CliConfig::load();
        let mut throttle = crate::throttle::TableLimits::default();
        throttle.load(&config.throttle);
        let mut history = crate::history::HistoryManager::new();
        history.try_to_load_history();
        let engine = RuntimeEngine::build(
            &args.region,
            args.host.as_deref(),
            args.port,
            config.allow_select_scan,
        )?;
        Ok(Self {
            config,
            engine,
            local_endpoint: args.host.as_ref().map(|host| (host.clone(), args.port)),
            region: args.region.clone(),
            history,
            throttle,
        })
    }

    pub fn run_command(&mut self, command: &str, use_json: bool) -> Result<(), EngineError> {
        let mut output_config = if use_json {
            OutputConfig::for_json_command()
        } else {
            self.config.output_config()
        };
        output_config.silent = use_json;
        let mut backend = DisplayMode::Stdout.backend();
        let trimmed = command.trim();
        let is_meta = trimmed
            .split_once(char::is_whitespace)
            .map(|(cmd, _)| crate::meta::is_meta_command(cmd))
            .unwrap_or_else(|| crate::meta::is_meta_command(trimmed));
        if is_meta {
            self.dispatch_line(trimmed, &output_config, backend.as_mut())?;
        } else {
            self.apply_rate_limit()?;
            if let Some(result) = self.engine.execute_fragment(trimmed)? {
                render_result(&result, &output_config, backend.as_mut())
                    .map_err(|err| EngineError::Runtime(err.to_string()))?;
            }
            if self.engine.partial() {
                self.apply_rate_limit()?;
                if let Some(result) = self.engine.execute_fragment(";")? {
                    render_result(&result, &output_config, backend.as_mut())
                        .map_err(|err| EngineError::Runtime(err.to_string()))?;
                }
            }
        }
        backend
            .finish()
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        Ok(())
    }

    pub fn dispatch_line(
        &mut self,
        line: &str,
        output_config: &OutputConfig,
        backend: &mut dyn dql_output::DisplayBackend,
    ) -> Result<(), EngineError> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let mut writer = backend.writer();
        if let Some(result) = crate::meta::dispatch(self, trimmed, writer.as_mut(), false)? {
            drop(writer);
            render_result(&result, output_config, backend)
                .map_err(|err| EngineError::Runtime(err.to_string()))?;
        }
        Ok(())
    }

    pub fn apply_rate_limit(&mut self) -> Result<(), EngineError> {
        let limit = if self.throttle.is_active() {
            let tables = self.engine.describe_all(false)?;
            self.throttle.get_limiter(&tables)
        } else {
            None
        };
        self.engine.set_rate_limit_option(limit);
        Ok(())
    }
}

impl Session {
    pub fn write_prompt(&self, partial: bool, out: &mut dyn Write) -> io::Result<()> {
        if partial {
            write!(out, "   | ")?;
        } else if let Some((host, port)) = &self.local_endpoint {
            writeln!(out)?;
            write!(out, "({host}:{port}) {}\n   ===> ", self.region)?;
        } else {
            writeln!(out)?;
            write!(out, "{}\n   ===> ", self.region)?;
        }
        out.flush()
    }
}
