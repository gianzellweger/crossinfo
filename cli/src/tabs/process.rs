use std::{sync::Mutex, time::Instant};

use ratatui::{
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem},
};

use crate::{INTERVAL, ProcessPopup, SortByProcess, column_width, format_duration, to_string_or_unknown};

// TODO: make a popup with more information
// TODO: implement process killing
pub fn process_tab(
    manager: &mut backend::Manager,
    ordering: SortByProcess,
    shift_pressed: bool,
    kill_current_process: bool,
    more_information: bool,
    current_line: u16,
) -> (List<'_>, Option<ProcessPopup>) {
    static LATEST_INFO: Mutex<(Option<Vec<backend::ProcessInfo>>, Option<Instant>)> = Mutex::new((None, None));
    let formatter = humansize::make_format(humansize::DECIMAL);
    let mut latest_info = LATEST_INFO.lock().expect("process info mutex poisoned");

    if latest_info.1.is_none() || latest_info.1.expect("just checked is_none").elapsed() > INTERVAL {
        *latest_info = (manager.process_information(), Some(Instant::now()));
    }

    let mut selected_process: Option<&backend::ProcessInfo>;

    let mut res = if let Some(process_info) = &mut latest_info.0
        && !process_info.is_empty()
    {
        let selected_label = "Kill [k]   ";
        let name_label = "Name";
        let cpu_label = format!("CPU usage [{}]", if shift_pressed { 'C' } else { 'c' });
        let memory_label = format!("Memory usage [{}]", if shift_pressed { 'M' } else { 'm' });
        let swap_label = format!("SWAP usage [{}]", if shift_pressed { 'S' } else { 's' });
        let runtime_label = format!("Runtime [{}]", if shift_pressed { 'R' } else { 'r' });

        let selected_width = selected_label.len();
        let name_width = column_width(name_label, process_info.iter().map(|process| process.name.len()));
        let cpu_width = cpu_label.len();
        let memory_width = column_width(&memory_label, process_info.iter().map(|process| formatter(process.memory_usage).len()));
        let swap_width = column_width(&swap_label, process_info.iter().map(|process| formatter(process.swap_usage).len()));
        let runtime_width = column_width(&runtime_label, process_info.iter().map(|process| format_duration(&process.run_time).len()));

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
                        sp.parent.map_or_else(
                            || "No parent".to_string(),
                            |parent| to_string_or_unknown(manager.get_process(parent).map(|p| p.name().to_string_lossy().into_owned()))
                        )
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
