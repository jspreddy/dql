//! Small marks copied from the web UI's inline SVG.
//!
//! These are symbolic SVGs, recolored at draw time. Canvas meshes are a
//! separate render pass and were painting on top of later windows.

use iced::widget::svg::{self, Handle};
use iced::{Color, Element, Length};

#[derive(Clone, Copy)]
pub enum Mark {
    Settings,
    Moon,
    Sun,
    Forward,
    Play,
    Schema,
    Info,
    Close,
    Eye,
    Hash,
    Range,
    Refresh,
}

pub fn mark<'a, Message: 'a>(kind: Mark, color: Color, px: f32) -> Element<'a, Message> {
    svg::Svg::new(handle(kind))
        .width(Length::Fixed(px))
        .height(Length::Fixed(px))
        .style(move |_theme, _status| svg::Style { color: Some(color) })
        .into()
}

fn handle(kind: Mark) -> Handle {
    Handle::from_memory(source(kind).as_bytes())
}

fn source(kind: Mark) -> &'static str {
    match kind {
        Mark::Settings => SETTINGS,
        Mark::Moon => MOON,
        Mark::Sun => SUN,
        Mark::Forward => FORWARD,
        Mark::Play => PLAY,
        Mark::Schema => SCHEMA,
        Mark::Info => INFO,
        Mark::Close => CLOSE,
        Mark::Eye => EYE,
        Mark::Hash => HASH,
        Mark::Range => RANGE,
        Mark::Refresh => REFRESH,
    }
}

const SETTINGS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M2.2 4.2H13.8M2.2 8H13.8M2.2 11.8H13.8"/><circle cx="6" cy="4.2" r="1.45"/><circle cx="10.4" cy="8" r="1.45"/><circle cx="5.2" cy="11.8" r="1.45"/></svg>"##;

const MOON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="#000" d="M10.2 2C5 2.4 2.4 5.4 2.8 9C3.2 12.8 6.2 14.8 10.2 14.2C6.4 12.6 5 10.4 5.4 7.8C5.8 5 7.6 3 10.2 2Z"/></svg>"##;

const SUN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round"><circle cx="8" cy="8" r="2.6"/><path d="M8 1.6V3.2M8 12.8V14.4M1.6 8H3.2M12.8 8H14.4M3.3 3.3L4.4 4.4M11.6 11.6L12.7 12.7M12.7 3.3L11.6 4.4M4.4 11.6L3.3 12.7"/></svg>"##;

const FORWARD: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="#000" d="M1.6 3.1L7.2 8L1.6 12.9Z"/><path fill="#000" d="M8.4 3.1L14 8L8.4 12.9Z"/></svg>"##;

const PLAY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="#000" d="M5.2 3.1L12.4 8L5.2 12.9Z"/></svg>"##;

const SCHEMA: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><rect x="2.5" y="3.2" width="11" height="9.6"/><path d="M2.5 6.4H13.5M6.7 3.2V12.8"/></svg>"##;

const INFO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14 14" fill="none" stroke="#000" stroke-linecap="round"><circle cx="7" cy="7" r="5.2" stroke-width="1.4"/><path d="M7 6.3V9.7M7 4.4V4.6" stroke-width="1.5"/></svg>"##;

const CLOSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round"><path d="M2 2L10 10M10 2L2 10"/></svg>"##;

const EYE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M1.4 8Q8 3.2 14.6 8Q8 12.8 1.4 8Z"/><circle cx="8" cy="8" r="1.7"/></svg>"##;

const HASH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none" stroke="#000" stroke-width="1.3" stroke-linecap="round"><path d="M4.2 1.2V10.8M7.8 1.2V10.8M1.3 4.2H10.7M1.3 7.8H10.7"/></svg>"##;

const RANGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none" stroke="#000" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"><path d="M6 1.3V10.7M6 1.3L3.7 3.6M6 1.3L8.3 3.6M6 10.7L3.7 8.4M6 10.7L8.3 8.4"/></svg>"##;

const REFRESH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12.43 10.72A5.2 5.2 0 1 1 12.6 5.58"/><path d="M13.4 2.2V5.3M13.4 2.2H10.3"/></svg>"##;
