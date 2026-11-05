//! Small marks copied from the web UI's inline SVG.
//!
//! Each mark is drawn in its own view box and scaled to the widget size, so a
//! 14px button icon keeps the same stroke as the 16px source.

use iced::widget::canvas::{self, LineCap, LineJoin, Path, Stroke};
use iced::{Color, Element, Point, Radians, Rectangle, Renderer, Theme};

#[derive(Clone, Copy)]
pub enum Mark {
    Settings,
    Moon,
    Sun,
    Forward,
    Play,
    Schema,
    Close,
    Eye,
    Hash,
    Range,
    Refresh,
}

pub fn mark<'a, Message: 'a>(kind: Mark, color: Color, px: f32) -> Element<'a, Message> {
    canvas::Canvas::new(Glyph { kind, color })
        .width(px)
        .height(px)
        .into()
}

struct Glyph {
    kind: Mark,
    color: Color,
}

impl<Message> canvas::Program<Message> for Glyph {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let view = view_box(self.kind);
        frame.scale(bounds.width / view);
        paint(&mut frame, self.kind, self.color);
        vec![frame.into_geometry()]
    }
}

fn view_box(kind: Mark) -> f32 {
    match kind {
        Mark::Close | Mark::Hash | Mark::Range => 12.0,
        _ => 16.0,
    }
}

fn paint(frame: &mut canvas::Frame, kind: Mark, color: Color) {
    match kind {
        Mark::Settings => {
            let pen = stroke(color, 1.4);
            for y in [4.2, 8.0, 11.8] {
                frame.stroke(&Path::line(pt(2.2, y), pt(13.8, y)), pen);
            }
            for (x, y) in [(6.0, 4.2), (10.4, 8.0), (5.2, 11.8)] {
                frame.stroke(&Path::circle(pt(x, y), 1.45), pen);
            }
        }
        Mark::Moon => frame.fill(&moon(), color),
        Mark::Sun => {
            let pen = stroke(color, 1.4);
            frame.stroke(&Path::circle(pt(8.0, 8.0), 2.6), pen);
            for (from, to) in [
                (pt(8.0, 1.6), pt(8.0, 3.2)),
                (pt(8.0, 12.8), pt(8.0, 14.4)),
                (pt(1.6, 8.0), pt(3.2, 8.0)),
                (pt(12.8, 8.0), pt(14.4, 8.0)),
                (pt(3.3, 3.3), pt(4.4, 4.4)),
                (pt(11.6, 11.6), pt(12.7, 12.7)),
                (pt(12.7, 3.3), pt(11.6, 4.4)),
                (pt(4.4, 11.6), pt(3.3, 12.7)),
            ] {
                frame.stroke(&Path::line(from, to), pen);
            }
        }
        Mark::Forward => {
            frame.fill(&triangle(1.6, 7.2), color);
            frame.fill(&triangle(8.4, 14.0), color);
        }
        Mark::Play => frame.fill(&triangle(5.2, 12.4), color),
        Mark::Schema => {
            let pen = stroke(color, 1.4);
            frame.stroke(
                &Path::rectangle(pt(2.5, 3.2), iced::Size::new(11.0, 9.6)),
                pen,
            );
            frame.stroke(&Path::line(pt(2.5, 6.4), pt(13.5, 6.4)), pen);
            frame.stroke(&Path::line(pt(6.7, 3.2), pt(6.7, 12.8)), pen);
        }
        Mark::Close => {
            let pen = stroke(color, 1.6);
            frame.stroke(&Path::line(pt(2.0, 2.0), pt(10.0, 10.0)), pen);
            frame.stroke(&Path::line(pt(10.0, 2.0), pt(2.0, 10.0)), pen);
        }
        Mark::Eye => {
            let pen = stroke(color, 1.4);
            frame.stroke(
                &Path::new(|path| {
                    path.move_to(pt(1.4, 8.0));
                    path.quadratic_curve_to(pt(8.0, 3.2), pt(14.6, 8.0));
                    path.quadratic_curve_to(pt(8.0, 12.8), pt(1.4, 8.0));
                    path.close();
                }),
                pen,
            );
            frame.stroke(&Path::circle(pt(8.0, 8.0), 1.7), pen);
        }
        Mark::Hash => {
            let pen = stroke(color, 1.3);
            frame.stroke(&Path::line(pt(4.2, 1.2), pt(4.2, 10.8)), pen);
            frame.stroke(&Path::line(pt(7.8, 1.2), pt(7.8, 10.8)), pen);
            frame.stroke(&Path::line(pt(1.3, 4.2), pt(10.7, 4.2)), pen);
            frame.stroke(&Path::line(pt(1.3, 7.8), pt(10.7, 7.8)), pen);
        }
        Mark::Range => {
            let pen = stroke(color, 1.3);
            frame.stroke(&Path::line(pt(6.0, 1.3), pt(6.0, 10.7)), pen);
            frame.stroke(&Path::line(pt(6.0, 1.3), pt(3.7, 3.6)), pen);
            frame.stroke(&Path::line(pt(6.0, 1.3), pt(8.3, 3.6)), pen);
            frame.stroke(&Path::line(pt(6.0, 10.7), pt(3.7, 8.4)), pen);
            frame.stroke(&Path::line(pt(6.0, 10.7), pt(8.3, 8.4)), pen);
        }
        Mark::Refresh => {
            let pen = stroke(color, 1.5);
            frame.stroke(
                &Path::new(|path| {
                    path.arc(canvas::path::Arc {
                        center: pt(8.0, 8.0),
                        radius: 5.2,
                        start_angle: Radians(0.55),
                        end_angle: Radians(5.8),
                    });
                }),
                pen,
            );
            frame.stroke(&Path::line(pt(13.4, 2.2), pt(13.4, 5.3)), pen);
            frame.stroke(&Path::line(pt(13.4, 2.2), pt(10.3, 2.2)), pen);
        }
    }
}

fn stroke(color: Color, width: f32) -> Stroke<'static> {
    Stroke::default()
        .with_color(color)
        .with_width(width)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round)
}

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn triangle(left: f32, right: f32) -> Path {
    Path::new(|path| {
        path.move_to(pt(left, 3.1));
        path.line_to(pt(right, 8.0));
        path.line_to(pt(left, 12.9));
        path.close();
    })
}

fn moon() -> Path {
    Path::new(|path| {
        path.move_to(pt(10.2, 2.0));
        path.bezier_curve_to(pt(5.0, 2.4), pt(2.4, 5.4), pt(2.8, 9.0));
        path.bezier_curve_to(pt(3.2, 12.8), pt(6.2, 14.8), pt(10.2, 14.2));
        path.bezier_curve_to(pt(6.4, 12.6), pt(5.0, 10.4), pt(5.4, 7.8));
        path.bezier_curve_to(pt(5.8, 5.0), pt(7.6, 3.0), pt(10.2, 2.0));
        path.close();
    })
}
