use dql_engine::format_throughput;
use dql_models::{ProjectionType, TableMeta, TableStatus};
use dql_output::TableStats;
use dql_output::{RichColumn, RichLayout};
use humansize::{format_size, BINARY};
use ratatui::backend::TestBackend;
use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Row, Table};
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
    let backend = TestBackend::new(table_area_width(&width_constraints, width), height);
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
    let Some(throughput) = throughput else {
        return None;
    };
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

    let column_names = columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let width_constraints = content_width_constraints(&column_names, rows);
    let header = Row::new(columns.iter().map(|column| header_cell(column)));
    let data_rows: Vec<Row> = rows
        .iter()
        .map(|row| Row::new(row.iter().map(|value| Cell::from(value.clone()))))
        .collect();
    let table = Table::new(data_rows, width_constraints.clone())
        .header(header)
        .block(
            Block::default()
                .title(title)
                .title_style(Style::default().fg(Color::Green)),
        );

    let height = (rows.len() + 3).max(3) as u16;
    let backend = TestBackend::new(table_area_width(&width_constraints, width), height);
    let mut terminal = Terminal::new(backend).expect("test backend");
    terminal
        .draw(|frame| frame.render_widget(table, frame.area()))
        .expect("draw table");
    buffer_to_lines(terminal.backend().buffer())
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

fn table_area_width(width_constraints: &[Constraint], preferred_width: u16) -> u16 {
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
    let chrome = width_constraints.len().saturating_sub(1) as u16 + 2;
    preferred_width.max(content_width + chrome).max(20)
}

fn header_cell(column: &RichColumn) -> Cell<'static> {
    let style = if column.ellipsis {
        Style::default().fg(Color::DarkGray)
    } else if column.important {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    Cell::from(column.name.clone()).style(style.add_modifier(Modifier::BOLD))
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
}
