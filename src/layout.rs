use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout as RatatuiLayout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};
use tui_markdown::from_str;

use super::ui_components::{Gauge, Theme};

pub struct Layout {
    pub theme: Theme,
    pub status_gauge: Gauge,
    pub activity_gauge: Gauge,
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
        }
    }

    /// Detect if terminal is in vertical orientation or narrow terminal
    /// This catches both tall terminals and very narrow ones that cause buffer issues
    fn is_vertical_terminal(area: Rect) -> bool {
        // Very narrow terminals (< 50 width) often cause buffer overflow issues
        let is_very_narrow = area.width < 50;
        // Tall terminals (height > width * 1.2)
        let is_tall = area.height as f32 > area.width as f32 * 1.2;

        is_very_narrow || is_tall
    }

    /// Get optimal menu dimensions based on terminal orientation
    fn get_menu_dimensions(area: Rect, is_exit_confirmation: bool, is_detail_menu: bool, menu_options_len: usize) -> (u16, u16, u16, u16) {
        let is_vertical = Self::is_vertical_terminal(area);

        if is_vertical {
            // For vertical terminals, use full width and center vertically
            let popup_width = area.width.saturating_sub(4); // Leave 2 chars padding on each side
            let popup_height = if is_exit_confirmation {
                8
            } else if is_detail_menu {
                area.height.saturating_sub(4) // Use most of the screen height
            } else {
                (menu_options_len + 4).min(area.height.saturating_sub(4) as usize) as u16
            };
            let popup_x = 2; // Start 2 chars from left edge
            let popup_y = (area.height.saturating_sub(popup_height)) / 2;

            (popup_width, popup_height, popup_x, popup_y)
        } else {
            // For horizontal terminals, use centered popup
            let popup_width = if is_exit_confirmation { 50 } else if is_detail_menu { 70 } else { 60 };
            let popup_height = if is_exit_confirmation { 8 } else if is_detail_menu { 20 } else { (menu_options_len + 4) as u16 };
            let popup_x = (area.width.saturating_sub(popup_width)) / 2;
            let popup_y = (area.height.saturating_sub(popup_height)) / 2;

            (popup_width, popup_height, popup_x, popup_y)
        }
    }

    pub fn render(&mut self, f: &mut Frame, app: &crate::app::App, messages: &[crate::chat::ChatMessage]) {
        // Clear the entire frame with background color
        f.render_widget(
            ratatui::widgets::Clear,
            f.area()
        );

        // Always render chat in full area (responsive layout removed)
        let full_chat_area = f.area();

        // Detect if on-screen keyboard is likely open (small terminal height)
        let keyboard_is_open = full_chat_area.height < 20;

        // Hide textarea when keyboard is open - let it go under the keyboard
        let textarea_area = if app.show_input && !keyboard_is_open {
            let textarea_height = 3;
            Rect {
                x: 0,
                y: full_chat_area.height.saturating_sub(textarea_height),
                width: full_chat_area.width,
                height: textarea_height,
            }
        } else {
            Rect::default() // Hide textarea when keyboard is open
        };

        // Chat area gets full screen when keyboard is open, otherwise leaves space for textarea
        let chat_area = if keyboard_is_open || !app.show_input {
            // Full screen for chat when keyboard is open or input is hidden
            full_chat_area
        } else {
            // Leave space at bottom for textarea when no keyboard
            Rect {
                x: full_chat_area.x,
                y: full_chat_area.y,
                width: full_chat_area.width,
                height: full_chat_area.height.saturating_sub(3), // Space for textarea
            }
        };

        // Extract values before rendering
        let menu_state = app.state.clone();
        let menu_selected = app.menu_selected;
        let textarea = app.textarea.clone();
        let is_ai_thinking = app.is_ai_thinking;
        let thinking_indicator = app.get_thinking_indicator();

        // Calculate scroll parameters
        let chat_scroll_offset = app.chat_scroll_offset;
        let auto_scroll = app.auto_scroll;

        // Render chat area (borderless) - uses immutable references
        self.chat_area_immutable(
            f,
            chat_area,
            messages,
            is_ai_thinking,
            &thinking_indicator,
            chat_scroll_offset,
            auto_scroll,
        );

        // Render textarea as overlay at absolute bottom position
        if app.show_input && textarea_area.height > 0 {
            f.render_widget(&textarea, textarea_area);
        }

        // Render menu if in menu mode (render last to be on top)
        if let crate::app::AppState::Menu(ref menu_type) = menu_state {
            self.render_menu(f, f.area(), app, menu_type, menu_selected);
        }

        // Update animations
        self.update();
    }

    fn render_header(&self, f: &mut Frame, area: Rect, app: &crate::app::App) {
        let colors = self.theme.get_colors();
        let timestamp = chrono::Local::now().format("%H:%M:%S");

        // Create ASCII art header
        let status_icon = if app.is_ai_thinking { "◉" } else { "◯" };
        let status_text = if app.is_ai_thinking { "PROCESSING" } else { "READY" };
        let status_color = if app.is_ai_thinking { colors.info } else { colors.success };

        let header_lines = vec![
            Line::from(vec![
                Span::styled("╔════════════════════════════════════════════════════════════════════╗",
                    Style::default().fg(colors.primary).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("║  ", Style::default().fg(colors.primary)),
                Span::styled("▰▰▰ ARULA CLI", Style::default().fg(colors.primary).add_modifier(Modifier::BOLD)),
                Span::styled("  │  ", Style::default().fg(colors.secondary)),
                Span::styled(status_icon, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {}", status_text), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                Span::styled("  │  ", Style::default().fg(colors.secondary)),
                Span::styled("⏰ ", Style::default().fg(colors.info)),
                Span::styled(timestamp.to_string(), Style::default().fg(colors.info)),
                Span::styled("  ║", Style::default().fg(colors.primary)),
            ]),
            Line::from(vec![
                Span::styled("╚════════════════════════════════════════════════════════════════════╝",
                    Style::default().fg(colors.primary).add_modifier(Modifier::BOLD)),
            ]),
        ];

        let header = Paragraph::new(header_lines)
            .style(Style::default().bg(colors.background))
            .alignment(Alignment::Left);

        f.render_widget(header, area);
    }

    fn render_input(&self, f: &mut Frame, area: Rect, app: &crate::app::App) {
        let colors = self.theme.get_colors();

        // Create custom input border with ASCII art
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors.primary).add_modifier(Modifier::BOLD))
            .border_type(ratatui::widgets::BorderType::Double)
            .title(Span::styled(
                " ⌨  INPUT ",
                Style::default().fg(colors.success).add_modifier(Modifier::BOLD)
            ))
            .title_alignment(Alignment::Left);

        // Clone the textarea and update its block
        let mut textarea = app.textarea.clone();
        textarea.set_block(input_block);

        f.render_widget(&textarea, area);
    }

    fn chat_area_immutable(
        &mut self,
        f: &mut Frame,
        area: Rect,
        messages: &[crate::chat::ChatMessage],
        is_ai_thinking: bool,
        thinking_indicator: &str,
        chat_scroll_offset: u16,
        auto_scroll: bool,
    ) {
        let colors = self.theme.get_colors();

        // Check if area is too small
        if area.width < 10 || area.height < 3 {
            return; // Not enough space to render anything
        }

        // Build chat content with all messages
        let mut lines: Vec<Line> = Vec::new();

        for msg in messages {
            let timestamp = msg.timestamp.format("%H:%M:%S").to_string();

            // Special handling for System messages (like logo)
            if msg.message_type == crate::chat::MessageType::System {
                // For system messages, render multi-line content with special coloring
                for content_line in msg.content.lines() {
                    lines.push(Line::from(
                        Span::styled(content_line, Style::default().fg(colors.primary).add_modifier(Modifier::BOLD))
                    ));
                }
                lines.push(Line::from("")); // Empty line for spacing
                continue;
            }

            // Get message color and prefix based on type
            let (prefix, msg_color) = match msg.message_type {
                crate::chat::MessageType::User => ("◈ ", colors.success),
                crate::chat::MessageType::Arula => ("⊙ ", colors.primary),
                crate::chat::MessageType::System => ("", colors.text),
                crate::chat::MessageType::Success => ("✓ ", colors.success),
                crate::chat::MessageType::Error => ("✗ ", colors.error),
                crate::chat::MessageType::Info => ("ℹ ", colors.info),
            };

            // For AI messages, parse markdown
            if msg.message_type == crate::chat::MessageType::Arula {
                // Parse markdown content
                let markdown_text = from_str(&msg.content);

                // Add prefix to first line
                if let Some(first_line) = markdown_text.lines.first() {
                    let mut spans = vec![Span::styled(prefix, Style::default().fg(msg_color))];
                    spans.extend(first_line.spans.clone());
                    lines.push(Line::from(spans));

                    // Add remaining lines without prefix
                    for line in markdown_text.lines.iter().skip(1) {
                        lines.push(line.clone());
                    }
                } else {
                    // Fallback if markdown parsing fails
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(msg_color)),
                        Span::styled(&msg.content, Style::default().fg(msg_color)),
                    ]));
                }
            } else {
                // For non-AI messages, use plain text
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(msg_color)),
                    Span::styled(&msg.content, Style::default().fg(msg_color)),
                ]));
            }
            lines.push(Line::from("")); // Empty line for spacing
        }

        // Add thinking indicator if AI is processing (spinner replaces ⊙)
        if is_ai_thinking {
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", thinking_indicator), Style::default().fg(colors.primary).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from("")); // Empty line for spacing
        }

        // Calculate scroll position
        let total_lines = lines.len();
        let visible_lines = area.height as usize;

        // Use manual scroll offset if auto-scroll is disabled
        let scroll_value = if auto_scroll {
            // Auto-scroll to bottom
            if total_lines > visible_lines {
                (total_lines - visible_lines) as u16
            } else {
                0
            }
        } else {
            // Use manual scroll offset, but clamp it to valid range
            let max_scroll = if total_lines > visible_lines {
                (total_lines - visible_lines) as u16
            } else {
                0
            };
            chat_scroll_offset.min(max_scroll)
        };

        // Create a paragraph widget with calculated scroll and wrapping
        let chat_paragraph = Paragraph::new(lines)
            .style(Style::default().bg(colors.background))
            .scroll((scroll_value, 0))
            .wrap(ratatui::widgets::Wrap { trim: false });

        f.render_widget(chat_paragraph, area);
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

        // Darker background for menu popup
        let menu_bg = Color::Rgb(15, 15, 20);

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

        // Calculate optimal popup dimensions based on terminal orientation
        let is_exit_confirmation = matches!(menu_type, crate::app::MenuType::ExitConfirmation);
        let (popup_width, popup_height, popup_x, popup_y) = Self::get_menu_dimensions(
            area,
            is_exit_confirmation,
            is_detail_menu,
            menu_options.len()
        );

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Ensure popup area is within terminal bounds
        let safe_popup_area = Rect {
            x: popup_x.min(area.width.saturating_sub(1)),
            y: popup_y.min(area.height.saturating_sub(1)),
            width: popup_width.min(area.width.saturating_sub(popup_x)),
            height: popup_height.min(area.height.saturating_sub(popup_y)),
        };

        // Clear the popup area first so background doesn't show through
        f.render_widget(ratatui::widgets::Clear, safe_popup_area);

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
                } else if is_selected { " → " } else { "   " };

                // Adaptive formatting for vertical vs horizontal terminals
                let is_vertical = Self::is_vertical_terminal(area);
                let content = if is_vertical || popup_width < 50 {
                    // Compact formatting for narrow screens
                    Line::from(vec![
                        Span::styled(
                            prefix,
                            Style::default().fg(if is_selected { colors.primary } else { colors.text }),
                        ),
                        Span::styled(
                            title,
                            Style::default()
                                .fg(if is_selected { colors.primary } else { colors.text })
                                .add_modifier(if is_selected { Modifier::BOLD } else { Modifier::empty() }),
                        ),
                    ])
                } else {
                    // Full formatting for wide screens
                    Line::from(vec![
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
                    ])
                };

                ListItem::new(content)
            })
            .collect();

        // Render menu
        f.render_widget(ratatui::widgets::Clear, safe_popup_area);

        // For detail menus, split into content area and menu area
        if is_detail_menu {
            // Check if menu has no action items or only has Back button
            let has_no_actions = menu_options.is_empty() ||
                (menu_options.len() == 1 && matches!(menu_options.first(), Some(crate::app::MenuOption::Back)));

            if has_no_actions {
                // Only show content area, no menu section
                if let Some(content) = app.menu_content(menu_type) {
                    let content_para = Paragraph::new(content)
                        .style(Style::default().fg(colors.text).bg(menu_bg))
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

                    f.render_widget(content_para, safe_popup_area);
                }
            } else {
                // Show both content and menu sections
                let split = RatatuiLayout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(10),  // Content area
                        Constraint::Length((menu_options.len() + 2) as u16), // Menu area
                    ])
                    .split(safe_popup_area);

                // Render content area if available
                if let Some(content) = app.menu_content(menu_type) {
                    let content_para = Paragraph::new(content)
                        .style(Style::default().fg(colors.text).bg(menu_bg))
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
                    .style(Style::default().bg(menu_bg));

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
                    .split(safe_popup_area);

                // Render content
                if let Some(content) = app.menu_content(menu_type) {
                    let content_para = Paragraph::new(content)
                        .style(Style::default().fg(colors.text).bg(menu_bg))
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
                    .style(Style::default().bg(menu_bg));

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
                    .style(Style::default().bg(menu_bg));

                f.render_widget(menu_list, safe_popup_area);
            }
        }

        // Render help text at bottom (skip for exit confirmation)
        if !is_exit_confirmation {
            let help_y = safe_popup_area.y + safe_popup_area.height;
            if help_y < area.height && help_y + 1 <= area.height {
                let help_area = Rect {
                    x: safe_popup_area.x,
                    y: help_y,
                    width: safe_popup_area.width,
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