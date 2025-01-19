use iced_wgpu::Renderer;
use iced_widget::{column, container, focus_next, row, slider, text, text_input};
use iced_winit::core::{Color, Element, Length::*, Theme};
use iced_winit::runtime::{Program, Task};

pub struct Controls {
    background_color: Color,
    input: String,
    input2: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    BackgroundColorChanged(Color),
    InputChanged(String),
    InputChanged2(String),
    FocusNext,
    FocusFirst,
}

impl Controls {
    pub fn new() -> Controls {
        Controls {
            background_color: Color::BLACK,
            input: String::default(),
            input2: String::default(),
        }
    }

    pub fn background_color(&self) -> Color {
        self.background_color
    }
}

impl Program for Controls {
    type Theme = Theme;
    type Message = Message;
    type Renderer = Renderer;

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::BackgroundColorChanged(color) => {
                self.background_color = color;
            }
            Message::InputChanged(input) => {
                self.input = input;
            }
            Message::InputChanged2(input) => {
                self.input2 = input;
            }
            Message::FocusNext => {
                return iced_widget::text_input::focus(String::from("second_text_input"));
            }
            Message::FocusFirst => {
                return iced_widget::text_input::focus(String::from("first_text_input"));
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<Message, Theme, Renderer> {
        let background_color = self.background_color;

        let sliders = row![
            slider(0.0..=1.0, background_color.r, move |r| {
                Message::BackgroundColorChanged(Color {
                    r,
                    ..background_color
                })
            })
            .step(0.01),
            slider(0.0..=1.0, background_color.g, move |g| {
                Message::BackgroundColorChanged(Color {
                    g,
                    ..background_color
                })
            })
            .step(0.01),
            slider(0.0..=1.0, background_color.b, move |b| {
                Message::BackgroundColorChanged(Color {
                    b,
                    ..background_color
                })
            })
            .step(0.01),
        ]
        .width(500)
        .spacing(20);

        container(
            column![
                text("Background color").color(Color::WHITE),
                text!("{background_color:?}").size(14).color(Color::WHITE),
                text_input("Placeholder", &self.input2).on_input(Message::InputChanged2)
                    .id(String::from("first_text_input"))
                    .on_submit(Message::FocusNext) ,
                text_input("Placeholder", &self.input).id(String::from("second_text_input"))
                    .on_input(Message::InputChanged)
                    .on_submit(Message::FocusFirst),
                sliders,
            ]
            .spacing(10),
        )
        .padding(10)
        .align_bottom(Fill)
        .into()
    }
}
