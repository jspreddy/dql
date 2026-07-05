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
    pub local_index: Option<LocalIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionKind {
    All,
    Keys,
    Include,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalIndex {
    pub name: String,
    pub projection: ProjectionKind,
    pub includes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalIndex {
    pub name: String,
    pub projection: ProjectionKind,
    pub hash_key: String,
    pub hash_key_type: Option<AttributeType>,
    pub range_key: Option<String>,
    pub range_key_type: Option<AttributeType>,
    pub includes: Vec<String>,
    pub throughput: Option<Throughput>,
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
    Timestamp(TimestampExpr),
    Interval(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimestampExpr {
    Now,
    UtcNow,
    Parse {
        function: String,
        value: String,
    },
    Ms(Box<TimestampExpr>),
    AddInterval {
        base: Box<TimestampExpr>,
        interval: String,
    },
    SubInterval {
        base: Box<TimestampExpr>,
        interval: String,
    },
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
        rhs: ConditionOperand,
    },
    Between {
        field: String,
        low: Value,
        high: Value,
    },
    In {
        field: String,
        values: Vec<Value>,
    },
    Function {
        name: String,
        args: Vec<ConditionOperand>,
    },
    Size {
        field: String,
        op: CompareOp,
        value: Value,
    },
    AttributeType {
        field: String,
        ty: String,
    },
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConditionOperand {
    Field(String),
    Value(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    All,
    CountAll,
    Items(Vec<SelectionItem>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionItem {
    pub expression: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateExpr {
    pub clauses: Vec<UpdateClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateClause {
    pub kind: UpdateClauseKind,
    pub path: String,
    pub expression: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdateClauseKind {
    Set,
    Add,
    Delete,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderBy {
    pub field: String,
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThrottleConfig {
    pub read_per_second: f64,
    pub write_per_second: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryOptions {
    pub limit: Option<usize>,
    pub scan_limit: Option<usize>,
    pub using_index: Option<String>,
    pub keys_in: Option<Vec<Vec<Value>>>,
    pub consistent: bool,
    pub order_by: Option<OrderBy>,
    pub throttle: Option<ThrottleConfig>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertForm {
    Values {
        columns: Vec<String>,
        rows: Vec<Vec<Value>>,
    },
    Keyword {
        rows: Vec<Vec<(String, Value)>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    CreateTable {
        if_not_exists: bool,
        name: String,
        attributes: Vec<Attribute>,
        throughput: Option<Throughput>,
        global_indexes: Vec<GlobalIndex>,
    },
    DropTable {
        if_exists: bool,
        name: String,
    },
    Insert {
        table: String,
        form: InsertForm,
        throttle: Option<ThrottleConfig>,
    },
    Delete {
        table: String,
        condition: Option<Condition>,
        options: QueryOptions,
    },
    Update {
        table: String,
        update: UpdateExpr,
        condition: Option<Condition>,
        returns: Option<String>,
        options: QueryOptions,
    },
    Scan {
        table: String,
        selection: Selection,
        condition: Option<Condition>,
        options: QueryOptions,
    },
    Select {
        table: String,
        selection: Selection,
        condition: Option<Condition>,
        options: QueryOptions,
    },
    AlterTable {
        table: String,
        action: AlterAction,
    },
    DumpSchema {
        tables: Option<Vec<String>>,
    },
    Load {
        file: String,
        table: String,
    },
    Explain(Box<Statement>),
    Analyze(Box<Statement>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterAction {
    SetThroughput {
        index: Option<String>,
        throughput: Throughput,
    },
    DropIndex {
        name: String,
        if_exists: bool,
    },
    CreateGlobalIndex {
        index: GlobalIndex,
        if_not_exists: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FragmentStatus {
    Incomplete,
    Complete(Vec<Statement>),
    Error(ParseError),
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

pub fn parse_selection(input: &str) -> Result<Selection, ParseError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let selection = parser.parse_selection_until(|parser| parser.is_eof())?;
    if parser.is_eof() {
        Ok(selection)
    } else {
        Err(parser.error("unexpected trailing input"))
    }
}

pub fn parse_update_expr(input: &str) -> Result<UpdateExpr, ParseError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let update = parser.parse_update_expr_until(|parser| parser.is_eof())?;
    if parser.is_eof() {
        Ok(update)
    } else {
        Err(parser.error("unexpected trailing input"))
    }
}

pub fn parse_fragment(input: &str) -> FragmentStatus {
    if !input.contains(';') {
        return FragmentStatus::Incomplete;
    }
    match parse_script(input) {
        Ok(statements) => FragmentStatus::Complete(statements),
        Err(err) => FragmentStatus::Error(err),
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
            '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | ';' | '=' | '<' | '>' | '!' | '+'
            | '/' => {
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

fn merge_query_options(mut base: QueryOptions, tail: QueryOptions) -> QueryOptions {
    if tail.limit.is_some() {
        base.limit = tail.limit;
    }
    if tail.scan_limit.is_some() {
        base.scan_limit = tail.scan_limit;
    }
    if tail.using_index.is_some() {
        base.using_index = tail.using_index;
    }
    if tail.keys_in.is_some() {
        base.keys_in = tail.keys_in;
    }
    if tail.order_by.is_some() {
        base.order_by = tail.order_by;
    }
    base.consistent |= tail.consistent;
    base
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
        } else if self.accept_keyword("DELETE") {
            self.parse_delete()
        } else if self.accept_keyword("UPDATE") {
            self.parse_update()
        } else if self.accept_keyword("SCAN") {
            self.parse_scan()
        } else if self.accept_keyword("SELECT") {
            self.parse_select()
        } else if self.accept_keyword("ALTER") {
            self.parse_alter()
        } else if self.accept_keyword("DUMP") {
            self.parse_dump()
        } else if self.accept_keyword("LOAD") {
            self.parse_load()
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
        let global_indexes = self.parse_global_indexes()?;
        Ok(Statement::CreateTable {
            if_not_exists,
            name,
            attributes,
            throughput,
            global_indexes,
        })
    }

    fn parse_attribute(&mut self) -> Result<Attribute, ParseError> {
        let name = self.expect_ident()?;
        let attr_type = self.parse_attribute_type()?;
        let mut local_index = None;
        let key_type = if self.accept_keyword("HASH") {
            self.expect_keyword("KEY")?;
            Some(KeyType::Hash)
        } else if self.accept_keyword("RANGE") {
            self.expect_keyword("KEY")?;
            Some(KeyType::Range)
        } else {
            local_index = self.parse_local_index()?;
            None
        };
        Ok(Attribute {
            name,
            attr_type,
            key_type,
            local_index,
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

    fn parse_local_index(&mut self) -> Result<Option<LocalIndex>, ParseError> {
        let projection = if self.accept_keyword("KEYS") {
            self.expect_keyword("INDEX")?;
            ProjectionKind::Keys
        } else if self.accept_keyword("INCLUDE") {
            self.expect_keyword("INDEX")?;
            ProjectionKind::Include
        } else if self.accept_keyword("INDEX") {
            ProjectionKind::All
        } else {
            return Ok(None);
        };
        self.expect_symbol('(')?;
        let name = self.expect_string_or_ident()?;
        let includes = if self.accept_symbol(',') {
            self.parse_string_list()?
        } else {
            Vec::new()
        };
        self.expect_symbol(')')?;
        Ok(Some(LocalIndex {
            name,
            projection,
            includes,
        }))
    }

    fn parse_global_indexes(&mut self) -> Result<Vec<GlobalIndex>, ParseError> {
        let mut indexes = Vec::new();
        while self.accept_keyword("GLOBAL") {
            let projection = if self.accept_keyword("KEYS") {
                ProjectionKind::Keys
            } else if self.accept_keyword("INCLUDE") {
                ProjectionKind::Include
            } else {
                let _ = self.accept_keyword("ALL");
                ProjectionKind::All
            };
            self.expect_keyword("INDEX")?;
            indexes.push(self.parse_global_index_body(projection)?);
        }
        Ok(indexes)
    }

    fn parse_global_index_body(
        &mut self,
        projection: ProjectionKind,
    ) -> Result<GlobalIndex, ParseError> {
        self.expect_symbol('(')?;
        let name = self.expect_string_or_ident()?;
        self.expect_symbol(',')?;
        let hash_key = self.expect_ident()?;
        let hash_key_type = self.parse_optional_attribute_type();
        let mut range_key = None;
        let mut range_key_type = None;
        let mut includes = Vec::new();
        let mut throughput = None;
        while self.accept_symbol(',') {
            if self.accept_keyword("THROUGHPUT") || self.accept_keyword("TP") {
                throughput = Some(self.parse_throughput_after_keyword()?);
            } else if self.peek_symbol('[') {
                includes = self.parse_string_list()?;
            } else {
                let part = self.expect_ident()?;
                let key_type = self.parse_optional_attribute_type();
                if range_key.replace(part).is_some() {
                    return Err(self.error("too many global index key fields"));
                }
                range_key_type = key_type;
            }
        }
        self.expect_symbol(')')?;
        Ok(GlobalIndex {
            name,
            projection,
            hash_key,
            hash_key_type,
            range_key,
            range_key_type,
            includes,
            throughput,
        })
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
        let form = if self.peek_keyword_form() {
            InsertForm::Keyword {
                rows: self.parse_keyword_insert_rows()?,
            }
        } else {
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
            InsertForm::Values { columns, rows }
        };
        let throttle = if self.accept_keyword("THROTTLE") {
            Some(self.parse_throttle_clause()?)
        } else {
            None
        };
        Ok(Statement::Insert {
            table,
            form,
            throttle,
        })
    }

    fn peek_keyword_form(&self) -> bool {
        if !self.peek_symbol('(') {
            return false;
        }
        matches!(
            (self.tokens.get(self.pos + 1), self.tokens.get(self.pos + 2)),
            (Some(Token::Ident(_)), Some(Token::Symbol('=')))
        )
    }

    fn parse_keyword_insert_rows(&mut self) -> Result<Vec<Vec<(String, Value)>>, ParseError> {
        let mut rows = Vec::new();
        loop {
            self.expect_symbol('(')?;
            let mut pairs = Vec::new();
            loop {
                let key = self.expect_ident()?;
                self.expect_symbol('=')?;
                let value = self.parse_value()?;
                pairs.push((key, value));
                if self.accept_symbol(',') {
                    continue;
                }
                self.expect_symbol(')')?;
                break;
            }
            rows.push(pairs);
            if !self.accept_symbol(',') {
                break;
            }
        }
        Ok(rows)
    }

    fn parse_delete(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword("FROM")?;
        let table = self.expect_ident()?;
        let (condition, options) = self.parse_mutation_tail()?;
        Ok(Statement::Delete {
            table,
            condition,
            options,
        })
    }

    fn parse_update(&mut self) -> Result<Statement, ParseError> {
        let table = self.expect_ident()?;
        let update = self.parse_update_expr_until(|parser| {
            parser.peek_keyword("WHERE")
                || parser.peek_keyword("KEYS")
                || parser.peek_keyword("USING")
                || parser.peek_keyword("RETURNS")
                || parser.peek_keyword("THROTTLE")
                || parser.peek_symbol(';')
                || parser.is_eof()
        })?;
        let mut options = QueryOptions::default();
        let condition = self.parse_query_condition(&mut options)?;
        if self.accept_keyword("USING") {
            options.using_index = Some(self.parse_index_name()?);
        }
        let returns = if self.accept_keyword("RETURNS") {
            Some(self.collect_until(|parser| {
                parser.peek_keyword("THROTTLE") || parser.peek_symbol(';') || parser.is_eof()
            }))
        } else {
            None
        };
        if self.accept_keyword("THROTTLE") {
            options.throttle = Some(self.parse_throttle_clause()?);
        }
        if !self.is_eof() && !self.peek_symbol(';') {
            return Err(self.error("unexpected token after UPDATE"));
        }
        Ok(Statement::Update {
            table,
            update,
            condition,
            returns,
            options,
        })
    }

    fn parse_scan(&mut self) -> Result<Statement, ParseError> {
        let mut options = QueryOptions::default();
        if self.accept_keyword("CONSISTENT") {
            options.consistent = true;
        }
        let selection = self.parse_selection_until(|parser| parser.peek_keyword("FROM"))?;
        self.expect_keyword("FROM")?;
        let table = self.expect_ident()?;
        let (condition, tail_options) = self.parse_query_tail()?;
        options = merge_query_options(options, tail_options);
        Ok(Statement::Scan {
            table,
            selection,
            condition,
            options,
        })
    }

    fn parse_select(&mut self) -> Result<Statement, ParseError> {
        let mut options = QueryOptions::default();
        if self.accept_keyword("CONSISTENT") {
            options.consistent = true;
        }
        let selection = self.parse_selection_until(|parser| parser.peek_keyword("FROM"))?;
        self.expect_keyword("FROM")?;
        let table = self.expect_ident()?;
        let (condition, tail_options) = self.parse_query_tail()?;
        options = merge_query_options(options, tail_options);
        Ok(Statement::Select {
            table,
            selection,
            condition,
            options,
        })
    }

    fn parse_query_tail(&mut self) -> Result<(Option<Condition>, QueryOptions), ParseError> {
        let mut options = QueryOptions::default();
        let condition = self.parse_query_condition(&mut options)?;
        self.parse_query_options(&mut options)?;
        Ok((condition, options))
    }

    fn parse_mutation_tail(&mut self) -> Result<(Option<Condition>, QueryOptions), ParseError> {
        let mut options = QueryOptions::default();
        let condition = self.parse_query_condition(&mut options)?;
        while !self.is_eof() && !self.peek_symbol(';') {
            if self.accept_keyword("USING") {
                options.using_index = Some(self.parse_index_name()?);
            } else if self.accept_keyword("THROTTLE") {
                options.throttle = Some(self.parse_throttle_clause()?);
            } else {
                return Err(self.error("unexpected token in DELETE/UPDATE"));
            }
        }
        Ok((condition, options))
    }

    fn parse_query_condition(
        &mut self,
        options: &mut QueryOptions,
    ) -> Result<Option<Condition>, ParseError> {
        if self.accept_keyword("KEYS") {
            self.expect_keyword("IN")?;
            options.keys_in = Some(self.parse_keys_in_list()?);
        }
        if self.accept_keyword("WHERE") {
            Ok(Some(self.parse_condition()?))
        } else {
            Ok(None)
        }
    }

    fn parse_query_options(&mut self, options: &mut QueryOptions) -> Result<(), ParseError> {
        while !self.is_eof() && !self.peek_symbol(';') {
            if self.accept_keyword("USING") {
                options.using_index = Some(self.parse_index_name()?);
            } else if self.accept_keyword("LIMIT") {
                options.limit = Some(self.expect_usize()?);
            } else if self.accept_keyword("SCAN") {
                self.expect_keyword("LIMIT")?;
                options.scan_limit = Some(self.expect_usize()?);
            } else if self.accept_keyword("ORDER") {
                self.expect_keyword("BY")?;
                let field = self.expect_ident()?;
                let descending = if self.accept_keyword("DESC") {
                    true
                } else {
                    self.accept_keyword("ASC");
                    false
                };
                options.order_by = Some(OrderBy { field, descending });
            } else if self.accept_keyword("THROTTLE") {
                options.throttle = Some(self.parse_throttle_clause()?);
            } else {
                return Err(self.error("unexpected token in query options"));
            }
        }
        Ok(())
    }

    fn parse_keys_in_list(&mut self) -> Result<Vec<Vec<Value>>, ParseError> {
        let mut keys = Vec::new();
        loop {
            let key_values = if self.accept_symbol('(') {
                let mut values = vec![self.parse_value()?];
                if self.accept_symbol(',') {
                    values.push(self.parse_value()?);
                }
                self.expect_symbol(')')?;
                values
            } else {
                vec![self.parse_value()?]
            };
            keys.push(key_values);
            if !self.accept_symbol(',') {
                break;
            }
        }
        Ok(keys)
    }

    fn parse_index_name(&mut self) -> Result<String, ParseError> {
        if self.accept_symbol('-') {
            Ok("-".to_string())
        } else {
            self.expect_ident()
        }
    }

    fn parse_throttle_clause(&mut self) -> Result<ThrottleConfig, ParseError> {
        Ok(ThrottleConfig {
            read_per_second: self.parse_throttle_rate()?,
            write_per_second: self.parse_throttle_rate()?,
        })
    }

    fn parse_throttle_rate(&mut self) -> Result<f64, ParseError> {
        if self.accept_symbol('*') {
            return Ok(f64::MAX);
        }
        let value = if let Some(Token::Number(value)) = self.tokens.get(self.pos).cloned() {
            self.pos += 1;
            value
        } else {
            self.expect_usize()?.to_string()
        };
        if self.accept_symbol('%') {
            return Ok(value
                .parse::<f64>()
                .map_err(|_| self.error("invalid throttle rate"))?
                / 100.0);
        }
        value
            .parse::<f64>()
            .map_err(|_| self.error("invalid throttle rate"))
    }

    fn skip_throttle_clause(&mut self) {
        let _ = self.parse_throttle_clause();
    }

    fn parse_alter(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword("TABLE")?;
        let table = self.expect_ident()?;
        let action = if self.accept_keyword("SET") {
            let index = if self.accept_keyword("INDEX") {
                Some(self.expect_ident()?)
            } else {
                None
            };
            self.expect_keyword("THROUGHPUT")
                .or_else(|_| self.expect_keyword("TP"))?;
            AlterAction::SetThroughput {
                index,
                throughput: self.parse_throughput_after_keyword()?,
            }
        } else if self.accept_keyword("DROP") {
            self.expect_keyword("INDEX")?;
            let name = self.expect_ident()?;
            let if_exists = if self.accept_keyword("IF") {
                self.expect_keyword("EXISTS")?;
                true
            } else {
                false
            };
            AlterAction::DropIndex { name, if_exists }
        } else if self.accept_keyword("CREATE") {
            self.expect_keyword("GLOBAL")?;
            let projection = if self.accept_keyword("KEYS") {
                ProjectionKind::Keys
            } else if self.accept_keyword("INCLUDE") {
                ProjectionKind::Include
            } else {
                ProjectionKind::All
            };
            self.expect_keyword("INDEX")?;
            let index = self.parse_global_index_body(projection)?;
            let if_not_exists = if self.accept_keyword("IF") {
                self.expect_keyword("NOT")?;
                self.expect_keyword("EXISTS")?;
                true
            } else {
                false
            };
            AlterAction::CreateGlobalIndex {
                index,
                if_not_exists,
            }
        } else {
            return Err(self.error("expected ALTER action"));
        };
        Ok(Statement::AlterTable { table, action })
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

    fn parse_load(&mut self) -> Result<Statement, ParseError> {
        let file = self.expect_string_or_ident()?;
        self.expect_keyword("INTO")?;
        let table = self.expect_ident()?;
        self.skip_statement_tail();
        Ok(Statement::Load { file, table })
    }

    fn parse_selection_until<F>(&mut self, stop: F) -> Result<Selection, ParseError>
    where
        F: Fn(&Self) -> bool,
    {
        let raw = self.collect_until(stop);
        let raw = raw.trim();
        if raw.is_empty() || raw == "*" {
            return Ok(Selection::All);
        }
        if raw.eq_ignore_ascii_case("count ( * )") || raw.eq_ignore_ascii_case("count(*)") {
            return Ok(Selection::CountAll);
        }
        let items = raw
            .split(',')
            .map(|item| {
                let item = item.trim();
                let upper = item.to_ascii_uppercase();
                if let Some(index) = upper.rfind(" AS ") {
                    SelectionItem {
                        expression: normalize_ws(&item[..index]),
                        alias: Some(item[index + 4..].trim().to_string()),
                    }
                } else {
                    SelectionItem {
                        expression: normalize_ws(item),
                        alias: None,
                    }
                }
            })
            .collect();
        Ok(Selection::Items(items))
    }

    fn parse_update_expr_until<F>(&mut self, stop: F) -> Result<UpdateExpr, ParseError>
    where
        F: Fn(&Self) -> bool,
    {
        let raw = self.collect_until(stop);
        parse_update_expr_raw(&raw)
    }

    fn parse_condition(&mut self) -> Result<Condition, ParseError> {
        self.parse_or_condition()
    }

    fn parse_or_condition(&mut self) -> Result<Condition, ParseError> {
        let mut conditions = vec![self.parse_and_condition()?];
        while self.accept_keyword("OR") {
            conditions.push(self.parse_and_condition()?);
        }
        if conditions.len() == 1 {
            Ok(conditions.remove(0))
        } else {
            Ok(Condition::Or(conditions))
        }
    }

    fn parse_and_condition(&mut self) -> Result<Condition, ParseError> {
        let mut conditions = vec![self.parse_primary_condition()?];
        while self.accept_keyword("AND") {
            conditions.push(self.parse_primary_condition()?);
        }
        if conditions.len() == 1 {
            Ok(conditions.remove(0))
        } else {
            Ok(Condition::And(conditions))
        }
    }

    fn parse_primary_condition(&mut self) -> Result<Condition, ParseError> {
        if self.accept_keyword("NOT") {
            return Ok(Condition::Not(Box::new(self.parse_primary_condition()?)));
        }
        if self.accept_symbol('(') {
            let condition = self.parse_condition()?;
            self.expect_symbol(')')?;
            return Ok(condition);
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Condition, ParseError> {
        if self.accept_keyword("SIZE") {
            self.expect_symbol('(')?;
            let field = self.parse_field_path()?;
            self.expect_symbol(')')?;
            let op = self.parse_compare_op()?;
            let value = self.parse_value()?;
            return Ok(Condition::Size { field, op, value });
        }
        if self.peek_function_condition() {
            return self.parse_function_condition();
        }
        let field = self.parse_field_path()?;
        if self.accept_keyword("BETWEEN") {
            let low = self.parse_value()?;
            self.expect_keyword("AND")?;
            let high = self.parse_value()?;
            return Ok(Condition::Between { field, low, high });
        }
        if self.accept_keyword("IN") {
            self.expect_symbol('(')?;
            let mut values = Vec::new();
            if !self.accept_symbol(')') {
                loop {
                    values.push(self.parse_value()?);
                    if self.accept_symbol(',') {
                        continue;
                    }
                    self.expect_symbol(')')?;
                    break;
                }
            }
            return Ok(Condition::In { field, values });
        }
        let op = self.parse_compare_op()?;
        let rhs = self.parse_condition_operand()?;
        Ok(Condition::Compare { field, op, rhs })
    }

    fn parse_function_condition(&mut self) -> Result<Condition, ParseError> {
        let name = self.expect_ident()?.to_ascii_lowercase();
        self.expect_symbol('(')?;
        if name.eq_ignore_ascii_case("ATTRIBUTE_TYPE") {
            let field = self.parse_field_path()?;
            self.expect_symbol(',')?;
            let ty = self.expect_string_or_ident()?;
            self.expect_symbol(')')?;
            return Ok(Condition::AttributeType { field, ty });
        }
        let mut args = Vec::new();
        if !self.accept_symbol(')') {
            loop {
                args.push(self.parse_condition_operand()?);
                if self.accept_symbol(',') {
                    continue;
                }
                self.expect_symbol(')')?;
                break;
            }
        }
        Ok(Condition::Function { name, args })
    }

    fn parse_condition_operand(&mut self) -> Result<ConditionOperand, ParseError> {
        match self.peek().cloned() {
            Some(Token::Ident(value))
                if !is_value_keyword(&value)
                    && !is_timestamp_keyword(&value)
                    && !value.eq_ignore_ascii_case("MS")
                    && !value.eq_ignore_ascii_case("INTERVAL") =>
            {
                Ok(ConditionOperand::Field(self.parse_field_path()?))
            }
            _ => Ok(ConditionOperand::Value(self.parse_value()?)),
        }
    }

    fn parse_field_path(&mut self) -> Result<String, ParseError> {
        let mut field = self.expect_ident()?;
        loop {
            if self.accept_symbol('[') {
                let index = self.expect_string_or_ident()?;
                self.expect_symbol(']')?;
                field.push('[');
                field.push_str(&index);
                field.push(']');
            } else if self.accept_symbol('.') {
                field.push('.');
                field.push_str(&self.expect_ident()?);
            } else {
                break;
            }
        }
        Ok(field)
    }

    fn peek_function_condition(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token::Ident(value))
                if matches!(
                    value.to_ascii_uppercase().as_str(),
                    "BEGINS_WITH"
                        | "ATTRIBUTE_EXISTS"
                        | "ATTRIBUTE_NOT_EXISTS"
                        | "ATTRIBUTE_TYPE"
                        | "CONTAINS"
                )
        )
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
            Some(Token::Ident(value)) if is_timestamp_keyword(&value) => self
                .parse_timestamp_after_keyword(value)
                .map(Value::Timestamp),
            Some(Token::Ident(value)) if value.eq_ignore_ascii_case("MS") => {
                self.expect_symbol('(')?;
                let expr = self.parse_timestamp_expr()?;
                self.expect_symbol(')')?;
                Ok(Value::Timestamp(TimestampExpr::Ms(Box::new(expr))))
            }
            Some(Token::Ident(value)) if value.eq_ignore_ascii_case("INTERVAL") => {
                Ok(Value::Interval(self.parse_interval_arg()?))
            }
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

    fn parse_timestamp_expr(&mut self) -> Result<TimestampExpr, ParseError> {
        let base = match self.next().cloned() {
            Some(Token::Ident(value)) if value.eq_ignore_ascii_case("MS") => {
                self.expect_symbol('(')?;
                let expr = self.parse_timestamp_expr()?;
                self.expect_symbol(')')?;
                TimestampExpr::Ms(Box::new(expr))
            }
            Some(Token::Ident(value)) if is_timestamp_keyword(&value) => {
                self.parse_timestamp_after_keyword(value)?
            }
            Some(token) => {
                return Err(
                    self.error_at_previous(format!("expected timestamp expression, got {token:?}"))
                )
            }
            None => return Err(self.error("expected timestamp expression")),
        };
        self.parse_timestamp_tail(base)
    }

    fn parse_timestamp_after_keyword(
        &mut self,
        keyword: String,
    ) -> Result<TimestampExpr, ParseError> {
        let expr = if keyword.eq_ignore_ascii_case("NOW") {
            let _ = self.accept_symbol('(') && self.accept_symbol(')');
            TimestampExpr::Now
        } else if keyword.eq_ignore_ascii_case("UTCNOW") {
            let _ = self.accept_symbol('(') && self.accept_symbol(')');
            TimestampExpr::UtcNow
        } else {
            let value = if self.accept_symbol('(') {
                let value = self.expect_string_or_ident()?;
                self.expect_symbol(')')?;
                value
            } else {
                self.expect_string_or_ident()?
            };
            TimestampExpr::Parse {
                function: keyword.to_ascii_lowercase(),
                value,
            }
        };
        self.parse_timestamp_tail(expr)
    }

    fn parse_timestamp_tail(&mut self, base: TimestampExpr) -> Result<TimestampExpr, ParseError> {
        if self.accept_symbol('+') {
            self.expect_keyword("INTERVAL")?;
            Ok(TimestampExpr::AddInterval {
                base: Box::new(base),
                interval: self.parse_interval_arg()?,
            })
        } else if self.accept_symbol('-') {
            self.expect_keyword("INTERVAL")?;
            Ok(TimestampExpr::SubInterval {
                base: Box::new(base),
                interval: self.parse_interval_arg()?,
            })
        } else {
            Ok(base)
        }
    }

    fn parse_interval_arg(&mut self) -> Result<String, ParseError> {
        if self.accept_symbol('(') {
            let value = self.expect_string_or_ident()?;
            self.expect_symbol(')')?;
            Ok(value)
        } else {
            self.expect_string_or_ident()
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

    fn parse_string_list(&mut self) -> Result<Vec<String>, ParseError> {
        self.expect_symbol('[')?;
        let mut values = Vec::new();
        if self.accept_symbol(']') {
            return Ok(values);
        }
        loop {
            values.push(self.expect_string_or_ident()?);
            if self.accept_symbol(',') {
                continue;
            }
            self.expect_symbol(']')?;
            break;
        }
        Ok(values)
    }

    fn expect_string_or_ident(&mut self) -> Result<String, ParseError> {
        match self.next().cloned() {
            Some(Token::String(value)) | Some(Token::Ident(value)) | Some(Token::Number(value)) => {
                Ok(value)
            }
            Some(token) => {
                Err(self.error_at_previous(format!("expected string or identifier, got {token:?}")))
            }
            None => Err(self.error("expected string or identifier")),
        }
    }

    fn parse_optional_attribute_type(&mut self) -> Option<AttributeType> {
        if !self.accept_type_name() {
            return None;
        }
        let ident = self.previous_ident()?;
        Some(match ident.to_ascii_uppercase().as_str() {
            "STRING" => AttributeType::String,
            "NUMBER" => AttributeType::Number,
            "BINARY" => AttributeType::Binary,
            "BOOL" | "BOOLEAN" => AttributeType::Bool,
            _ => AttributeType::Other(ident),
        })
    }

    fn previous_ident(&self) -> Option<String> {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .and_then(|token| {
                if let Token::Ident(value) = token {
                    Some(value.clone())
                } else {
                    None
                }
            })
    }

    fn accept_type_name(&mut self) -> bool {
        matches!(
            self.peek(),
            Some(Token::Ident(value))
                if matches!(
                    value.to_ascii_uppercase().as_str(),
                    "STRING" | "NUMBER" | "BINARY" | "BOOL" | "BOOLEAN"
                )
        ) && {
            self.pos += 1;
            true
        }
    }

    fn collect_until<F>(&mut self, stop: F) -> String
    where
        F: Fn(&Self) -> bool,
    {
        let mut tokens = Vec::new();
        let mut depth = 0usize;
        while !self.is_eof() {
            if depth == 0 && stop(self) {
                break;
            }
            let Some(token) = self.next().cloned() else {
                break;
            };
            match token {
                Token::Symbol('(') | Token::Symbol('[') | Token::Symbol('{') => depth += 1,
                Token::Symbol(')') | Token::Symbol(']') | Token::Symbol('}') => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
            tokens.push(token_to_source(&token));
        }
        normalize_ws(&tokens.join(" "))
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

    fn peek_keyword(&self, keyword: &str) -> bool {
        matches!(self.peek(), Some(Token::Ident(value)) if value.eq_ignore_ascii_case(keyword))
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

    fn expect_usize(&mut self) -> Result<usize, ParseError> {
        match self.next().cloned() {
            Some(Token::Number(value)) => value
                .parse()
                .map_err(|_| self.error_at_previous(format!("expected integer, got {value}"))),
            Some(token) => Err(self.error_at_previous(format!("expected integer, got {token:?}"))),
            None => Err(self.error("expected integer")),
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

fn is_value_keyword(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "TRUE" | "FALSE" | "NULL"
    )
}

fn is_timestamp_keyword(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "TIMESTAMP" | "TS" | "UTCTIMESTAMP" | "UTCTS" | "NOW" | "UTCNOW"
    )
}

fn normalize_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn token_to_source(token: &Token) -> String {
    match token {
        Token::Ident(value) | Token::Number(value) => value.clone(),
        Token::String(value) => format!("'{value}'"),
        Token::Binary(value) => format!("b'{}'", String::from_utf8_lossy(value)),
        Token::Symbol(value) => value.to_string(),
        Token::Star => "*".to_string(),
    }
}

fn parse_update_expr_raw(raw: &str) -> Result<UpdateExpr, ParseError> {
    let mut clauses = Vec::new();
    let mut current_kind = None;
    let mut current = String::new();
    for part in raw.split_whitespace() {
        let upper = part.to_ascii_uppercase();
        if matches!(upper.as_str(), "SET" | "ADD" | "DELETE" | "REMOVE") {
            flush_update_clause(current_kind.take(), &current, &mut clauses)?;
            current.clear();
            current_kind = Some(match upper.as_str() {
                "SET" => UpdateClauseKind::Set,
                "ADD" => UpdateClauseKind::Add,
                "DELETE" => UpdateClauseKind::Delete,
                "REMOVE" => UpdateClauseKind::Remove,
                _ => unreachable!(),
            });
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(part);
        }
    }
    flush_update_clause(current_kind, &current, &mut clauses)?;
    if clauses.is_empty() {
        return Err(ParseError::new("expected update expression"));
    }
    Ok(UpdateExpr { clauses })
}

fn flush_update_clause(
    kind: Option<UpdateClauseKind>,
    raw: &str,
    clauses: &mut Vec<UpdateClause>,
) -> Result<(), ParseError> {
    let Some(kind) = kind else {
        if raw.trim().is_empty() {
            return Ok(());
        }
        return Err(ParseError::new("expected update clause"));
    };
    for part in split_top_level_commas(raw) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (path, expression) = match kind {
            UpdateClauseKind::Set => {
                let Some((path, expr)) = part.split_once('=') else {
                    return Err(ParseError::new("SET update requires '='"));
                };
                (path.trim().to_string(), Some(normalize_ws(expr)))
            }
            UpdateClauseKind::Remove => (part.to_string(), None),
            UpdateClauseKind::Add | UpdateClauseKind::Delete => {
                let mut pieces = part.splitn(2, char::is_whitespace);
                let path = pieces.next().unwrap_or_default().trim().to_string();
                let expr = pieces.next().unwrap_or_default().trim();
                if path.is_empty() || expr.is_empty() {
                    return Err(ParseError::new("ADD/DELETE update requires path and value"));
                }
                (path, Some(normalize_ws(expr)))
            }
        };
        clauses.push(UpdateClause {
            kind: kind.clone(),
            path,
            expression,
        });
    }
    Ok(())
}

fn split_top_level_commas(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in raw.chars() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
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
                form: InsertForm::Values { columns, rows },
                throttle,
            } => {
                assert_eq!(table, "t");
                assert_eq!(columns, vec!["id", "payload"]);
                assert_eq!(rows.len(), 2);
                assert!(throttle.is_none());
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
                ..
            } => {
                assert_eq!(table, "t");
                assert_eq!(parts.len(), 2);
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_query_limits() {
        let statement =
            parse_statement("SCAN * FROM t WHERE score >= 2 LIMIT 3 SCAN LIMIT 4").unwrap();
        match statement {
            Statement::Scan {
                table,
                condition: Some(_),
                options,
                ..
            } => {
                assert_eq!(table, "t");
                assert_eq!(options.limit, Some(3));
                assert_eq!(options.scan_limit, Some(4));
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn rejects_trailing_input_without_semicolon() {
        assert!(parse_script("DROP TABLE t garbage").is_err());
    }
}
