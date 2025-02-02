use std::{
    str::FromStr,
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use http::Uri;
use ratatui::{
    layout::{Constraint, Layout},
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, List, ListItem, Paragraph},
    DefaultTerminal, Frame,
};
use std::sync::Mutex as SyncMutex;
use tokio::sync::Mutex;
use tokio_websockets::{ClientBuilder, MaybeTlsStream, Message, WebSocketStream};

type WS = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

type ArcSink =
    Arc<Mutex<Option<SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>>>>;

pub struct App {
    sink: ArcSink,
    sender: Sender<String>,
    running: bool,
    messages: Arc<SyncMutex<Vec<ChatMessage>>>,
    text_input_content: String,
    url_content: String,
    input_field: InputField,
    error_while_sending: bool,
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

#[derive(Debug, Clone)]
struct ChatMessage {
    author: Author,
    content: String,
}

#[derive(Debug, Clone, Copy)]
enum Author {
    User,
    Origin,
}

async fn stream(stream: SplitStream<WS>, chan: mpsc::Sender<String>) {
    let mut s = stream;

    while let Some(Ok(m)) = s.next().await {
        let Some(m) = m.as_text() else { continue };
        chan.send(m.to_string()).expect("channel should be open");
    }
}

async fn connect(url: String) -> Option<(SplitSink<WS, Message>, SplitStream<WS>)> {
    let Ok(uri) = Uri::from_str(&url) else {
        return None;
    };
    let Ok((client, _)) = ClientBuilder::from_uri(uri).connect().await else {
        return None;
    };

    Some(client.split())
}

impl App {
    pub fn new(url: String) -> Self {
        let (sender, receiver) = mpsc::channel();

        let messages = Arc::new(SyncMutex::new(Vec::new()));
        let messages_ref = Arc::clone(&messages);

        thread::spawn(move || {
            for m in receiver {
                messages_ref.lock().unwrap().push(ChatMessage {
                    author: Author::Origin,
                    content: m,
                });
            }
        });

        App {
            sink: Arc::new(Mutex::new(None)),
            running: true,
            sender,
            messages,
            text_input_content: String::new(),
            url_content: url,
            input_field: InputField::Message,
            error_while_sending: false,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        {
            let sink = Arc::clone(&self.sink);
            let sender = self.sender.clone();
            let url = self.url_content.clone();

            if !self.url_content.is_empty() {
                tokio::spawn(async move {
                    let Some((new_sink, st)) = connect(url).await else {
                        return;
                    };
                    tokio::spawn(stream(st, sender));
                    let mut s = sink.lock().await;

                    *s = Some(new_sink);
                });
            }
        }

        while self.running {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_crossterm_events().await?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let title = Line::from(" WSTest ").bold().blue().centered();
        let text = "\n\
            Press `Esc` or `Ctrl-C` to stop running.\n Press `TAB` to switch from URL setting to chatting.\n Press `Ctrl-R` to reset connection (uses current URL).";

        let vertical = Layout::vertical([
            Constraint::Length(6),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(3),
        ]);

        let [prelude_area, messages_area, input_area_name, input_area] =
            vertical.areas(frame.area());

        let horizontal = Layout::horizontal([Constraint::Min(3), Constraint::Length(35)]);
        let [input_area_name, input_error_area] = horizontal.areas(input_area_name);

        frame.render_widget(
            Paragraph::new(text)
                .block(Block::bordered().title(title))
                .centered(),
            prelude_area,
        );

        let messages: Vec<_> = {
            let messages = self.messages.lock().unwrap();
            messages
                .iter()
                .cloned()
                .map(|m| {
                    ListItem::new(match m.author {
                        Author::User => Text::raw("USER: ".to_string() + &m.content)
                            .fg(ratatui::style::Color::Cyan),
                        Author::Origin => Text::raw("ORIG: ".to_string() + &m.content)
                            .fg(ratatui::style::Color::LightYellow),
                    })
                })
                .collect()
        };
        let messages = List::new(messages).block(Block::bordered());

        frame.render_widget(messages, messages_area);

        match self.input_field {
            InputField::Message => {
                frame.render_widget(Paragraph::new("Chat Message"), input_area_name);
                if self.error_while_sending {
                    frame.render_widget(
                        Paragraph::new(
                            "ERROR SENDING MESSAGE! Verify URL.".fg(ratatui::style::Color::Red),
                        ),
                        input_error_area,
                    );
                }
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

    /// If your application needs to perform work in between handling events, you can use the
    /// [`event::poll`] function to check if there are any events available with a timeout.
    async fn handle_crossterm_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                // it's important to check KeyEventKind::Press to avoid handling key release events
                Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key).await,
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
                _ => {}
            }
            Ok(())
        } else {
            Ok(())
        }
    }

    async fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Esc)
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            (KeyModifiers::CONTROL, KeyCode::Char('r') | KeyCode::Char('R')) => {
                let sink = Arc::clone(&self.sink);
                let sender = self.sender.clone();
                let url = self.url_content.clone();

                tokio::spawn(async move {
                    let mut s = sink.lock().await;
                    let Some((new_sink, st)) = connect(url).await else {
                        *s = None;
                        return;
                    };
                    tokio::spawn(stream(st, sender));
                    *s = Some(new_sink);
                });

                self.messages.lock().unwrap().clear();
            }
            (_, KeyCode::Char(c)) => match self.input_field {
                InputField::Message => self.text_input_content.push(c),
                InputField::Url => self.url_content.push(c),
            },

            (_, KeyCode::Enter) => match self.input_field {
                InputField::Message => {
                    let mut s = self.sink.lock().await;
                    if let Some(s) = s.as_mut() {
                        if let Ok(_) = s.send(Message::text(self.text_input_content.clone())).await
                        {
                            self.messages.lock().unwrap().push(ChatMessage {
                                author: Author::User,
                                content: self.text_input_content.clone(),
                            });
                            self.error_while_sending = false;
                        } else {
                            self.error_while_sending = true;
                        }
                    } else {
                        self.error_while_sending = true;
                    }

                    self.text_input_content.clear();
                }
                InputField::Url => {
                    let sink = Arc::clone(&self.sink);
                    let sender = self.sender.clone();
                    let url = self.url_content.clone();

                    tokio::spawn(async move {
                        let mut s = sink.lock().await;
                        let Some((new_sink, st)) = connect(url).await else {
                            *s = None;
                            return;
                        };
                        tokio::spawn(stream(st, sender));
                        *s = Some(new_sink);
                    });

                    self.error_while_sending = false;
                    self.input_field = InputField::Message;
                    self.messages.lock().unwrap().clear();
                }
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
