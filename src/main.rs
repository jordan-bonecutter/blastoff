mod config;
mod desktop;
mod hover;
mod motion;

use std::time::Instant;

use config::{Config, parse_color};
use desktop::App;
use iced::widget::{
    button, column, container, image, row, rule, scrollable, space, svg, text, text_input,
};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Subscription, Task, Theme, event,
    keyboard, mouse, window,
};

#[derive(Debug, Clone)]
enum Message {
    WindowEvent(window::Id, iced::Event),
    Query(String),
    Move(i32),
    Launch,
    Click(usize),
    Hover(usize),
    Close,
    Focus(bool),
    Scrolled(scrollable::Viewport),
    PrepareOverlay,
    Frame(Instant),
    Wheel(f32),
    MonitorSize(window::Id, Option<iced::Size>),
    Resized(iced::Size),
    Pointer(iced::Point),
    PointerLeft,
    PointerPressed,
}
struct Launcher {
    config: Config,
    apps: Vec<App>,
    query: String,
    results: Vec<usize>,
    selected: usize,
    error: Option<String>,
    has_focused: bool,
    scroll_offset: f32,
    button_launch: bool,
    overlay_size: Option<iced::Size>,
    overlay_window: Option<window::Id>,
    window_size: iced::Size,
    pointer: Option<iced::Point>,
    pointer_entered: bool,
    appearance: f32,
    closing: bool,
    last_frame: Option<Instant>,
    scroll_target: f32,
    viewport_height: Option<f32>,
}
impl Launcher {
    fn boot(config: Config, button_launch: bool) -> (Self, Task<Message>) {
        let apps = desktop::discover(&config);
        let results = desktop::rank(&apps, "");
        let window_size = panel_size(&config);
        let overlay = if button_launch {
            window::latest().and_then(|id| {
                window::monitor_size(id).map(move |size| Message::MonitorSize(id, size))
            })
        } else {
            Task::none()
        };
        (
            Self {
                config,
                apps,
                query: String::new(),
                results,
                selected: 0,
                error: None,
                has_focused: false,
                scroll_offset: 0.0,
                button_launch,
                overlay_size: None,
                overlay_window: None,
                window_size,
                pointer: None,
                pointer_entered: false,
                appearance: 0.0,
                closing: false,
                last_frame: None,
                scroll_target: 0.0,
                viewport_height: None,
            },
            Task::batch([iced::widget::operation::focus("search"), overlay]),
        )
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.closing && !matches!(message, Message::Frame(_)) {
            return Task::none();
        }
        match message {
            Message::WindowEvent(id, event) => {
                if (!self.button_launch || self.overlay_window == Some(id))
                    && let Some(message) = input_message(event)
                {
                    return self.update(message);
                }
            }
            Message::Query(query) => {
                self.query = query;
                self.results = desktop::rank(&self.apps, &self.query);
                self.selected = 0;
                self.error = None;
                self.viewport_height = None;
                self.scroll_offset = 0.0;
                self.scroll_target = 0.0;
                return iced::widget::operation::scroll_to(
                    "results",
                    scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
                );
            }
            Message::Move(delta) => {
                if !self.results.is_empty() {
                    self.selected = self
                        .selected
                        .saturating_add_signed(delta as isize)
                        .min(self.results.len() - 1);
                    self.scroll_target =
                        selection_offset(self.selected, self.scroll_target, self.list_height())
                            .min(self.max_scroll());
                    return self.wake_animation();
                }
            }
            Message::Scrolled(viewport) => {
                self.viewport_height = Some(viewport.bounds().height);
                let offset = viewport.absolute_offset().y;
                // Redraw notifications also report our own animated operations.
                // Only scrollbar/touch changes should replace the destination.
                if (offset - self.scroll_offset).abs() > 0.5 {
                    self.scroll_target = offset;
                }
                self.scroll_offset = offset;
                self.scroll_target = self.scroll_target.min(self.max_scroll());
            }
            Message::Wheel(delta) => {
                self.scroll_target = (self.scroll_target + delta).clamp(0.0, self.max_scroll());
                return self.wake_animation();
            }
            Message::Frame(now) => {
                if self.button_launch && !self.overlay_ready() {
                    self.last_frame = None;
                    return Task::none();
                }
                let dt = self
                    .last_frame
                    .map_or(1.0 / 60.0, |last| {
                        now.saturating_duration_since(last).as_secs_f32()
                    })
                    .min(0.05);
                self.last_frame = Some(now);
                self.appearance = advance_appearance(self.appearance, self.closing, dt);
                if self.closing && self.appearance == 0.0 {
                    return iced::exit();
                }
                let offset = smooth_offset(self.scroll_offset, self.scroll_target, dt);
                if offset != self.scroll_offset {
                    self.scroll_offset = offset;
                    return iced::widget::operation::scroll_to(
                        "results",
                        scrollable::AbsoluteOffset { x: 0.0, y: offset },
                    );
                }
            }
            Message::Hover(index) => {
                if index < self.results.len() {
                    self.selected = index;
                }
            }
            Message::Click(index) => {
                self.selected = index;
                return self.update(Message::Launch);
            }
            Message::Launch => {
                if let Some(&index) = self.results.get(self.selected) {
                    match self.apps[index].launch(&self.config) {
                        Ok(()) => return self.dismiss(),
                        Err(e) => {
                            self.error = Some(e);
                            self.viewport_height = None;
                            self.scroll_target = selection_offset(
                                self.selected,
                                self.scroll_target,
                                self.list_height(),
                            )
                            .min(self.max_scroll());
                            return self.wake_animation();
                        }
                    }
                }
            }
            Message::PrepareOverlay => {
                if self.overlay_size.is_none() {
                    return window::latest().and_then(|id| {
                        window::monitor_size(id).map(move |size| Message::MonitorSize(id, size))
                    });
                }
            }
            Message::MonitorSize(id, Some(size)) => {
                if self.overlay_size.is_none() {
                    self.overlay_size = Some(size);
                    let (overlay_id, open) = window::open(window_settings(size, self.config.blur));
                    self.overlay_window = Some(overlay_id);
                    return open.then(move |_| {
                        Task::batch([window::close(id), iced::widget::operation::focus("search")])
                    });
                }
            }
            // Wayland may not assign a monitor until the surface is mapped.
            // Retry on frames rather than treating the startup race as an error.
            Message::MonitorSize(_, None) => {}
            Message::Resized(size) => {
                self.window_size = size;
                if self.overlay_ready() && self.pointer_inside() {
                    self.pointer_entered = true;
                }
            }
            Message::Pointer(position) => {
                self.pointer = Some(position);
                if self.overlay_ready() {
                    if self.pointer_inside() {
                        self.pointer_entered = true;
                    } else if self.pointer_entered {
                        return self.dismiss();
                    }
                }
            }
            Message::PointerLeft => {
                self.pointer = None;
                // Leaving the Wayland surface can mean another window was
                // raised under a stationary pointer. Panel exit is determined
                // by actual motion across its bounds, not pointer focus loss.
            }
            Message::PointerPressed => {
                if self.overlay_ready() && !self.pointer_inside() {
                    return self.dismiss();
                }
            }
            Message::Close => return self.dismiss(),
            Message::Focus(focused) => {
                if focused {
                    self.has_focused = true;
                    return iced::widget::operation::focus("search");
                } else if !self.button_launch && self.has_focused && self.config.close_on_unfocus {
                    return self.dismiss();
                }
            }
        }
        Task::none()
    }
    fn dismiss(&mut self) -> Task<Message> {
        self.closing = true;
        self.wake_animation()
    }
    fn wake_animation(&mut self) -> Task<Message> {
        self.last_frame = None;
        Task::done(Message::Frame(Instant::now()))
    }
    fn list_height(&self) -> f32 {
        self.viewport_height.unwrap_or_else(|| {
            panel_size(&self.config).height
                - 48.0
                - (26.0 * 1.3 + 28.0)
                - 1.0
                - 32.0
                - if self.error.is_some() { 32.0 } else { 0.0 }
        })
    }
    fn max_scroll(&self) -> f32 {
        (self.results.len() as f32 * 70.0 - 4.0 - self.list_height()).max(0.0)
    }
    fn overlay_ready(&self) -> bool {
        self.button_launch && self.overlay_size == Some(self.window_size)
    }
    fn pointer_inside(&self) -> bool {
        self.pointer.is_some_and(|point| {
            panel_bounds(self.window_size, panel_size(&self.config)).contains(point)
        })
    }
    fn subscription(&self) -> Subscription<Message> {
        let events = event::listen_with(|event, _, id| Some(Message::WindowEvent(id, event)));
        let animating =
            self.appearance < 1.0 || self.closing || self.scroll_offset != self.scroll_target;
        let mut subscriptions = vec![events];
        if self.button_launch && self.overlay_size.is_none() {
            subscriptions.push(window::frames().map(|_| Message::PrepareOverlay));
        }
        if animating {
            subscriptions.push(window::frames().map(Message::Frame));
        }
        Subscription::batch(subscriptions)
    }

    fn view(&self) -> Element<'_, Message> {
        if self.button_launch && self.overlay_size.is_none() {
            return space().into();
        }
        let bg = Color {
            a: self.config.opacity * self.appearance,
            ..parse_color(&self.config.background).unwrap()
        };
        let fg = fade(
            parse_color(&self.config.foreground).unwrap(),
            self.appearance,
        );
        let accent = fade(parse_color(&self.config.accent).unwrap(), self.appearance);
        let muted = fade(fg, 0.5);
        let search = text_input("", &self.query)
            .id("search")
            .on_input(Message::Query)
            .on_submit(Message::Launch)
            .size(26)
            .padding([14, 0])
            .style(move |_, _| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                icon: accent,
                placeholder: muted,
                value: fg,
                selection: fade(accent, 0.3),
            });
        let header = row![
            svg(svg::Handle::from_memory(br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><circle cx="13" cy="13" r="8" fill="none" stroke="#d4d4d4" stroke-width="2.2"/><path d="M19 19L27 27" fill="none" stroke="#d4d4d4" stroke-width="2.2" stroke-linecap="round"/></svg>"##.as_slice())).width(30).height(30).opacity(self.appearance).style(move |_, _| svg::Style { color: Some(accent) }),
            search
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        let mut entries = column![].spacing(4);
        for (position, &index) in self.results.iter().enumerate() {
            let app = &self.apps[index];
            let selected = position == self.selected;
            let icon: Element<'_, Message> = match &app.icon_path {
                Some(path) if path.extension().is_some_and(|e| e == "svg") => {
                    svg(svg::Handle::from_path(path))
                        .width(38)
                        .height(38)
                        .opacity(self.appearance)
                        .into()
                }
                Some(path) => image(image::Handle::from_path(path))
                    .filter_method(image::FilterMethod::Linear)
                    .width(38)
                    .height(38)
                    .opacity(self.appearance)
                    .into(),
                None => container(
                    text(app.name.chars().next().unwrap_or('?').to_string())
                        .size(22)
                        .color(accent),
                )
                .center(38)
                .style(move |_| container::Style {
                    background: Some(Background::Color(fade(accent, 0.12))),
                    border: Border {
                        radius: 10.0.into(),
                        ..Border::default()
                    },
                    ..Default::default()
                })
                .into(),
            };
            let description = if app.description.is_empty() {
                "Application"
            } else {
                &app.description
            };
            let content = row![
                icon,
                column![
                    text(&app.name)
                        .size(self.config.font_size)
                        .color(fg)
                        .wrapping(text::Wrapping::None),
                    text(description)
                        .size(12)
                        .color(muted)
                        .wrapping(text::Wrapping::None)
                ]
                .spacing(3)
                .width(Length::Fill)
            ]
            .spacing(14)
            .align_y(Alignment::Center);
            entries = entries.push(hover::row(
                button(content)
                    .on_press(Message::Click(position))
                    .width(Length::Fill)
                    .height(66)
                    .padding([10, 14])
                    .style(move |_, _| button::Style {
                        background: Some(Background::Color(if selected {
                            fade(accent, 0.10)
                        } else {
                            Color::TRANSPARENT
                        })),
                        text_color: fg,
                        border: Border {
                            radius: 12.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                Message::Hover(position),
            ));
        }
        let list: Element<'_, Message> = if self.results.is_empty() {
            container(
                column![
                    text(if self.apps.is_empty() {
                        "No applications found"
                    } else {
                        "No matches"
                    })
                    .size(20)
                    .color(fg)
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .center(Length::Fill)
            .into()
        } else {
            motion::smooth_scroll(
                scrollable(entries)
                    .id("results")
                    .height(Length::Fill)
                    .direction(scrollable::Direction::Vertical(
                        scrollable::Scrollbar::new()
                            .width(6)
                            .scroller_width(4)
                            .spacing(8),
                    ))
                    .on_scroll(Message::Scrolled)
                    .style(move |_, _| scrollable::Style {
                        container: container::Style::default(),
                        vertical_rail: scrollable::Rail {
                            background: None,
                            border: Border::default(),
                            scroller: scrollable::Scroller {
                                background: Background::Color(fade(fg, 0.18)),
                                border: Border {
                                    radius: 2.0.into(),
                                    ..Border::default()
                                },
                            },
                        },
                        horizontal_rail: scrollable::Rail {
                            background: None,
                            border: Border::default(),
                            scroller: scrollable::Scroller {
                                background: Background::Color(Color::TRANSPARENT),
                                border: Border::default(),
                            },
                        },
                        gap: None,
                        auto_scroll: scrollable::AutoScroll {
                            background: Background::Color(Color::TRANSPARENT),
                            border: Border::default(),
                            shadow: iced::Shadow::default(),
                            icon: Color::TRANSPARENT,
                        },
                    }),
                Message::Wheel,
            )
        };
        let mut body = column![
            header,
            rule::horizontal(1).style(move |_| rule::Style {
                color: fade(fg, 0.08),
                radius: 0.0.into(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }),
            container(list).height(Length::Fill)
        ]
        .spacing(16);
        if let Some(error) = &self.error {
            body = body.push(
                text(error)
                    .size(12)
                    .color(fade(Color::from_rgb8(255, 150, 150), self.appearance)),
            );
        }
        let panel = container(body)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(fg),
                border: Border {
                    radius: self.config.radius.into(),
                    ..Border::default()
                },
                ..Default::default()
            });
        if self.button_launch {
            let size = panel_size(&self.config);
            container(motion::scale(
                panel.width(size.width).height(size.height),
                appearance_scale(self.appearance),
            ))
            .center(Length::Fill)
            .into()
        } else {
            motion::scale(panel, appearance_scale(self.appearance))
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut path = config::path();
    let mut list = false;
    let mut button_launch = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => path = args.next().ok_or("--config requires a path")?.into(),
            "--list" => list = true,
            "--button-launch" => button_launch = true,
            "--help" | "-h" => {
                println!(
                    "Blastoff — application launcher\n\nUsage: blastoff [--config PATH] [--list] [--button-launch]\n\n--button-launch: dismiss on outside click, pointer enter/exit, or Escape\n\nConfig: {}\nKeys: ↑/↓ navigate, Enter launch, Escape close",
                    config::path().display()
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg:?}").into()),
        }
    }
    let config = Config::load(&path)?;
    if list {
        for app in desktop::discover_entries() {
            println!("{}\t{}", app.id, app.name);
        }
        return Ok(());
    }
    let size = panel_size(&config);
    let blur = config.blur;
    iced::application(
        move || Launcher::boot(config.clone(), button_launch),
        Launcher::update,
        Launcher::view,
    )
    .title("Blastoff")
    .antialiasing(true)
    .theme(Theme::Dark)
    .style(|_, _| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .subscription(Launcher::subscription)
    .window(window_settings(size, blur))
    .run()?;
    Ok(())
}

fn input_message(event: iced::Event) -> Option<Message> {
    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(key),
            ..
        }) => match key {
            keyboard::key::Named::ArrowDown => Some(Message::Move(1)),
            keyboard::key::Named::ArrowUp => Some(Message::Move(-1)),
            keyboard::key::Named::Escape => Some(Message::Close),
            _ => None,
        },
        iced::Event::Window(window::Event::Focused) => Some(Message::Focus(true)),
        iced::Event::Window(window::Event::Unfocused) => Some(Message::Focus(false)),
        iced::Event::Window(window::Event::Resized(size) | window::Event::Opened { size, .. }) => {
            Some(Message::Resized(size))
        }
        iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Some(Message::Pointer(position))
        }
        iced::Event::Mouse(mouse::Event::CursorLeft) => Some(Message::PointerLeft),
        iced::Event::Mouse(mouse::Event::ButtonPressed(_)) => Some(Message::PointerPressed),
        _ => None,
    }
}

fn window_settings(size: iced::Size, blur: bool) -> window::Settings {
    window::Settings {
        size,
        position: window::Position::Centered,
        decorations: false,
        transparent: true,
        blur,
        resizable: false,
        level: window::Level::AlwaysOnTop,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "blastoff".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn panel_size(config: &Config) -> iced::Size {
    iced::Size::new(config.width, 170.0 + config.max_results as f32 * 70.0)
}

fn panel_bounds(window: iced::Size, panel: iced::Size) -> iced::Rectangle {
    iced::Rectangle {
        x: (window.width - panel.width).max(0.0) / 2.0,
        y: (window.height - panel.height).max(0.0) / 2.0,
        width: panel.width.min(window.width),
        height: panel.height.min(window.height),
    }
}

/// Keep the full selected row visible without moving a viewport that already contains it.
fn selection_offset(selected: usize, offset: f32, height: f32) -> f32 {
    let top = selected as f32 * 70.0;
    if top < offset {
        top
    } else {
        offset.max(top + 66.0 - height).max(0.0)
    }
}

fn fade(color: Color, opacity: f32) -> Color {
    Color {
        a: color.a * opacity,
        ..color
    }
}
fn advance_appearance(value: f32, closing: bool, dt: f32) -> f32 {
    (value + if closing { -dt / 0.075 } else { dt / 0.10 }).clamp(0.0, 1.0)
}
fn appearance_scale(appearance: f32) -> f32 {
    // Ease out over just 1.2% of the panel size.
    1.0 - 0.012 * (1.0 - appearance).powi(3)
}
fn smooth_offset(offset: f32, target: f32, dt: f32) -> f32 {
    if (offset - target).abs() < 0.5 {
        target
    } else {
        offset + (target - offset) * (1.0 - (-32.0 * dt).exp())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dismissal_launcher(button_launch: bool, close_on_unfocus: bool) -> Launcher {
        let size = iced::Size::new(1280.0, 960.0);
        Launcher {
            config: Config {
                max_results: 3,
                close_on_unfocus,
                ..Config::default()
            },
            apps: vec![],
            query: String::new(),
            results: vec![],
            selected: 0,
            error: None,
            has_focused: true,
            scroll_offset: 0.0,
            button_launch,
            overlay_size: Some(size),
            overlay_window: None,
            window_size: size,
            pointer: None,
            pointer_entered: false,
            appearance: 1.0,
            closing: false,
            last_frame: None,
            scroll_target: 0.0,
            viewport_height: None,
        }
    }

    #[test]
    fn button_launch_waits_for_entry_and_ignores_unfocus() {
        let mut launcher = dismissal_launcher(true, true);
        assert_eq!(launcher.update(Message::Focus(false)).units(), 0);
        assert_eq!(
            launcher
                .update(Message::Pointer(iced::Point::new(20.0, 20.0)))
                .units(),
            0
        );
        assert_eq!(launcher.update(Message::PointerLeft).units(), 0);
        assert_eq!(
            launcher
                .update(Message::Pointer(iced::Point::new(420.0, 320.0)))
                .units(),
            0
        );
        assert!(launcher.pointer_entered);
        assert_eq!(launcher.update(Message::PointerPressed).units(), 0);
        assert!(
            launcher
                .update(Message::Pointer(iced::Point::new(20.0, 20.0)))
                .units()
                > 0
        );
    }

    #[test]
    fn button_launch_outside_click_and_escape_ignore_config() {
        let mut launcher = dismissal_launcher(true, false);
        let _ = launcher.update(Message::Pointer(iced::Point::new(20.0, 20.0)));
        assert!(launcher.update(Message::PointerPressed).units() > 0);
        assert!(launcher.closing);
        assert_eq!(launcher.update(Message::Close).units(), 0);
        let mut launcher = dismissal_launcher(true, false);
        assert!(launcher.update(Message::Close).units() > 0);
        let _ = launcher.update(Message::Pointer(iced::Point::new(420.0, 320.0)));
        assert_eq!(launcher.update(Message::PointerLeft).units(), 0);
    }

    #[test]
    fn default_dismissal_still_obeys_focus_config() {
        for close_on_unfocus in [true, false] {
            let mut launcher = dismissal_launcher(false, close_on_unfocus);
            let _ = launcher.update(Message::Pointer(iced::Point::new(420.0, 320.0)));
            assert_eq!(launcher.update(Message::PointerLeft).units(), 0);
            assert_eq!(launcher.update(Message::PointerPressed).units(), 0);
            assert_eq!(
                launcher.update(Message::Focus(false)).units() > 0,
                close_on_unfocus
            );
        }
    }

    #[test]
    fn button_launch_ignores_events_from_the_startup_window() {
        let mut launcher = dismissal_launcher(true, true);
        let startup = window::Id::unique();
        let overlay = window::Id::unique();
        launcher.overlay_window = Some(overlay);
        let _ = launcher.update(Message::WindowEvent(
            overlay,
            iced::Event::Mouse(mouse::Event::CursorMoved {
                position: iced::Point::new(420.0, 320.0),
            }),
        ));
        assert!(launcher.pointer_entered);
        assert_eq!(
            launcher
                .update(Message::WindowEvent(
                    startup,
                    iced::Event::Mouse(mouse::Event::CursorLeft)
                ))
                .units(),
            0
        );
        assert!(launcher.pointer_inside());
        let size = launcher.window_size;
        let _ = launcher.update(Message::WindowEvent(
            startup,
            iced::Event::Window(window::Event::Resized(iced::Size::new(680.0, 380.0))),
        ));
        assert_eq!(launcher.window_size, size);
        assert_eq!(
            launcher
                .update(Message::WindowEvent(
                    overlay,
                    iced::Event::Window(window::Event::Unfocused)
                ))
                .units(),
            0
        );
    }

    #[test]
    fn button_launch_arms_a_stationary_pointer_after_window_open() {
        let mut launcher = dismissal_launcher(true, true);
        launcher.window_size = iced::Size::new(680.0, 380.0);
        let _ = launcher.update(Message::Pointer(iced::Point::new(420.0, 320.0)));
        assert!(!launcher.pointer_entered);
        let _ = launcher.update(Message::Resized(iced::Size::new(1280.0, 960.0)));
        assert!(launcher.pointer_entered);
        assert!(
            launcher
                .update(Message::Pointer(iced::Point::new(20.0, 20.0)))
                .units()
                > 0
        );
    }

    #[test]
    fn focus_switch_with_stationary_pointer_does_not_dismiss_button_launch() {
        let mut launcher = dismissal_launcher(true, true);
        let inside = iced::Point::new(420.0, 320.0);
        let _ = launcher.update(Message::Pointer(inside));
        assert!(launcher.pointer_entered);
        assert_eq!(launcher.update(Message::Focus(false)).units(), 0);
        assert_eq!(launcher.update(Message::PointerLeft).units(), 0);
        assert_eq!(launcher.update(Message::Pointer(inside)).units(), 0);
        assert!(
            launcher
                .update(Message::Pointer(iced::Point::new(20.0, 20.0)))
                .units()
                > 0
        );
    }

    #[test]
    fn animation_settles_and_dismissal_reaches_zero() {
        let mut appearance = 0.0;
        for _ in 0..20 {
            appearance = advance_appearance(appearance, false, 1.0 / 60.0);
        }
        assert_eq!(appearance, 1.0);
        assert_eq!(appearance_scale(appearance), 1.0);
        assert!((appearance_scale(0.0) - 0.988).abs() < 0.0001);
        for _ in 0..20 {
            appearance = advance_appearance(appearance, true, 1.0 / 60.0);
        }
        assert_eq!(appearance, 0.0);
        let mut offset = 0.0;
        for _ in 0..30 {
            let next = smooth_offset(offset, 140.0, 1.0 / 60.0);
            assert!(next > offset && next <= 140.0 || next == 140.0);
            offset = next;
        }
        assert_eq!(offset, 140.0);
        assert!(smooth_offset(140.0, 0.0, 1.0 / 60.0) < 140.0);
    }

    #[test]
    fn wheel_scrolling_accumulates_reverses_and_clamps() {
        let mut launcher = dismissal_launcher(false, false);
        launcher.results = (0..20).collect();
        let _ = launcher.update(Message::Wheel(42.0));
        let _ = launcher.update(Message::Wheel(42.0));
        assert_eq!(launcher.scroll_target, 84.0);
        assert_eq!(launcher.scroll_offset, 0.0);
        let _ = launcher.update(Message::Frame(Instant::now()));
        assert!(launcher.scroll_offset > 0.0 && launcher.scroll_offset < 84.0);
        let _ = launcher.update(Message::Wheel(-42.0));
        assert_eq!(launcher.scroll_target, 42.0);
        let _ = launcher.update(Message::Wheel(f32::MAX));
        assert_eq!(launcher.scroll_target, launcher.max_scroll());
        let _ = launcher.update(Message::Wheel(-f32::MAX));
        assert_eq!(launcher.scroll_target, 0.0);
        let _ = launcher.update(Message::Close);
        let offset = launcher.scroll_target;
        let _ = launcher.update(Message::Wheel(42.0));
        assert_eq!(launcher.scroll_target, offset);
    }

    #[test]
    fn selection_scrolls_only_when_row_leaves_viewport() {
        assert_eq!(selection_offset(0, 0.0, 206.0), 0.0);
        assert_eq!(selection_offset(2, 0.0, 206.0), 0.0);
        assert_eq!(selection_offset(3, 0.0, 206.0), 70.0);
        assert_eq!(selection_offset(1, 140.0, 206.0), 70.0);
        assert_eq!(selection_offset(0, 70.0, 206.0), 0.0);
    }
}
