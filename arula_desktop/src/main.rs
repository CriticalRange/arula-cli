use arula_core::utils::config::Config;
use arula_core::{AgentBackend, SessionConfig, SessionRunner, StreamEvent};
use chrono::Utc;
use futures::StreamExt;
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, Subscription, Task, Theme};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

#[derive(Debug, Clone)]
enum UiEvent {
    StreamStarted(Uuid),
    Token(Uuid, String, bool),
    StreamFinished(Uuid),
    StreamErrored(Uuid, String),
}

struct Dispatcher {
    runtime: Runtime,
    events: broadcast::Sender<UiEvent>,
    runner: SessionRunner<AgentBackend>,
}

impl Dispatcher {
    fn new() -> anyhow::Result<Self> {
        let config = Config::load_or_default()?;
        let backend = AgentBackend::new(&config, String::new())?;
        let runtime = Runtime::new()?;
        let (events, _) = broadcast::channel(128);
        let runner = SessionRunner::new(backend);
        Ok(Self { runtime, events, runner })
    }

    fn start_stream(&self, session_id: Uuid, prompt: String) -> anyhow::Result<()> {
        let tx = self.events.clone();
        let runner = self.runner.clone();
        self.runtime.spawn(async move {
            let _ = tx.send(UiEvent::StreamStarted(session_id));
            let cfg = SessionConfig {
                system_prompt: String::new(),
                model: "default".into(),
                max_tokens: 1024,
                temperature: 0.7,
            };
            match runner.stream_session(prompt, None, cfg) {
                Ok(mut stream) => {
                    while let Some(event) = stream.next().await {
                        match event {
                            StreamEvent::Start { .. } => {}
                            StreamEvent::Text { text } => {
                                let _ = tx.send(UiEvent::Token(session_id, text, false));
                            }
                            StreamEvent::Reasoning { text } => {
                                let _ = tx.send(UiEvent::Token(session_id, format!("[thinking] {text}"), false));
                            }
                            StreamEvent::ToolCall { id, name, arguments } => {
                                let _ = tx.send(UiEvent::Token(session_id, format!("[tool:{name}] {arguments} ({id})"), false));
                            }
                            StreamEvent::ToolResult { tool_call_id, result } => {
                                let _ = tx.send(UiEvent::Token(session_id, format!("[tool-result:{tool_call_id}] {:?}", result), false));
                            }
                            StreamEvent::Finished => {
                                let _ = tx.send(UiEvent::Token(session_id, String::new(), true));
                                let _ = tx.send(UiEvent::StreamFinished(session_id));
                                break;
                            }
                            StreamEvent::Error(err) => {
                                let _ = tx.send(UiEvent::StreamErrored(session_id, err));
                                break;
                            }
                        }
                    }
                }
                Err(err) => {
                    let _ = tx.send(UiEvent::StreamErrored(session_id, err.to_string()));
                }
            }
        });
        Ok(())
    }

    fn subscription(&self) -> Subscription<UiEvent> {
        let rx = self.events.subscribe();
        iced::Subscription::run_with_id("dispatcher-stream", {
            let rx_stream = BroadcastStream::new(rx);
            iced::futures::stream::unfold(rx_stream, |mut stream| async {
                while let Some(next) = stream.next().await {
                    match next {
                        Ok(ev) => return Some((ev, stream)),
                        Err(_) => continue,
                    }
                }
                None
            })
        })
    }
}

#[derive(Debug, Clone)]
struct MessageEntry {
    role: String,
    content: String,
    timestamp: String,
}

#[derive(Debug, Clone)]
struct Session {
    id: Uuid,
    messages: Vec<MessageEntry>,
    is_streaming: bool,
}

struct App {
    dispatcher: Dispatcher,
    sessions: Vec<Session>,
    current: usize,
    draft: String,
}

#[derive(Debug, Clone)]
enum Message {
    DraftChanged(String),
    SendPrompt,
    Received(UiEvent),
    NewTab,
}

impl App {
    fn init() -> (Self, Task<Message>) {
        let dispatcher = Dispatcher::new().expect("dispatcher");
        let session = Session {
            id: Uuid::new_v4(),
            messages: Vec::new(),
            is_streaming: false,
        };
        (
            Self {
                dispatcher,
                sessions: vec![session],
                current: 0,
                draft: String::new(),
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DraftChanged(s) => self.draft = s,
            Message::SendPrompt => {
                if let Some(session) = self.sessions.get_mut(self.current) {
                    let prompt = std::mem::take(&mut self.draft);
                    if prompt.is_empty() {
                        return Task::none();
                    }
                    session.messages.push(MessageEntry {
                        role: "User".into(),
                        content: prompt.clone(),
                        timestamp: Utc::now().to_rfc3339(),
                    });
                    session.is_streaming = true;
                    let sid = session.id;
                    if let Err(err) = self.dispatcher.start_stream(sid, prompt) {
                        eprintln!("dispatch error: {err}");
                        session.is_streaming = false;
                    }
                }
            }
            Message::Received(ev) => match ev {
                UiEvent::StreamStarted(id) => {
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        s.is_streaming = true;
                    }
                }
                UiEvent::Token(id, delta, is_final) => {
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        if let Some(last) = s.messages.last_mut() {
                            if last.role == "Arula" {
                                last.content.push_str(&delta);
                            } else {
                                s.messages.push(MessageEntry {
                                    role: "Arula".into(),
                                    content: delta,
                                    timestamp: Utc::now().to_rfc3339(),
                                });
                            }
                        } else {
                            s.messages.push(MessageEntry {
                                role: "Arula".into(),
                                content: delta,
                                timestamp: Utc::now().to_rfc3339(),
                            });
                        }
                        if is_final {
                            s.is_streaming = false;
                        }
                    }
                }
                UiEvent::StreamFinished(id) => {
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        s.is_streaming = false;
                    }
                }
                UiEvent::StreamErrored(id, err) => {
                    eprintln!("stream error {id}: {err}");
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        s.is_streaming = false;
                    }
                }
            },
            Message::NewTab => {
                self.sessions.push(Session {
                    id: Uuid::new_v4(),
                    messages: Vec::new(),
                    is_streaming: false,
                });
                self.current = self.sessions.len() - 1;
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.dispatcher.subscription().map(Message::Received)
    }

    fn view(&self) -> Element<'_, Message> {
        let session = &self.sessions[self.current];
        let message_elems: Vec<Element<Message>> = session
            .messages
            .iter()
            .map(|m| text(format!("{} [{}]: {}", m.role, m.timestamp, m.content)).into())
            .collect();

        let messages = scrollable(column(message_elems).spacing(8)).height(Length::Fill);

        let input = row![
            text_input("Message", &self.draft)
                .on_input(Message::DraftChanged)
                .padding(10)
                .width(Length::Fill),
            button("Send").on_press(Message::SendPrompt),
            button("New Tab").on_press(Message::NewTab),
        ]
        .spacing(10);

        container(column![messages, input].spacing(12)).padding(12).into()
    }
}

fn main() -> iced::Result {
    iced::application("Arula Desktop", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| Theme::Light)
        .run_with(App::init)
}
