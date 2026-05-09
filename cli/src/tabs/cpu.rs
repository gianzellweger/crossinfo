use std::{collections::HashMap, sync::Mutex, time::Instant};

use itertools::Itertools;
use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    symbols::Marker,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, LegendPosition, List, ListItem},
};

use crate::{DataPoint, INTERVAL, column_width};

pub const COLORS: [Color; 15] = [
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
pub fn cpu_tab<'a>(manager: &'a mut backend::Manager, starting_time: Instant, cpu_dataset: &HashMap<&'a backend::CpuInfo, &'a [DataPoint]>) -> (Vec<(List<'a>, Chart<'a>)>, u16) {
    static LATEST_INFO: Mutex<(Option<Vec<backend::CpuInfo>>, Option<Instant>)> = Mutex::new((None, None));

    let mut latest_info = LATEST_INFO.lock().expect("cpu info mutex poisoned");

    if latest_info.1.is_none() || latest_info.1.expect("just checked is_none").elapsed() > INTERVAL {
        *latest_info = (manager.cpu_information(), Some(Instant::now()));
    }

    let elapsed = starting_time.elapsed();

    #[allow(clippy::cast_possible_truncation)]
    let max_items = latest_info.0.as_ref().map_or(0u16, |cpu_info| {
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for core in cpu_info {
            *counts.entry(core.manufacturer.as_str()).or_insert(0) += 1;
        }
        counts.values().copied().max().unwrap_or(0) as u16
    });

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
                            let usage_width = column_width(usage_label, cpu.iter().map(|c| format!("{:.2}", c.usage).len()));
                            let model_width = column_width(model_label, cpu.iter().map(|c| c.model.len()));
                            let manufacturer_width = column_width(manufacturer_label, cpu.iter().map(|c| c.manufacturer.len()));
                            let frequency_width = column_width(frequency_label, cpu.iter().map(|c| format!("{:.2}", c.frequency.get::<uom::si::frequency::gigahertz>()).len()));
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
            .legend_position(Some(LegendPosition::TopRight))
            .hidden_legend_constraints((Constraint::Min(0), Constraint::Min(0)))
            .x_axis(
                Axis::default()
                    .title(Span::raw("Seconds Elapsed"))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .bounds([0.0, elapsed.as_secs_f64()])
                    .labels(["0".to_string(), (elapsed / 2).as_secs().to_string(), elapsed.as_secs().to_string()]),
            )
            .y_axis(
                Axis::default()
                    .title(Span::raw("CPU usage"))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .bounds([0.0, 100.0])
                    .labels(["0%", "50%", "100%"]),
            );
    }
    (res, max_items)
}
