use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout as RatatuiLayout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};
use tui_scrollview::{ScrollView, ScrollViewState};

use super::ui_components::{Gauge, Theme};

pub struct Layout {
    pub theme: Theme,
    pub status_gauge: Gauge,
    pub activity_gauge: Gauge,
    pub scroll_state: ScrollViewState,
}

impl Layout {
    pub fn new(theme: Theme) -> Self {
        let colors = theme.get_colors();

        Self {
            status_gauge: Gauge::new("AI Processing", colors.gradient.clone()),
            activity_gauge: Gauge::new("Network Activity", vec![
                Color::Green,
                Color::Yellow,
                Color::Red,
            ]),
            theme,
            scroll_state: ScrollViewState::default(),
        }
    }

    pub fn render(&mut self, f: &mut Frame, app: &crate::app::App, messages: &[crate::chat::ChatMessage]) {
        // Sync scroll states
        self.scroll_state = app.scroll_state.clone();
        // Clear the entire frame with background color
        f.render_widget(
            ratatui::widgets::Clear,
            f.area()
        );

        // Main layout - Clean chat only
        let main_chunks = RatatuiLayout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),     // Content (full screen)
                Constraint::Length(3),  // TextArea input
            ])
            .split(f.area());

        // Render chat area
        self.chat_area(f, main_chunks[0], messages);

        // Render textarea widget
        f.render_widget(&app.textarea, main_chunks[1]);

        // Render menu if in menu mode
        if let crate::app::AppState::Menu(ref menu_type) = app.state {
            self.render_menu(f, f.area(), app, menu_type, app.menu_selected);
        }

        // Update animations
        self.update();
    }

    fn header(&self, f: &mut Frame, area: Rect) {
        let colors = self.theme.get_colors();
        let timestamp = chrono::Local::now().format("%H:%M:%S");

        let header_text = Line::from(vec![
            Span::styled("🚀 ARULA", Style::default().fg(colors.primary).add_modifier(Modifier::BOLD)),
            Span::styled(" • ", Style::default().fg(colors.secondary)),
            Span::styled(timestamp.to_string(), Style::default().fg(colors.info)),
            Span::styled(" • ", Style::default().fg(colors.secondary)),
            Span::styled(
                self.theme.to_string(),
                Style::default().fg(colors.info).add_modifier(Modifier::BOLD),
            ),
        ]);

        let header = Paragraph::new(header_text)
            .style(Style::default().fg(colors.text))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors.primary))
                    .padding(Padding::horizontal(1)),
            )
            .alignment(Alignment::Center);

        f.render_widget(header, area);
    }

    fn chat_area(&mut self, f: &mut Frame, area: Rect, messages: &[crate::chat::ChatMessage]) {
        let colors = self.theme.get_colors();

        // Check if area is too small for scroll view
        if area.width < 10 || area.height < 3 {
            return; // Not enough space to render anything
        }

        // Build chat content with all messages (ScrollView handles scrolling)
        let mut chat_content = String::new();

        for msg in messages {
            let timestamp = msg.timestamp.format("%H:%M:%S").to_string();
            let (icon, _color) = match msg.message_type {
                crate::chat::MessageType::User => ("👤", colors.success),
                crate::chat::MessageType::Arula => ("🤖", colors.primary),
                crate::chat::MessageType::System => ("🔧", colors.text),
                crate::chat::MessageType::Success => ("✅", colors.success),
                crate::chat::MessageType::Error => ("❌", colors.error),
                crate::chat::MessageType::Info => ("ℹ️", colors.info),
            };

            let formatted_msg = format!("[{}] {} {}\n", timestamp, icon, msg.content);
            chat_content.push_str(&formatted_msg);
        }

        // Create a paragraph widget with the chat content
        let chat_paragraph = Paragraph::new(chat_content)
            .style(Style::default()
                .fg(colors.text)
                .bg(colors.background))
            .wrap(ratatui::widgets::Wrap { trim: true })
            .block(ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(colors.primary))
                .title(" Chat ")
                .padding(ratatui::widgets::Padding::horizontal(1)));

        // Create scroll view with the area size
        let mut scroll_view = ScrollView::new(area.as_size());

        // Render the paragraph inside the scroll view
        scroll_view.render_widget(chat_paragraph, area);

        // Render the scroll view
        f.render_stateful_widget(scroll_view, area, &mut self.scroll_state);
    }

    
    
    
    #[allow(dead_code)]
    fn status_bar(&self, f: &mut Frame, area: Rect) {
        let colors = self.theme.get_colors();

        let current_section = "Chat";

        let status_text = vec![
            Span::styled("● ", Style::default().fg(colors.success).add_modifier(Modifier::BOLD)),
            Span::styled("Connected", Style::default().fg(colors.text).add_modifier(Modifier::BOLD)),
            Span::styled(" • ", Style::default().fg(colors.secondary)),
            Span::styled(
                current_section,
                Style::default()
                    .fg(colors.primary)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED),
            ),
            Span::styled(" • ", Style::default().fg(colors.secondary)),
            Span::styled("Esc: menu", Style::default().fg(colors.info).add_modifier(Modifier::BOLD)),
        ];

        let status = Paragraph::new(Line::from(status_text))
            .style(Style::default().fg(colors.text).bg(colors.background))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors.border).bg(colors.background)),
            );

        f.render_widget(status, area);
    }

    fn update(&mut self) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Update gauges with smooth animation
        let phase = (secs % 10) as f32 / 10.0;
        self.status_gauge.update(phase * 2.0);
        self.activity_gauge.update((phase * 3.0).sin().abs() * 50.0 + 25.0);
    }

    
    #[allow(dead_code)]
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        // Reinitialize components with new theme
        let colors = self.theme.get_colors();
        self.status_gauge.colors = colors.gradient.clone();
    }

    fn render_menu(&self, f: &mut Frame, area: Rect, app: &crate::app::App, menu_type: &crate::app::MenuType, selected: usize) {
        let colors = self.theme.get_colors();

        // Get menu options
        let menu_options = crate::app::App::menu_options(menu_type);
        let menu_title = crate::app::App::menu_title(menu_type);

        // For detail menus, show larger popup with content area
        let is_detail_menu = matches!(menu_type,
            crate::app::MenuType::GitCommandsDetail |
            crate::app::MenuType::SessionInfoDetail |
            crate::app::MenuType::KeyboardShortcutsDetail |
            crate::app::MenuType::AboutArulaDetail |
            crate::app::MenuType::DocumentationDetail |
            crate::app::MenuType::GitSettingsDetail |
            crate::app::MenuType::ExecCommandsDetail
        );

        // Center the menu popup
        let is_exit_confirmation = matches!(menu_type, crate::app::MenuType::ExitConfirmation);
        let popup_width = if is_exit_confirmation { 50 } else if is_detail_menu { 70 } else { 60 };
        let popup_height = if is_exit_confirmation { 8 } else if is_detail_menu { 20 } else { (menu_options.len() + 4) as u16 };
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Create menu list items
        let items: Vec<ListItem> = menu_options
            .iter()
            .enumerate()
            .map(|(i, option)| {
                let is_selected = i == selected;
                let (title, desc) = app.option_display(option);

                // Check if this is a Back or Close button
                let is_back_button = matches!(option, crate::app::MenuOption::Back | crate::app::MenuOption::Close);

                // For Back/Close buttons, show left arrow instead of right arrow
                let prefix = if is_back_button {
                    if is_selected { " ← " } else { "   " }
                } else {
                    if is_selected { " → " } else { "   " }
                };

                let content = Line::from(vec![
                    Span::styled(
                        prefix,
                        Style::default().fg(if is_selected { colors.primary } else { colors.text }),
                    ),
                    Span::styled(
                        format!("{:<30}", title),  // Increased width for value display
                        Style::default()
                            .fg(if is_selected { colors.primary } else { colors.text })
                            .add_modifier(if is_selected { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                    Span::styled(
                        desc,
                        Style::default().fg(colors.secondary),
                    ),
                ]);

                ListItem::new(content)
            })
            .collect();

        // Render menu
        f.render_widget(ratatui::widgets::Clear, popup_area);

        // For detail menus, split into content area and menu area
        if is_detail_menu {
            // Check if menu has no action items or only has Back button
            let has_no_actions = menu_options.is_empty() ||
                (menu_options.len() == 1 && matches!(menu_options.first(), Some(crate::app::MenuOption::Back)));

            if has_no_actions {
                // Only show content area, no menu section
                if let Some(content) = app.menu_content(menu_type) {
                    let content_para = Paragraph::new(content)
                        .style(Style::default().fg(colors.text).bg(colors.background))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(colors.primary))
                                .title(Span::styled(
                                    menu_title,
                                    Style::default().fg(colors.primary).add_modifier(Modifier::BOLD),
                                ))
                                .padding(Padding::uniform(1)),
                        )
                        .wrap(ratatui::widgets::Wrap { trim: true });

                    f.render_widget(content_para, popup_area);
                }
            } else {
                // Show both content and menu sections
                let split = RatatuiLayout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(10),  // Content area
                        Constraint::Length((menu_options.len() + 2) as u16), // Menu area
                    ])
                    .split(popup_area);

                // Render content area if available
                if let Some(content) = app.menu_content(menu_type) {
                    let content_para = Paragraph::new(content)
                        .style(Style::default().fg(colors.text).bg(colors.background))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(colors.primary))
                                .title(Span::styled(
                                    menu_title,
                                    Style::default().fg(colors.primary).add_modifier(Modifier::BOLD),
                                ))
                                .padding(Padding::uniform(1)),
                        )
                        .wrap(ratatui::widgets::Wrap { trim: true });

                    f.render_widget(content_para, split[0]);
                }

                // Render menu at bottom without "Actions" title
                let menu_list_detail = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                            .border_style(Style::default().fg(colors.primary))
                            .padding(Padding::horizontal(1)),
                    )
                    .style(Style::default().bg(colors.background));

                f.render_widget(menu_list_detail, split[1]);
            }
        } else {
            // Regular menu or exit confirmation
            if is_exit_confirmation {
                // For exit confirmation, split into content and buttons
                let split = RatatuiLayout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(5),  // Content area
                        Constraint::Min(0),     // Menu buttons
                    ])
                    .split(popup_area);

                // Render content
                if let Some(content) = app.menu_content(menu_type) {
                    let content_para = Paragraph::new(content)
                        .style(Style::default().fg(colors.text).bg(colors.background))
                        .block(
                            Block::default()
                                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                                .border_style(Style::default().fg(colors.primary))
                                .title(Span::styled(
                                    menu_title,
                                    Style::default().fg(colors.primary).add_modifier(Modifier::BOLD),
                                ))
                                .padding(Padding::uniform(1)),
                        )
                        .wrap(ratatui::widgets::Wrap { trim: true });

                    f.render_widget(content_para, split[0]);
                }

                // Render buttons (no top border to remove the dividing line)
                let menu_list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                            .border_style(Style::default().fg(colors.primary))
                            .padding(Padding::horizontal(1)),
                    )
                    .style(Style::default().bg(colors.background));

                f.render_widget(menu_list, split[1]);
            } else {
                // Regular menu - just render the list
                let menu_list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(colors.primary))
                            .title(Span::styled(
                                menu_title,
                                Style::default().fg(colors.primary).add_modifier(Modifier::BOLD),
                            ))
                            .padding(Padding::uniform(1)),
                    )
                    .style(Style::default().bg(colors.background));

                f.render_widget(menu_list, popup_area);
            }
        }

        // Render help text at bottom (skip for exit confirmation)
        if !is_exit_confirmation {
            let help_y = popup_y + popup_height;
            if help_y < area.height {
                let help_area = Rect {
                    x: popup_x,
                    y: help_y,
                    width: popup_width,
                    height: 1,
                };

                // Check if this is the main menu or a submenu
                let is_main_menu = matches!(menu_type, crate::app::MenuType::Main);
                let esc_text = if is_main_menu { " Close" } else { " Back" };

                let help_text = Paragraph::new(Line::from(vec![
                    Span::styled("↑↓", Style::default().fg(colors.info).add_modifier(Modifier::BOLD)),
                    Span::styled(" Navigate  ", Style::default().fg(colors.text)),
                    Span::styled("Enter", Style::default().fg(colors.success).add_modifier(Modifier::BOLD)),
                    Span::styled(" Select  ", Style::default().fg(colors.text)),
                    Span::styled("Esc", Style::default().fg(colors.error).add_modifier(Modifier::BOLD)),
                    Span::styled(esc_text, Style::default().fg(colors.text)),
                ]))
                .alignment(Alignment::Center)
                .style(Style::default().bg(colors.background));

                f.render_widget(help_text, help_area);
            }
        }
    }
}