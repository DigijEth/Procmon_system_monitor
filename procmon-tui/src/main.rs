mod app;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt::init();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new().await?;

    // Run app
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Handle search mode separately
                    if app.search_mode {
                        match key.code {
                            KeyCode::Char(c) => app.add_search_char(c),
                            KeyCode::Backspace => app.remove_search_char(),
                            KeyCode::Esc => app.toggle_search_mode(),
                            KeyCode::Enter => app.toggle_search_mode(),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(());
                            }
                            KeyCode::Char('/') => app.toggle_search_mode(),
                            KeyCode::Up => {
                                if app.current_tab == app::Tab::Partitions {
                                    app.previous_partition();
                                } else if app.current_tab == app::Tab::Services {
                                    app.previous_service();
                                } else {
                                    app.previous_process();
                                }
                            }
                            KeyCode::Down => {
                                if app.current_tab == app::Tab::Partitions {
                                    app.next_partition();
                                } else if app.current_tab == app::Tab::Services {
                                    app.next_service();
                                } else {
                                    app.next_process();
                                }
                            }
                            KeyCode::PageUp => app.scroll_up(10),
                            KeyCode::PageDown => app.scroll_down(10, 20),
                            KeyCode::Left if app.current_tab == app::Tab::Partitions => {
                                app.previous_disk();
                            }
                            KeyCode::Right if app.current_tab == app::Tab::Partitions => {
                                app.next_disk();
                            }
                            KeyCode::Tab => app.next_tab(),
                            KeyCode::BackTab => app.previous_tab(),
                            KeyCode::Char('1') => app.set_tab(0),
                            KeyCode::Char('2') => app.set_tab(1),
                            KeyCode::Char('3') => app.set_tab(2),
                            KeyCode::Char('4') => app.set_tab(3),
                            KeyCode::Char('5') => app.set_tab(4),
                            KeyCode::Char('6') => app.set_tab(5),
                            KeyCode::Char('7') => app.set_tab(6),
                            KeyCode::Char('a') => app.toggle_sort_ascending(),
                            KeyCode::Char('s') => app.next_sort_column(),
                            KeyCode::Char('f') => app.toggle_filter(),
                            KeyCode::Char('m') | KeyCode::Enter => {
                                if app.current_tab == app::Tab::Partitions {
                                    app.toggle_partition_menu();
                                } else if app.current_tab == app::Tab::Services {
                                    app.toggle_service_menu();
                                } else {
                                    app.toggle_context_menu();
                                }
                            }
                            KeyCode::Char('r') if app.current_tab == app::Tab::Partitions => {
                                app.refresh_disks();
                            }
                            KeyCode::Char('d') if app.show_partition_menu => {
                                let _ = app.delete_selected_partition();
                                app.show_partition_menu = false;
                            }
                            KeyCode::Char('c') if app.show_partition_menu => {
                                let _ = app.check_selected_partition();
                                app.show_partition_menu = false;
                            }
                            KeyCode::Char('e') if app.show_partition_menu => {
                                let _ = app.format_selected_partition("ext4");
                                app.show_partition_menu = false;
                            }
                            KeyCode::Char('x') if app.show_partition_menu => {
                                let _ = app.format_selected_partition("xfs");
                                app.show_partition_menu = false;
                            }
                            KeyCode::Char('b') if app.show_partition_menu => {
                                let _ = app.format_selected_partition("btrfs");
                                app.show_partition_menu = false;
                            }
                            KeyCode::Char('n') if app.show_partition_menu => {
                                let _ = app.format_selected_partition("ntfs");
                                app.show_partition_menu = false;
                            }
                            KeyCode::Char('k') if app.show_context_menu => {
                                let _ = app.kill_process();
                            }
                            KeyCode::Char('t') if app.show_context_menu => {
                                let _ = app.kill_process_tree();
                            }
                            KeyCode::Char('o') if app.show_context_menu => {
                                let _ = app.open_process_folder();
                            }
                            KeyCode::Char('r') if app.show_context_menu => {
                                let _ = app.restart_process();
                            }
                            // Service menu actions
                            KeyCode::Char('s') if app.show_service_menu => {
                                let _ = app.start_service();
                            }
                            KeyCode::Char('p') if app.show_service_menu => {
                                let _ = app.stop_service();
                            }
                            KeyCode::Char('r') if app.show_service_menu => {
                                let _ = app.restart_service();
                            }
                            KeyCode::Char('e') if app.show_service_menu => {
                                let _ = app.enable_service();
                            }
                            KeyCode::Char('d') if app.show_service_menu => {
                                let _ = app.disable_service();
                            }
                            KeyCode::Esc => {
                                if app.show_context_menu {
                                    app.show_context_menu = false;
                                    app.context_menu_pid = None;
                                } else if app.show_service_menu {
                                    app.show_service_menu = false;
                                    app.context_menu_service = None;
                                } else if app.search_mode {
                                    app.toggle_search_mode();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollDown => app.scroll_down(3, 20),
                        MouseEventKind::ScrollUp => app.scroll_up(3),
                        MouseEventKind::Down(_button) => {
                            // Handle mouse click
                            app.handle_mouse_click(mouse.column, mouse.row);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        app.update().await?;
    }
}
