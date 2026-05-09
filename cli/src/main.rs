#![feature(stmt_expr_attributes)]
#![feature(thread_sleep_until)]
#![forbid(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![deny(mismatched_lifetime_syntaxes)]
#![forbid(clippy::enum_glob_use)]
#![forbid(clippy::unwrap_used)]
#![allow(clippy::doc_markdown)]
// #![allow(clippy::unwrap_used)]
#![allow(clippy::too_many_lines)]

mod tabs;

use std::{
    collections::HashMap,
    io,
    sync::Mutex,
    time::{Duration, Instant},
};

use backend::{EnumCount, IntoEnumIterator};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, ModifierKeyCode, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, ListState, Paragraph, Tabs, Wrap},
};
use tabs::{battery_tab, bluetooth_tab, component_tab, cpu_tab, disk_tab, display_tab, memory_tab, network_tab, process_tab, system_tab};

pub(crate) type DataPoint = (f64, f64);
pub(crate) type DataPoints = Vec<DataPoint>;

#[derive(Copy, Clone, Debug)]
pub(crate) enum Ordering {
    Ascending,
    Descending,
}

impl Ordering {
    pub(crate) fn sort_by<T>(&self) -> impl Fn(T, T) -> std::cmp::Ordering + '_
    where
        T: std::cmp::PartialOrd,
    {
        move |a, b| match self {
            Self::Ascending => a.partial_cmp(&b).expect("values should be comparable"),
            Self::Descending => b.partial_cmp(&a).expect("values should be comparable"),
        }
    }
}

// Function copied straight from https://github.com/ratatui-org/ratatui/blob/main/examples/popup.rs
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum SortByProcess {
    CpuUsage(Ordering),
    MemoryUsage(Ordering),
    SwapUsage(Ordering),
    Runtime(Ordering),
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum SortByComponent {
    Temperature(Ordering),
    Critical(Ordering),
}

#[derive(Clone, Debug)]
pub(crate) enum ProcessPopup {
    KillProcess { process_name: String, pid: sysinfo::Pid },
    MoreInformation { contents: String },
    NoSelected,
}

struct AppState {
    manager:               backend::Manager,
    current_line:          u16,
    max_scroll:            u16,
    current_tab:           usize,
    ram_important_digits:  Option<f64>,
    swap_important_digits: Option<f64>,
    starting_time:         Instant,
    process_ordering:      SortByProcess,
    component_ordering:    SortByComponent,
    shift_pressed:         bool,
    kill_current_process:  bool,
    more_information:      bool,
    process_to_kill:       Option<(String, sysinfo::Pid)>,
    confirm_kill:          Option<bool>,
    cpu_dataset:           HashMap<backend::CpuInfo, DataPoints>,
    ram_dataset:           DataPoints,
    swap_dataset:          DataPoints,
}

pub(crate) static NETWORK_INFO: Mutex<Option<backend::NetworkInfo>> = Mutex::new(None);
pub(crate) const INTERVAL: Duration = Duration::from_secs(1);
pub(crate) const MAX_FPS: u32 = 60;

struct Logo;

impl Logo {
    const LOGO_20: &'static str = include_str!("../../logo/ascii_logo_20.txt");
    const LOGO_40: &'static str = include_str!("../../logo/ascii_logo_40.txt");
    const LOGO_80: &'static str = include_str!("../../logo/ascii_logo_80.txt");
    const LOGO_MINI: &'static str = include_str!("../../logo/ascii_logo_mini.txt");
}

impl Logo {
    const fn get(index: usize) -> &'static str {
        match index {
            ..20 => Self::LOGO_MINI,
            20..40 => Self::LOGO_20,
            40..80 => Self::LOGO_40,
            80.. => Self::LOGO_80,
        }
    }
}

const WIDTH_NUMERATOR: usize = 1400; // This is basically a magic number I found using trial and error. If there
// is a mathematical way to get this same number or an even better one,
// tell me about it.

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, fps_test: bool) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let mut parallel_manager = backend::Manager::new();
        loop {
            if receiver.try_recv().is_ok() {
                break;
            }
            let network_info_temp = Some(parallel_manager.network_information()); // This temporary must be used otherwise
            // network_tab blocks on NETWORK_INFO.lock
            let mut network_info = NETWORK_INFO.lock().expect("network info mutex poisoned");
            *network_info = network_info_temp;
        }
    });

    let mut app_state = AppState {
        manager:               backend::Manager::new(),
        current_line:          0,
        max_scroll:            u16::MAX,
        current_tab:           0,
        ram_important_digits:  None,
        swap_important_digits: None,
        starting_time:         Instant::now(),
        process_ordering:      SortByProcess::CpuUsage(Ordering::Descending),
        component_ordering:    SortByComponent::Temperature(Ordering::Descending),
        shift_pressed:         false,
        kill_current_process:  false,
        more_information:      false,
        process_to_kill:       None,
        confirm_kill:          None,
        cpu_dataset:           HashMap::new(),
        ram_dataset:           vec![],
        swap_dataset:          vec![],
    };

    let mut latest_update = Instant::now();
    let mut elapsed: Duration;

    // Note: This assumes that the amount of RAM and SWAP stays constant. I
    // would guess the chance of this breaking is quite low (I hope)
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::while_float)]
    if let Some(memory_info) = app_state.manager.memory_information() {
        app_state.ram_important_digits = Some(memory_info.total_memory as f64);
        while app_state.ram_important_digits.expect("just set above") > 1000.0 {
            app_state.ram_important_digits = Some(app_state.ram_important_digits.expect("just set above") / 1000.0);
        }
        app_state.ram_important_digits = Some(app_state.ram_important_digits.expect("just set above").floor());

        app_state.swap_important_digits = Some(memory_info.total_swap as f64);
        while app_state.swap_important_digits.expect("just set above") > 1000.0 {
            app_state.swap_important_digits = Some(app_state.swap_important_digits.expect("just set above") / 1000.0);
        }
        app_state.swap_important_digits = Some(app_state.swap_important_digits.expect("just set above").floor());
    }

    let welcome_parts = [
        r"Welcome to the Crossinfo TUI, the place to get infos about your system at the command-line!

",
        r"

Press Enter to continue using the program if you're already familiar with it.

Otherwise, read carefully!

This program uses three major interactive elements: Tabs, Paragraphs and Lists

The tabs can be navigated using the left and right arrow keys. They are shown at the top of the screen.

The paragraphs can be scrolled using either the up and down arrow or the scroll wheel.

The lists can be scrolled in the same way paragraphs can be, but they (sometimes) offer an extra element of interactivity: sorting. If you want to sort a list by a certain property, look out for the list header, where different properties are listed. If the list can be sorted after a certain property, there is a pair of square brackets containing a letter next to it. If you press this letter in its small form (without shift), the list is sorted after that property in ascending order. If you press the letter in its capital form (with shift), the list is sorted in descending order.

To exit the program, press 'q' or Esc.
",
    ];

    loop {
        let _ = terminal.draw(|f| {
            let height = f.area().height as usize;
            let width = f.area().width as usize;
            let welcome_text = welcome_parts[0].to_string()
                + Logo::get(
                    height
                        - std::cmp::min(
                            WIDTH_NUMERATOR / width,
                            height, /* This
                                    is add so there is no underflow */
                        ),
                )
                + welcome_parts[1];
            f.render_widget(
                Paragraph::new(welcome_text.split('\n').map(|line| Line::from(Span::raw(line))).collect::<Vec<Line>>())
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: false }),
                f.area(),
            );
        });
        if crossterm::event::poll(Duration::from_millis(0)).expect("failed to poll for events")
            && let Ok(Event::Key(event)) = crossterm::event::read()
        {
            match event.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    let _ = sender.send(());
                    let _ = thread.join();
                    return;
                }
                KeyCode::Enter => {
                    break;
                }
                _ => (),
            }
        }
    }
    app_state.starting_time = Instant::now(); // I don't want there to be a big gap in the data if the tutorial screen is

    let mut fps_accumulator = 0;
    loop {
        if fps_test {
            let seconds_passed = app_state.starting_time.elapsed().as_secs();
            let mut fps = FPS.lock().expect("FPS mutex poisoned");
            #[allow(clippy::cast_possible_truncation)]
            if let Some(current_fps) = fps.get_mut(seconds_passed as usize)
                && *current_fps > 0
            {
                *current_fps += 1;
            } else {
                fps_accumulator += 1;
                if fps_accumulator == 5 {
                    app_state.current_tab += 1;
                    fps_accumulator = 0;
                }
                if app_state.current_tab == backend::Tab::COUNT {
                    let _ = std::fs::write("log.txt", format!("{fps:#?}"));
                    sender.send(()).expect("Failed to send shutdown signal");
                    thread.join().expect("Failed to join network thread");
                    return;
                }
                #[allow(clippy::cast_possible_truncation)]
                if let Some(slot) = fps.get_mut(seconds_passed as usize) {
                    *slot = 1;
                }
                drop(fps);
            }
        }

        let frame_start = Instant::now();

        let _ = terminal.draw(|f| ui(f, &mut app_state));
        app_state.confirm_kill = None;
        app_state.shift_pressed = false;

        elapsed = app_state.starting_time.elapsed();

        if let Some(cpu_info) = app_state.manager.cpu_information()
            && let Some(memory_info) = app_state.manager.memory_information()
        {
            if app_state.cpu_dataset.is_empty() {
                latest_update = Instant::now();
                for cpu_core in cpu_info {
                    app_state.cpu_dataset.insert(cpu_core.clone(), vec![(elapsed.as_secs_f64(), f64::from(cpu_core.usage))]);
                }
            } else if latest_update.elapsed() > INTERVAL {
                latest_update = Instant::now();
                for cpu_core in cpu_info {
                    app_state
                        .cpu_dataset
                        .get_mut(&cpu_core)
                        .expect("The core should exist")
                        .push((elapsed.as_secs_f64(), f64::from(cpu_core.usage)));
                }

                app_state.ram_dataset.push((elapsed.as_secs_f64(), match memory_info.total_memory {
                    // This is highly unlikely to ever trigger as computers tend to have memory
                    0 => 0.0,
                    #[allow(clippy::cast_precision_loss)]
                    _ => (memory_info.used_memory as f64 / memory_info.total_memory as f64) * app_state.ram_important_digits.expect("ram digits should be set when memory info is available"),
                }));

                app_state.swap_dataset.push((elapsed.as_secs_f64(), match memory_info.total_swap {
                    0 => 0.0,
                    #[allow(clippy::cast_precision_loss)]
                    _ => (memory_info.used_swap as f64 / memory_info.total_swap as f64) * app_state.swap_important_digits.expect("swap digits should be set when memory info is available"),
                }));
            }
        }

        if crossterm::event::poll(Duration::from_millis(0)).expect("failed to poll for events") {
            match crossterm::event::read() {
                Ok(Event::Key(event)) => match event.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        let _ = sender.send(());
                        let _ = thread.join();
                        return;
                    }
                    KeyCode::Char(chr) => match chr {
                        'c' => match app_state.current_tab {
                            6 => app_state.process_ordering = SortByProcess::CpuUsage(Ordering::Ascending),
                            7 => app_state.component_ordering = SortByComponent::Critical(Ordering::Ascending),
                            _ => (),
                        },
                        'C' => match app_state.current_tab {
                            6 => app_state.process_ordering = SortByProcess::CpuUsage(Ordering::Descending),
                            7 => app_state.component_ordering = SortByComponent::Critical(Ordering::Descending),
                            _ => (),
                        },
                        'm' => {
                            app_state.process_ordering = SortByProcess::MemoryUsage(Ordering::Ascending);
                        }
                        'M' => {
                            app_state.process_ordering = SortByProcess::MemoryUsage(Ordering::Descending);
                        }
                        's' => {
                            app_state.process_ordering = SortByProcess::SwapUsage(Ordering::Ascending);
                        }
                        'S' => {
                            app_state.process_ordering = SortByProcess::SwapUsage(Ordering::Descending);
                        }
                        'r' => {
                            app_state.process_ordering = SortByProcess::Runtime(Ordering::Ascending);
                        }
                        'R' => {
                            app_state.process_ordering = SortByProcess::Runtime(Ordering::Descending);
                        }
                        't' => {
                            app_state.component_ordering = SortByComponent::Temperature(Ordering::Ascending);
                        }
                        'T' => {
                            app_state.component_ordering = SortByComponent::Temperature(Ordering::Descending);
                        }
                        'k' => {
                            app_state.kill_current_process = true;
                        }
                        'i' => {
                            app_state.more_information = true;
                        }
                        'x' => {
                            app_state.more_information = false;
                            app_state.kill_current_process = false;
                            app_state.process_to_kill = None;
                        }
                        'y' => {
                            app_state.confirm_kill = Some(true);
                            app_state.kill_current_process = false;
                        }
                        'n' => {
                            app_state.confirm_kill = Some(false);
                            app_state.kill_current_process = false;
                            app_state.process_to_kill = None;
                        }
                        _ => (),
                    },
                    KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift) => {
                        app_state.shift_pressed = true;
                    }
                    KeyCode::Up => app_state.current_line = app_state.current_line.saturating_sub(1),
                    KeyCode::Down => app_state.current_line = app_state.current_line.saturating_add(1).min(app_state.max_scroll),
                    KeyCode::Left => {
                        app_state.current_tab = app_state.current_tab.saturating_sub(1);
                        app_state.current_line = 0;
                        app_state.max_scroll = u16::MAX;
                    }
                    KeyCode::Right => {
                        if app_state.current_tab < backend::Tab::COUNT - 1 {
                            app_state.current_tab += 1;
                        }
                        app_state.current_line = 0;
                        app_state.max_scroll = u16::MAX;
                    }
                    _ => (),
                },
                Ok(Event::Mouse(event)) => match event.kind {
                    MouseEventKind::ScrollDown => app_state.current_line = app_state.current_line.saturating_add(1).min(app_state.max_scroll),
                    MouseEventKind::ScrollUp => app_state.current_line = app_state.current_line.saturating_sub(1),
                    _ => (),
                },
                _ => (),
            }
        }
        std::thread::sleep_until(frame_start + Duration::from_secs(1) / MAX_FPS);
    }
}

pub(crate) fn format_duration(duration: &Duration) -> String {
    format!("{:0>2}:{:0>2}:{:0>2}", duration.as_secs() / 3600, (duration.as_secs() / 60) % 60, duration.as_secs() % 60)
}

pub(crate) fn to_string_or_unknown<T: ToString>(opt: Option<T>) -> String {
    opt.map_or_else(|| String::from("unknown"), |t| t.to_string())
}

pub(crate) fn format_or_unknown<T>(opt: Option<T>, formatter: &impl Fn(T) -> String) -> String {
    opt.map_or_else(|| "unknown".to_string(), formatter)
}

pub(crate) fn column_width(label: &str, widths: impl Iterator<Item = usize>) -> usize {
    widths.fold(label.len(), std::cmp::max)
}

pub(crate) const fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

static FPS: Mutex<[u16; 50]> = Mutex::new([0; 50]);

fn ui(f: &mut Frame, app_state: &mut AppState) {
    let titles = backend::Tab::iter().map(|tab| Line::from(tab.to_string())).collect::<Vec<Line>>();

    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(size);

    let cpu_vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(chunks[1]);

    let network_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)])
        .split(chunks[1]);

    let block = Block::default().style(Style::default().bg(Color::Black).fg(Color::White));

    f.render_widget(block, size);

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL))
        .select(app_state.current_tab)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::White).fg(Color::Black));

    let popup_rect = centered_rect(50, 70, chunks[1]);

    f.render_widget(tabs, chunks[0]);

    let mut list_state = ListState::default();
    list_state.select(Some(app_state.current_line as usize));

    match app_state.current_tab {
        0 => {
            let (widget, count) = system_tab(&mut app_state.manager, app_state.current_line);
            app_state.max_scroll = count.saturating_sub(1);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            f.render_widget(widget, chunks[1]);
        }
        #[allow(clippy::cast_possible_truncation)]
        1 => {
            let (cpu_tab_widgets, count) = cpu_tab(
                &mut app_state.manager,
                app_state.starting_time,
                &app_state.cpu_dataset.iter().map(|(cpu_core, dataset)| (cpu_core, dataset.as_slice())).collect(),
            );
            app_state.max_scroll = count.saturating_sub(1);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            list_state.select(Some(app_state.current_line as usize));

            let cpu_list_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(100 / cpu_tab_widgets.len() as u16); cpu_tab_widgets.len()])
                .split(cpu_vertical_chunks[0]);

            let cpu_chart_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(100 / cpu_tab_widgets.len() as u16); cpu_tab_widgets.len()])
                .split(cpu_vertical_chunks[1]);

            for (index, (list, chart)) in cpu_tab_widgets.iter().enumerate() {
                f.render_stateful_widget(list.clone(), cpu_list_chunks[index], &mut list_state);
                f.render_widget(chart.clone(), cpu_chart_chunks[index]);
            }
        }
        2 => f.render_widget(
            memory_tab(
                &mut app_state.manager,
                app_state.starting_time,
                app_state.ram_dataset.as_slice(),
                app_state.swap_dataset.as_slice(),
                app_state.ram_important_digits,
                app_state.swap_important_digits,
            ),
            chunks[1],
        ),
        3 => {
            let (widget, count) = disk_tab(&mut app_state.manager, app_state.current_line);
            app_state.max_scroll = count.saturating_sub(1);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            f.render_widget(widget, chunks[1]);
        }
        4 => {
            let (widget, count) = battery_tab(&app_state.manager, app_state.current_line);
            app_state.max_scroll = count.saturating_sub(1);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            f.render_widget(widget, chunks[1]);
        }
        5 => {
            let networks_count = NETWORK_INFO
                .lock()
                .ok()
                .and_then(|g| g.as_ref().and_then(|ni| ni.networks.as_ref().map(Vec::len)))
                .unwrap_or(0);
            app_state.max_scroll = u16::try_from(networks_count.saturating_sub(1)).unwrap_or(u16::MAX);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            let mut network_list_state = ListState::default();
            network_list_state.select(Some(app_state.current_line as usize));
            let network_tab_widgets = network_tab(app_state.more_information, app_state.current_line);
            f.render_widget(network_tab_widgets.0, network_chunks[0]);
            f.render_widget(network_tab_widgets.1, network_chunks[1]);
            f.render_stateful_widget(network_tab_widgets.2, network_chunks[2], &mut network_list_state);
            if let Some(text) = network_tab_widgets.3 {
                f.render_widget(Clear, popup_rect);
                f.render_widget(
                    Paragraph::new(text)
                        .block(Block::default().title(Line::from("[x]").alignment(Alignment::Right)).borders(Borders::ALL))
                        .style(Style::default().fg(Color::White).bg(Color::Black))
                        .alignment(Alignment::Left)
                        .wrap(Wrap { trim: false }),
                    popup_rect,
                );
            }
        }
        6 => {
            let (process_list, process_popup, count) = process_tab(
                &mut app_state.manager,
                app_state.process_ordering,
                app_state.shift_pressed,
                app_state.kill_current_process,
                app_state.more_information,
                app_state.current_line,
            );
            app_state.max_scroll = count.saturating_sub(1);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            list_state.select(Some(app_state.current_line as usize));
            f.render_stateful_widget(process_list, chunks[1], &mut list_state);
            let popup_information: Option<(&str, String)> = match process_popup {
                Some(ProcessPopup::KillProcess { process_name, pid }) => {
                    if app_state.process_to_kill.is_none() {
                        app_state.process_to_kill = Some((process_name, pid));
                    }
                    Some((
                        "Kill process?",
                        format!(
                            r#"Do you really want to kill the process "{}"?

[y]es        [n]o"#,
                            app_state.process_to_kill.as_ref().expect("process_to_kill was just set above").0
                        ),
                    ))
                }
                Some(ProcessPopup::MoreInformation { contents }) => Some(("More information", contents)),
                Some(ProcessPopup::NoSelected) => Some(("No process selected!", "You don't have a process selected!".to_string())),
                None => None,
            };
            if app_state.confirm_kill.is_some_and(|x| x) {
                app_state.manager.kill_process(app_state.process_to_kill.as_ref().expect("Pid should be set at this point. Report").1);
                app_state.process_to_kill = None;
            }
            if let Some((title, body)) = popup_information {
                f.render_widget(Clear, popup_rect);
                f.render_widget(
                    Paragraph::new(body)
                        .block(
                            Block::default()
                                .title(Line::from("[x]").alignment(Alignment::Right))
                                .title(Line::from(title).alignment(Alignment::Center))
                                .borders(Borders::ALL),
                        )
                        .style(Style::default().fg(Color::White).bg(Color::Black))
                        .alignment(Alignment::Center)
                        .wrap(Wrap { trim: false }),
                    popup_rect,
                );
            }
        }
        7 => {
            let (widget, count) = component_tab(&mut app_state.manager, app_state.component_ordering, app_state.shift_pressed);
            app_state.max_scroll = count.saturating_sub(1);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            list_state.select(Some(app_state.current_line as usize));
            f.render_stateful_widget(widget, chunks[1], &mut list_state);
        }
        8 => {
            let (widget, count) = display_tab(&app_state.manager, app_state.current_line);
            app_state.max_scroll = count.saturating_sub(1);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            f.render_widget(widget, chunks[1]);
        }
        9 => {
            let (widget, count) = bluetooth_tab(&app_state.manager);
            app_state.max_scroll = count.saturating_sub(1);
            app_state.current_line = app_state.current_line.min(app_state.max_scroll);
            list_state.select(Some(app_state.current_line as usize));
            f.render_stateful_widget(widget, chunks[1], &mut list_state);
        }
        _ => unreachable!(),
    }
}

fn main() -> Result<(), io::Error> {
    let fps_test = std::env::args().any(|arg| arg == "--fps-test");

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    run_app(&mut terminal, fps_test);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}
