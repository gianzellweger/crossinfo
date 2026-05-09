use std::{sync::Mutex, time::Instant};

use ratatui::{
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem},
};

use crate::{INTERVAL, column_width, to_string_or_unknown, yes_no};

// TODO: make a popup with more information
pub fn bluetooth_tab(manager: &backend::Manager) -> (List<'_>, u16) {
    static LATEST_INFO: Mutex<(Option<Vec<backend::BluetoothInfo>>, Option<Instant>)> = Mutex::new((None, None));
    let mut latest_info = LATEST_INFO.lock().expect("bluetooth info mutex poisoned");

    if latest_info.1.is_none() || latest_info.1.expect("just checked is_none").elapsed() > INTERVAL {
        *latest_info = (manager.bluetooth_information(), Some(Instant::now()));
    }

    #[allow(clippy::cast_possible_truncation)]
    let item_count = latest_info.0.as_ref().map_or(0u16, |v| v.len() as u16);

    let mut res = if let Some(bluetooth_info) = &mut latest_info.0
        && !bluetooth_info.is_empty()
    {
        let name_label = "Local Name";
        let id_label = "ID";
        let address_label = "Address";
        let transmission_label = "Transmission power";
        let signal_label = "Signal Strength";
        let connected_label = "Connected";

        let name_width = column_width(name_label, bluetooth_info.iter().map(|device| to_string_or_unknown(device.local_name.clone()).len()));
        let id_width = column_width(id_label, bluetooth_info.iter().map(|device| device.id.clone().len()));
        let address_width = column_width(address_label, bluetooth_info.iter().map(|device| device.address.to_string().len()));
        let transmission_width = column_width(transmission_label, bluetooth_info.iter().map(|device| to_string_or_unknown(device.transmission_power_level).len()));
        let signal_width = column_width(signal_label, bluetooth_info.iter().map(|device| to_string_or_unknown(device.signal_strength).len()));
        let connected_width = connected_label.len();

        bluetooth_info.sort_by(|a, b| {
            let a_signal = a.signal_strength.unwrap_or(i16::MIN);
            let b_signal = b.signal_strength.unwrap_or(i16::MIN);
            b_signal.cmp(&a_signal)
        });

        let items = bluetooth_info
            .iter()
            .map(|device| {
                ListItem::new(format!(
                    "{:name_width$}  {:id_width$}  {:address_width$}  {:transmission_width$}  {:signal_width$}  {:connected_width$}",
                    to_string_or_unknown(device.local_name.clone()),
                    device.id,
                    device.address,
                    to_string_or_unknown(device.transmission_power_level),
                    to_string_or_unknown(device.signal_strength),
                    yes_no(device.is_connected),
                ))
            })
            .collect::<Vec<ListItem>>();
        List::new(items).block(
            Block::default()
                .title(format!(
                    "{name_label:name_width$}  {id_label:id_width$}  {address_label:address_width$}  {transmission_label:transmission_width$}  {signal_label:signal_width$}  \
                     {connected_label:connected_width$}",
                ))
                .borders(Borders::ALL),
        )
    } else {
        List::new(vec![ListItem::new("No information available!")]).block(Block::default().title("Bluetooth").borders(Borders::ALL))
    };

    drop(latest_info);

    res = res
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
    (res, item_count)
}
