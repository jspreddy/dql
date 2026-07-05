use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeType {
    String,
    Number,
    Binary,
    Bool,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyType {
    Hash,
    Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub attr_type: AttributeType,
    pub key_type: Option<KeyType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Throughput {
    pub read: Value,
    pub write: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Binary(Vec<u8>),
    List(Vec<Value>),
    Set(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Compare {
        field: String,
        op: CompareOp,
        value: Value,
    },
    And(Vec<Condition>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    CreateTable {
        if_not_exists: bool,
        name: String,
        attributes: Vec<Attribute>,
        throughput: Option<Throughput>,
    },
    DropTable {
        if_exists: bool,
        name: String,
    },
    Insert {
        table: String,
        columns: Vec<String>,
        rows: Vec<Vec<Value>>,
    },
    Scan {
        table: String,
    },
    Select {
        table: String,
        condition: Option<Condition>,
    },
    DumpSchema {
        tables: Option<Vec<String>>,
    },
    Explain(Box<Statement>),
    Analyze(Box<Statement>),
}

pub fn parse_script(input: &str) -> Result<Vec<Statement>, ParseError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let mut statements = Vec::new();
    parser.consume_semicolons();
    while !parser.is_eof() {
        statements.push(parser.parse_statement()?);
        if parser.accept_symbol(';') {
            parser.consume_semicolons();
        } else if !parser.is_eof() {
            return Err(parser.error("expected ';' or end of input"));
        }
    }
    Ok(statements)
}

pub fn parse_statement(input: &str) -> Result<Statement, ParseError> {
    let statements = parse_script(input)?;
    match statements.as_slice() {
        [statement] => Ok(statement.clone()),
        [] => Err(ParseError::new("expected a statement")),
        _ => Err(ParseError::new("expected exactly one statement")),
    }
}

pub fn parse_value(input: &str) -> Result<Value, ParseError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let value = parser.parse_value()?;
    if parser.is_eof() {
        Ok(value)
    } else {
        Err(parser.error("unexpected trailing input"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Number(String),
    String(String),
    Binary(Vec<u8>),
    Symbol(char),
    Star,
}

fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(ch) = chars.peek().copied() {
        match ch {
            c if c.is_whitespace() => {
                chars.next();
            }
            '-' => {
                chars.next();
                if chars.peek() == Some(&'-') {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                } else if chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                    let mut number = String::from("-");
                    read_number(&mut chars, &mut number);
                    tokens.push(Token::Number(number));
                } else {
                    tokens.push(Token::Symbol('-'));
                }
            }
            '\'' | '"' => tokens.push(Token::String(read_quoted(&mut chars)?)),
            'b' | 'B' => {
                chars.next();
                if matches!(chars.peek(), Some('"') | Some('\'')) {
                    tokens.push(Token::Binary(read_quoted(&mut chars)?.into_bytes()));
                } else {
                    let mut ident = String::from(ch);
                    read_ident(&mut chars, &mut ident);
                    tokens.push(Token::Ident(ident));
                }
            }
            c if c.is_ascii_digit() => {
                let mut number = String::new();
                read_number(&mut chars, &mut number);
                tokens.push(Token::Number(number));
            }
            '*' => {
                chars.next();
                tokens.push(Token::Star);
            }
            '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | ';' | '=' | '<' | '>' | '!' => {
                chars.next();
                tokens.push(Token::Symbol(ch));
            }
            c if is_ident_start(c) => {
                let mut ident = String::new();
                chars.next();
                ident.push(c);
                read_ident(&mut chars, &mut ident);
                tokens.push(Token::Ident(ident));
            }
            _ => {
                return Err(ParseError::new(format!(
                    "unexpected character '{ch}' while lexing"
                )))
            }
        }
    }
    Ok(tokens)
}

fn read_quoted(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String, ParseError> {
    let quote = chars
        .next()
        .ok_or_else(|| ParseError::new("expected quote"))?;
    let mut value = String::new();
    while let Some(ch) = chars.next() {
        if ch == quote {
            return Ok(value);
        }
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                value.push(escaped);
            }
        } else {
            value.push(ch);
        }
    }
    Err(ParseError::new("unterminated string literal"))
}

fn read_number(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, number: &mut String) {
    while let Some(ch) = chars.peek().copied() {
        if ch.is_ascii_digit() || ch == '.' {
            number.push(ch);
            chars.next();
        } else {
            break;
        }
    }
}

fn read_ident(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, ident: &mut String) {
    while let Some(ch) = chars.peek().copied() {
        if is_ident_part(ch) {
            ident.push(ch);
            chars.next();
        } else {
            break;
        }
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        if self.accept_keyword("EXPLAIN") {
            return Ok(Statement::Explain(Box::new(self.parse_statement()?)));
        }
        if self.accept_keyword("ANALYZE") {
            return Ok(Statement::Analyze(Box::new(self.parse_statement()?)));
        }
        if self.accept_keyword("CREATE") {
            self.parse_create()
        } else if self.accept_keyword("DROP") {
            self.parse_drop()
        } else if self.accept_keyword("INSERT") {
            self.parse_insert()
        } else if self.accept_keyword("SCAN") {
            self.parse_scan()
        } else if self.accept_keyword("SELECT") {
            self.parse_select()
        } else if self.accept_keyword("DUMP") {
            self.parse_dump()
        } else {
            Err(self.error("expected a DQL statement"))
        }
    }

    fn parse_create(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword("TABLE")?;
        let if_not_exists = if self.accept_keyword("IF") {
            self.expect_keyword("NOT")?;
            self.expect_keyword("EXISTS")?;
            true
        } else {
            false
        };
        let name = self.expect_ident()?;
        self.expect_symbol('(')?;
        let mut attributes = Vec::new();
        let mut throughput = None;
        loop {
            if self.accept_keyword("THROUGHPUT") || self.accept_keyword("TP") {
                throughput = Some(self.parse_throughput_after_keyword()?);
            } else {
                attributes.push(self.parse_attribute()?);
            }
            if self.accept_symbol(',') {
                continue;
            }
            self.expect_symbol(')')?;
            break;
        }
        self.skip_global_indexes()?;
        Ok(Statement::CreateTable {
            if_not_exists,
            name,
            attributes,
            throughput,
        })
    }

    fn parse_attribute(&mut self) -> Result<Attribute, ParseError> {
        let name = self.expect_ident()?;
        let attr_type = self.parse_attribute_type()?;
        let key_type = if self.accept_keyword("HASH") {
            self.expect_keyword("KEY")?;
            Some(KeyType::Hash)
        } else if self.accept_keyword("RANGE") {
            self.expect_keyword("KEY")?;
            Some(KeyType::Range)
        } else {
            self.skip_local_index()?;
            None
        };
        Ok(Attribute {
            name,
            attr_type,
            key_type,
        })
    }

    fn parse_attribute_type(&mut self) -> Result<AttributeType, ParseError> {
        let ident = self.expect_ident()?;
        Ok(match ident.to_ascii_uppercase().as_str() {
            "STRING" => AttributeType::String,
            "NUMBER" => AttributeType::Number,
            "BINARY" => AttributeType::Binary,
            "BOOL" | "BOOLEAN" => AttributeType::Bool,
            _ => AttributeType::Other(ident),
        })
    }

    fn skip_local_index(&mut self) -> Result<(), ParseError> {
        if self.accept_keyword("KEYS") || self.accept_keyword("INCLUDE") {
            self.expect_keyword("INDEX")?;
            self.skip_parenthesized()
        } else if self.accept_keyword("INDEX") {
            self.skip_parenthesized()
        } else {
            Ok(())
        }
    }

    fn skip_global_indexes(&mut self) -> Result<(), ParseError> {
        while self.accept_keyword("GLOBAL") {
            let _ = self.accept_keyword("KEYS") || self.accept_keyword("INCLUDE");
            self.expect_keyword("INDEX")?;
            self.skip_parenthesized()?;
        }
        Ok(())
    }

    fn parse_throughput_after_keyword(&mut self) -> Result<Throughput, ParseError> {
        self.expect_symbol('(')?;
        let read = self.parse_value_or_star()?;
        self.expect_symbol(',')?;
        let write = self.parse_value_or_star()?;
        self.expect_symbol(')')?;
        Ok(Throughput { read, write })
    }

    fn parse_drop(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword("TABLE")?;
        let if_exists = if self.accept_keyword("IF") {
            self.expect_keyword("EXISTS")?;
            true
        } else {
            false
        };
        let name = self.expect_ident()?;
        Ok(Statement::DropTable { if_exists, name })
    }

    fn parse_insert(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword("INTO")?;
        let table = self.expect_ident()?;
        self.expect_symbol('(')?;
        let columns = self.parse_ident_list(')')?;
        self.expect_keyword("VALUES")?;
        let mut rows = Vec::new();
        loop {
            self.expect_symbol('(')?;
            let mut row = Vec::new();
            if !self.accept_symbol(')') {
                loop {
                    row.push(self.parse_value()?);
                    if self.accept_symbol(',') {
                        continue;
                    }
                    self.expect_symbol(')')?;
                    break;
                }
            }
            rows.push(row);
            if !self.accept_symbol(',') {
                break;
            }
        }
        Ok(Statement::Insert {
            table,
            columns,
            rows,
        })
    }

    fn parse_scan(&mut self) -> Result<Statement, ParseError> {
        self.skip_until_keyword("FROM")?;
        let table = self.expect_ident()?;
        self.skip_statement_tail();
        Ok(Statement::Scan { table })
    }

    fn parse_select(&mut self) -> Result<Statement, ParseError> {
        self.skip_until_keyword("FROM")?;
        let table = self.expect_ident()?;
        let condition = if self.accept_keyword("WHERE") {
            Some(self.parse_condition()?)
        } else {
            None
        };
        self.skip_statement_tail();
        Ok(Statement::Select { table, condition })
    }

    fn parse_dump(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword("SCHEMA")?;
        let mut tables = Vec::new();
        while !self.is_eof() && !self.peek_symbol(';') {
            tables.push(self.expect_ident()?);
            if !self.accept_symbol(',') {
                break;
            }
        }
        Ok(Statement::DumpSchema {
            tables: if tables.is_empty() {
                None
            } else {
                Some(tables)
            },
        })
    }

    fn parse_condition(&mut self) -> Result<Condition, ParseError> {
        let mut conditions = vec![self.parse_comparison()?];
        while self.accept_keyword("AND") {
            conditions.push(self.parse_comparison()?);
        }
        if conditions.len() == 1 {
            Ok(conditions.remove(0))
        } else {
            Ok(Condition::And(conditions))
        }
    }

    fn parse_comparison(&mut self) -> Result<Condition, ParseError> {
        let field = self.expect_ident()?;
        let op = self.parse_compare_op()?;
        let value = self.parse_value()?;
        Ok(Condition::Compare { field, op, value })
    }

    fn parse_compare_op(&mut self) -> Result<CompareOp, ParseError> {
        if self.accept_symbol('=') {
            Ok(CompareOp::Eq)
        } else if self.accept_symbol('<') {
            if self.accept_symbol('=') {
                Ok(CompareOp::Le)
            } else if self.accept_symbol('>') {
                Ok(CompareOp::Ne)
            } else {
                Ok(CompareOp::Lt)
            }
        } else if self.accept_symbol('>') {
            if self.accept_symbol('=') {
                Ok(CompareOp::Ge)
            } else {
                Ok(CompareOp::Gt)
            }
        } else if self.accept_symbol('!') {
            self.expect_symbol('=')?;
            Ok(CompareOp::Ne)
        } else {
            Err(self.error("expected comparison operator"))
        }
    }

    fn parse_value_or_star(&mut self) -> Result<Value, ParseError> {
        if self.accept_star() {
            Ok(Value::String("*".to_string()))
        } else {
            self.parse_value()
        }
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        match self.next().cloned() {
            Some(Token::String(value)) => Ok(Value::String(value)),
            Some(Token::Binary(value)) => Ok(Value::Binary(value)),
            Some(Token::Number(value)) => Ok(Value::Number(value)),
            Some(Token::Ident(value)) if value.eq_ignore_ascii_case("TRUE") => {
                Ok(Value::Bool(true))
            }
            Some(Token::Ident(value)) if value.eq_ignore_ascii_case("FALSE") => {
                Ok(Value::Bool(false))
            }
            Some(Token::Ident(value)) if value.eq_ignore_ascii_case("NULL") => Ok(Value::Null),
            Some(Token::Symbol('[')) => self.parse_list(),
            Some(Token::Symbol('(')) => self.parse_set(),
            Some(Token::Symbol('{')) => self.parse_map(),
            Some(token) => Err(self.error_at_previous(format!("expected value, got {token:?}"))),
            None => Err(self.error("expected value")),
        }
    }

    fn parse_list(&mut self) -> Result<Value, ParseError> {
        let mut values = Vec::new();
        if self.accept_symbol(']') {
            return Ok(Value::List(values));
        }
        loop {
            values.push(self.parse_value()?);
            if self.accept_symbol(',') {
                continue;
            }
            self.expect_symbol(']')?;
            break;
        }
        Ok(Value::List(values))
    }

    fn parse_set(&mut self) -> Result<Value, ParseError> {
        let mut values = Vec::new();
        if self.accept_symbol(')') {
            return Ok(Value::Set(values));
        }
        loop {
            values.push(self.parse_value()?);
            if self.accept_symbol(',') {
                continue;
            }
            self.expect_symbol(')')?;
            break;
        }
        Ok(Value::Set(values))
    }

    fn parse_map(&mut self) -> Result<Value, ParseError> {
        let mut values = BTreeMap::new();
        if self.accept_symbol('}') {
            return Ok(Value::Map(values));
        }
        loop {
            let key = match self.next().cloned() {
                Some(Token::String(key)) => key,
                Some(token) => {
                    return Err(
                        self.error_at_previous(format!("expected quoted map key, got {token:?}"))
                    )
                }
                None => return Err(self.error("expected quoted map key")),
            };
            self.expect_symbol(':')?;
            values.insert(key, self.parse_value()?);
            if self.accept_symbol(',') {
                continue;
            }
            self.expect_symbol('}')?;
            break;
        }
        Ok(Value::Map(values))
    }

    fn parse_ident_list(&mut self, end: char) -> Result<Vec<String>, ParseError> {
        let mut values = Vec::new();
        if self.accept_symbol(end) {
            return Ok(values);
        }
        loop {
            values.push(self.expect_ident()?);
            if self.accept_symbol(',') {
                continue;
            }
            self.expect_symbol(end)?;
            break;
        }
        Ok(values)
    }

    fn skip_parenthesized(&mut self) -> Result<(), ParseError> {
        self.expect_symbol('(')?;
        let mut depth = 1usize;
        while let Some(token) = self.next() {
            match token {
                Token::Symbol('(') => depth += 1,
                Token::Symbol(')') => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        Err(self.error("unterminated parenthesized expression"))
    }

    fn skip_until_keyword(&mut self, keyword: &str) -> Result<(), ParseError> {
        while !self.is_eof() && !self.peek_symbol(';') {
            if self.accept_keyword(keyword) {
                return Ok(());
            }
            self.pos += 1;
        }
        Err(self.error(format!("expected {keyword}")))
    }

    fn skip_statement_tail(&mut self) {
        while !self.is_eof() && !self.peek_symbol(';') {
            self.pos += 1;
        }
    }

    fn consume_semicolons(&mut self) {
        while self.accept_symbol(';') {}
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), ParseError> {
        if self.accept_keyword(keyword) {
            Ok(())
        } else {
            Err(self.error(format!("expected {keyword}")))
        }
    }

    fn accept_keyword(&mut self, keyword: &str) -> bool {
        if matches!(self.peek(), Some(Token::Ident(value)) if value.eq_ignore_ascii_case(keyword)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.next().cloned() {
            Some(Token::Ident(value)) => Ok(value),
            Some(token) => {
                Err(self.error_at_previous(format!("expected identifier, got {token:?}")))
            }
            None => Err(self.error("expected identifier")),
        }
    }

    fn expect_symbol(&mut self, symbol: char) -> Result<(), ParseError> {
        if self.accept_symbol(symbol) {
            Ok(())
        } else {
            Err(self.error(format!("expected '{symbol}'")))
        }
    }

    fn accept_symbol(&mut self, symbol: char) -> bool {
        if self.peek_symbol(symbol) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn peek_symbol(&self, symbol: char) -> bool {
        matches!(self.peek(), Some(Token::Symbol(found)) if *found == symbol)
    }

    fn accept_star(&mut self) -> bool {
        if matches!(self.peek(), Some(Token::Star)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos);
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError::new(format!("{} at token {}", message.into(), self.pos))
    }

    fn error_at_previous(&self, message: impl Into<String>) -> ParseError {
        ParseError::new(format!(
            "{} at token {}",
            message.into(),
            self.pos.saturating_sub(1)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_create_table() {
        let statement = parse_statement(
            "CREATE TABLE foobars (foo string hash key, bar NUMBER RANGE KEY, THROUGHPUT (1, 2))",
        )
        .unwrap();
        match statement {
            Statement::CreateTable {
                name,
                attributes,
                throughput,
                ..
            } => {
                assert_eq!(name, "foobars");
                assert_eq!(attributes.len(), 2);
                assert_eq!(attributes[0].key_type, Some(KeyType::Hash));
                assert_eq!(attributes[1].key_type, Some(KeyType::Range));
                assert_eq!(
                    throughput,
                    Some(Throughput {
                        read: Value::Number("1".to_string()),
                        write: Value::Number("2".to_string())
                    })
                );
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_insert_literals() {
        let statement =
            parse_statement("INSERT INTO t (id, payload) VALUES ('a', {'n': 1}), ('b', [true])")
                .unwrap();
        match statement {
            Statement::Insert {
                table,
                columns,
                rows,
            } => {
                assert_eq!(table, "t");
                assert_eq!(columns, vec!["id", "payload"]);
                assert_eq!(rows.len(), 2);
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_multiple_statements() {
        let statements = parse_script(
            "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t",
        )
        .unwrap();
        assert_eq!(statements.len(), 3);
    }

    #[test]
    fn parses_select_condition() {
        let statement = parse_statement("SELECT * FROM t WHERE id = 'a' AND score >= 2").unwrap();
        match statement {
            Statement::Select {
                table,
                condition: Some(Condition::And(parts)),
            } => {
                assert_eq!(table, "t");
                assert_eq!(parts.len(), 2);
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn rejects_trailing_input_without_semicolon() {
        assert!(parse_script("DROP TABLE t garbage").is_err());
    }
}
