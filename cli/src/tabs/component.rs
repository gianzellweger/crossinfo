use std::{sync::Mutex, time::Instant};

use ratatui::{
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem},
};

use crate::{INTERVAL, SortByComponent, column_width};

pub fn component_tab(manager: &mut backend::Manager, ordering: SortByComponent, shift_pressed: bool) -> (List<'_>, u16) {
    static LATEST_INFO: Mutex<(Option<Vec<backend::ComponentInfo>>, Option<Instant>)> = Mutex::new((None, None));

    let mut latest_info = LATEST_INFO.lock().expect("component info mutex poisoned");

    if latest_info.1.is_none() || latest_info.1.expect("just checked is_none").elapsed() > INTERVAL {
        *latest_info = (manager.component_information(), Some(Instant::now()));
    }

    #[allow(clippy::cast_possible_truncation)]
    let item_count = latest_info.0.as_ref().map_or(0u16, |v| v.len() as u16);

    let mut res = if let Some(component_info) = &mut latest_info.0
        && !component_info.is_empty()
    {
        let selected_label = ">";
        let name_label = "Name";
        let temperature_label = format!("Temperature [{}]", if shift_pressed { 'T' } else { 't' });
        let critical_label = format!("Critical Temperature [{}]", if shift_pressed { 'C' } else { 'c' });

        let selected_width = selected_label.len();
        let name_width = column_width(name_label, component_info.iter().map(|component| component.name.len()));
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
        List::new(vec![ListItem::new("No information available!")]).block(Block::default().title("Components").borders(Borders::ALL))
    };

    drop(latest_info);

    res = res
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
    (res, item_count)
}
