use std::time::Instant;

use ratatui::{
    style::{Color, Style},
    symbols::Marker,
    text::Span,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
};

use crate::DataPoint;

pub fn memory_tab<'a>(
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
        let ram_important_digits = ram_important_digits.expect("ram digits should be set when memory info is available");
        let swap_important_digits = swap_important_digits.expect("swap digits should be set when memory info is available");

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
                    .labels(["0".to_string(), (elapsed / 2).as_secs().to_string(), elapsed.as_secs().to_string()]),
            )
            .y_axis(
                Axis::default()
                    .title(Span::raw("Used Memory/SWAP"))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .bounds([0.0, max_y_axis_bound])
                    .labels([formatter(0), formatter(max_y_axis_label / 2), formatter(max_y_axis_label)]),
            );
    }
    Chart::new(vec![Dataset::default()]).block(Block::default().title("No memory/SWAP information was able to be obtained!"))
}
