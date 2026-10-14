use super::app::{live_panel_bg, status_panel_bg};
use dql_engine::format_throughput;
use dql_models::{ProjectionType, TableMeta, TableStatus};
use dql_output::TableStats;
use dql_output::{ColumnKey, RichColumn, RichLayout};
use humansize::{format_size, BINARY};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Row, Table, Widget};
use ratatui::Terminal;

struct StyledColumn {
    name: String,
    header_style: Style,
    cell_style: Option<Style>,
}

pub fn rich_layout_to_lines(layout: &RichLayout, width: u16) -> Vec<Line<'static>> {
    if layout.columns.is_empty() {
        return vec![Line::from("No results")];
    }

    let mut lines = render_table_to_lines("Results", &layout.columns, &layout.rows, width);
    if layout.overflow_columns.is_empty() {
        return lines;
    }

    lines.push(Line::from(""));
    let overflow_rows: Vec<Vec<String>> = layout
        .overflow_columns
        .iter()
        .map(|column| vec![column.clone()])
        .collect();
    let overflow_columns = vec![RichColumn {
        name: "...".to_string(),
        important: false,
        ellipsis: true,
        table_key: ColumnKey::None,
        index_key: ColumnKey::None,
    }];
    lines.extend(render_table_to_lines(
        "More columns available",
        &overflow_columns,
        &overflow_rows,
        width,
    ));
    lines
}

pub fn table_summary_to_lines(
    tables: &[(TableMeta, TableStats)],
    width: u16,
) -> Vec<Line<'static>> {
    let green = Style::default().fg(Color::Green);
    let columns = vec![
        StyledColumn {
            name: "Name".to_string(),
            header_style: green.add_modifier(Modifier::BOLD),
            cell_style: Some(green),
        },
        styled_column("Items"),
        styled_column("Read"),
        styled_column("Write"),
        styled_column("Status"),
        styled_column("Size"),
    ];
    let rows = tables
        .iter()
        .map(|(meta, stats)| {
            let read = meta
                .total_read_throughput()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string());
            let write = meta
                .total_write_throughput()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string());
            vec![
                meta.name.clone(),
                stats.item_count.to_string(),
                read,
                write,
                status_label(meta.status).to_string(),
                format_size(stats.size_bytes, BINARY),
            ]
        })
        .collect::<Vec<_>>();
    styled_table_to_lines("Tables", &columns, &rows, width)
}

pub fn table_detail_to_lines(
    meta: &TableMeta,
    stats: &TableStats,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let cap = meta.consumed_capacity.get("__table__");
    lines.push(label_line("Name", &meta.name));
    lines.push(label_line("Status", status_label(meta.status)));
    lines.push(label_line("Items", &stats.item_count.to_string()));
    lines.push(label_line("Size", &format_size(stats.size_bytes, BINARY)));
    lines.push(label_line(
        "Read",
        &format_throughput(meta.table_read_throughput(), cap.map(|c| c.read)),
    ));
    lines.push(label_line(
        "Write",
        &format_throughput(meta.table_write_throughput(), cap.map(|c| c.write)),
    ));
    if let Some(hash) = meta.attrs.get(&meta.hash_key) {
        lines.push(label_line(
            "Hash Key",
            &format!("{} ({})", hash.name, type_label(&hash.data_type)),
        ));
    }
    if let Some(range_key) = &meta.range_key {
        if let Some(range) = meta.attrs.get(range_key) {
            lines.push(label_line(
                "Range Key",
                &format!("{} ({})", range.name, type_label(&range.data_type)),
            ));
        }
    }
    if !meta.local_indexes.is_empty() {
        lines.push(Line::from(""));
        lines.push(label_line("Local Indexes", ""));
        for index in meta.local_indexes.values() {
            let range = index.range_key.as_deref().unwrap_or("-");
            lines.push(Line::from(format!(
                "  {}  hash={}  range={}  projection={}",
                index.name,
                index.hash_key,
                range,
                projection_label(&index.projection)
            )));
        }
    }
    if !meta.global_indexes.is_empty() {
        lines.push(Line::from(""));
        let green = Style::default().fg(Color::Green);
        let columns = vec![
            StyledColumn {
                name: "Name".to_string(),
                header_style: green.add_modifier(Modifier::BOLD),
                cell_style: Some(green.add_modifier(Modifier::BOLD)),
            },
            styled_column("Projection"),
            styled_column("Read"),
            styled_column("Write"),
            styled_column("HashKey"),
            styled_column("RangeKey"),
            styled_column("Status"),
        ];
        let rows = meta
            .global_indexes
            .iter()
            .map(|(index_name, gindex)| {
                let idx_cap = meta.consumed_capacity.get(index_name);
                let read = format_throughput(
                    throughput_number(gindex.throughput.as_ref(), true),
                    idx_cap.map(|c| c.read),
                );
                let write = format_throughput(
                    throughput_number(gindex.throughput.as_ref(), false),
                    idx_cap.map(|c| c.write),
                );
                let hash_key = format!(
                    "{} ({})",
                    gindex.hash_key.name,
                    type_label(&gindex.hash_key.data_type)
                );
                let range_key = gindex
                    .range_key
                    .as_ref()
                    .map(|field| format!("{} ({})", field.name, type_label(&field.data_type)))
                    .unwrap_or_else(|| "-".to_string());
                vec![
                    gindex.name.clone(),
                    projection_label(&gindex.projection),
                    read,
                    write,
                    hash_key,
                    range_key,
                    status_label(gindex.status).to_string(),
                ]
            })
            .collect::<Vec<_>>();
        lines.extend(styled_table_to_lines(
            "Global Indexes",
            &columns,
            &rows,
            width,
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(meta.schema_dql()));
    lines
}

fn styled_column(name: &str) -> StyledColumn {
    StyledColumn {
        name: name.to_string(),
        header_style: Style::default().add_modifier(Modifier::BOLD),
        cell_style: None,
    }
}

fn label_line(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}:"), Style::default().fg(Color::Green)),
        Span::raw(" "),
        Span::raw(value.to_string()),
    ])
}

fn styled_table_to_lines(
    title: &str,
    columns: &[StyledColumn],
    rows: &[Vec<String>],
    width: u16,
) -> Vec<Line<'static>> {
    if columns.is_empty() {
        return Vec::new();
    }
    let column_names = columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let width_constraints = content_width_constraints(&column_names, rows);
    let header = Row::new(
        columns
            .iter()
            .map(|column| Cell::from(column.name.clone()).style(column.header_style)),
    );
    let data_rows: Vec<Row> = rows
        .iter()
        .map(|row| {
            Row::new(row.iter().zip(columns).map(|(value, column)| {
                let style = column.cell_style.unwrap_or_default();
                Cell::from(value.clone()).style(style)
            }))
        })
        .collect();
    let table = Table::new(data_rows, width_constraints.clone())
        .header(header)
        .block(
            Block::default()
                .title(title)
                .title_style(Style::default().fg(Color::Green)),
        );
    let height = (rows.len() + 3).max(3) as u16;
    let backend = TestBackend::new(table_area_width(&width_constraints, width, 1), height);
    let mut terminal = Terminal::new(backend).expect("test backend");
    terminal
        .draw(|frame| frame.render_widget(table, frame.area()))
        .expect("draw table");
    buffer_to_lines(terminal.backend().buffer())
}

fn status_label(status: TableStatus) -> &'static str {
    match status {
        TableStatus::Active => "ACTIVE",
        TableStatus::Creating => "CREATING",
        TableStatus::Updating => "UPDATING",
        TableStatus::Deleting => "DELETING",
    }
}

fn type_label(data_type: &dql_parser::AttributeType) -> &str {
    match data_type {
        dql_parser::AttributeType::String => "STRING",
        dql_parser::AttributeType::Number => "NUMBER",
        dql_parser::AttributeType::Binary => "BINARY",
        dql_parser::AttributeType::Bool => "BOOL",
        dql_parser::AttributeType::Other(value) => value.as_str(),
    }
}

fn projection_label(projection: &ProjectionType) -> String {
    match projection {
        ProjectionType::All => "ALL".to_string(),
        ProjectionType::KeysOnly => "KEYS".to_string(),
        ProjectionType::Include(fields) => format!("INCLUDE({})", fields.join(",")),
    }
}

fn throughput_number(throughput: Option<&dql_parser::Throughput>, read: bool) -> Option<f64> {
    let throughput = throughput?;
    let value = if read {
        &throughput.read
    } else {
        &throughput.write
    };
    match value {
        dql_parser::Value::Number(number) => number.parse().ok(),
        _ => None,
    }
}

fn render_table_to_lines(
    title: &str,
    columns: &[RichColumn],
    rows: &[Vec<String>],
    width: u16,
) -> Vec<Line<'static>> {
    if columns.is_empty() {
        return Vec::new();
    }

    let labels = columns.iter().map(header_label).collect::<Vec<_>>();
    let width_constraints = content_width_constraints(&labels, rows);
    let header = Row::new(columns.iter().map(header_cell))
        .style(header_row_style())
        .bottom_margin(1);
    let data_rows: Vec<Row> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            Row::new(
                row.iter()
                    .zip(columns)
                    .map(|(value, column)| data_cell(value, column, index)),
            )
            .style(data_row_style(index))
        })
        .collect();
    let table = Table::new(data_rows, width_constraints.clone())
        .header(header)
        .column_spacing(0)
        .style(card_style())
        .block(result_block(title, rows.len()));

    let height = u16::try_from(rows.len() + 4).unwrap_or(u16::MAX).max(4);
    let area = Rect::new(0, 0, table_area_width(&width_constraints, width, 0), height);
    let mut buffer = Buffer::empty(area);
    table.render(area, &mut buffer);
    paint_header_rule(&mut buffer);
    buffer_to_lines(&buffer)
}

fn card_bg() -> Color {
    live_panel_bg()
}

fn table_key_bg() -> Color {
    status_panel_bg(true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColumnTint {
    Plain,
    Table,
    Index,
    Both,
}

fn card_style() -> Style {
    Style::default().bg(card_bg())
}

fn header_row_style() -> Style {
    Style::default().bg(card_bg()).add_modifier(Modifier::BOLD)
}

fn data_row_style(_row_index: usize) -> Style {
    Style::default().bg(card_bg())
}

fn result_block(title: &str, row_count: usize) -> Block<'static> {
    let meta = row_meta(title, row_count);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .style(card_style())
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .title(
            Line::from(Span::styled(
                format!(" {meta} "),
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Right),
        )
}

fn row_meta(title: &str, count: usize) -> String {
    let (singular, plural) = if title == "More columns available" {
        ("column", "columns")
    } else {
        ("row", "rows")
    };
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn column_tint(column: &RichColumn) -> ColumnTint {
    if column.ellipsis {
        return ColumnTint::Plain;
    }
    let table = column.table_key != ColumnKey::None
        || (column.important && column.index_key == ColumnKey::None);
    let index = column.index_key != ColumnKey::None;
    match (table, index) {
        (true, true) => ColumnTint::Both,
        (true, false) => ColumnTint::Table,
        (false, true) => ColumnTint::Index,
        (false, false) => ColumnTint::Plain,
    }
}

fn accent(tint: ColumnTint) -> Option<Color> {
    match tint {
        ColumnTint::Plain => None,
        ColumnTint::Table => Some(Color::Green),
        ColumnTint::Index => Some(Color::Cyan),
        ColumnTint::Both => Some(Color::Yellow),
    }
}

fn header_bg(tint: ColumnTint) -> Color {
    match tint {
        ColumnTint::Table => table_key_bg(),
        _ => card_bg(),
    }
}

fn cell_bg(tint: ColumnTint, _row_index: usize) -> Color {
    match tint {
        ColumnTint::Table => table_key_bg(),
        _ => card_bg(),
    }
}

fn key_marks(column: &RichColumn) -> Vec<&'static str> {
    let mut marks = Vec::new();
    if column.table_key == ColumnKey::Hash || column.index_key == ColumnKey::Hash {
        marks.push("#");
    }
    if column.table_key == ColumnKey::Range || column.index_key == ColumnKey::Range {
        marks.push("↕");
    }
    marks
}

fn header_label(column: &RichColumn) -> String {
    let mut label = column.name.clone();
    for mark in key_marks(column) {
        label.push(' ');
        label.push_str(mark);
    }
    label
}

fn paint_header_rule(buffer: &mut Buffer) {
    let width = buffer.area.width;
    let y = 2;
    if width < 2 || buffer.area.height <= y {
        return;
    }
    let rule = Style::default().fg(Color::Cyan).bg(card_bg());
    buffer[(0, y)].set_symbol("├").set_style(rule);
    buffer[(width - 1, y)].set_symbol("┤").set_style(rule);
    for x in 1..width - 1 {
        buffer[(x, y)].set_symbol("─").set_style(rule);
    }
}

fn content_width_constraints(headers: &[String], rows: &[Vec<String>]) -> Vec<Constraint> {
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| Constraint::Length(column_width(header, rows, index)))
        .collect()
}

fn column_width(header: &str, rows: &[Vec<String>], index: usize) -> u16 {
    let content_width = rows
        .iter()
        .filter_map(|row| row.get(index))
        .map(|value| value.chars().count())
        .fold(header.chars().count(), usize::max);
    content_width.saturating_add(2).min(u16::MAX as usize) as u16
}

fn table_area_width(
    width_constraints: &[Constraint],
    preferred_width: u16,
    column_spacing: u16,
) -> u16 {
    if width_constraints.is_empty() {
        return preferred_width.max(20);
    }
    let content_width: u16 = width_constraints
        .iter()
        .map(|constraint| match constraint {
            Constraint::Length(width) => *width,
            _ => 0,
        })
        .sum();
    let gaps = width_constraints.len().saturating_sub(1) as u16 * column_spacing;
    // Two extra columns cover a border, or a little trailing room when there is no border.
    let chrome = gaps + 2;
    preferred_width.max(content_width + chrome).max(20)
}

fn header_cell(column: &RichColumn) -> Cell<'static> {
    let tint = column_tint(column);
    let mut name_style = styled_accent(tint);
    if column.ellipsis {
        name_style = Style::default().fg(Color::DarkGray);
    } else {
        name_style = name_style.add_modifier(Modifier::BOLD);
    }
    let mut spans = vec![Span::styled(format!(" {}", column.name), name_style)];
    if let Some(color) = accent(tint) {
        let icon = Style::default().fg(color);
        for mark in key_marks(column) {
            spans.push(Span::styled(format!(" {mark}"), icon));
        }
    }
    Cell::from(Line::from(spans)).style(styled_accent(tint).bg(header_bg(tint)))
}

fn data_cell(value: &str, column: &RichColumn, row_index: usize) -> Cell<'static> {
    let mut style = styled_accent(column_tint(column)).bg(cell_bg(column_tint(column), row_index));
    if column.ellipsis {
        style = style.fg(Color::DarkGray);
    }
    Cell::from(format!(" {value}")).style(style)
}

fn styled_accent(tint: ColumnTint) -> Style {
    match accent(tint) {
        Some(color) => Style::default().fg(color),
        None => Style::default(),
    }
}

fn buffer_to_lines(buffer: &ratatui::buffer::Buffer) -> Vec<Line<'static>> {
    let area = buffer.area();
    let mut lines = Vec::new();
    for y in 0..area.height {
        let mut spans = Vec::new();
        let mut x = 0;
        while x < area.width {
            let style = buffer[(x, y)].style();
            let mut text = String::new();
            while x < area.width && buffer[(x, y)].style() == style {
                text.push_str(buffer[(x, y)].symbol());
                x += 1;
            }
            if !text.is_empty() {
                spans.push(Span::styled(text, style));
            }
        }
        if spans.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(spans));
        }
    }
    while line_is_blank(lines.last()) {
        lines.pop();
    }
    lines
}

fn line_is_blank(line: Option<&Line<'_>>) -> bool {
    match line {
        None => true,
        Some(line) if line.spans.is_empty() => true,
        Some(line) => line.spans.iter().all(|span| span.content.trim().is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::app::{live_panel_bg, status_panel_bg};
    use super::*;
    use dql_models::TableMeta;
    use dql_output::{build_rich_layout, RichContext};
    use dql_parser::Value;
    use std::collections::BTreeMap;

    type Item = BTreeMap<String, Value>;

    #[test]
    fn rich_layout_to_lines_includes_headers() {
        let mut item = Item::new();
        item.insert("id".to_string(), Value::String("1".to_string()));
        item.insert("name".to_string(), Value::String("alpha".to_string()));
        let context = RichContext {
            important_columns: vec!["id".to_string()],
            preserve_column_order: false,
            ..RichContext::default()
        };
        let layout = build_rich_layout(&[item], Some(&context));
        let lines = rich_layout_to_lines(&layout, 80);
        let rendered = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Results"));
        assert!(rendered.contains("id"));
        assert!(rendered.contains("name"));
    }

    #[test]
    fn table_summary_to_lines_uses_green_title_and_name_column() {
        use dql_parser::parse_statement;
        let name = "parity_table";
        let statement =
            parse_statement(&format!("CREATE TABLE {name} (id STRING HASH KEY)")).unwrap();
        let meta = TableMeta::from_create_statement(&statement).unwrap();
        let rows = [(
            meta,
            TableStats {
                item_count: 2,
                size_bytes: 24,
            },
        )];
        let lines = table_summary_to_lines(&rows, 100);
        let rendered = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Tables"));
        assert!(rendered.contains(name));
        assert!(lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg == Some(Color::Green))
        }));
    }

    #[test]
    fn table_columns_use_content_width_plus_two_padding() {
        let headers = vec!["Name".to_string(), "Items".to_string()];
        let rows = vec![
            vec!["short".to_string(), "1".to_string()],
            vec!["a-long-table-name".to_string(), "123".to_string()],
        ];

        assert_eq!(
            content_width_constraints(&headers, &rows),
            vec![Constraint::Length(19), Constraint::Length(7)]
        );
    }

    #[test]
    fn table_columns_size_from_header_when_header_is_wider() {
        let headers = vec!["Throughput".to_string()];
        let rows = vec![vec!["1".to_string()], vec!["2".to_string()]];

        assert_eq!(
            content_width_constraints(&headers, &rows),
            vec![Constraint::Length(12)]
        );
    }

    fn column(name: &str, table_key: ColumnKey, index_key: ColumnKey) -> RichColumn {
        RichColumn {
            name: name.to_string(),
            important: table_key != ColumnKey::None || index_key != ColumnKey::None,
            ellipsis: false,
            table_key,
            index_key,
        }
    }

    fn span_style(lines: &[Line<'_>], needle: &str) -> Style {
        for line in lines {
            for span in &line.spans {
                if span.content.contains(needle) {
                    return span.style;
                }
            }
        }
        panic!("missing {needle}");
    }

    #[test]
    fn results_table_uses_the_dark_panel() {
        let layout = RichLayout {
            columns: vec![
                column("id", ColumnKey::Hash, ColumnKey::None),
                column("name", ColumnKey::None, ColumnKey::None),
            ],
            rows: vec![
                vec!["a".to_string(), "Earth".to_string()],
                vec!["b".to_string(), "Mars".to_string()],
            ],
            overflow_columns: Vec::new(),
        };
        let lines = rich_layout_to_lines(&layout, 40);
        let rendered = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Results"), "{rendered}");
        assert!(rendered.contains("2 rows"), "{rendered}");
        assert!(rendered.contains('╭'), "{rendered}");
        assert!(lines[2].to_string().contains('├'), "{rendered}");

        let title = span_style(&lines, "Results");
        assert_eq!(title.fg, Some(Color::Green));
        assert_eq!(title.bg, Some(live_panel_bg()));
        assert!(title.add_modifier.contains(Modifier::BOLD));
        assert_eq!(span_style(&lines, "2 rows").fg, Some(Color::DarkGray));
        assert_eq!(span_style(&lines, "╭").fg, Some(Color::Cyan));

        let id = span_style(&lines, "id");
        assert_eq!(id.fg, Some(Color::Green));
        assert_eq!(id.bg, Some(status_panel_bg(true)));
        assert_eq!(span_style(&lines, "#").fg, Some(Color::Green));

        let earth = span_style(&lines, "Earth");
        assert_eq!(earth.fg, Some(Color::Reset));
        assert_eq!(earth.bg, Some(live_panel_bg()));
        let mars = span_style(&lines, "Mars");
        assert_eq!(mars.bg, Some(live_panel_bg()));

        let id_value = span_style(&lines, " a");
        assert_eq!(id_value.fg, Some(Color::Green));
        assert_eq!(id_value.bg, Some(status_panel_bg(true)));
        let second_key = span_style(&lines, " b");
        assert_eq!(second_key.bg, Some(status_panel_bg(true)));
    }

    #[test]
    fn index_and_shared_keys_use_tui_accents() {
        let layout = RichLayout {
            columns: vec![
                column("name", ColumnKey::None, ColumnKey::Hash),
                column("sk", ColumnKey::Range, ColumnKey::Range),
            ],
            rows: vec![vec!["Earth".to_string(), "a".to_string()]],
            overflow_columns: Vec::new(),
        };
        let lines = rich_layout_to_lines(&layout, 48);
        assert_eq!(span_style(&lines, "name").fg, Some(Color::Cyan));
        assert_eq!(span_style(&lines, "name").bg, Some(live_panel_bg()));
        assert_eq!(span_style(&lines, "Earth").fg, Some(Color::Cyan));
        assert_eq!(span_style(&lines, "Earth").bg, Some(live_panel_bg()));
        assert_eq!(span_style(&lines, "#").fg, Some(Color::Cyan));
        assert_eq!(span_style(&lines, "sk").fg, Some(Color::Yellow));
        assert_eq!(span_style(&lines, "sk").bg, Some(live_panel_bg()));
        assert_eq!(span_style(&lines, " a").fg, Some(Color::Yellow));
        assert_eq!(span_style(&lines, "↕").fg, Some(Color::Yellow));
        let rendered = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("1 row"), "{rendered}");
    }
}
