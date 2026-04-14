use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::io;

use crate::cli::app::App;
use crate::cli::theme;

pub struct Tui {
    pub terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl Tui {
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn restore(&mut self) -> io::Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    pub fn draw(&mut self, app: &mut App) -> io::Result<()> {
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(10), // Banner
                    Constraint::Min(1),     // Chat history
                    Constraint::Length(3),  // Input box
                    Constraint::Length(1),  // Status bar
                ])
                .split(f.size());

            // 1. Banner
            let banner = Paragraph::new(theme::ASCII_BANNER)
                .style(Style::default().fg(theme::FIAP_PINK))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(theme::ALURA_CYAN)));
            f.render_widget(banner, chunks[0]);

            // 2. Chat History
            let mut text = Vec::new();
            for msg in &app.messages {
                let sender_style = if msg.is_error {
                    Style::default().fg(theme::ERROR_RED).add_modifier(Modifier::BOLD)
                } else if msg.is_system {
                    Style::default().fg(theme::FIAP_PINK).add_modifier(Modifier::BOLD)
                } else if msg.sender == "User" {
                    Style::default().fg(theme::ALURA_CYAN).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme::EXPERT_COLOR).add_modifier(Modifier::BOLD)
                };

                text.push(Line::from(vec![
                    Span::styled(format!("{}: ", msg.sender), sender_style),
                ]));
                
                // Add the content block
                for line in msg.content.lines() {
                    let content_style = if msg.is_error {
                        Style::default().fg(theme::ERROR_RED)
                    } else if msg.is_system {
                        Style::default().fg(theme::TEXT_MUTED)
                    } else {
                        Style::default().fg(theme::TEXT_NORMAL)
                    };
                    text.push(Line::from(Span::styled(line, content_style)));
                }
                text.push(Line::from("")); // Empty line separator
            }

            // Handle scrolling
            let content_height = text.len() as u16;
            let container_height = chunks[1].height.saturating_sub(2); // subtract borders
            
            let max_scroll = if content_height > container_height {
                content_height - container_height
            } else {
                0
            };
            
            // App.scroll is offset from bottom
            let current_scroll = max_scroll.saturating_sub(app.scroll as u16);

            let chat_block = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title(" POS TECH Shell ").border_style(Style::default().fg(theme::ALURA_CYAN)))
                .scroll((current_scroll, 0))
                .wrap(Wrap { trim: false });
            f.render_widget(chat_block, chunks[1]);

            // 3. Input Box
            let input_title = if app.is_loading {
                " Carregando... "
            } else {
                " Digite sua mensagem (/ajuda para comandos) "
            };

            let input_style = if app.is_loading {
                Style::default().fg(theme::TEXT_MUTED)
            } else {
                Style::default().fg(theme::TEXT_NORMAL)
            };

            let input_border_style = if app.is_loading {
                Style::default().fg(theme::TEXT_MUTED)
            } else {
                Style::default().fg(theme::FIAP_PINK)
            };

            let input = Paragraph::new(app.input.value())
                .style(input_style)
                .block(Block::default().borders(Borders::ALL).title(input_title).border_style(input_border_style));
            f.render_widget(input, chunks[2]);

            // Setup cursor
            if !app.is_loading {
                f.set_cursor(
                    chunks[2].x + app.input.visual_cursor() as u16 + 1,
                    chunks[2].y + 1,
                );
            }

            // 4. Status Bar
            let active_expert = match &app.active_persona_name {
                Some(name) => format!("Especialista Ativo: {}", name),
                None => "Nenhum especialista selecionado (/persona)".to_string(),
            };
            let status_line = Line::from(vec![
                Span::styled(" FIAP + Alura ", Style::default().fg(theme::BG_COLOR).bg(theme::FIAP_PINK)),
                Span::raw(" | "),
                Span::styled(active_expert, Style::default().fg(theme::ALURA_CYAN)),
                Span::raw(" | Scroll (PgUp/PgDn) | ESC para Sair "),
            ]);
            let status = Paragraph::new(status_line).alignment(Alignment::Left);
            f.render_widget(status, chunks[3]);

        })?;
        Ok(())
    }
}
