mod config;
mod desktop;

use config::{Config, parse_color};
use desktop::App;
use iced::widget::{button, column, container, image, row, rule, space, svg, text, text_input};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Subscription, Task, Theme, event,
    keyboard, window,
};

#[derive(Debug, Clone)]
enum Message {
    Query(String),
    Move(i32),
    Launch,
    Click(usize),
    Close,
    Focus(bool),
}
struct Launcher {
    config: Config,
    apps: Vec<App>,
    query: String,
    results: Vec<usize>,
    selected: usize,
    error: Option<String>,
    has_focused: bool,
}
impl Launcher {
    fn boot(config: Config) -> (Self, Task<Message>) {
        let apps = desktop::discover(&config);
        let results = desktop::rank(&apps, "", config.max_results);
        (
            Self {
                config,
                apps,
                query: String::new(),
                results,
                selected: 0,
                error: None,
                has_focused: false,
            },
            iced::widget::operation::focus("search"),
        )
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Query(query) => {
                self.query = query;
                self.results = desktop::rank(&self.apps, &self.query, self.config.max_results);
                self.selected = 0;
                self.error = None;
            }
            Message::Move(delta) => {
                if !self.results.is_empty() {
                    self.selected = (self.selected as i32 + delta)
                        .rem_euclid(self.results.len() as i32)
                        as usize;
                }
            }
            Message::Click(index) => {
                self.selected = index;
                return self.update(Message::Launch);
            }
            Message::Launch => {
                if let Some(&index) = self.results.get(self.selected) {
                    match self.apps[index].launch(&self.config) {
                        Ok(()) => return iced::exit(),
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            Message::Close => return iced::exit(),
            Message::Focus(focused) => {
                if focused {
                    self.has_focused = true;
                    return iced::widget::operation::focus("search");
                } else if self.has_focused && self.config.close_on_unfocus {
                    return iced::exit();
                }
            }
        }
        Task::none()
    }
    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, _, _| match event {
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
            _ => None,
        })
    }
    fn view(&self) -> Element<'_, Message> {
        let bg = parse_color(&self.config.background).unwrap();
        let fg = parse_color(&self.config.foreground).unwrap();
        let accent = parse_color(&self.config.accent).unwrap();
        let muted = Color { a: 0.5, ..fg };
        let search = text_input("Search applications…", &self.query)
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
                selection: Color { a: 0.3, ..accent },
            });
        let header = row![
            svg(svg::Handle::from_memory(br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><circle cx="13" cy="13" r="8" fill="none" stroke="#a5b4fc" stroke-width="2.2"/><path d="M19 19L27 27" fill="none" stroke="#a5b4fc" stroke-width="2.2" stroke-linecap="round"/></svg>"##.as_slice())).width(30).height(30).style(move |_, _| svg::Style { color: Some(accent) }),
            search,
            text("esc").size(12).color(muted)
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        let heading = row![
            text(if self.query.is_empty() {
                "APPLICATIONS"
            } else {
                "RESULTS"
            })
            .size(11)
            .color(muted),
            space::horizontal(),
            text(format!("{} apps", self.apps.len()))
                .size(11)
                .color(muted)
        ];
        let mut entries = column![].spacing(4);
        for (position, &index) in self.results.iter().enumerate() {
            let app = &self.apps[index];
            let selected = position == self.selected;
            let icon: Element<'_, Message> = match &app.icon_path {
                Some(path) if path.extension().is_some_and(|e| e == "svg") => {
                    svg(svg::Handle::from_path(path))
                        .width(38)
                        .height(38)
                        .into()
                }
                Some(path) => image(image::Handle::from_path(path))
                    .width(38)
                    .height(38)
                    .into(),
                None => container(
                    text(app.name.chars().next().unwrap_or('?').to_string())
                        .size(22)
                        .color(accent),
                )
                .center(38)
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color { a: 0.12, ..accent })),
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
                .width(Length::Fill),
                text(if selected { "↵" } else { "" }).size(20).color(accent)
            ]
            .spacing(14)
            .align_y(Alignment::Center);
            entries = entries.push(
                button(content)
                    .on_press(Message::Click(position))
                    .width(Length::Fill)
                    .height(66)
                    .padding([10, 14])
                    .style(move |_, status| button::Style {
                        background: Some(Background::Color(if selected {
                            Color { a: 0.16, ..accent }
                        } else if matches!(status, button::Status::Hovered) {
                            Color { a: 0.05, ..fg }
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
            );
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
                    .color(fg),
                    text("Try a different app name or keyword")
                        .size(13)
                        .color(muted)
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .center(Length::Fill)
            .into()
        } else {
            entries.into()
        };
        let mut body = column![
            header,
            rule::horizontal(1),
            heading,
            container(list).height(Length::Fill)
        ]
        .spacing(16);
        if let Some(error) = &self.error {
            body = body.push(text(error).size(12).color(Color::from_rgb8(255, 150, 150)));
        }
        let footer = row![
            text("↑ ↓  Navigate").size(11).color(muted),
            text("↵  Open").size(11).color(muted),
            space::horizontal(),
            text("BLASTOFF").size(10).color(accent)
        ]
        .spacing(20);
        let panel = container(body.push(rule::horizontal(1)).push(footer))
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(fg),
                border: Border {
                    color: Color { a: 0.12, ..fg },
                    width: 1.0,
                    radius: self.config.radius.into(),
                },
                ..Default::default()
            });
        container(panel)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut path = config::path();
    let mut list = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => path = args.next().ok_or("--config requires a path")?.into(),
            "--list" => list = true,
            "--help" | "-h" => {
                println!(
                    "Blastoff — application launcher\n\nUsage: blastoff [--config PATH] [--list]\n\nConfig: {}\nKeys: ↑/↓ navigate, Enter launch, Escape close",
                    config::path().display()
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg:?}").into()),
        }
    }
    let config = Config::load(&path)?;
    if list {
        for app in desktop::discover(&config) {
            println!("{}\t{}", app.id, app.name);
        }
        return Ok(());
    }
    let size = iced::Size::new(
        config.width + 48.0,
        234.0 + config.max_results as f32 * 70.0,
    );
    iced::application(
        move || Launcher::boot(config.clone()),
        Launcher::update,
        Launcher::view,
    )
    .title("Blastoff")
    .theme(Theme::Dark)
    .style(|_, _| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .subscription(Launcher::subscription)
    .window(window::Settings {
        size,
        position: window::Position::Centered,
        decorations: false,
        transparent: true,
        resizable: false,
        level: window::Level::AlwaysOnTop,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "blastoff".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .run()?;
    Ok(())
}
