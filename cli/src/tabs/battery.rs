use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::to_string_or_unknown;

pub fn battery_tab(manager: &backend::Manager, scroll: u16) -> (Paragraph<'_>, u16) {
    #[allow(clippy::cast_possible_truncation)]
    let (paragraph, line_count) = manager.battery_information().map_or_else(
        || (Paragraph::new("No battery information was able to be obtained!"), 1),
        |battery_info| {
            let count = (battery_info.len() * 11) as u16;
            let batteries = battery_info
                .iter()
                .flat_map(|battery| {
                    vec![
                        Line::from(Span::styled(to_string_or_unknown(battery.model.clone()), Style::default().add_modifier(Modifier::BOLD))),
                        Line::from(vec![Span::raw("Manufacturer: "), Span::raw(to_string_or_unknown(battery.manufacturer.clone()))]),
                        Line::from(vec![Span::raw("Charge: "), Span::raw((battery.charge * 100.0).floor().to_string()), Span::raw("%")]),
                        Line::from(vec![Span::raw("Status: "), Span::raw(battery.state.to_string())]),
                        Line::from(vec![Span::raw("Capacity: "), Span::raw(format!("{:.2}", battery.capacity_wh)), Span::raw("kWh")]),
                        Line::from(vec![Span::raw("Intended Capacity: "), Span::raw(format!("{:.2}", battery.capacity_new_wh)), Span::raw("kWh")]),
                        Line::from(vec![Span::raw("Health: "), Span::raw(format!("{:.2}", battery.health)), Span::raw("%")]),
                        Line::from(vec![Span::raw("Voltage: "), Span::raw(format!("{:.2}", battery.voltage)), Span::raw("V")]),
                        Line::from(vec![Span::raw("Technology: "), Span::raw(format!("{:.2}", battery.technology))]),
                        Line::from(vec![Span::raw("Cycle Count: "), Span::raw(to_string_or_unknown(battery.cycle_count))]),
                        Line::from(Span::raw("\n".repeat(3))),
                    ]
                })
                .collect::<Vec<Line>>();
            (Paragraph::new(batteries).scroll((scroll, 0)), count)
        },
    );
    (
        paragraph
            .block(Block::default().title("Batteries").borders(Borders::ALL))
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        line_count,
    )
}
