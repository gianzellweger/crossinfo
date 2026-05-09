use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::yes_no;

// MAYBE: This could be a list. I don't know if I like that better. You'd
// have to have quite a few disks to make it worth it. Currently this is a
// paragraph. If you have an idea (maybe something like a list with
// multiple lines per item) then feel free to experiment. That is what FOSS
// software is for
pub fn display_tab(manager: &backend::Manager, scroll: u16) -> (Paragraph<'_>, u16) {
    #[allow(clippy::cast_possible_truncation)]
    let (paragraph, line_count) = manager.display_information().map_or_else(
        || (Paragraph::new("No information available!"), 1),
        |display_info| {
            let count = (display_info.len() * 6) as u16;
            let text = display_info
                .iter()
                .flat_map(|display| {
                    vec![
                        Line::from(Span::styled(format!("Display #{}", display.id), Style::default().add_modifier(Modifier::BOLD))),
                        Line::from(vec![
                            Span::raw("Display size: "),
                            Span::raw(display.size.width.to_string()),
                            Span::raw("x"),
                            Span::raw(display.size.height.to_string()),
                        ]),
                        Line::from(vec![Span::raw("Scale factor: "), Span::raw(display.scale_factor.to_string())]),
                        Line::from(vec![Span::raw("Rotation: "), Span::raw(display.rotation.to_string()), Span::raw("°")]),
                        Line::from(vec![Span::raw("Primary monitor: "), Span::raw(yes_no(display.is_primary))]),
                        Line::from(Span::raw("\n")),
                    ]
                })
                .collect::<Vec<Line>>();
            (Paragraph::new(text).scroll((scroll, 0)), count)
        },
    );
    (
        paragraph
            .block(Block::default().title("Displays").borders(Borders::ALL))
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        line_count,
    )
}
