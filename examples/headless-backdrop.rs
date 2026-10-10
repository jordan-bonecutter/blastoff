//! A deterministic backdrop for alpha-compositing tests, in the isolated compositor only.
use iced::widget::container;
use iced::{Color, Element, Length, Theme, window};
fn view(_: &()) -> Element<'_, (), Theme> {
    container(iced::widget::space())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::var("BLASTOFF_TEST_DIR")?;
    if !std::env::var("SWAYSOCK")?.starts_with(&format!("{root}/")) {
        return Err("requires the isolated compositor".into());
    }
    iced::application(|| (), |_: &mut (), _: ()| {}, view)
        .title("Headless backdrop")
        .style(|_, _| iced::theme::Style {
            background_color: Color::from_rgb8(60, 110, 170),
            text_color: Color::WHITE,
        })
        .window(window::Settings {
            size: iced::Size::new(1280.0, 960.0),
            decorations: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "blastoff-test-background".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .run()?;
    Ok(())
}
