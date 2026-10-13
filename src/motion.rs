use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
    widget::{Operation, Tree},
};
use iced::{Element, Event, Length, Rectangle, Size, Transformation, Vector};

/// Apply a centered visual scale without resizing the window or reflowing rows.
pub fn scale<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    scale: f32,
) -> Element<'a, Message> {
    Element::new(Surface {
        content: content.into(),
        scale,
        on_wheel: None,
    })
}

/// Route wheel deltas to the frame-driven scroll animation. Scrollbar dragging
/// and touch input continue to use the native scrollable widget.
pub fn smooth_scroll<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_wheel: fn(f32) -> Message,
) -> Element<'a, Message> {
    Element::new(Surface {
        content: content.into(),
        scale: 1.0,
        on_wheel: Some(on_wheel),
    })
}
struct Surface<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    scale: f32,
    on_wheel: Option<fn(f32) -> Message>,
}
impl<Message, Theme, Renderer> Surface<'_, Message, Theme, Renderer> {
    fn transform(&self, bounds: Rectangle) -> Transformation {
        let center = bounds.center();
        Transformation::translate(center.x, center.y)
            * Transformation::scale(self.scale)
            * Transformation::translate(-center.x, -center.y)
    }
}
impl<Message: Clone, Theme, Renderer: renderer::Renderer> Widget<Message, Theme, Renderer>
    for Surface<'_, Message, Theme, Renderer>
{
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let inverse = self.transform(layout.bounds()).inverse();
        let cursor = cursor * inverse;
        if let Some(on_wheel) = self.on_wheel
            && cursor.is_over(layout.bounds())
            && let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
        {
            let pixels = match delta {
                mouse::ScrollDelta::Lines { y, .. } => -y * 42.0,
                mouse::ScrollDelta::Pixels { y, .. } => -y,
            };
            shell.publish(on_wheel(pixels));
            shell.capture_event();
            return;
        }
        let event = match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                Event::Mouse(mouse::Event::CursorMoved {
                    position: *position * inverse,
                })
            }
            _ => event.clone(),
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            &event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &(*viewport * inverse),
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let transform = self.transform(layout.bounds());
        renderer.with_transformation(transform, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor * transform.inverse(),
                &(*viewport * transform.inverse()),
            );
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor * self.transform(layout.bounds()).inverse(),
            viewport,
            renderer,
        )
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
