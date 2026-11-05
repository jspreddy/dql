//! Colors and widget chrome copied from `dqlrs-web/static/theme.css`.
//!
//! The header stays the dark slate bar in both themes. The page under it
//! follows the light or dark paper tokens.

use iced::widget::{button, container, text_input};
use iced::{Background, Border, Color, Shadow, Theme};

#[derive(Clone, Copy)]
pub struct Palette {
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
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
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
        }
    } else {
        Palette {
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
                radius: 8.0.into(),
            })
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
            Look::Danger => (None, p.danger, Color::TRANSPARENT, 0.0, 6.0),
            Look::Stop => (Some(p.danger), Color::WHITE, Color::TRANSPARENT, 0.0, 6.0),
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
