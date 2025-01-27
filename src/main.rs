#![allow(clippy::unnecessary_wraps)]

use std::{
    io::{self, Read, Write},
    path::PathBuf,
    str::FromStr,
};

#[cfg(not(debug_assertions))]
use std::panic::catch_unwind;

use chrono::{Local, NaiveDate};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use rusqlite::{params, Connection, Result};

#[derive(Parser)]
struct Args {
    #[clap(long, short)]
    db: Option<PathBuf>,
    #[clap(subcommand)]
    action: Option<Action>,
}

#[derive(Subcommand, Default)]
enum Action {
    #[default]
    Tui,
    Cli,
    Add {
        date: Option<String>,
    },
    Remove {
        date: Option<String>,
    },
    List,
    DataDir,
    Export,
}

fn interactive_mode(conn: &Connection) -> Result<()> {
    println!("Home Office Tracker");
    println!("1. Add (t)oday's home office day (default)");
    println!("2. (A)dd a specific home office day");
    println!("3. (L)ist all home office days");
    println!("4. (D)elete a home office day");
    println!("5. (E)xport all home office days");
    print!("Choose an option (default is 1): ");
    io::stdout().flush().unwrap();

    enable_raw_mode().unwrap();
    let input = std::io::stdin()
        .bytes()
        .next()
        .and_then(Result::ok)
        .map(|byte| byte as char)
        .unwrap();
    disable_raw_mode().unwrap();

    println!();

    match input.to_ascii_lowercase() {
        '\r' | '\n' | '1' | 't' => add_today(conn)?,
        '2' | 'a' => add_specific_date(conn)?,
        '3' | 'l' => list_dates(conn)?,
        '4' | 'd' => delete_home_office_day(conn)?,
        '5' | 'e' => export_dates(conn)?,
        _ => println!("Invalid option."),
    }

    Ok(())
}

fn parse_dates_or_default(input: Option<String>) -> Vec<NaiveDate> {
    input.map_or_else(
        || {
            vec![NaiveDate::parse_from_str(
                &Local::now().format("%Y-%m-%d").to_string(),
                "%Y-%m-%d",
            )
            .unwrap()]
        },
        |i| {
            let v: Vec<_> = i
                .split("::")
                .map(|date| {
                    NaiveDate::parse_from_str(dbg!(date).trim(), "%Y-%m-%d").unwrap_or_else(|_| {
                        NaiveDate::parse_from_str(date.trim(), "%d.%m.%Y").unwrap()
                    })
                })
                .collect();
            if v.len() == 1 {
                v
            } else if v.len() == 2 {
                let first = v.first().unwrap();
                let last = v.last().unwrap();
                let mut dates = Vec::new();
                let mut current = *first;
                loop {
                    dates.push(current);
                    if current == *last {
                        break;
                    }
                    current = current.succ_opt().unwrap();
                }
                dates
            } else {
                panic!("Invalid date range");
            }
        },
    )
}

fn run() -> Result<()> {
    let Args { action, db } = Args::parse();

    let data_dir = if let Some(db) = db {
        db
    } else if let Some(dir) = dirs::data_dir() {
        dir.join("home_office_tracker")
    } else {
        PathBuf::from_str("home_office_tracker").unwrap()
    };

    std::fs::create_dir_all(&data_dir).unwrap();

    let db_path = data_dir.join("home_office_tracker.db");

    // Initialize SQLite database
    let conn = Connection::open(&db_path)?;
    create_table(&conn)?;

    match action.unwrap_or_default() {
        Action::Tui => {
            run_tui(conn).unwrap();
            Ok(())
        }
        Action::Export => export_dates(&conn),
        Action::DataDir => {
            println!("{}", data_dir.display());
            Ok(())
        }
        Action::Cli => interactive_mode(&conn),
        Action::List => list_dates(&conn),
        Action::Add { date } => add_dates(&conn, &parse_dates_or_default(date)),
        Action::Remove { date } => remove_dates(&conn, &parse_dates_or_default(date)),
    }
}

fn main() -> Result<()> {
    #[cfg(not(debug_assertions))]
    let result = catch_unwind(run);

    #[cfg(debug_assertions)]
    let result = run();

    if is_raw_mode_enabled().unwrap() {
        disable_raw_mode().unwrap();
    };

    #[cfg(not(debug_assertions))]
    {
        result.unwrap()
    }

    #[cfg(debug_assertions)]
    {
        result
    }
}

fn create_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS home_office_days (
            date TEXT PRIMARY KEY
        )",
        [],
    )?;
    Ok(())
}

fn add_dates(conn: &Connection, date: &[NaiveDate]) -> Result<()> {
    for date in date {
        add_date(conn, *date)?;
    }
    Ok(())
}

fn add_date(conn: &Connection, date: NaiveDate) -> Result<()> {
    let date = date.format("%Y-%m-%d").to_string();
    match conn.execute(
        "INSERT INTO home_office_days (date) VALUES (?1)",
        params![date],
    ) {
        Ok(_) => println!("Date added successfully: {date}"),
        Err(err) => {
            if let Some(sqlite_error) = err.sqlite_error() {
                if sqlite_error.code == rusqlite::ErrorCode::ConstraintViolation {
                    println!("Date added successfully: {date}");
                    return Ok(());
                }
            }
            println!("Error adding date: {err}");
        }
    }

    Ok(())
}

fn add_today(conn: &Connection) -> Result<()> {
    add_date(
        conn,
        NaiveDate::parse_from_str(&Local::now().format("%Y-%m-%d").to_string(), "%Y-%m-%d")
            .unwrap(),
    )
}

fn add_specific_date(conn: &Connection) -> Result<()> {
    let today = Local::now();
    let today_string = today.format("%Y-%m-%d").to_string();

    print!("Enter a date (YYYY-MM-DD) or press Enter to use today [{today_string}]: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim();

    let date_to_add = if input.is_empty() {
        NaiveDate::parse_from_str(&today.format("%Y-%m-%d").to_string(), "%Y-%m-%d").unwrap()
    } else {
        let date = NaiveDate::parse_from_str(input, "%Y-%m-%d");
        if let Ok(date) = date {
            date
        } else {
            println!("Invalid date format. Please use YYYY-MM-DD.");
            return Ok(());
        }
    };

    add_date(conn, date_to_add)
}

fn list_dates(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT date FROM home_office_days ORDER BY date")?;
    let rows = stmt.query_map([], |row| {
        let date: String = row.get(0)?;
        Ok(date)
    })?;

    println!("Home Office Days:");
    for row in rows {
        println!("{}", row?);
    }

    Ok(())
}

fn remove_dates(conn: &Connection, date: &[NaiveDate]) -> Result<()> {
    for date in date {
        remove_date(conn, *date)?;
    }
    Ok(())
}

fn remove_date(conn: &Connection, date: NaiveDate) -> Result<()> {
    let date = date.format("%Y-%m-%d").to_string();

    match conn.execute(
        "DELETE FROM home_office_days WHERE date = ?1",
        params![date],
    ) {
        Ok(_) => println!("Date deleted successfully."),
        Err(err) => println!("Error deleting date: {err}"),
    }

    Ok(())
}

fn delete_home_office_day(conn: &Connection) -> Result<()> {
    print!("Enter a date to delete (YYYY-MM-DD): ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim();

    remove_date(conn, NaiveDate::parse_from_str(input, "%Y-%m-%d").unwrap())
}

fn get_export(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT date FROM home_office_days ORDER BY date")?;
    let rows: Vec<NaiveDate> = stmt
        .query_map([], |row| {
            let date: String = row.get(0)?;
            NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let mut ranges = Vec::new();
    let mut start = rows[0];
    let mut end = rows[0];

    for date in &rows[1..] {
        if *date
            == end
                .succ_opt()
                .expect("I'm probably not alive anymore at this point.")
        {
            end = *date;
        } else {
            ranges.push((start, end));
            start = *date;
            end = *date;
        }
    }
    ranges.push((start, end));

    let mut result = Vec::new();

    for (start, end) in ranges {
        if start == end {
            result.push(format!("{}", start.format("%Y-%m-%d")));
        } else {
            result.push(format!(
                "{} :: {}",
                start.format("%Y-%m-%d"),
                end.format("%Y-%m-%d")
            ));
        }
    }

    Ok(result)
}

fn export_dates(conn: &Connection) -> Result<()> {
    let export = get_export(conn)?;
    for v in export {
        println!("{v}");
    }
    Ok(())
}

#[derive(PartialEq, Eq, Copy, Clone)]
enum InputMode {
    Add,
    Remove,
}

struct AppState {
    conn: Connection,
    dates: Vec<String>,
    selected_index: usize,
    input_box: Option<String>,
    input_mode: InputMode,
}

impl AppState {
    fn new(conn: Connection) -> Self {
        let export = get_export(&conn).unwrap();
        Self {
            conn,
            dates: export,
            selected_index: 0,
            input_box: None,
            input_mode: InputMode::Add,
        }
    }

    fn update(&mut self) {
        let export = get_export(&self.conn).unwrap();
        self.dates = export;
    }

    fn start_input(&mut self, input_mode: InputMode) {
        self.input_mode = input_mode;
        self.input_box = Some(if self.input_mode == InputMode::Add {
            Local::now().format("%Y-%m-%d").to_string()
        } else {
            self.dates
                .get(self.selected_index)
                .cloned()
                .unwrap_or_default()
        });
    }

    fn take_input(&mut self) -> Option<String> {
        self.input_box.take()
    }

    fn add_string(&mut self, input: String) {
        add_dates(&self.conn, &parse_dates_or_default(Some(input))).unwrap();
        self.update();
    }

    fn remove_selected_string(&mut self, input: String) {
        remove_dates(&self.conn, &parse_dates_or_default(Some(input))).unwrap();
        self.update();
    }

    fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    fn move_selection_down(&mut self) {
        if self.selected_index < self.dates.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run_tui(conn: Connection) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(conn);
    terminal.clear()?;
    enable_raw_mode().unwrap();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(f.area());

            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(if state.input_box.is_some() {
                    [Constraint::Percentage(80), Constraint::Percentage(20)]
                } else {
                    [Constraint::Percentage(100), Constraint::Percentage(0)]
                })
                .split(chunks[0]);

            // Render the list of strings
            let items: Vec<ListItem> = state
                .dates
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let style = if i == state.selected_index {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Span::styled(s.clone(), style))
                })
                .collect();

            let list =
                List::new(items).block(Block::default().borders(Borders::ALL).title("Strings"));
            f.render_widget(list, left_chunks[0]);

            // Render the input box
            if let Some(ref input) = state.input_box {
                let input_paragraph = Paragraph::new(input.clone())
                    .block(Block::default().borders(Borders::ALL).title("Input"));
                f.render_widget(input_paragraph, left_chunks[1]);
            }

            const NL: &str = "\n- ";

            // Render the help box
            let help_paragraph = Paragraph::new(format!("Keybindings:{NL}Enter to add the current day{NL}A to add a specific day{NL}D to delete the selected day{NL}Esc or Q to exit"))
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::ALL).title("Help"));
            f.render_widget(help_paragraph, chunks[1]);
        })?;

        if let Event::Key(key) = event::read()? {
            if let (KeyCode::Char('c'), KeyEventKind::Press, modifiers) =
                (key.code, key.kind, key.modifiers)
            {
                if modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
            }
            if state.input_box.is_some() {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc => {
                            _ = state.take_input();
                        }
                        KeyCode::Enter => {
                            if let Some(input) = state.take_input() {
                                if input.trim().is_empty() {
                                    continue;
                                }
                                if state.input_mode == InputMode::Add {
                                    state.add_string(input);
                                } else {
                                    state.remove_selected_string(input);
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            if let Some(ref mut input) = state.input_box {
                                input.push(c);
                            }
                        }
                        KeyCode::Backspace => {
                            if let Some(ref mut input) = state.input_box {
                                input.pop();
                            }
                        }
                        _ => {}
                    }
                }
            } else if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('a') => {
                        state.start_input(InputMode::Add);
                    }
                    KeyCode::Char('d') => {
                        if !state.dates.is_empty() {
                            state.start_input(InputMode::Remove);
                        }
                    }
                    KeyCode::Enter => {
                        state.add_string(Local::now().format("%Y-%m-%d").to_string());
                        state.update();
                    }
                    KeyCode::Up => {
                        state.move_selection_up();
                    }
                    KeyCode::Down => {
                        state.move_selection_down();
                    }
                    _ => {}
                }
            }
        }
        terminal.clear()?;
    }

    disable_raw_mode().unwrap();

    terminal.clear()?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_table(&conn).unwrap();
        conn
    }

    #[test]
    fn test_create_table() {
        let conn = setup_test_db();
        let table_exists: bool = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='home_office_days'",
            )
            .unwrap()
            .exists([])
            .unwrap();

        assert!(table_exists);
    }

    #[test]
    fn test_add_date() {
        let conn = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

        add_date(&conn, date).unwrap();

        let result: String = conn
            .query_row("SELECT date FROM home_office_days", [], |row| row.get(0))
            .unwrap();

        assert_eq!(result, "2025-01-01");
    }

    #[test]
    fn test_remove_date() {
        let conn = setup_test_db();
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

        add_date(&conn, date).unwrap();
        remove_date(&conn, date).unwrap();

        let count: usize = conn
            .query_row("SELECT COUNT(*) FROM home_office_days", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_list_dates() {
        let conn = setup_test_db();
        let date1 = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let date2 = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();

        add_date(&conn, date1).unwrap();
        add_date(&conn, date2).unwrap();

        let mut output = Vec::new();
        list_dates(&conn).unwrap();

        conn.prepare("SELECT date FROM home_office_days ORDER BY date")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .for_each(|date| output.push(date.unwrap()));

        assert_eq!(output, vec!["2025-01-01", "2025-01-02"]);
    }

    #[test]
    fn test_parse_dates_or_default_single_date() {
        let input = Some("2025-01-01".to_string());
        let dates = parse_dates_or_default(input);

        assert_eq!(dates, vec![NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()]);
    }

    #[test]
    fn test_parse_dates_or_default_range() {
        let input = Some("2025-01-01::2025-01-03".to_string());
        let dates = parse_dates_or_default(input);

        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
                NaiveDate::from_ymd_opt(2025, 1, 3).unwrap(),
            ]
        );
    }

    #[test]
    fn test_export_dates() {
        let conn = setup_test_db();
        add_date(&conn, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()).unwrap();
        add_date(&conn, NaiveDate::from_ymd_opt(2025, 1, 2).unwrap()).unwrap();
        add_date(&conn, NaiveDate::from_ymd_opt(2025, 1, 3).unwrap()).unwrap();

        let export = get_export(&conn).unwrap();

        assert_eq!(export, vec!["2025-01-01 :: 2025-01-03"]);
    }
}
