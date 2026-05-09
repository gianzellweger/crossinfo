use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{format_duration, to_string_or_unknown};

pub fn system_tab(manager: &mut backend::Manager, scroll: u16) -> Paragraph<'_> {
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
