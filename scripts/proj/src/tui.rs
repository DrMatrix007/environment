use crate::project::{self, Wt};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{List, ListItem};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(1500);

struct Row {
    name: String,
    dir: PathBuf,
    active: bool,
    claude_count: usize,
}

enum Mode {
    Browse,
    NewProject(String),
    ConfirmClose(String, PathBuf),
}

struct App {
    projects: Vec<(String, PathBuf)>,
    statuses: HashMap<String, usize>,
    filter: String,
    selected: usize,
    mode: Mode,
}

enum Action {
    Quit,
    OpenOrSwitch(Row),
    SwitchToAi(Row),
    SwitchToAiAndAddClaude(Row),
    CreateAndOpen(String),
    Close(String, PathBuf),
}

impl App {
    fn filtered_rows(&self) -> Vec<Row> {
        let needle = self.filter.to_lowercase();
        let mut rows: Vec<Row> = self
            .projects
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&needle))
            .map(|(name, dir)| {
                let window = project::window_name(name);
                let claude_count = self.statuses.get(&window).copied();
                Row {
                    name: name.clone(),
                    dir: dir.clone(),
                    active: claude_count.is_some(),
                    claude_count: claude_count.unwrap_or(0),
                }
            })
            .collect();
        rows.sort_by_key(|row| !row.active);
        rows
    }

    fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        match &mut self.mode {
            Mode::Browse => match key {
                KeyCode::Up => {
                    self.selected = self.selected.saturating_sub(1);
                    None
                }
                KeyCode::Down => {
                    let max = self.filtered_rows().len();
                    self.selected = (self.selected + 1).min(max);
                    None
                }
                KeyCode::Enter => {
                    let mut rows = self.filtered_rows();
                    if self.selected == rows.len() {
                        self.mode = Mode::NewProject(String::new());
                        None
                    } else {
                        let row = rows.swap_remove(self.selected);
                        if modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT) {
                            Some(Action::SwitchToAiAndAddClaude(row))
                        } else if modifiers.contains(KeyModifiers::CONTROL) {
                            Some(Action::SwitchToAi(row))
                        } else {
                            Some(Action::OpenOrSwitch(row))
                        }
                    }
                }
                KeyCode::Delete => {
                    let rows = self.filtered_rows();
                    if let Some(row) = rows.get(self.selected) {
                        self.mode = Mode::ConfirmClose(row.name.clone(), row.dir.clone());
                    }
                    None
                }
                KeyCode::Esc => Some(Action::Quit),
                KeyCode::Char('q') if self.filter.is_empty() => Some(Action::Quit),
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.selected = 0;
                    None
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.selected = 0;
                    None
                }
                _ => None,
            },
            Mode::NewProject(buf) => match key {
                KeyCode::Char(c) => {
                    buf.push(c);
                    None
                }
                KeyCode::Backspace => {
                    buf.pop();
                    None
                }
                KeyCode::Enter if !buf.is_empty() => Some(Action::CreateAndOpen(buf.clone())),
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    None
                }
                _ => None,
            },
            Mode::ConfirmClose(name, dir) => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') => Some(Action::Close(name.clone(), dir.clone())),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    None
                }
                _ => None,
            },
        }
    }
}

fn poll_status(sh: &xshell::Shell) -> HashMap<String, usize> {
    let fmt = "#{window_name}\t#{pane_current_command}";
    let Ok(output) = xshell::cmd!(sh, "psmux list-panes -a -F {fmt}").read() else {
        return HashMap::new();
    };
    let mut statuses = HashMap::new();
    for line in output.lines() {
        let Some((window, command)) = line.split_once('\t') else {
            continue;
        };
        let count = statuses.entry(window.to_string()).or_insert(0);
        if command == "claude" {
            *count += 1;
        }
    }
    statuses
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, sh: &xshell::Shell, app: &mut App) -> Result<Action> {
    let mut last_poll = Instant::now();
    loop {
        if last_poll.elapsed() >= POLL_INTERVAL {
            app.statuses = poll_status(sh);
            last_poll = Instant::now();
        }
        terminal.draw(|f| render(f, app))?;
        let remaining = POLL_INTERVAL.saturating_sub(last_poll.elapsed());
        if event::poll(remaining)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && let Some(action) = app.handle_key(key.code, key.modifiers)
        {
            return Ok(action);
        }
    }
}

fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let input_line = match &app.mode {
        Mode::Browse => format!("Filter: {}", app.filter),
        Mode::NewProject(buf) => format!("New project name: {buf}"),
        Mode::ConfirmClose(name, _) => format!("Close '{name}'? This kills its tools and ai windows. (y/n)"),
    };
    frame.render_widget(input_line, chunks[0]);

    let rows = app.filtered_rows();
    let mut items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let dot = if row.active { "●" } else { "○" };
            let claude = if row.claude_count > 0 {
                format!("  claude x{}", row.claude_count)
            } else {
                String::new()
            };
            let text = format!("{dot} {}{claude}", row.name);
            style_item(text, i == app.selected)
        })
        .collect();
    let create_text = "+ create new project".to_string();
    items.push(style_item(create_text, rows.len() == app.selected));
    frame.render_widget(List::new(items), chunks[1]);

    frame.render_widget(
        "↑/↓ select · Enter focus/switch/create · Ctrl+Enter select ai · Ctrl+Shift+Enter select ai+claude · Delete close · Esc cancel/quit · q quit (when filter empty)",
        chunks[2],
    );
}

fn style_item(text: String, selected: bool) -> ListItem<'static> {
    let style = if selected {
        Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    ListItem::new(text).style(style)
}

pub fn run(home: &Path, initial_query: Option<&str>) -> Result<()> {
    let sh = xshell::Shell::new()?;
    let projects = project::list_projects(home)?;
    let statuses = poll_status(&sh);
    let mut app = App {
        projects,
        statuses,
        filter: initial_query.unwrap_or_default().to_string(),
        selected: 0,
        mode: Mode::Browse,
    };

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &sh, &mut app);
    ratatui::restore();

    let action = result?;

    match action {
        Action::Quit => Ok(()),
        Action::OpenOrSwitch(row) => Wt::new(sh, row.name, row.dir).tools(),
        Action::SwitchToAi(row) => Wt::new(sh, row.name, row.dir).select_ai(),
        Action::SwitchToAiAndAddClaude(row) => Wt::new(sh, row.name, row.dir).ai(),
        Action::CreateAndOpen(name) => {
            let (name, dir) = project::create(home, &name)?;
            Wt::new(sh, name, dir).open()
        }
        Action::Close(name, dir) => Wt::new(sh, name, dir).close(),
    }
}
