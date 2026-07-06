use dql_parser::{parse_fragment, FragmentStatus, ParseError};

use crate::{Engine, EngineError, StatementResult};

pub struct FragmentEngine<B: crate::DynamoBackend> {
    inner: Engine<B>,
    fragments: String,
    pub last_query: String,
}

impl<B: crate::DynamoBackend> FragmentEngine<B> {
    pub fn new(inner: Engine<B>) -> Self {
        Self {
            inner,
            fragments: String::new(),
            last_query: String::new(),
        }
    }

    pub fn into_inner(self) -> Engine<B> {
        self.inner
    }

    pub fn inner(&self) -> &Engine<B> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut Engine<B> {
        &mut self.inner
    }

    pub fn partial(&self) -> bool {
        !self.fragments.is_empty()
    }

    pub fn reset(&mut self) {
        self.fragments.clear();
    }

    pub fn execute(&mut self, fragment: &str) -> Result<Option<StatementResult>, EngineError> {
        if self.fragments.is_empty() {
            self.fragments.push_str(fragment);
        } else {
            self.fragments.push('\n');
            self.fragments.push_str(fragment);
        }
        let trimmed_start = self.fragments.trim_start();
        if trimmed_start.len() != self.fragments.len() {
            self.fragments = trimmed_start.to_string();
        }

        match parse_fragment(&self.fragments) {
            FragmentStatus::Incomplete => Ok(None),
            FragmentStatus::Error(err) => {
                if looks_like_incomplete_fragment(&self.fragments) {
                    Ok(None)
                } else {
                    self.last_query = self.fragments.trim().to_string();
                    Err(EngineError::Parse(err))
                }
            }
            FragmentStatus::Complete(_) => {
                self.last_query = self.fragments.trim().to_string();
                self.fragments.clear();
                let result = self.inner.execute(&self.last_query)?;
                Ok(Some(result))
            }
        }
    }

    pub fn pformat_exc(&self, err: &ParseError) -> String {
        let query = if self.last_query.is_empty() {
            &self.fragments
        } else {
            &self.last_query
        };
        let loc = err.location().unwrap_or(query.len().saturating_sub(1));
        let pre_nl = query[..loc].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let post_nl = query[loc..]
            .find('\n')
            .map(|i| loc + i)
            .unwrap_or(query.len());
        format!("{}\n{}", &query[..post_nl], " ".repeat(loc - pre_nl) + "^")
    }
}

fn looks_like_incomplete_fragment(fragments: &str) -> bool {
    !fragments.trim_end().ends_with(';')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryBackend;

    #[test]
    fn incomplete_fragment_without_terminator_stays_buffered() {
        let mut engine = FragmentEngine::new(Engine::new(MemoryBackend::new()));
        assert!(engine.execute("SELECT * FROM t WHERE").unwrap().is_none());
        assert!(engine.execute("foo = 'bar'; DROP").unwrap().is_none());
    }

    #[test]
    fn invalid_complete_fragment_returns_parse_error() {
        let mut engine = FragmentEngine::new(Engine::new(MemoryBackend::new()));
        let err = engine.execute("DROP nope;").unwrap_err();
        assert!(matches!(err, EngineError::Parse(_)));
    }
}
