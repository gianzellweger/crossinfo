use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{format_duration, to_string_or_unknown};

pub fn system_tab(manager: &mut backend::Manager, scroll: u16) -> (Paragraph<'_>, u16) {
    #[allow(clippy::cast_possible_truncation)]
    let (paragraph, line_count) = if let Some(system_info) = manager.system_information() {
        let count = (5 + system_info.users.len()) as u16;
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
        (Paragraph::new(text).scroll((scroll, 0)), count)
    } else {
        (Paragraph::new("No information available!"), 1)
    };
    (
        paragraph
            .block(Block::default().title("System").borders(Borders::ALL))
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        line_count,
    )
}
