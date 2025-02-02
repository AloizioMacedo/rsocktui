use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, List, ListItem, Paragraph},
    DefaultTerminal, Frame,
};

#[derive(Debug, Default)]
pub struct App {
    running: bool,
    messages: Vec<ChatMessage>,
    text_input_content: String,
    url_content: String,
    input_field: InputField,
}

#[derive(Debug, Default)]
enum InputField {
    Url,

    #[default]
    Message,
}

impl InputField {
    fn other(&self) -> Self {
        match self {
            InputField::Url => InputField::Message,
            InputField::Message => InputField::Url,
        }
    }
}

#[derive(Debug)]
struct ChatMessage {
    author: Author,
    content: String,
}

#[derive(Debug)]
enum Author {
    User,
    Origin,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_url(url: String) -> Self {
        let mut app = Self::default();
        app.url_content = url;

        app
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        while self.running {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_crossterm_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let title = Line::from(" WSTest ").bold().blue().centered();
        let text = "\n\
            Press `Esc` or `Ctrl-C` to stop running. Press tab to switch from URL setting to chatting. Press Ctrl-R to reset connection.";

        let vertical = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(3),
        ]);

        let [prelude_area, messages_area, input_area_name, input_area] =
            vertical.areas(frame.area());

        frame.render_widget(
            Paragraph::new(text)
                .block(Block::bordered().title(title))
                .centered(),
            prelude_area,
        );

        let messages: Vec<_> = self
            .messages
            .iter()
            .map(|m| ListItem::new(Text::raw(&m.content)))
            .collect();
        let messages = List::new(messages).block(Block::bordered());

        frame.render_widget(messages, messages_area);

        match self.input_field {
            InputField::Message => {
                frame.render_widget(Paragraph::new("Chat Message"), input_area_name);
                frame.render_widget(
                    Paragraph::new(Text::raw(&self.text_input_content)).block(Block::bordered()),
                    input_area,
                );
            }
            InputField::Url => {
                frame.render_widget(Paragraph::new("WS URL"), input_area_name);
                frame.render_widget(
                    Paragraph::new(
                        Text::raw(&self.url_content).fg(ratatui::style::Color::Rgb(255, 165, 0)),
                    )
                    .block(Block::bordered()),
                    input_area,
                );
            }
        }
    }

    /// Reads the crossterm events and updates the state of [`App`].
    ///
    /// If your application needs to perform work in between handling events, you can use the
    /// [`event::poll`] function to check if there are any events available with a timeout.
    fn handle_crossterm_events(&mut self) -> Result<()> {
        match event::read()? {
            // it's important to check KeyEventKind::Press to avoid handling key release events
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key),
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Esc)
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            (_, KeyCode::Char(c)) => match self.input_field {
                InputField::Message => self.text_input_content.push(c),
                InputField::Url => self.url_content.push(c),
            },

            (_, KeyCode::Enter) => match self.input_field {
                InputField::Message => self.text_input_content.clear(),
                InputField::Url => self.url_content.clear(),
            },
            (_, KeyCode::Tab) => self.input_field = self.input_field.other(),
            (_, KeyCode::Backspace) => match self.input_field {
                InputField::Message => {
                    self.text_input_content.pop();
                }
                InputField::Url => {
                    self.url_content.pop();
                }
            },
            _ => {}
        }
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
