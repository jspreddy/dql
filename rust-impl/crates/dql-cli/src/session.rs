use crate::config::CliConfig;
use dql_engine::{Engine, EngineError, FragmentEngine, SdkBackend, SdkConfig, StatementResult};
use dql_output::{render_result, DisplayMode, OutputConfig};
use std::env;
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

/// Returns true when `DQL_BACKEND=memory` requests the offline in-memory backend.
pub fn memory_backend_requested() -> bool {
    env::var("DQL_BACKEND")
        .map(|value| value.eq_ignore_ascii_case("memory"))
        .unwrap_or(false)
}

#[allow(clippy::large_enum_variant)]
pub enum RuntimeEngine {
    Memory(FragmentEngine<dql_engine::MemoryBackend>),
    Remote(FragmentEngine<SdkBackend>),
}

impl RuntimeEngine {
    /// In-memory backend for tests and `DQL_BACKEND=memory`.
    pub fn in_memory(allow_select_scan: bool) -> Self {
        Self::Memory(FragmentEngine::new(
            Engine::new(dql_engine::MemoryBackend::new()).with_allow_select_scan(allow_select_scan),
        ))
    }

    fn connect_sdk(config: SdkConfig, allow_select_scan: bool) -> Result<Self, EngineError> {
        let backend = SdkBackend::connect(config)?;
        Ok(Self::Remote(FragmentEngine::new(
            Engine::new(backend).with_allow_select_scan(allow_select_scan),
        )))
    }

    /// Build the runtime engine for CLI startup.
    ///
    /// - `-H` / host set → DynamoDB Local (`SdkBackend`)
    /// - `DQL_BACKEND=memory` and no host → in-memory backend
    /// - otherwise → live AWS (`SdkBackend` + default credential chain)
    pub fn build(
        region: &str,
        host: Option<&str>,
        port: u16,
        allow_select_scan: bool,
    ) -> Result<Self, EngineError> {
        Self::build_with_preference(
            region,
            host,
            port,
            allow_select_scan,
            memory_backend_requested(),
        )
    }

    /// Like [`build`](Self::build) but with an explicit memory preference (for tests).
    pub fn build_with_preference(
        region: &str,
        host: Option<&str>,
        port: u16,
        allow_select_scan: bool,
        prefer_memory: bool,
    ) -> Result<Self, EngineError> {
        if let Some(host) = host {
            Self::connect_sdk(SdkConfig::local(region, host, port), allow_select_scan)
        } else if prefer_memory {
            Ok(Self::in_memory(allow_select_scan))
        } else {
            Self::connect_sdk(SdkConfig::aws(region), allow_select_scan)
        }
    }

    pub fn is_memory(&self) -> bool {
        matches!(self, Self::Memory(_))
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
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

    /// Rebuild the engine for a new region and optional Local endpoint.
    ///
    /// Without a local endpoint this always connects to live AWS, which promotes
    /// an in-memory session to `Remote` (e.g. after `local off` or `use`).
    pub fn reconnect(
        &mut self,
        region: &str,
        local: Option<(String, u16)>,
        allow_select_scan: bool,
    ) -> Result<(), EngineError> {
        *self = if let Some((host, port)) = local {
            Self::connect_sdk(SdkConfig::local(region, host, port), allow_select_scan)?
        } else {
            Self::connect_sdk(SdkConfig::aws(region), allow_select_scan)?
        };
        Ok(())
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

    /// Describe a table and optionally attach CloudWatch consumed capacity.
    pub fn describe_with_metrics(
        &mut self,
        table: &str,
        refresh: bool,
        metrics: bool,
    ) -> Result<Option<dql_models::TableMeta>, EngineError> {
        let mut meta = self.describe(table, refresh)?;
        if metrics {
            if let Some(meta) = meta.as_mut() {
                self.attach_cloudwatch_metrics(meta)?;
            }
        }
        Ok(meta)
    }

    pub fn attach_cloudwatch_metrics(
        &self,
        meta: &mut dql_models::TableMeta,
    ) -> Result<(), EngineError> {
        match self {
            Self::Memory(_) => {
                // In-memory / offline: no CloudWatch.
                Ok(())
            }
            Self::Remote(engine) => engine.inner().backend().attach_cloudwatch_metrics(meta),
        }
    }

    pub fn is_local_or_memory(&self) -> bool {
        match self {
            Self::Memory(_) => true,
            Self::Remote(engine) => engine.inner().backend().is_local(),
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

    /// Build a session that always uses the in-memory backend (for unit tests).
    pub fn new_memory(region: &str) -> Self {
        let config = CliConfig::load();
        let mut throttle = crate::throttle::TableLimits::default();
        throttle.load(&config.throttle);
        let mut history = crate::history::HistoryManager::new();
        history.try_to_load_history();
        Self {
            engine: RuntimeEngine::in_memory(config.allow_select_scan),
            config,
            local_endpoint: None,
            region: region.to_string(),
            history,
            throttle,
        }
    }

    pub fn run_command(&mut self, command: &str, use_json: bool) -> Result<(), EngineError> {
        let mut output_config = if use_json {
            OutputConfig::for_json_command()
        } else {
            self.config.output_config()
        };
        output_config.silent = use_json;
        // Honor `opt display less` for -c / one-shot paths. JSON and non-TTY
        // stay on stdout so pipes remain pipe-friendly.
        let mut backend = if use_json || !io::IsTerminal::is_terminal(&io::stdout()) {
            DisplayMode::Stdout.backend()
        } else {
            DisplayMode::from_name(&self.config.display)
                .unwrap_or(DisplayMode::Stdout)
                .backend()
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_defaults_to_remote_without_host() {
        let engine =
            RuntimeEngine::build_with_preference("us-west-1", None, 8000, false, false).unwrap();
        assert!(engine.is_remote());
        assert!(!engine.is_memory());
        assert_eq!(engine.region(), "us-west-1");
    }

    #[test]
    fn build_prefers_memory_when_requested() {
        let engine =
            RuntimeEngine::build_with_preference("us-west-1", None, 8000, false, true).unwrap();
        assert!(engine.is_memory());
        assert_eq!(engine.region(), "memory");
    }

    #[test]
    fn build_local_host_uses_remote_backend() {
        let engine =
            RuntimeEngine::build_with_preference("us-west-1", Some("localhost"), 8000, false, true)
                .unwrap();
        assert!(engine.is_remote());
        assert_eq!(engine.session_identity(), "local");
    }

    #[test]
    fn reconnect_promotes_memory_to_aws() {
        let mut engine = RuntimeEngine::in_memory(false);
        assert!(engine.is_memory());
        engine.reconnect("eu-west-1", None, false).unwrap();
        assert!(engine.is_remote());
        assert_eq!(engine.region(), "eu-west-1");
        assert_ne!(engine.session_identity(), "memory");
    }

    #[test]
    fn reconnect_switches_local_and_aws() {
        let mut engine =
            RuntimeEngine::build_with_preference("us-west-1", None, 8000, false, false).unwrap();
        engine
            .reconnect("us-west-1", Some(("localhost".to_string(), 8000)), false)
            .unwrap();
        assert!(engine.is_remote());
        assert_eq!(engine.session_identity(), "local");

        engine.reconnect("us-east-1", None, false).unwrap();
        assert!(engine.is_remote());
        assert_eq!(engine.region(), "us-east-1");
        assert_ne!(engine.session_identity(), "local");
    }

    #[test]
    fn session_new_memory_stays_offline() {
        let session = Session::new_memory("us-west-1");
        assert!(session.engine.is_memory());
        assert!(session.local_endpoint.is_none());
        assert_eq!(session.region, "us-west-1");
    }
}
