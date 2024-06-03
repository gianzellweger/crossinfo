#![forbid(unsafe_code)]
#![feature(let_chains)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![forbid(clippy::enum_glob_use)]
#![forbid(clippy::unwrap_used)]
#![allow(clippy::doc_markdown)]

/*
Frontend checklist: These things should be in any crossinfo-frontend

- All info that can be obtained from calling the functions under the Manager struct and makes sense for the platform
- Handling of Option types
- Ability to quit processes if displayed
- Nice display of data (i.e. don't just display the bytes as bytes (use humansize instead), don't just list different CPU cores as different CPUs, display as much graphically as you can)
- Refresh things like uptime and usage automatically
- Handle things like multiple CPUs and multiple batteries or no battery
- Manager::network_information can be very slow; It is recommended the value is stored in a static variable (Mutex) which is then refresh on a separate thread
*/

use std::{
    hash::Hash,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use battery::units::{electric_potential::volt, energy::watt_hour};
use btleplug::api::{Central as _, Manager as _, Peripheral as _};
pub use strum::{EnumCount, IntoEnumIterator};
pub use strum_macros::{EnumCount as EnumCountMacro, EnumIter};
use sysinfo::{Components, Disks, Networks, System, Users};
use uom::si::{f64::Frequency, frequency::megahertz};

#[derive(EnumIter, EnumCountMacro, Debug, Copy, Clone)]
pub enum Tab {
    /// OS information, Users, Kernel version,
    /// etc.
    System,
    /// CPU usage, model, manufacturer, specs
    Cpu,
    /// RAM amount, usage, model, SWAP (specs
    /// maybe?)
    Memory,
    /// Disk amount, usage (specs maybe? disk
    /// speed benchmark maybe?)
    Disk,
    // One day, there doesn't seem to be a good crate or unified method to get GPU info like usage
    // and model
    // Gpu,
    /// Installed battery/batteries info like
    /// charge, capacity, cycles, state
    /// (charching, etc.), health
    Battery,
    /// Speedtest using reqwest and speedtest.net
    /// api, Network usage, available WiFi
    /// connections (LAN detection maybe?)
    Network,
    /// CPU/RAM/SWAP/Disk usage, killing the
    /// process, extra nerd info like PID, exe
    /// path, etc
    Processes,
    /// Name, temperature, sometimes critical
    /// temperatures
    Components,
    /// ID, display resolution, rotation and scale factor
    Display,
    /// ID-String, address, name, transmission strength, signal strength,
    /// connection status
    Bluetooth,
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::System => "System",
            Self::Cpu => "CPU",
            Self::Memory => "Memory/SWAP",
            Self::Disk => "Disks",
            Self::Battery => "Battery",
            Self::Network => "Networks",
            Self::Processes => "Processes",
            Self::Components => "Components",
            Self::Display => "Display",
            Self::Bluetooth => "Bluetooth",
        })
    }
}

// constants to indicate if there is support for
// the crates used for the information
// TODO: figure out cross compilation
const SYSINFO_SUPPORT: bool = sysinfo::IS_SUPPORTED_SYSTEM;
static BATTERY_SUPPORT: AtomicBool = AtomicBool::new(false);

#[cfg(any(windows, unix))]
fn populate_battery_support() {
    if let Ok(manager) = battery::Manager::new() {
        if let Ok(batteries) = manager.batteries() {
            // The filter is necessary because Mac Desktops
            // find it funny to return a battery but they
            // don't actually have one and just return an
            // empty one
            let battery_count = batteries.flatten().count();
            BATTERY_SUPPORT.store(battery_count != 0, Ordering::SeqCst);
        }
    }
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os:             Option<String>,
    pub os_version:     Option<String>,
    pub kernel_version: Option<String>,
    pub users:          Vec<String>,
    pub uptime:         Duration,
}

#[derive(Debug, Clone)]
pub struct CpuInfo {
    pub usage:        f32,
    pub model:        String,
    pub manufacturer: String,
    pub frequency:    Frequency,
}

impl Hash for CpuInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.manufacturer.hash(state);
        self.model.hash(state);
    }
}

impl PartialEq for CpuInfo {
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model && self.manufacturer == other.manufacturer
    }
}

impl Eq for CpuInfo {}

// TODO: Find a way to get more info about RAM
// like frequency, DDR(N), manufacturer
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_memory: u64,
    pub used_memory:  u64,
    pub total_swap:   u64,
    pub used_swap:    u64,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub total:       u64,
    pub used:        u64,
    pub name:        String,
    pub file_system: Option<String>,
    pub mount_point: String,
}

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    pub charge:          f32,
    pub capacity_wh:     f32,
    pub capacity_new_wh: f32,
    pub health:          f32,
    pub voltage:         f32,
    pub state:           battery::State,
    pub technology:      battery::Technology,
    pub cycle_count:     Option<u32>,
    pub manufacturer:    Option<String>,
    pub model:           Option<String>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct NetworkFlags {
    pub raw:               u32,
    pub is_up:             bool,
    pub is_broadcast:      bool,
    pub is_loopback:       bool,
    pub is_point_to_point: bool,
    pub is_multicast:      bool,
    // pub is_lower_up:       bool, These three only exist on Linux or Unix, and would break full crossplatform support with windows.
    // pub is_dormant:        bool,
    // pub is_running:        bool,
}

#[derive(Debug, Clone, Default)]
pub struct Network {
    pub name:                         String,
    pub description:                  Option<String>,
    pub index:                        Option<u32>,
    pub ips:                          Option<Vec<std::net::IpAddr>>,
    pub flags:                        Option<NetworkFlags>,
    pub received_recently:            Option<u64>,
    pub received_total:               Option<u64>,
    pub transmitted_recently:         Option<u64>,
    pub transmitted_total:            Option<u64>,
    pub packets_received_recently:    Option<u64>,
    pub packets_received_total:       Option<u64>,
    pub packets_transmitted_recently: Option<u64>,
    pub packets_transmitted_total:    Option<u64>,
    pub mac_address:                  Option<sysinfo::MacAddr>,
}

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub connected:     bool,
    pub wifis:         Option<Vec<wifiscanner::Wifi>>,
    pub networks:      Option<Vec<Network>>,
    pub ip_address_v4: Option<std::net::IpAddr>,
    pub ip_address_v6: Option<std::net::IpAddr>,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub name:         String,
    pub path:         Option<String>,
    pub memory_usage: u64,
    pub swap_usage:   u64,
    pub cpu_usage:    f32,
    // TODO: add disk usage
    pub run_time:     Duration,
    pub pid:          sysinfo::Pid,
    pub parent:       Option<sysinfo::Pid>,
}

#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name:                 String,
    pub temperature:          f32,
    pub critical_temperature: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct DisplaySize {
    pub width:  u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id:           u32,
    pub size:         DisplaySize,
    pub scale_factor: f64,
    pub rotation:     f64,
    pub is_primary:   bool,
}

#[derive(Debug, Clone)]
pub struct BluetoothInfo {
    pub id:                       String,
    pub address:                  btleplug::api::BDAddr,
    pub local_name:               Option<String>,
    pub transmission_power_level: Option<i16>,
    pub signal_strength:          Option<i16>,
    pub is_connected:             bool,
}

pub struct Manager {
    system:           Option<System>,
    components:       Option<Components>,
    users:            Option<Users>,
    networks:         Option<Networks>,
    disks:            Option<Disks>,
    battery_manager:  Option<battery::Manager>,
    btleplug_adapter: Option<btleplug::platform::Adapter>,
    tokio_runtime:    tokio::runtime::Runtime,
}

impl Default for Manager {
    fn default() -> Self {
        let tokio_runtime = tokio::runtime::Runtime::new().expect("Constructing a tokio Runtime failed");
        populate_battery_support();
        Self {
            system: if SYSINFO_SUPPORT { Some(System::new_all()) } else { None },
            components: if SYSINFO_SUPPORT { Some(Components::new()) } else { None },
            users: if SYSINFO_SUPPORT { Some(Users::new_with_refreshed_list()) } else { None },
            networks: if SYSINFO_SUPPORT { Some(Networks::new()) } else { None },
            disks: if SYSINFO_SUPPORT { Some(Disks::new()) } else { None },
            battery_manager: if BATTERY_SUPPORT.load(Ordering::Relaxed) { battery::Manager::new().ok() } else { None },
            btleplug_adapter: tokio_runtime
                .block_on(btleplug::platform::Manager::new())
                .map(|manager| tokio_runtime.block_on(manager.adapters()).ok().map(|adapters| adapters.into_iter().nth(0)))
                .ok()
                .flatten()
                .flatten(),
            tokio_runtime,
        }
    }
}

impl Manager {
    #[must_use]
    pub fn new() -> Self {
        let new_self = Self::default();
        new_self
            .btleplug_adapter
            .as_ref()
            .map(|adapter| new_self.tokio_runtime.block_on(adapter.start_scan(btleplug::api::ScanFilter::default())));
        new_self
    }

    pub fn system_information(&mut self) -> Option<SystemInfo> {
        self.users.as_mut().map(|users| {
            users.refresh_list();
            SystemInfo {
                os:             System::name(),
                os_version:     System::os_version(),
                kernel_version: System::kernel_version(),
                users:          users.list().iter().map(|v| v.name().to_string()).collect(),
                uptime:         Duration::from_secs(System::uptime()),
            }
        })
    }

    pub fn cpu_information(&mut self) -> Option<Vec<CpuInfo>> {
        self.system.as_mut().map(|sys| {
            sys.refresh_cpu();
            #[allow(clippy::cast_precision_loss)]
            sys.cpus()
                .iter()
                .map(|cpu| CpuInfo {
                    usage:        cpu.cpu_usage(),
                    model:        cpu.name().to_string(),
                    manufacturer: cpu.brand().to_string(),
                    frequency:    Frequency::new::<megahertz>(cpu.frequency() as f64), /* TODO: figure out how to
                                                                                        * use uom for this */
                })
                .collect()
        })
    }

    pub fn memory_information(&mut self) -> Option<MemoryInfo> {
        self.system.as_mut().map(|sys| {
            sys.refresh_memory();
            MemoryInfo {
                total_memory: sys.total_memory(),
                used_memory:  sys.used_memory(),
                total_swap:   sys.total_swap(),
                used_swap:    sys.used_swap(),
            }
        })
    }

    pub fn disk_information(&mut self) -> Option<Vec<DiskInfo>> {
        self.disks.as_mut().map(|disks| {
            disks.refresh_list();
            disks
                .list()
                .iter()
                .map(|disk| DiskInfo {
                    total:       disk.total_space(),
                    used:        (disk.total_space() - disk.available_space()),
                    name:        disk.name().to_string_lossy().to_string(),
                    file_system: disk.file_system().to_str().map(ToString::to_string),
                    mount_point: disk.mount_point().to_string_lossy().to_string(),
                })
                .collect()
        })
    }

    // TODO: potential error source: batteries may
    // need to be stored in the Manager struct and
    // refreshed every time
    pub fn battery_information(&self) -> Option<Vec<BatteryInfo>> {
        self.battery_manager.as_ref().and_then(|battery_manager| {
            let batteries_res = battery_manager.batteries();
            batteries_res.map_or(None, |batteries| {
                Some(
                    batteries
                        .filter_map(|battery_res| {
                            let mut battery = battery_res.ok()?;
                            let _ = battery_manager.refresh(&mut battery);
                            Some(BatteryInfo {
                                charge:          f32::from(battery.state_of_charge()),
                                capacity_wh:     battery.energy_full().get::<watt_hour>(),
                                capacity_new_wh: battery.energy_full_design().get::<watt_hour>(),
                                health:          100.0 * f32::from(battery.state_of_health()),
                                voltage:         battery.voltage().get::<volt>(),
                                state:           battery.state(),
                                technology:      battery.technology(),
                                cycle_count:     battery.cycle_count(),
                                manufacturer:    battery.vendor().map(std::string::ToString::to_string),
                                model:           battery.model().map(std::string::ToString::to_string),
                            })
                        })
                        .collect(),
                )
            })
        })
    }

    // This is quite a complex function and I do not
    // see many advantages to refactoring it to if let
    pub fn network_information(&mut self) -> NetworkInfo {
        if let Some(networks) = self.networks.as_mut() {
            networks.refresh();
            networks.refresh_list();
        }

        let mut networks = self.networks.as_ref().map_or_else(Vec::new, |n| {
            n.list()
                .iter()
                .map(|(name, data)| Network {
                    name: name.to_string(),
                    received_recently: Some(data.received()),
                    received_total: Some(data.total_received()),
                    transmitted_recently: Some(data.transmitted()),
                    transmitted_total: Some(data.total_transmitted()),
                    packets_received_recently: Some(data.packets_received()),
                    packets_received_total: Some(data.total_packets_received()),
                    packets_transmitted_recently: Some(data.packets_transmitted()),
                    packets_transmitted_total: Some(data.total_packets_transmitted()),
                    mac_address: Some(data.mac_address()),
                    ..Default::default()
                })
                .collect::<Vec<Network>>()
        });

        for interface in pnet_datalink::interfaces() {
            let network_flags = NetworkFlags {
                raw:               interface.flags,
                is_up:             interface.is_up(),
                is_broadcast:      interface.is_broadcast(),
                is_loopback:       interface.is_loopback(),
                is_point_to_point: interface.is_point_to_point(),
                is_multicast:      interface.is_multicast(),
            };
            if let Some(network_index) = networks.iter().position(|network| network.name == interface.name) {
                networks[network_index].description = Some(interface.description);
                networks[network_index].index = Some(interface.index);
                networks[network_index].ips = Some(interface.ips.iter().map(ipnetwork::IpNetwork::ip).collect());
                networks[network_index].flags = Some(network_flags);
            } else {
                networks.push(Network {
                    name: interface.name,
                    description: Some(interface.description),
                    index: Some(interface.index),
                    ips: Some(interface.ips.iter().map(ipnetwork::IpNetwork::ip).collect()),
                    flags: Some(network_flags),
                    ..Default::default()
                });
            }
        }

        NetworkInfo {
            connected:     self.tokio_runtime.block_on(reqwest::get("https://google.com")).is_ok(),
            wifis:         wifiscanner::scan().ok(),
            networks:      match networks.len() {
                0 => None,
                _ => Some(networks),
            },
            ip_address_v4: local_ip_address::local_ip().ok(),
            ip_address_v6: local_ip_address::local_ipv6().ok(),
        }
    }

    pub fn process_information(&mut self) -> Option<Vec<ProcessInfo>> {
        self.system.as_mut().map(|sys| {
            sys.refresh_processes();
            sys.processes()
                .iter()
                .map(|(pid, process)| ProcessInfo {
                    name:         process.name().to_string(),
                    path:         process.exe().map(|p| p.to_string_lossy().into_owned()),
                    memory_usage: process.memory(),
                    swap_usage:   process.virtual_memory(),
                    cpu_usage:    process.cpu_usage(),
                    run_time:     Duration::from_secs(process.run_time()),
                    pid:          *pid,
                    parent:       process.parent(),
                })
                .collect()
        })
    }

    pub fn kill_process(&self, pid: sysinfo::Pid) -> bool {
        self.system.as_ref().map_or(false, |sys| sys.process(pid).is_some_and(sysinfo::Process::kill))
    }

    pub fn get_process(&self, pid: sysinfo::Pid) -> Option<&sysinfo::Process> {
        self.system.as_ref().and_then(|sys| sys.process(pid))
    }

    pub fn component_information(&mut self) -> Option<Vec<ComponentInfo>> {
        self.components.as_mut().map(|components| {
            components.refresh();
            components.refresh_list();
            components
                .list()
                .iter()
                .map(|component| ComponentInfo {
                    name:                 component.label().to_string(),
                    temperature:          component.temperature(),
                    critical_temperature: component.critical(),
                })
                .collect()
        })
    }

    pub fn display_information(&self) -> Option<Vec<DisplayInfo>> {
        display_info::DisplayInfo::all().ok().map(|monitors| {
            monitors
                .iter()
                .map(|monitor| DisplayInfo {
                    id:           monitor.id,
                    size:         DisplaySize {
                        width:  monitor.width,
                        height: monitor.height,
                    },
                    scale_factor: f64::from(monitor.scale_factor),
                    rotation:     f64::from(monitor.rotation),
                    is_primary:   monitor.is_primary,
                })
                .collect()
        })
    }

    pub fn bluetooth_information(&self) -> Option<Vec<BluetoothInfo>> {
        if let Some(adapter) = self.btleplug_adapter.as_ref() {
            Some(
                self.tokio_runtime
                    .block_on(adapter.peripherals())
                    .ok()?
                    .iter()
                    .map(|peripheral| {
                        let properties = self.tokio_runtime.block_on(peripheral.properties()).ok().flatten();
                        BluetoothInfo {
                            id:                       peripheral.id().to_string(),
                            address:                  peripheral.address(),
                            local_name:               properties.as_ref().and_then(|props| props.local_name.clone()),
                            transmission_power_level: properties.as_ref().and_then(|props| props.tx_power_level),
                            signal_strength:          properties.as_ref().and_then(|props| props.rssi),
                            is_connected:             self.tokio_runtime.block_on(peripheral.is_connected()).is_ok_and(|is_connected| is_connected),
                        }
                    })
                    .collect(),
            )
        } else {
            None
        }
    }
}

#[test]
fn test1() {
    println!("{:#?}", crate::Manager::new().display_information());
}
