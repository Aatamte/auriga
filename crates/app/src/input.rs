use crate::app::App;
use alacritty_terminal::grid::Scroll;
use auriga_core::{Page, ScrollDirection};
use auriga_grid::WidgetId;
use auriga_widgets::{RenderContext, WidgetAction};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Global ctrl shortcuts always handled
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('q') => {
                app.running = false;
                return;
            }
            KeyCode::Char('t') => {
                if let Err(e) = app.spawn_shell() {
                    tracing::error!(error = %e, "failed to spawn shell");
                }
                return;
            }
            KeyCode::Char('l') => {
                let config = app.default_agent_config("claude");
                if let Err(e) = app.spawn_agent(&config) {
                    tracing::error!(error = %e, "failed to spawn claude agent");
                }
                return;
            }
            KeyCode::Char('o') => {
                let config = app.default_agent_config("codex");
                if let Err(e) = app.spawn_agent(&config) {
                    tracing::error!(error = %e, "failed to spawn codex agent");
                }
                return;
            }
            KeyCode::Char('w') => {
                if let Some(id) = app.focus.active_agent {
                    app.kill_agent(id);
                }
                return;
            }
            KeyCode::Char('b') => {
                app.toggle_pane_mode();
                return;
            }
            _ => {}
        }
    }

    // Page-specific key handling
    match app.focus.page {
        Page::Settings => {
            if let Some(action) = app.widgets.settings_page.handle_key(key) {
                app.handle_widget_action(action);
            }
        }
        Page::Home => {
            // Check if active agent is in native mode
            let is_native = app
                .focus
                .active_agent
                .and_then(|id| app.agents.get(id))
                .map(|a| a.display_mode == auriga_core::DisplayMode::Native)
                .unwrap_or(false);

            if is_native {
                handle_native_input(app, key);
            } else {
                forward_key(app, key);
            }
        }
        Page::Database => {
            if let Some(action) = app.widgets.database_page.handle_key(key) {
                app.handle_widget_action(action);
            }
        }
        Page::Classifiers => {
            if let Some(action) = app.widgets.classifiers_page.handle_key(key) {
                app.handle_widget_action(action);
            }
        }
        Page::Prompts => {
            if let Some(action) = app.widgets.prompts_page.handle_key(key) {
                app.handle_widget_action(action);
            }
        }
        Page::Context => {
            // Placeholder — no key handling yet
        }
        Page::Doctor => {
            if app.widgets.doctor_page.agent_id.is_some() {
                // Forward keys to the doctor agent's PTY
                let bytes = key_to_bytes(key);
                if !bytes.is_empty() {
                    if let Some(id) = app.widgets.doctor_page.agent_id {
                        if let Some(pty) = app.ptys.get_mut(&id) {
                            if let Err(e) = pty.write_input(&bytes) {
                                tracing::warn!(error = %e, "doctor PTY write failed");
                            }
                        }
                    }
                }
            } else if let Some(action) = app.widgets.doctor_page.handle_key(key) {
                app.handle_widget_action(action);
            }
        }
    }
}

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        // Check tab bar clicks
        let nav = app.last_nav_rect;
        if mouse.row >= nav.y && mouse.row < nav.y + nav.height {
            if let Some(page) =
                app.widgets
                    .nav_bar
                    .handle_click(mouse.column, nav, &crate::app::hidden_pages())
            {
                app.handle_widget_action(WidgetAction::NavigateTo(page));
            }
            return;
        }
    }

    // Normal widget dispatch
    let hit = app.hit_test(mouse.column, mouse.row);
    let Some((widget_name, local_row, local_col)) = hit else {
        return;
    };

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let action = {
                let terms = &app.terms;
                let term_renderer =
                    |id: auriga_core::AgentId, buf: &mut ratatui::buffer::Buffer, area: Rect| {
                        if let Some(term) = terms.get(&id) {
                            auriga_terminal::render_term(term, buf, area);
                        }
                    };
                let hidden = crate::app::hidden_pages();
                let ctx = RenderContext {
                    agents: &app.agents,
                    turns: &app.turns,
                    traces: &app.traces,
                    focus: &app.focus,
                    file_tree: &app.file_tree,
                    render_term: &term_renderer,
                    hidden_pages: &hidden,
                };
                let widget = app.widgets.get_mut(widget_name);
                widget.handle_click(local_row, local_col, &ctx)
            };
            if let Some(action) = action {
                app.handle_widget_action(action);
            }
        }
        MouseEventKind::ScrollUp => {
            if widget_name == WidgetId::AgentPane {
                if let Some(id) = app.focus.active_agent {
                    if let Some(term) = app.terms.get_mut(&id) {
                        term.scroll_display(Scroll::Delta(3));
                    }
                }
            } else {
                app.widgets
                    .get_mut(widget_name)
                    .handle_scroll(ScrollDirection::Up);
            }
        }
        MouseEventKind::ScrollDown => {
            if widget_name == WidgetId::AgentPane {
                if let Some(id) = app.focus.active_agent {
                    if let Some(term) = app.terms.get_mut(&id) {
                        term.scroll_display(Scroll::Delta(-3));
                    }
                }
            } else {
                app.widgets
                    .get_mut(widget_name)
                    .handle_scroll(ScrollDirection::Down);
            }
        }
        _ => {}
    }
}

fn handle_native_input(app: &mut App, key: KeyEvent) {
    if app.widgets.agent_pane.generating {
        return; // Ignore input while generating
    }
    match key.code {
        KeyCode::Enter => {
            app.send_native_message();
        }
        KeyCode::Char(c) => {
            app.widgets.agent_pane.input_buffer.push(c);
        }
        KeyCode::Backspace => {
            app.widgets.agent_pane.input_buffer.pop();
        }
        KeyCode::Esc => {
            app.widgets.agent_pane.input_buffer.clear();
        }
        _ => {}
    }
}

fn forward_key(app: &mut App, key: KeyEvent) {
    let bytes = key_to_bytes(key);
    if !bytes.is_empty() {
        app.write_to_active(&bytes);
    }
}

pub fn key_to_bytes(key: KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let byte = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                vec![byte]
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![127],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![27],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn regular_char_produces_utf8() {
        let key = make_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), b"a");
    }

    #[test]
    fn multibyte_char_produces_utf8() {
        let key = make_key(KeyCode::Char('é'), KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), "é".as_bytes());
    }

    #[test]
    fn ctrl_c_produces_etx() {
        let key = make_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_to_bytes(key), vec![3]);
    }

    #[test]
    fn ctrl_a_produces_soh() {
        let key = make_key(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(key_to_bytes(key), vec![1]);
    }

    #[test]
    fn ctrl_z_produces_sub() {
        let key = make_key(KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(key_to_bytes(key), vec![26]);
    }

    #[test]
    fn enter_produces_cr() {
        let key = make_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), vec![b'\r']);
    }

    #[test]
    fn backspace_produces_del() {
        let key = make_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), vec![127]);
    }

    #[test]
    fn tab_produces_tab() {
        let key = make_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), vec![b'\t']);
    }

    #[test]
    fn backtab_produces_escape_sequence() {
        let key = make_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), b"\x1b[Z");
    }

    #[test]
    fn escape_produces_esc() {
        let key = make_key(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), vec![27]);
    }

    #[test]
    fn arrow_keys_produce_escape_sequences() {
        assert_eq!(
            key_to_bytes(make_key(KeyCode::Up, KeyModifiers::NONE)),
            b"\x1b[A"
        );
        assert_eq!(
            key_to_bytes(make_key(KeyCode::Down, KeyModifiers::NONE)),
            b"\x1b[B"
        );
        assert_eq!(
            key_to_bytes(make_key(KeyCode::Right, KeyModifiers::NONE)),
            b"\x1b[C"
        );
        assert_eq!(
            key_to_bytes(make_key(KeyCode::Left, KeyModifiers::NONE)),
            b"\x1b[D"
        );
    }

    #[test]
    fn home_end_delete_produce_escape_sequences() {
        assert_eq!(
            key_to_bytes(make_key(KeyCode::Home, KeyModifiers::NONE)),
            b"\x1b[H"
        );
        assert_eq!(
            key_to_bytes(make_key(KeyCode::End, KeyModifiers::NONE)),
            b"\x1b[F"
        );
        assert_eq!(
            key_to_bytes(make_key(KeyCode::Delete, KeyModifiers::NONE)),
            b"\x1b[3~"
        );
    }

    #[test]
    fn unknown_key_produces_empty() {
        let key = make_key(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(key_to_bytes(key), Vec::<u8>::new());
    }
}
