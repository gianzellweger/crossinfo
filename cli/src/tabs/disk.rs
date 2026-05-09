use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::to_string_or_unknown;

// MAYBE: This could be a list. I don't know if I like that better. You'd
// have to have quite a few disks to make it worth it. Currently this is a
// paragraph. If you have an idea (maybe something like a list with
// multiple lines per item) then feel free to experiment. That is what FOSS
// software is for
pub fn disk_tab(manager: &mut backend::Manager, scroll: u16) -> Paragraph<'_> {
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
                            Line::from(vec![Span::raw("Filesystem: "), Span::raw(to_string_or_unknown(disk.file_system.clone()))]),
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
