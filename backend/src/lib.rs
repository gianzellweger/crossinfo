#![allow(unused_imports)]
use sysinfo::{
    ComponentExt, CpuExt, DiskExt, NetworkExt, NetworksExt, PidExt, ProcessExt, System, SystemExt,
    UserExt,
};

use dioxus::events::{KeyCode, KeyboardEvent};
use dioxus::prelude::*;

/// Returns the structure of the main app. This is a Dioxus component.
pub fn crossinfo_app_structure<'a>(cx: Scope) -> Element {
    if !System::IS_SUPPORTED {
        return cx.render(rsx! {
            p {
                "OS not supported"
            }
        });
    }

    let system = use_ref(cx, || System::new_all());
    let cpu_expanded = use_state(cx, || false);
    let sys_expanded = use_state(cx, || false);
    let mem_expanded = use_state(cx, || false);
    let dsk_expanded = use_state(cx, || false);
    let net_expanded = use_state(cx, || false);
    let temps_expanded = use_state(cx, || false); // Naming convention break :(

    // cx.spawn({
    //     async move {
    //         loop {
    //             system.write().refresh_all();
    //             std::thread::sleep(std::time::Duration::from_millis(500));
    //         }
    //     }
    // });

    cx.render(rsx! {
        link { 
            rel: "preconnect", href: "https://fonts.googleapis.com", 
        },
        link { 
            rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "true",
        },
        link { 
            href: "https://fonts.googleapis.com/css2?family=Tilt+Prism&display=swap", rel: "stylesheet",
        },
        style {
            "html, body {{
                margin: 0;
                padding: 0;
            }}"
        },
        nav {
            display: "flex",
            position: "fixed",
            box_sizing: "border-box",
            width: "100%",
            top: "0px",
            right: "0px",
            z_index: "69",
            justify_content: "space-around",
            align_items: "center",
            background_color: "navy",
            height: "10vh",
            h2 {
                color: "white",
                font_family: "'Tilt Prism', cursive",
                font_size: "5vh",
                "Crossinfo"
            },
            button {
                onclick: move |_| {
                    system.write().refresh_all();
                },
                "Refresh"
            }
        },
        main {
            div {
                margin_top: "10vh",
                crossinfo_dropdown {
                    title: "System information",
                    expanded: sys_expanded,
                },
            },
            if **sys_expanded {
                rsx! {
                    crossinfo_section { 
                        left: cx.render(rsx! {
                            span { "System name" }
                        }), 
                        right: cx.render(rsx! {
                            span { r#"{system.read().name().unwrap_or_else(|| "System name unknown".to_string())}"# }
                        }),
                    },
                    crossinfo_section { 
                        left: cx.render(rsx! {
                            span { "Operating System version" }
                        }), 
                        right: cx.render(rsx! {
                            span { r#"{system.read().os_version().unwrap_or_else(|| "OS version unknown".to_string())}"# }
                        }),
                    },
                    crossinfo_section { 
                        left: cx.render(rsx! {
                            span { "Kernel version" }
                        }), 
                        right: cx.render(rsx! {
                            span { r#"{system.read().kernel_version().unwrap_or_else(|| "Kernel version unknown".to_string())}"# }
                        }),
                    },
                    crossinfo_section { 
                        left: cx.render(rsx! {
                            span { "System host name" }
                        }), 
                        right: cx.render(rsx! {
                            span { r#"{system.read().host_name().unwrap_or_else(|| "Host name unknown".to_string())}"# }
                        }),
                    },
                }
            },
            crossinfo_dropdown {
                title: "CPU Usage",
                expanded: cpu_expanded,
            },
            for (index, cpu_core) in system.read().cpus().iter().enumerate() {
                if **cpu_expanded {
                    rsx! {
                        crossinfo_section {
                            left: cx.render(rsx! {
                                span { "CPU #{index} ({cpu_core.brand()} {cpu_core.name()})" }
                            }),
                            right: cx.render(rsx! {
                                crossinfo_loading_bar {
                                    progress: cpu_core.cpu_usage() as f64,
                                    max: 100.0,
                                    unit: "%",
                                }
                            })
                        }
                    }
                }
            },
            crossinfo_dropdown {
                title: "Memory information",
                expanded: mem_expanded
            },
            if **mem_expanded {
                rsx! {
                    crossinfo_section {
                        left: cx.render(rsx! {
                            span { "Memory usage" }
                        }),
                        right: cx.render(rsx! {
                            crossinfo_loading_bar {
                                progress: system.read().used_memory() as f64 / 1_000_000_000.0,
                                max: system.read().total_memory() as f64 / 1_000_000_000.0,
                                unit: "GB",
                            }
                        })
                    },
                    crossinfo_section {
                        left: cx.render(rsx! {
                            span { "SWAP usage" }
                        }),
                        right: cx.render(rsx! {
                            crossinfo_loading_bar {
                                progress: system.read().used_swap() as f64 / 1_000_000_000.0,
                                max: system.read().total_swap() as f64 / 1_000_000_000.0,
                                unit: "GB",
                            }
                        })
                    }
                }
            },
            crossinfo_dropdown {
                title: "Disk information",
                expanded: dsk_expanded
            },
            for disk in system.read().disks() {
                if **dsk_expanded {
                    rsx! {
                        crossinfo_section {
                            left: cx.render(rsx! {
                                // TODO: remove quotes
                                span { "{disk.name():?} at {disk.mount_point().as_os_str():?} ({disk.type_():?})" }
                            }),
                            right: cx.render(rsx! {
                                crossinfo_loading_bar {
                                    progress: (disk.total_space() - disk.available_space()) as f64 / 1_000_000_000.0,
                                    max: disk.total_space() as f64 / 1_000_000_000.0,
                                    unit: "GB"
                                }
                            })
                        }
                    }
                }
            },
            crossinfo_dropdown {
                title: "Component temperatures",
                expanded: temps_expanded,
            },
            for component in system.read().components() {
                if **temps_expanded {
                    rsx! {
                        crossinfo_section {
                            left: cx.render(rsx! {
                                span { component.label() },
                            }),
                            right: cx.render(rsx! {
                                crossinfo_loading_bar {
                                    progress: component.temperature() as f64,
                                    max: match component.critical() {
                                        Some(critical) => critical as f64,
                                        None => 100.0
                                    },
                                    unit: "°C"
                                }
                            })
                        }
                    }
                }
            },
            crossinfo_dropdown {
                title: "Network information",
                expanded: net_expanded,
            },
            for (network_name, data) in system.read().networks() {
                if **net_expanded {
                    rsx! {
                        crossinfo_section {
                            left: cx.render(rsx! {
                               span { "{network_name} data received (Download)" },
                            }),
                            right: cx.render(rsx! {
                                crossinfo_loading_bar {
                                    progress: data.received() as f64 / 1_000_000.0,
                                    max: 10f64.powf((data.received() as f64).log10().ceil()) / 1_000_000.0, // This makes little sense but ¯\_(ツ)_/¯
                                    unit: "MB"
                                }
                            }),
                        },
                        crossinfo_section {
                            left: cx.render(rsx! {
                               span { "{network_name} data transmitted (Upload)" },
                            }),
                            right: cx.render(rsx! {
                                crossinfo_loading_bar {
                                    progress: data.transmitted() as f64 / 1_000_000.0,
                                    max: 10f64.powf((data.transmitted() as f64).log10().ceil()) / 1_000_000.0, // This makes little sense but ¯\_(ツ)_/¯
                                    unit: "MB"
                                }
                            })
                        },
                    }
                }
            }
        }
    })
}

// TODO: Make tabs (with an Enum): 
// - Basic information (System, RAM, CPU usage)
// - Processes
// - Graphs
// - Maybe network? (Network seems to be really esoteric, especially to the normal user)

/// The struct used to pass elements [crossinfo_section]
#[derive(Props)]
struct CrossinfoSectionElems<'a> {
    left: Element<'a>,
    right: Option<Element<'a>>,
}

/// The main component of the app. Has a right and a left element
fn crossinfo_section<'a>(cx: Scope<'a, CrossinfoSectionElems<'a>>) -> Element {
    cx.render(rsx! {
        section {
            display: "flex",
            align_items: "center",
            justify_content: "space-between",
            height: "75px",
            border_top: "1px solid black",
            border_bottom: "1px solid black",
            margin_bottom: "-1px",
            div {
                margin: "0px 10px",
                &cx.props.left,
            }
            if let Some(right) = &cx.props.right {
                rsx! { 
                    div {
                        margin: "0px 10px",
                        right
                    }
                }
            }
        }
    })
}

/// Used to pass the title to a [crossinfo_dropdown]
#[derive(Props)]
struct CrossinfoDropdown<'a> {
    title: &'static str,
    expanded: &'a UseState<bool>,
}

/// Used to toggle some similar sections
fn crossinfo_dropdown<'a>(cx: Scope<'a, CrossinfoDropdown<'a>>) -> Element {
    cx.render(rsx! {
        button {
            outline: "none",
            width: "100%",
            onclick: move |_| cx.props.expanded.modify(|v| !v),
            margin: "0",
            padding: "0",
            background_color: "white",
            section {
                display: "flex",
                align_items: "center",
                justify_content: "flex-start",
                height: "75px",
                border_top: "1px solid black",
                border_bottom: "1px solid black",
                margin_bottom: "-1px",
                if **cx.props.expanded { rsx! {
                    img {
                        margin_left: "5px",
                        src: "../../Arrow.png",
                        height: "25px",
                        width: "25px",
                        transform: "rotate(90deg)",
                    }
                }},
                if !cx.props.expanded { rsx! {
                    img {
                        margin_left: "5px",
                        src: "../../Arrow.png",
                        height: "25px",
                        width: "25px",
                    }
                }}
                b {
                    cx.props.title
                }
            }
        }
    })
}

/// The struct used to pass data to [crossinfo_loading_bar]
#[derive(PartialEq, Props)]
struct LoadingBarData {
    progress: f64,
    max: f64,
    unit: &'static str,
}

/// This is a Dioxus component with a similar appearance to a loading bar, hence the name. It changes color based on what percentage `progress / max` is.
fn crossinfo_loading_bar(cx: Scope<LoadingBarData>) -> Element {
    let progress_color = match cx.props.progress / cx.props.max {
        x if x < 0.6  => "limegreen",
        x if x < 0.8  => "gold",
        x if x < 0.95 => "darkorange",
        _             => "crimson"
    };
    cx.render(rsx! {
        div {
            display: "flex",
            align_items: "center",
            div {
                margin: "0 10px",
                svg {
                    width: "200px",
                    height: "50px",
                    view_box: "0 0 200 50",
                    rect {
                        x: "0",
                        y: "0",
                        width: "200",
                        height: "50",
                        style: "fill: lightgray; stroke-width: 3; stroke: black;",
                    },
                    rect {
                        x: "1.5",
                        y: "1.5",
                        width: "{cx.props.progress / cx.props.max * 197.0}",
                        height: "47",
                        style: "fill: {progress_color}"
                    },
                }
            }
            span {
                width: "100px",
                "{cx.props.progress:.2}{cx.props.unit} / {cx.props.max:.2}{cx.props.unit}"
            }
        }
    })
}


