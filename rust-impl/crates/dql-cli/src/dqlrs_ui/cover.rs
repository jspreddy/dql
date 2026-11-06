//! A window that paints above every float.
//!
//! iced draws `float` widgets as overlays, and those overlays paint after the
//! stack. A sheet that is only a later stack layer ends up underneath the
//! gutter play button and any other float. This cover is itself an overlay
//! with a higher index, so the sheet stays above them. Tooltips inside the
//! sheet nest on top of the cover.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{tree, Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::{mouse, Element, Event, Length, Rectangle, Size, Theme, Vector};

pub fn cover<'a, Message: 'a>(
    content: impl Into<Element<'a, Message, Theme, iced::Renderer>>,
) -> Element<'a, Message, Theme, iced::Renderer> {
    Element::new(Cover {
        content: content.into(),
    })
}

struct CoverState;

struct Cover<'a, Message> {
    content: Element<'a, Message, Theme, iced::Renderer>,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Cover<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<CoverState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(CoverState)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        if tree.tag != self.tag() || tree.children.len() != 1 {
            tree.tag = self.tag();
            tree.state = self.state();
            tree.children = self.children();
            return;
        }
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let size = child.size();
        layout::Node::with_children(size, vec![child])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let Some(child) = layout.children().next() else {
            return;
        };
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            child,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let Some(child) = layout.children().next() else {
            return;
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            child,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let Some(child) = layout.children().next() else {
            return;
        };
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], child, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let Some(child) = layout.children().next() else {
            return mouse::Interaction::None;
        };
        let interaction = self.content.as_widget().mouse_interaction(
            &tree.children[0],
            child,
            cursor,
            viewport,
            renderer,
        );
        if interaction == mouse::Interaction::None && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            interaction
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        _renderer: &iced::Renderer,
        viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        let child = layout.children().next()?;
        Some(overlay::Element::new(Box::new(CoverOverlay {
            content: &mut self.content,
            tree: &mut tree.children[0],
            layout: child,
            viewport: *viewport,
        })))
    }
}

struct CoverOverlay<'a, 'b, Message> {
    content: &'a mut Element<'b, Message, Theme, iced::Renderer>,
    tree: &'a mut Tree,
    layout: Layout<'a>,
    viewport: Rectangle,
}

impl<Message> overlay::Overlay<Message, Theme, iced::Renderer> for CoverOverlay<'_, '_, Message> {
    fn layout(&mut self, _renderer: &iced::Renderer, _bounds: Size) -> layout::Node {
        let bounds = self.layout.bounds();
        layout::Node::new(bounds.size()).move_to(bounds.position())
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.content.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            self.layout,
            cursor,
            &self.viewport,
        );
    }

    fn update(
        &mut self,
        event: &Event,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let press = matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_)));
        self.content.as_widget_mut().update(
            self.tree,
            event,
            self.layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &self.viewport,
        );
        if press && cursor.is_over(self.layout.bounds()) {
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let interaction = self.content.as_widget().mouse_interaction(
            self.tree,
            self.layout,
            cursor,
            &self.viewport,
            renderer,
        );
        if interaction == mouse::Interaction::None && cursor.is_over(self.layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            interaction
        }
    }

    fn overlay<'a>(
        &'a mut self,
        _layout: Layout<'a>,
        renderer: &iced::Renderer,
    ) -> Option<overlay::Element<'a, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            self.tree,
            self.layout,
            renderer,
            &self.viewport,
            Vector::ZERO,
        )
    }

    fn index(&self) -> f32 {
        // Floats use `scale * 0.5` (0.5). Tooltips use the default 1.
        // Autoscroll icons use f32::MAX and stay above this during a drag.
        20.0
    }
}
