//! Colors and widget chrome copied from `dqlrs-web/static/theme.css`.
//!
//! The header stays the dark slate bar in both themes. The page under it
//! follows the light or dark paper tokens.

use iced::widget::{button, container, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bar: Color,
    pub header_fg: Color,
    pub header_text: Color,
    pub header_line: Color,
    pub teal: Color,
    pub text: Color,
    pub muted: Color,
    pub side: Color,
    pub paper: Color,
    pub canvas: Color,
    pub line: Color,
    pub line_strong: Color,
    pub blue: Color,
    pub danger: Color,
    pub ok: Color,
    pub ok_bg: Color,
    pub amber: Color,
    pub amber_bg: Color,
    pub status: Color,
    pub gutter: Color,
    pub chip: Color,
    pub selected: Color,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            dark: true,
            bar: rgb(0x0c, 0x11, 0x17),
            header_fg: rgb(0xe8, 0xee, 0xf5),
            header_text: rgb(0xc9, 0xd4, 0xe0),
            header_line: rgb(0x3a, 0x4d, 0x63),
            teal: rgb(0x0f, 0x76, 0x6e),
            text: rgb(0xe7, 0xee, 0xf6),
            muted: rgb(0x8b, 0x9e, 0xb3),
            side: rgb(0x12, 0x19, 0x22),
            paper: rgb(0x17, 0x20, 0x2b),
            canvas: rgb(0x0e, 0x14, 0x1b),
            line: rgb(0x2a, 0x38, 0x48),
            line_strong: rgb(0x33, 0x45, 0x56),
            blue: rgb(0x60, 0xa5, 0xfa),
            danger: rgb(0xfc, 0xa5, 0xa5),
            ok: rgb(0x86, 0xef, 0xac),
            ok_bg: rgb(0x14, 0x2a, 0x1e),
            amber: rgb(0xfd, 0xe6, 0x8a),
            amber_bg: rgb(0x3a, 0x2e, 0x14),
            status: rgb(0x14, 0x1c, 0x26),
            gutter: rgb(0x14, 0x1c, 0x26),
            chip: rgb(0x2a, 0x3a, 0x4e),
            selected: rgb(0x1c, 0x3d, 0x5c),
        }
    } else {
        Palette {
            dark: false,
            bar: rgb(0x1b, 0x28, 0x38),
            header_fg: rgb(0xe8, 0xee, 0xf5),
            header_text: rgb(0xd5, 0xdd, 0xe6),
            header_line: rgb(0x45, 0x56, 0x6b),
            teal: rgb(0x0f, 0x76, 0x6e),
            text: rgb(0x1c, 0x24, 0x30),
            muted: rgb(0x66, 0x74, 0x84),
            side: rgb(0xf3, 0xf5, 0xf7),
            paper: rgb(0xff, 0xff, 0xff),
            canvas: rgb(0xf7, 0xf9, 0xfb),
            line: rgb(0xe4, 0xe7, 0xeb),
            line_strong: rgb(0xe1, 0xe6, 0xeb),
            blue: rgb(0x25, 0x63, 0xeb),
            danger: rgb(0xb9, 0x1c, 0x1c),
            ok: rgb(0x16, 0x65, 0x34),
            ok_bg: rgb(0xf0, 0xfd, 0xf4),
            amber: rgb(0x92, 0x40, 0x0e),
            amber_bg: rgb(0xff, 0xfb, 0xeb),
            status: rgb(0xf1, 0xf5, 0xf9),
            gutter: rgb(0xfa, 0xfb, 0xfc),
            chip: rgb(0xe8, 0xee, 0xf5),
            selected: rgb(0xd9, 0xe6, 0xf6),
        }
    }
}

pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb8(r, g, b)
}

pub fn fill(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style::default().background(color)
}

pub fn card(p: Palette) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        container::Style::default()
            .background(p.paper)
            .color(p.text)
            .border(Border {
                color: p.line_strong,
                width: 1.0,
                radius: 10.0.into(),
            })
    }
}

pub fn menu_card(p: Palette) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        container::Style::default()
            .background(p.paper)
            .color(p.text)
            .border(Border {
                color: p.line,
                width: 1.0,
                radius: 6.0.into(),
            })
            .shadow(Shadow {
                color: Color {
                    r: 0.08,
                    g: 0.12,
                    b: 0.16,
                    a: if p.dark { 0.45 } else { 0.16 },
                },
                offset: Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            })
    }
}

pub fn pop_card(p: Palette) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        container::Style::default()
            .background(p.paper)
            .color(p.text)
            .border(Border {
                color: p.line,
                width: 1.0,
                radius: 8.0.into(),
            })
            .shadow(Shadow {
                color: Color {
                    r: 0.06,
                    g: 0.09,
                    b: 0.15,
                    a: if p.dark { 0.55 } else { 0.28 },
                },
                offset: Vector::new(0.0, 18.0),
                blur_radius: 48.0,
            })
    }
}

pub fn scrim(dark: bool) -> Color {
    Color {
        r: if dark { 0.0 } else { 0.06 },
        g: if dark { 0.0 } else { 0.09 },
        b: if dark { 0.0 } else { 0.15 },
        a: if dark { 0.62 } else { 0.38 },
    }
}

pub struct ColumnPaint {
    pub head: Color,
    pub head_line: Color,
    pub cell: Color,
    pub cell_line: Color,
    pub mark: Color,
    pub keyed: bool,
}

/// Header and body colors for one result column, matching `.col-table`,
/// `.col-index`, and `.col-both`. Plain columns use the gutter and paper.
pub fn column_paint(p: Palette, table_key: Option<&str>, index_key: Option<&str>) -> ColumnPaint {
    let table_key = table_key.filter(|key| !key.is_empty());
    let index_key = index_key.filter(|key| !key.is_empty());
    let (head, head_line, cell, cell_line, mark) = match (table_key, index_key, p.dark) {
        (Some(_), Some(_), true) => (
            (0x16, 0x30, 0x34),
            (0x2a, 0x48, 0x4e),
            (0x12, 0x24, 0x28),
            (0x23, 0x40, 0x44),
            (0x5e, 0xea, 0xd4),
        ),
        (Some(_), Some(_), false) => (
            (0xe7, 0xf3, 0xf4),
            (0xd7, 0xe2, 0xe3),
            (0xf5, 0xfa, 0xfb),
            (0xe7, 0xec, 0xed),
            (0x3d, 0x74, 0x80),
        ),
        (Some(_), None, true) => (
            (0x17, 0x35, 0x28),
            (0x2c, 0x4a, 0x3a),
            (0x12, 0x24, 0x1c),
            (0x23, 0x40, 0x33),
            (0x86, 0xef, 0xac),
        ),
        (Some(_), None, false) => (
            (0xe7, 0xf6, 0xec),
            (0xd7, 0xe5, 0xdc),
            (0xf5, 0xfb, 0xf7),
            (0xe7, 0xed, 0xe9),
            (0x3f, 0x6f, 0x57),
        ),
        (None, Some(_), true) => (
            (0x17, 0x28, 0x3c),
            (0x2a, 0x40, 0x60),
            (0x12, 0x1e, 0x2c),
            (0x22, 0x32, 0x48),
            (0x93, 0xc5, 0xfd),
        ),
        (None, Some(_), false) => (
            (0xe7, 0xf1, 0xfc),
            (0xd7, 0xe0, 0xea),
            (0xf5, 0xf9, 0xfe),
            (0xe7, 0xeb, 0xf0),
            (0x3c, 0x63, 0x99),
        ),
        (None, None, _) => {
            return ColumnPaint {
                head: p.gutter,
                head_line: p.line,
                cell: p.paper,
                cell_line: p.line,
                mark: p.muted,
                keyed: false,
            };
        }
    };
    ColumnPaint {
        head: rgb(head.0, head.1, head.2),
        head_line: rgb(head_line.0, head_line.1, head_line.2),
        cell: rgb(cell.0, cell.1, cell.2),
        cell_line: rgb(cell_line.0, cell_line.1, cell_line.2),
        mark: rgb(mark.0, mark.1, mark.2),
        keyed: true,
    }
}

pub fn chip(bg: Color, _fg: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        container::Style::default().background(bg).border(Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 999.0.into(),
        })
    }
}

#[derive(Clone, Copy)]
pub enum Look {
    Bar,
    ModeOn,
    ModeOff,
    Pill,
    Primary,
    Quiet,
    Danger,
    Stop,
    Control,
    Selected,
    Outline,
}

pub fn paint(p: Palette, look: Look) -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    move |_, status| {
        let (background, text_color, border, width, radius) = match look {
            Look::Bar => (None, p.header_text, Color::TRANSPARENT, 0.0, 4.0),
            Look::ModeOn => (Some(p.teal), Color::WHITE, Color::TRANSPARENT, 0.0, 0.0),
            Look::ModeOff => (None, p.header_text, Color::TRANSPARENT, 0.0, 0.0),
            Look::Pill => (None, p.header_fg, p.header_line, 1.0, 999.0),
            Look::Primary => (Some(p.blue), Color::WHITE, Color::TRANSPARENT, 0.0, 6.0),
            Look::Quiet => (None, p.text, Color::TRANSPARENT, 0.0, 6.0),
            Look::Danger => (None, p.danger, Color::TRANSPARENT, 0.0, 4.0),
            Look::Stop => (Some(p.danger), Color::WHITE, Color::TRANSPARENT, 0.0, 4.0),
            Look::Control => (Some(p.paper), p.text, p.line_strong, 1.0, 4.0),
            Look::Selected => (Some(p.selected), p.text, Color::TRANSPARENT, 0.0, 0.0),
            Look::Outline => (Some(p.paper), p.danger, p.danger, 1.0, 4.0),
        };
        let background = background.map(|color| {
            let color = match status {
                button::Status::Hovered | button::Status::Pressed => darken(color, 0.08),
                _ => color,
            };
            Background::Color(color)
        });
        button::Style {
            background,
            text_color,
            border: Border {
                color: border,
                width,
                radius: radius.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub fn header_field(p: Palette) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + Copy {
    move |_, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: p.header_line,
            width: 1.0,
            radius: 999.0.into(),
        },
        icon: p.header_text,
        placeholder: p.header_text,
        value: p.header_fg,
        selection: p.blue,
    }
}

pub fn field(p: Palette) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + Copy {
    move |_, status| {
        let border = match status {
            text_input::Status::Focused { .. } => p.blue,
            _ => p.line_strong,
        };
        text_input::Style {
            background: Background::Color(p.paper),
            border: Border {
                color: border,
                width: 1.0,
                radius: 6.0.into(),
            },
            icon: p.muted,
            placeholder: p.muted,
            value: p.text,
            selection: p.blue,
        }
    }
}

fn darken(color: Color, amount: f32) -> Color {
    Color {
        r: color.r * (1.0 - amount),
        g: color.g * (1.0 - amount),
        b: color.b * (1.0 - amount),
        a: color.a,
    }
}
