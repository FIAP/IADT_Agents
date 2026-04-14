use tui_input::Input;

pub enum AppMode {
    Normal,
    EnteringCommand,
}

pub struct ChatMessage {
    pub sender: String,
    pub content: String,
    pub is_system: bool,
    pub is_error: bool,
}

pub struct App {
    pub input: Input,
    pub messages: Vec<ChatMessage>,
    pub mode: AppMode,
    pub scroll: usize,
    pub is_loading: bool,
    pub active_persona_name: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: Input::default(),
            messages: Vec::new(),
            mode: AppMode::Normal,
            scroll: 0,
            is_loading: false,
            active_persona_name: None,
        }
    }

    pub fn add_message(&mut self, sender: &str, content: &str, is_system: bool, is_error: bool) {
        self.messages.push(ChatMessage {
            sender: sender.to_string(),
            content: content.to_string(),
            is_system,
            is_error,
        });
        self.scroll = 0; // Auto-scroll to bottom
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }
}
