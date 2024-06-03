#![feature(let_chains)]
#![forbid(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![forbid(clippy::enum_glob_use)]
// #![forbid(clippy::unwrap_used)]
#![allow(clippy::doc_markdown)]
// TODO: remove this one
#![allow(clippy::unwrap_used)]
#![allow(clippy::too_many_lines)]

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
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use itertools::Itertools;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{block::Title, Axis, Block, Borders, Chart, Clear, Dataset, GraphType, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};

type DataPoint = (f64, f64);
type DataPoints = Vec<DataPoint>;

#[derive(Copy, Clone, Debug)]
enum Ordering {
    Ascending,
    Descending,
}

impl Ordering {
    fn sort_by<T>(&self) -> impl Fn(T, T) -> std::cmp::Ordering + '_
    where
        T: std::cmp::PartialOrd,
    {
        move |a, b| match self {
            Self::Ascending => a.partial_cmp(&b).unwrap(),
            Self::Descending => b.partial_cmp(&a).unwrap(),
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
enum SortByProcess {
    CpuUsage(Ordering),
    MemoryUsage(Ordering),
    SwapUsage(Ordering),
    Runtime(Ordering),
}

#[derive(Copy, Clone, Debug)]
enum SortByComponent {
    Temperature(Ordering),
    Critical(Ordering),
}

#[derive(Clone, Debug)]
enum ProcessPopup {
    KillProcess { process_name: String, pid: sysinfo::Pid },
    MoreInformation { contents: String },
    NoSelected,
}

struct AppState {
    manager:               backend::Manager,
    current_line:          u16,
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

static NETWORK_INFO: Mutex<Option<backend::NetworkInfo>> = Mutex::new(None);
const INTERVAL: Duration = Duration::from_secs(1);

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

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let mut parallel_manager = backend::Manager::new();
        loop {
            if receiver.try_recv().is_ok() {
                break;
            }
            let network_info_temp = Some(parallel_manager.network_information()); // This temporary must be used otherwise
                                                                                  // network_tab blocks on NETWORK_INFO.lock
            let mut network_info = NETWORK_INFO.lock().unwrap();
            *network_info = network_info_temp;
        }
    });

    let mut app_state = AppState {
        manager:               backend::Manager::new(),
        current_line:          0,
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
        while app_state.ram_important_digits.unwrap() > 1000.0 {
            app_state.ram_important_digits = Some(app_state.ram_important_digits.unwrap() / 1000.0);
        }
        app_state.ram_important_digits = Some(app_state.ram_important_digits.unwrap().floor());

        app_state.swap_important_digits = Some(memory_info.total_swap as f64);
        while app_state.swap_important_digits.unwrap() > 1000.0 {
            app_state.swap_important_digits = Some(app_state.swap_important_digits.unwrap() / 1000.0);
        }
        app_state.swap_important_digits = Some(app_state.swap_important_digits.unwrap().floor());
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
            let height = f.size().height as usize;
            let width = f.size().width as usize;
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
                f.size(),
            );
        });
        if crossterm::event::poll(Duration::from_millis(0)).unwrap() {
            if let Ok(Event::Key(event)) = crossterm::event::read() {
                match event.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        sender.send(()).unwrap();
                        thread.join().unwrap();
                        return;
                    }
                    KeyCode::Enter => {
                        break;
                    }
                    _ => (),
                }
            }
        }
    }
    app_state.starting_time = Instant::now(); // I don't want there to be a big gap in the data if the tutorial screen is
                                              // read

    // let mut accumulator = 0;
    loop {
        // Code to test FPS
        // TODO delete this
        // let seconds_passed = app_state.starting_time.elapsed().as_secs();
        // let mut fps = FPS.lock().unwrap();
        // if let Some(current_fps) = fps.get_mut(seconds_passed as usize)
        //     && *current_fps > 0
        // {
        //     *current_fps += 1;
        // } else {
        //     accumulator += 1;
        //     if accumulator == 5 {
        //         app_state.current_tab += 1;
        //         accumulator = 0;
        //     }
        //     if app_state.current_tab == 8 {
        //         std::fs::write("log2.txt", format!("{fps:#?}")).expect("wtf");
        //         panic!();
        //     }
        //     fps[seconds_passed as usize] = 1;
        // }

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
                    _ => (memory_info.used_memory as f64 / memory_info.total_memory as f64) * app_state.ram_important_digits.unwrap(),
                }));

                app_state.swap_dataset.push((elapsed.as_secs_f64(), match memory_info.total_swap {
                    0 => 0.0,
                    #[allow(clippy::cast_precision_loss)]
                    _ => (memory_info.used_swap as f64 / memory_info.total_swap as f64) * app_state.swap_important_digits.unwrap(),
                }));
            }
        }

        if crossterm::event::poll(Duration::from_millis(0)).unwrap() {
            match crossterm::event::read() {
                Ok(Event::Key(event)) => match event.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        sender.send(()).unwrap();
                        thread.join().unwrap();
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
                        // This just straight up doesn't work
                        app_state.shift_pressed = true;
                    }
                    KeyCode::Up => app_state.current_line = app_state.current_line.saturating_sub(1),
                    KeyCode::Down => app_state.current_line = app_state.current_line.saturating_add(1),
                    KeyCode::Left => {
                        app_state.current_tab = app_state.current_tab.saturating_sub(1);
                        app_state.current_line = 0;
                    }
                    KeyCode::Right => {
                        if app_state.current_tab < backend::Tab::COUNT - 1 {
                            app_state.current_tab += 1;
                        }
                        app_state.current_line = 0;
                    }
                    _ => (),
                },
                Ok(Event::Mouse(event)) => match event.kind {
                    // TODO: Limit scrolling
                    MouseEventKind::ScrollDown => app_state.current_line = app_state.current_line.saturating_add(1),
                    MouseEventKind::ScrollUp => app_state.current_line = app_state.current_line.saturating_sub(1),
                    _ => (),
                },
                _ => (),
            }
        }
    }
}

fn format_duration(duration: &Duration) -> String {
    format!("{:0>2}:{:0>2}:{:0>2}", duration.as_secs() / 3600, (duration.as_secs() / 60) % 60, duration.as_secs() % 60)
}

// TODO: Convert as much as possible to this function
fn to_string_or_unknown<T: ToString>(opt: Option<T>) -> String {
    opt.map_or_else(|| String::from("unknown"), |t| t.to_string())
}

fn format_or_unknown<T>(opt: Option<T>, formatter: &impl Fn(T) -> String) -> String {
    opt.map_or("unknown".to_string(), formatter)
}

static FPS: Mutex<[u16; 40]> = Mutex::new([0; 40]);

fn ui(f: &mut Frame, app_state: &mut AppState) {
    let titles = backend::Tab::iter().map(|tab| Line::from(tab.to_string())).collect::<Vec<Line>>();

    let size = f.size();

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
        0 => f.render_widget(system_tab(&mut app_state.manager, app_state.current_line), chunks[1]),
        #[allow(clippy::cast_possible_truncation)]
        1 => {
            let cpu_tab_widgets = cpu_tab(
                &mut app_state.manager,
                app_state.starting_time,
                &app_state.cpu_dataset.iter().map(|(cpu_core, dataset)| (cpu_core, dataset.as_slice())).collect(),
            );

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
        3 => f.render_widget(disk_tab(&mut app_state.manager, app_state.current_line), chunks[1]),
        4 => f.render_widget(battery_tab(&app_state.manager, app_state.current_line), chunks[1]),
        5 => {
            let network_tab_widgets = network_tab(app_state.more_information, app_state.current_line);
            f.render_widget(network_tab_widgets.0, network_chunks[0]);
            f.render_stateful_widget(network_tab_widgets.1, network_chunks[1], &mut list_state);
            f.render_stateful_widget(network_tab_widgets.2, network_chunks[2], &mut list_state);
            if let Some(text) = network_tab_widgets.3 {
                f.render_widget(Clear, popup_rect);
                f.render_widget(
                    Paragraph::new(text)
                        .block(Block::default().title(Title::from("[x]").alignment(Alignment::Right)).borders(Borders::ALL))
                        .style(Style::default().fg(Color::White).bg(Color::Black))
                        .alignment(Alignment::Left)
                        .wrap(Wrap { trim: false }),
                    popup_rect,
                );
            }
        }
        6 => {
            let process_tab_widgets = process_tab(
                &mut app_state.manager,
                app_state.process_ordering,
                app_state.shift_pressed,
                app_state.kill_current_process,
                app_state.more_information,
                app_state.current_line,
            );
            f.render_stateful_widget(process_tab_widgets.0, chunks[1], &mut list_state);
            let popup_information: Option<(&str, String)> = match process_tab_widgets.1 {
                Some(ProcessPopup::KillProcess { process_name, pid }) => {
                    if app_state.process_to_kill.is_none() {
                        app_state.process_to_kill = Some((process_name, pid));
                    }
                    Some((
                        "Kill process?",
                        format!(
                            r#"Do you really want to kill the process "{}"?
                        
[y]es        [n]o"#,
                            app_state.process_to_kill.as_ref().unwrap().0
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
                                .title(Title::from("[x]").alignment(Alignment::Right))
                                .title(Title::from(title).alignment(Alignment::Center))
                                .borders(Borders::ALL),
                        )
                        .style(Style::default().fg(Color::White).bg(Color::Black))
                        .alignment(Alignment::Center)
                        .wrap(Wrap { trim: false }),
                    popup_rect,
                );
            }
        }
        7 => f.render_stateful_widget(component_tab(&mut app_state.manager, app_state.component_ordering, app_state.shift_pressed), chunks[1], &mut list_state),
        // 8 => f.render_widget(display_tab(&mut app_state.manager, app_state.current_line), chunks[1]),
        // 9 => f.render_widget(bluetooth_tab(&mut app_state.manager, app_state.current_line), chunks[1]),
        _ => unreachable!(),
    };
}

fn system_tab(manager: &mut backend::Manager, scroll: u16) -> Paragraph {
    if let Some(system_info) = manager.system_information() {
        let text = [
            vec![
                Line::from(vec![Span::raw("Operating System: "), Span::raw(to_string_or_unknown(system_info.os))]),
                Line::from(vec![Span::raw("Operating System Version: "), Span::raw(to_string_or_unknown(system_info.os_version))]),
                Line::from(vec![Span::raw("Kernel Version: "), Span::raw(to_string_or_unknown(system_info.kernel_version))]),
                Line::from(vec![Span::raw("Uptime: "), Span::raw(format_duration(&system_info.uptime))]),
                Line::from(Span::raw("Users: ")),
            ],
            system_info.users.iter().map(|user| Line::from(Span::raw(format!("   {user}\n")))).collect(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<Line>>();

        Paragraph::new(text).scroll((scroll, 0))
    } else {
        Paragraph::new("No information available!")
    }
    .block(Block::default().title("System").borders(Borders::ALL))
    .style(Style::default().fg(Color::White).bg(Color::Black))
    .alignment(Alignment::Left)
    .wrap(Wrap { trim: false })
}

const COLORS: [Color; 15] = [
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

// TODO: Make the charts a lil better in manycpu
// setups
fn cpu_tab<'a>(manager: &'a mut backend::Manager, starting_time: Instant, cpu_dataset: &HashMap<&'a backend::CpuInfo, &'a [DataPoint]>) -> Vec<(List<'a>, Chart<'a>)> {
    static LATEST_INFO: Mutex<(Option<Vec<backend::CpuInfo>>, Option<Instant>)> = Mutex::new((None, None));

    let mut latest_info = LATEST_INFO.lock().unwrap();

    if latest_info.1.is_none() || latest_info.1.unwrap().elapsed() > INTERVAL {
        *latest_info = (manager.cpu_information(), Some(Instant::now()));
    }

    let elapsed = starting_time.elapsed();

    let mut res = latest_info.0.clone().map_or_else(
        || vec![(List::new::<Vec<&str>>(vec![]), Chart::new(vec![]))],
        |mut cpu_info| {
            cpu_info.sort_unstable_by(|a, b| a.manufacturer.cmp(&b.manufacturer));
            let sorted_cpu_info = cpu_info
                .iter()
                .chunk_by(|cpu_core| cpu_core.manufacturer.clone())
                .into_iter()
                .map(|(_key, info)| info.cloned().collect())
                .collect::<Vec<Vec<backend::CpuInfo>>>(); // This is only ever necessary in multi CPU
                                                          // setups, but I don't want a issue six years down
                                                          // the line when multi CPU has become the norm
            sorted_cpu_info
                .iter()
                .map(|cpu| {
                    (
                        {
                            let usage_label = "Usage";
                            let model_label = "Model/Core Nr.";
                            let manufacturer_label = "Manufacturer";
                            let frequency_label = "Frequency (GHz)";
                            let mut usage_width = usage_label.len();
                            let mut model_width = model_label.len();
                            let mut manufacturer_width = manufacturer_label.len();
                            let mut frequency_width = frequency_label.len();
                            for cpu_core in cpu {
                                let usage_candidate = format!("{:.2}", cpu_core.usage).len();
                                if usage_width < usage_candidate {
                                    usage_width = usage_candidate;
                                }
                                if model_width < cpu_core.model.len() {
                                    model_width = cpu_core.model.len();
                                }
                                if manufacturer_width < cpu_core.manufacturer.len() {
                                    manufacturer_width = cpu_core.manufacturer.len();
                                }
                                let frequency_candidate = format!("{:.2}", cpu_core.frequency.get::<uom::si::frequency::gigahertz>()).len();
                                if frequency_width < frequency_candidate {
                                    frequency_width = frequency_candidate;
                                }
                            }
                            List::new(cpu.iter().map(|cpu_core| {
                                ListItem::new(format!(
                                    "{:manufacturer_width$}  {:model_width$}  {:frequency_width$.2}  {:usage_width$.2}%",
                                    "",
                                    cpu_core.model.clone(),
                                    cpu_core.frequency.get::<uom::si::frequency::gigahertz>(),
                                    cpu_core.usage
                                ))
                            }))
                            .block(
                                Block::default()
                                    .title(format!(
                                        "{:manufacturer_width$}  {model_label:model_width$}  {frequency_label:frequency_width$}  {usage_label:usage_width$}",
                                        cpu[0].manufacturer.clone()
                                    ))
                                    .borders(Borders::ALL),
                            )
                        },
                        Chart::new(
                            cpu.iter()
                                .enumerate()
                                .map(|(index, cpu_core)| {
                                    Dataset::default()
                                        .name(cpu_core.model.clone())
                                        .marker(Marker::Braille)
                                        .graph_type(GraphType::Line)
                                        .style(Style::default().fg(if index < COLORS.len() {
                                            COLORS[index]
                                        } else {
                                            #[allow(clippy::cast_possible_truncation)]
                                            Color::Rgb(((index * 100) % 255) as u8, ((index * 50) % 255) as u8, ((index * 75) % 255) as u8)
                                        }))
                                        .data(cpu_dataset[cpu_core])
                                })
                                .collect(),
                        ),
                    )
                })
                .collect()
        },
    );
    drop(latest_info);
    for (list, chart) in &mut res {
        *list = list
            .clone()
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
        *chart = chart
            .clone()
            .style(Style::default().bg(Color::Black).fg(Color::White))
            .x_axis(
                Axis::default()
                    .title(Span::raw("Seconds Elapsed"))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .bounds([0.0, elapsed.as_secs_f64()])
                    .labels(
                        ["0".to_string(), (elapsed / 2).as_secs().to_string(), elapsed.as_secs().to_string()]
                            .iter()
                            .cloned()
                            .map(Span::from)
                            .collect(),
                    ),
            )
            .y_axis(
                Axis::default()
                    .title(Span::raw("CPU usage"))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .bounds([0.0, 100.0])
                    .labels(["0%", "50%", "100%"].iter().copied().map(Span::raw).collect()),
            );
    }
    res
}

fn memory_tab<'a>(
    manager: &mut backend::Manager,
    starting_time: Instant,
    ram_dataset: &'a [DataPoint],
    swap_dataset: &'a [DataPoint],
    ram_important_digits: Option<f64>,
    swap_important_digits: Option<f64>,
) -> Chart<'a> {
    let formatter = humansize::make_format(humansize::DECIMAL);

    let elapsed = starting_time.elapsed();

    if let Some(memory_info) = manager.memory_information() {
        let ram_important_digits = ram_important_digits.unwrap();
        let swap_important_digits = swap_important_digits.unwrap();

        let max_y_axis_bound = ram_important_digits.max(swap_important_digits);
        let max_y_axis_label = memory_info.total_memory.max(memory_info.total_swap);
        let datasets = vec![
            Dataset::default()
                .name("RAM used")
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(ram_dataset),
            Dataset::default()
                .name("SWAP used")
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Green))
                .data(swap_dataset),
        ];

        return Chart::new(datasets)
            .block(Block::default().title(format!(
                "Memory: {}/{}, SWAP: {}/{}",
                formatter(memory_info.used_memory),
                formatter(memory_info.total_memory),
                formatter(memory_info.used_swap),
                formatter(memory_info.total_swap)
            )))
            .style(Style::default().bg(Color::Black).fg(Color::White))
            .x_axis(
                Axis::default()
                    .title(Span::raw("Seconds Elapsed"))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .bounds([0.0, elapsed.as_secs_f64()])
                    .labels(
                        ["0".to_string(), (elapsed / 2).as_secs().to_string(), elapsed.as_secs().to_string()]
                            .iter()
                            .cloned()
                            .map(Span::from)
                            .collect(),
                    ),
            )
            .y_axis(
                Axis::default()
                    .title(Span::raw("Used Memory/SWAP"))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .bounds([0.0, max_y_axis_bound])
                    .labels([formatter(0), formatter(max_y_axis_label / 2), formatter(max_y_axis_label)].iter().cloned().map(Span::from).collect()),
            );
    }
    return Chart::new(vec![Dataset::default()]).block(Block::default().title("No memory/SWAP information was able to be obtained!"));
}

// MAYBE: This could be a list. I don't know if I like that better. You'd
// have to have quite a few disks to make it worth it. Currently this is a
// paragraph. If you have an idea (maybe something like a list with
// multiple lines per item) then feel free to experiment. That is what FOSS
// software is for
fn disk_tab(manager: &mut backend::Manager, scroll: u16) -> Paragraph {
    let formatter = humansize::make_format(humansize::DECIMAL);
    manager
        .disk_information()
        .map_or_else(
            || Paragraph::new("No information available!"),
            |disk_info| {
                let text = disk_info
                    .iter()
                    .flat_map(|disk| {
                        vec![
                            Line::from(Span::styled(disk.name.clone(), Style::default().add_modifier(Modifier::BOLD))),
                            Line::from(vec![Span::raw("Used Space: "), Span::raw(formatter(disk.used))]),
                            Line::from(vec![Span::raw("Total Space: "), Span::raw(formatter(disk.total))]),
                            Line::from(vec![Span::raw("Mount Point: "), Span::raw(disk.mount_point.clone())]),
                            Line::from(vec![Span::raw("Filesystem: "), Span::raw(disk.file_system.clone().unwrap_or_else(|| "unknown".to_string()))]),
                            Line::from(Span::raw("\n")),
                        ]
                    })
                    .collect::<Vec<Line>>();
                Paragraph::new(text).scroll((scroll, 0))
            },
        )
        .block(Block::default().title("Disks").borders(Borders::ALL))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
}

fn battery_tab(manager: &backend::Manager, scroll: u16) -> Paragraph {
    manager
        .battery_information()
        .map_or_else(
            || Paragraph::new("No battery information was able to be obtained!"),
            |battery_info| {
                let batteries = battery_info
                    .iter()
                    .flat_map(|battery| {
                        vec![
                            Line::from(Span::styled(
                                battery.model.clone().unwrap_or_else(|| "unknown".to_string()),
                                Style::default().add_modifier(Modifier::BOLD),
                            )),
                            Line::from(vec![Span::raw("Manufacturer: "), Span::raw(battery.manufacturer.clone().unwrap_or_else(|| "unknown".to_string()))]),
                            Line::from(vec![Span::raw("Charge: "), Span::raw((battery.charge * 100.0).floor().to_string()), Span::raw("%")]),
                            Line::from(vec![Span::raw("Status: "), Span::raw(battery.state.to_string())]),
                            Line::from(vec![Span::raw("Capacity: "), Span::raw(format!("{:.2}", battery.capacity_wh)), Span::raw("kWh")]),
                            Line::from(vec![Span::raw("Intended Capacity: "), Span::raw(format!("{:.2}", battery.capacity_new_wh)), Span::raw("kWh")]),
                            Line::from(vec![Span::raw("Health: "), Span::raw(format!("{:.2}", battery.health)), Span::raw("%")]),
                            Line::from(vec![Span::raw("Voltage: "), Span::raw(format!("{:.2}", battery.voltage)), Span::raw("V")]),
                            Line::from(vec![Span::raw("Technology: "), Span::raw(format!("{:.2}", battery.technology))]),
                            Line::from(vec![
                                Span::raw("Cycle Count: "),
                                Span::raw(battery.cycle_count.map_or_else(|| "unknown".to_string(), |cycle_count| cycle_count.to_string())),
                            ]),
                            Line::from(Span::raw("\n".repeat(3))),
                        ]
                    })
                    .collect::<Vec<Line>>();
                Paragraph::new(batteries).scroll((scroll, 0))
            },
        )
        .block(Block::default().title("Batteries").borders(Borders::ALL))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
}

// TODO: Make all "find max width" type statements
// into one per iterator

fn network_tab<'a>(more_info: bool, selected: u16) -> (Paragraph<'a>, List<'a>, List<'a>, Option<String>) {
    let formatter = humansize::make_format(humansize::DECIMAL);

    let popup_input_label = "Display more [i]nformation   ";
    let popup_input_width = popup_input_label.len();

    let mut selected_network: Option<backend::Network> = None;

    let mut res = if let Some(network_info) = (*NETWORK_INFO.lock().unwrap()).clone() {
        let text = vec![
            Line::from(vec![Span::raw("Connected to the internet: "), Span::raw(network_info.connected.to_string())]),
            Line::from(vec![
                Span::raw("IP Address (IPv4): "),
                Span::raw(network_info.ip_address_v4.map_or_else(|| "unknown".to_string(), |addr| addr.to_string())),
            ]),
            Line::from(vec![
                Span::raw("IP Address (IPv6): "),
                Span::raw(network_info.ip_address_v6.map_or_else(|| "unknown".to_string(), |addr| addr.to_string())),
            ]),
        ];

        let (wifis, wifi_title) = network_info.wifis.map_or_else(
            || (vec![ListItem::new("No WiFi information available!")], "WiFi networks".to_string()),
            |wifis| {
                let wifi_name_label = "Name";
                let wifi_mac_label = "MAC Address";
                let wifi_channel_label = "Channel";
                let wifi_security_label = "Security";
                let wifi_signal_label = "Signal Level";

                let mut wifi_name_width = wifi_name_label.len();
                let mut wifi_mac_width = wifi_mac_label.len();
                let mut wifi_channel_width = wifi_channel_label.len();
                let mut wifi_security_width = wifi_security_label.len();
                let mut wifi_signal_width = wifi_signal_label.len();

                for wifi in &wifis {
                    if wifi_name_width < wifi.ssid.len() {
                        wifi_name_width = wifi.ssid.len();
                    }
                    if wifi_mac_width < wifi.mac.len() {
                        wifi_mac_width = wifi.mac.len();
                    }
                    if wifi_channel_width < wifi.channel.len() {
                        wifi_channel_width = wifi.channel.len();
                    }
                    if wifi_security_width < wifi.security.len() {
                        wifi_security_width = wifi.security.len();
                    }
                    if wifi_signal_width < wifi.signal_level.len() {
                        wifi_signal_width = wifi.signal_level.len();
                    }
                }

                (
                    wifis
                        .iter()
                        .map(|wifi| {
                            ListItem::new(format!(
                                "{:wifi_name_width$}  {:wifi_mac_width$}  {:wifi_channel_width$}  {:wifi_security_width$}  {:wifi_signal_width$}",
                                wifi.ssid.clone(),
                                if wifi.mac.is_empty() { "unknown".to_string() } else { wifi.mac.clone() },
                                wifi.channel.clone(),
                                wifi.security.clone(),
                                wifi.signal_level.clone()
                            ))
                        })
                        .collect(),
                    format!(
                        "{wifi_name_label:wifi_name_width$}  {wifi_mac_label:wifi_mac_width$}  {wifi_channel_label:wifi_channel_width$}  {wifi_security_label:wifi_security_width$}  \
                         {wifi_signal_label:wifi_signal_width$}"
                    ),
                )
            },
        );

        let (networks, network_title) = network_info.networks.map_or_else(
            || (vec![ListItem::new("No network/interface information available!")], "Networks/Interfaces".to_string()),
            |networks| {
                let network_name_label = "Name";
                let network_index_label = "Index";
                let network_mac_label = "MAC Address";
                let network_flags_label = "Flags";

                let mut network_name_width = network_name_label.len();
                let mut network_index_width = network_index_label.len();
                let mut network_mac_width = network_mac_label.len();
                let mut network_flags_width = network_flags_label.len();

                for network in &networks {
                    if network_name_width < network.name.len() {
                        network_name_width = network.name.len();
                    }

                    let index_width_candidate = to_string_or_unknown(network.index).len();
                    if network_index_width < index_width_candidate {
                        network_index_width = index_width_candidate;
                    }

                    let mac_width_candidate = to_string_or_unknown(network.mac_address).len();
                    if network_mac_width < mac_width_candidate {
                        network_mac_width = mac_width_candidate;
                    }

                    let flags_width_candidate = format_or_unknown(network.flags, &|flags: backend::NetworkFlags| format!("{:b}", flags.raw)).len();
                    if network_flags_width < flags_width_candidate {
                        network_flags_width = flags_width_candidate;
                    }
                }
                (
                    networks
                        .iter()
                        .enumerate()
                        .map(|(index, network)| {
                            if more_info && index == selected as usize {
                                selected_network = Some(network.clone());
                            }
                            ListItem::new(format!(
                                "{:network_name_width$}  {:network_index_width$}  {:network_mac_width$}  {:network_flags_width$}",
                                network.name, /* TODO: Convert this to a more human readable format
                                               * on MacOS (and maybe others) */
                                to_string_or_unknown(network.index),
                                to_string_or_unknown(network.mac_address),
                                format_or_unknown(network.flags, &|flags: backend::NetworkFlags| format!("{:b}", flags.raw)),
                            ))
                        })
                        .collect(),
                    format!(
                        "{} {network_name_label:network_name_width$}  {network_index_label:network_index_width$}  {network_mac_label:network_mac_width$}  {network_flags_label:network_flags_width$}",
                        "─".repeat(popup_input_width)
                    ),
                )
            },
        );

        (
            Paragraph::new(text),
            List::new(wifis).block(Block::default().title(wifi_title).borders(Borders::ALL)),
            List::new(networks).block(Block::default().title(network_title).borders(Borders::ALL)),
            None,
        )
    } else {
        (
            Paragraph::new("Loading..."),
            List::new(vec![ListItem::new("Loading...")]).block(Block::default().title("WiFi Networks").borders(Borders::ALL)),
            List::new(vec![ListItem::new("Loading...")]).block(Block::default().title("Networks/Interfaces").borders(Borders::ALL)),
            None,
        )
    };
    res.0 = res
        .0
        .block(Block::default().title("Networks").borders(Borders::ALL))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });
    res.1 = res
        .1
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
    res.2 = res
        .2
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
        .highlight_symbol(popup_input_label);
    if more_info {
        if let Some(n) = selected_network {
            let flags_text = n.flags.map_or_else(
                || "Flags: unknown".to_string(),
                |flags| {
                    format!(
                        r"
Flags (Raw): {:b}
    Is up? {}
    Is broadcast? {}
    Is loopback interface? {}
    Is point-to-point interface? {}
    Is multicast interface? {}
                ",
                        flags.raw, flags.is_up, flags.is_broadcast, flags.is_loopback, flags.is_point_to_point, flags.is_multicast,
                    )
                },
            );

            res.3 = Some(format!(
                r"Name: {}
Description: {}
MAC-Address: {}
Index: {}
IP-addresses: 
{}
{}
Received: {}
Transmitted: {}
Packets received: {}
Packets transmitted: {}",
                n.name,
                to_string_or_unknown(n.description),
                to_string_or_unknown(n.mac_address),
                to_string_or_unknown(n.index),
                to_string_or_unknown(n.ips.map(|ips| ips.iter().map(ToString::to_string).join("\n"))),
                flags_text,
                format_or_unknown(n.received_total, &formatter),
                format_or_unknown(n.transmitted_total, &formatter),
                to_string_or_unknown(n.packets_received_total),
                to_string_or_unknown(n.packets_transmitted_total),
            ));
        } else {
            res.3 = Some("Select a network to display information about it!".to_string());
        }
    }
    res
}

// TODO: make a popup with more information
// TODO: implement process killing
fn process_tab(manager: &mut backend::Manager, ordering: SortByProcess, shift_pressed: bool, kill_current_process: bool, more_information: bool, current_line: u16) -> (List, Option<ProcessPopup>) {
    static LATEST_INFO: Mutex<(Option<Vec<backend::ProcessInfo>>, Option<Instant>)> = Mutex::new((None, None));
    let formatter = humansize::make_format(humansize::DECIMAL);
    let mut latest_info = LATEST_INFO.lock().unwrap();

    if latest_info.1.is_none() || latest_info.1.unwrap().elapsed() > INTERVAL {
        *latest_info = (manager.process_information(), Some(Instant::now()));
    }

    let mut selected_process: Option<&backend::ProcessInfo>;

    let mut res = if let Some(ref mut process_info) = &mut latest_info.0
        && !process_info.is_empty()
    {
        let selected_label = "Kill [k]   ";
        let name_label = "Name";
        let cpu_label = format!("CPU usage [{}]", if shift_pressed { 'C' } else { 'c' });
        let memory_label = format!("Memory usage [{}]", if shift_pressed { 'M' } else { 'm' });
        let swap_label = format!("SWAP usage [{}]", if shift_pressed { 'S' } else { 's' });
        let runtime_label = format!("Runtime [{}]", if shift_pressed { 'R' } else { 'r' });

        let selected_width = selected_label.len();

        let name_width = std::cmp::max(process_info.iter().map(|process| process.name.len()).max().unwrap(), name_label.len());

        let cpu_width = cpu_label.len();

        let memory_width = std::cmp::max(process_info.iter().map(|process| formatter(process.memory_usage).len()).max().unwrap(), memory_label.len());

        let swap_width = std::cmp::max(process_info.iter().map(|process| formatter(process.swap_usage).len()).max().unwrap(), swap_label.len());

        let runtime_width = std::cmp::max(process_info.iter().map(|process| format_duration(&process.run_time).len()).max().unwrap(), runtime_label.len());

        let sort_fn = |a: &backend::ProcessInfo, b: &backend::ProcessInfo| match ordering {
            SortByProcess::CpuUsage(ord) => ord.sort_by()(a.cpu_usage, b.cpu_usage),
            SortByProcess::MemoryUsage(ord) => ord.sort_by()(a.memory_usage, b.memory_usage),
            SortByProcess::SwapUsage(ord) => ord.sort_by()(a.swap_usage, b.swap_usage),
            SortByProcess::Runtime(ord) => ord.sort_by()(a.run_time, b.run_time),
        };

        process_info.sort_by(sort_fn);

        selected_process = process_info.get(current_line as usize);

        let items = process_info
            .iter()
            .enumerate()
            .map(|(index, process)| {
                if index == current_line as usize {
                    selected_process = Some(process);
                }
                ListItem::new(format!(
                    "{:name_width$}  {:cpu_width$.2}%  {:memory_width$}  {:swap_width$}  {:runtime_width$}",
                    process.name,
                    process.cpu_usage,
                    formatter(process.memory_usage),
                    formatter(process.swap_usage),
                    format_duration(&process.run_time)
                ))
            })
            .collect::<Vec<ListItem>>();
        (
            List::new(items)
                .block(
                    Block::default()
                        .title(format!(
                            "{:selected_width$}{:name_width$}  {:cpu_width$}   {:memory_width$}  {:swap_width$}  {:runtime_width$}",
                            "", name_label, cpu_label, memory_label, swap_label, runtime_label
                        ))
                        .borders(Borders::ALL),
                )
                .highlight_symbol(selected_label),
            if kill_current_process {
                Some(selected_process.map_or(ProcessPopup::NoSelected, |selected_process| ProcessPopup::KillProcess {
                    process_name: selected_process.name.clone(),
                    pid:          selected_process.pid,
                }))
            } else if more_information {
                Some(selected_process.map_or(ProcessPopup::NoSelected, |sp| ProcessPopup::MoreInformation {
                    contents: format!(
                        r"Name: {}
Path: {}
Memory Usage: {}
SWAP Usage: {}
CPU Usage: {}%
Runtime: {}
PID: {}
Parent: {}",
                        sp.name,
                        to_string_or_unknown(sp.path.clone()),
                        humansize::format_size(sp.memory_usage, humansize::DECIMAL),
                        humansize::format_size(sp.swap_usage, humansize::DECIMAL),
                        sp.cpu_usage,
                        format_duration(&sp.run_time),
                        sp.pid,
                        sp.parent.map_or_else(|| "No parent".to_string(), |parent| to_string_or_unknown(manager.get_process(parent).map(sysinfo::Process::name)))
                    ),
                }))
            } else {
                None
            },
        )
    } else {
        (
            List::new(vec![ListItem::new("No information available!")]).block(Block::default().title("Processes").borders(Borders::ALL)),
            None,
        )
    };

    drop(latest_info);

    res.0 = res
        .0
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
    res
}

fn component_tab(manager: &mut backend::Manager, ordering: SortByComponent, shift_pressed: bool) -> List {
    if let Some(mut component_info) = manager.component_information()
        && !component_info.is_empty()
    {
        let selected_label = ">";
        let name_label = "Name";
        let temperature_label = format!("Temperature [{}]", if shift_pressed { 'T' } else { 't' });
        let critical_label = format!("Critical Temperature [{}]", if shift_pressed { 'C' } else { 'c' });

        let selected_width = selected_label.len();
        let name_width = std::cmp::max(component_info.iter().map(|component| component.name.len()).max().unwrap(), name_label.len());
        let temperature_width = temperature_label.len(); // This is a bit of a gamble as it assumes that the label will always be
                                                         // longer than a temperature reading
        let critical_width = critical_label.len();

        let sort_fn = |a: &backend::ComponentInfo, b: &backend::ComponentInfo| match ordering {
            SortByComponent::Temperature(ord) => ord.sort_by()(a.temperature, b.temperature),
            SortByComponent::Critical(ord) => ord.sort_by()(a.critical_temperature.unwrap_or(0.0), b.critical_temperature.unwrap_or(0.0)),
        };
        component_info.sort_by(sort_fn);
        let items = component_info
            .iter()
            .map(|component| {
                ListItem::new(format!(
                    "{:name_width$}  {:temperature_width$.2}°C  {:critical_width$}",
                    component.name,
                    component.temperature,
                    component.critical_temperature.map_or_else(|| "None".to_string(), |critical_temp| format!("{critical_temp:.2}°C"))
                ))
            })
            .collect::<Vec<ListItem>>();
        List::new(items)
            .block(
                Block::default()
                    .title(format!(
                        "{:selected_width$}{:name_width$}  {:temperature_width$}    {:critical_width$}",
                        "", name_label, temperature_label, critical_label
                    ))
                    .borders(Borders::ALL),
            )
            .highlight_symbol(selected_label)
    } else {
        List::new(vec![ListItem::new("No information available!")])
    }
    .style(Style::default().fg(Color::White).bg(Color::Black))
    .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}
