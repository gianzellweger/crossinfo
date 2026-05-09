use itertools::Itertools;
use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::{NETWORK_INFO, column_width, format_or_unknown, to_string_or_unknown, yes_no};

pub fn network_tab<'a>(more_info: bool, selected: u16) -> (Paragraph<'a>, List<'a>, List<'a>, Option<String>) {
    let formatter = humansize::make_format(humansize::DECIMAL);

    let popup_input_label = "Display more [i]nformation   ";
    let popup_input_width = popup_input_label.len();

    let mut selected_network: Option<backend::Network> = None;

    let net_info_maybe = (*NETWORK_INFO.lock().expect("network info mutex poisoned")).clone();
    let mut res = if let Some(network_info) = net_info_maybe {
        let text = vec![
            Line::from(vec![Span::raw("Connected to the internet: "), Span::raw(yes_no(network_info.connected))]),
            Line::from(vec![
                Span::raw("IP Address (IPv4): "),
                Span::raw(to_string_or_unknown(network_info.ip_address_v4)),
            ]),
            Line::from(vec![
                Span::raw("IP Address (IPv6): "),
                Span::raw(to_string_or_unknown(network_info.ip_address_v6)),
            ]),
        ];

        let (wifis, wifi_title) = network_info.wifis.map_or_else(
            || (vec![ListItem::new("No WiFi information available!")], "WiFi networks".to_string()),
            |wifis| {
                let wifi_name_label = "Name";
                let wifi_mac_label = "MAC Address";
                let wifi_channel_label = "Channel";
                let wifi_security_label = "Security";
                let wifi_signal_label = "Signal Level";

                let wifi_name_width = column_width(wifi_name_label, wifis.iter().map(|w| w.ssid.len()));
                let wifi_mac_width = column_width(wifi_mac_label, wifis.iter().map(|w| w.mac.len()));
                let wifi_channel_width = column_width(wifi_channel_label, wifis.iter().map(|w| w.channel.len()));
                let wifi_security_width = column_width(wifi_security_label, wifis.iter().map(|w| w.security.len()));
                let wifi_signal_width = column_width(wifi_signal_label, wifis.iter().map(|w| w.signal_level.len()));

                (
                    wifis
                        .iter()
                        .map(|wifi| {
                            ListItem::new(format!(
                                "{:wifi_name_width$}  {:wifi_mac_width$}  {:wifi_channel_width$}  {:wifi_security_width$}  {:wifi_signal_width$}",
                                wifi.ssid.clone(),
                                if wifi.mac.is_empty() { "unknown".to_string() } else { wifi.mac.clone() },
                                wifi.channel.clone(),
                                wifi.security.clone(),
                                wifi.signal_level.clone()
                            ))
                        })
                        .collect(),
                    format!(
                        "{wifi_name_label:wifi_name_width$}  {wifi_mac_label:wifi_mac_width$}  {wifi_channel_label:wifi_channel_width$}  {wifi_security_label:wifi_security_width$}  \
                         {wifi_signal_label:wifi_signal_width$}"
                    ),
                )
            },
        );

        let (networks, network_title) = network_info.networks.map_or_else(
            || (vec![ListItem::new("No network/interface information available!")], "Networks/Interfaces".to_string()),
            |networks| {
                let network_name_label = "Name";
                let network_index_label = "Index";
                let network_mac_label = "MAC Address";
                let network_flags_label = "Flags";

                let network_name_width = column_width(network_name_label, networks.iter().map(|n| n.name.len()));
                let network_index_width = column_width(network_index_label, networks.iter().map(|n| to_string_or_unknown(n.index).len()));
                let network_mac_width = column_width(network_mac_label, networks.iter().map(|n| to_string_or_unknown(n.mac_address).len()));
                let network_flags_width = column_width(network_flags_label, networks.iter().map(|n| format_or_unknown(n.flags, &|flags: backend::NetworkFlags| format!("{:b}", flags.raw)).len()));
                (
                    networks
                        .iter()
                        .enumerate()
                        .map(|(index, network)| {
                            if more_info && index == selected as usize {
                                selected_network = Some(network.clone());
                            }
                            ListItem::new(format!(
                                "{:network_name_width$}  {:network_index_width$}  {:network_mac_width$}  {:network_flags_width$}",
                                network.name, /* TODO: Convert this to a more human readable format
                                               * on MacOS (and maybe others) */
                                to_string_or_unknown(network.index),
                                to_string_or_unknown(network.mac_address),
                                format_or_unknown(network.flags, &|flags: backend::NetworkFlags| format!("{:b}", flags.raw)),
                            ))
                        })
                        .collect(),
                    format!(
                        "{} {network_name_label:network_name_width$}  {network_index_label:network_index_width$}  {network_mac_label:network_mac_width$}  {network_flags_label:network_flags_width$}",
                        "─".repeat(popup_input_width)
                    ),
                )
            },
        );

        (
            Paragraph::new(text),
            List::new(wifis).block(Block::default().title(wifi_title).borders(Borders::ALL)),
            List::new(networks).block(Block::default().title(network_title).borders(Borders::ALL)),
            None,
        )
    } else {
        (
            Paragraph::new("Loading..."),
            List::new(vec![ListItem::new("Loading...")]).block(Block::default().title("WiFi Networks").borders(Borders::ALL)),
            List::new(vec![ListItem::new("Loading...")]).block(Block::default().title("Networks/Interfaces").borders(Borders::ALL)),
            None,
        )
    };
    res.0 = res
        .0
        .block(Block::default().title("Networks").borders(Borders::ALL))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });
    res.1 = res
        .1
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));
    res.2 = res
        .2
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
        .highlight_symbol(popup_input_label);
    if more_info {
        if let Some(n) = selected_network {
            let flags_text = n.flags.map_or_else(
                || "Flags: unknown".to_string(),
                |flags| {
                    format!(
                        r"
Flags (Raw): {:b}
    Is up? {}
    Is broadcast? {}
    Is loopback interface? {}
    Is point-to-point interface? {}
    Is multicast interface? {}
                ",
                        flags.raw,
                        yes_no(flags.is_up),
                        yes_no(flags.is_broadcast),
                        yes_no(flags.is_loopback),
                        yes_no(flags.is_point_to_point),
                        yes_no(flags.is_multicast),
                    )
                },
            );

            res.3 = Some(format!(
                r"Name: {}
Description: {}
MAC-Address: {}
Index: {}
IP-addresses:
{}
{}
Received: {}
Transmitted: {}
Packets received: {}
Packets transmitted: {}",
                n.name,
                to_string_or_unknown(n.description),
                to_string_or_unknown(n.mac_address),
                to_string_or_unknown(n.index),
                to_string_or_unknown(n.ips.map(|ips| ips.iter().map(ToString::to_string).join("\n"))),
                flags_text,
                format_or_unknown(n.received_total, &formatter),
                format_or_unknown(n.transmitted_total, &formatter),
                to_string_or_unknown(n.packets_received_total),
                to_string_or_unknown(n.packets_transmitted_total),
            ));
        } else {
            res.3 = Some("Select a network to display information about it!".to_string());
        }
    }
    res
}
