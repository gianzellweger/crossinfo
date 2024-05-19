// #![allow(unused_imports, dead_code,
// unused_variables)]
#![forbid(unsafe_code)]
#![feature(let_chains)]

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

// Big TODO: Make this library infallable

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
use uom::si::{
    f32::*,
    frequency::{gigahertz, megahertz},
};

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
            let battery_count = batteries.filter(|battery| battery.is_ok()).count();
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
    pub usage:         f32,
    pub model:         String,
    pub manufacturer:  String,
    pub frequency_ghz: f32,
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
    pub total_memory: usize,
    pub used_memory:  usize,
    pub total_swap:   usize,
    pub used_swap:    usize,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub total:       usize,
    pub used:        usize,
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
    pub received_recently:            Option<usize>,
    pub received_total:               Option<usize>,
    pub transmitted_recently:         Option<usize>,
    pub transmitted_total:            Option<usize>,
    pub packets_received_recently:    Option<usize>,
    pub packets_received_total:       Option<usize>,
    pub packets_transmitted_recently: Option<usize>,
    pub packets_transmitted_total:    Option<usize>,
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
    pub memory_usage: usize,
    pub swap_usage:   usize,
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
    pub id:           usize,
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
            system: match SYSINFO_SUPPORT {
                true => Some(System::new_all()),
                false => None,
            },
            components: match SYSINFO_SUPPORT {
                true => Some(Components::new()),
                false => None,
            },
            users: match SYSINFO_SUPPORT {
                true => Some(Users::new_with_refreshed_list()),
                false => None,
            },
            networks: match SYSINFO_SUPPORT {
                true => Some(Networks::new()),
                false => None,
            },
            disks: match SYSINFO_SUPPORT {
                true => Some(Disks::new()),
                false => None,
            },
            battery_manager: match BATTERY_SUPPORT.load(Ordering::Relaxed) {
                true => battery::Manager::new().ok(),
                false => None,
            },
            btleplug_adapter: tokio_runtime
                .block_on(btleplug::platform::Manager::new())
                .map(|manager| tokio_runtime.block_on(manager.adapters()).ok().map(|adapters| adapters.into_iter().nth(0).unwrap()))
                .ok()
                .flatten(),
            tokio_runtime,
        }
    }
}

impl Manager {
    pub fn new() -> Self {
        let new_self = Self { ..Default::default() };
        new_self
            .btleplug_adapter
            .as_ref()
            .map(|adapter| new_self.tokio_runtime.block_on(adapter.start_scan(btleplug::api::ScanFilter::default())));
        new_self
    }

    pub fn system_information(&mut self) -> Option<SystemInfo> {
        if let Some(users) = self.users.as_mut() {
            users.refresh_list();
            Some(SystemInfo {
                os:             System::name(),
                os_version:     System::os_version(),
                kernel_version: System::kernel_version(),
                users:          users.list().iter().map(|v| v.name().to_string()).collect(),
                // .uptime() actually exists, but since the only way to
                //  refresh that also refreshes the CPU information,
                //  which
                //  needs some interval to display properly, this is
                //  probably the easier solution
                uptime:         (std::time::UNIX_EPOCH + Duration::from_secs(System::boot_time())).elapsed().unwrap(),
            })
        } else {
            None
        }
    }

    pub fn cpu_information(&mut self) -> Option<Vec<CpuInfo>> {
        if let Some(sys) = self.system.as_mut() {
            sys.refresh_cpu();
            Some(
                sys.cpus()
                    .iter()
                    .map(|cpu| CpuInfo {
                        usage:         cpu.cpu_usage(),
                        model:         cpu.name().to_string(),
                        manufacturer:  cpu.brand().to_string(),
                        frequency_ghz: Frequency::new::<megahertz>(cpu.frequency() as f32).get::<gigahertz>(), /* TODO: figure out how to
                                                                                                                * use uom for this */
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn memory_information(&mut self) -> Option<MemoryInfo> {
        if let Some(sys) = self.system.as_mut() {
            sys.refresh_memory();
            Some(MemoryInfo {
                total_memory: sys.total_memory() as usize,
                used_memory:  sys.used_memory() as usize,
                total_swap:   sys.total_swap() as usize,
                used_swap:    sys.used_swap() as usize,
            })
        } else {
            None
        }
    }

    pub fn disk_information(&mut self) -> Option<Vec<DiskInfo>> {
        if let Some(disks) = self.disks.as_mut() {
            disks.refresh_list();
            Some(
                disks
                    .list()
                    .iter()
                    .map(|disk| DiskInfo {
                        total:       disk.total_space() as usize,
                        used:        (disk.total_space() - disk.available_space()) as usize,
                        name:        disk.name().to_string_lossy().to_string(),
                        file_system: disk.file_system().to_str().map(|s| s.to_string()),
                        mount_point: disk.mount_point().to_string_lossy().to_string(),
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    // TODO: potential error source: batteries may
    // need to be stored in the Manager struct and
    // refreshed every time
    // TODO: refactor this to also use if let
    pub fn battery_information(&mut self) -> Option<Vec<BatteryInfo>> {
        self.battery_manager.as_ref()?;
        let batteries_res = self.battery_manager.as_ref().unwrap().batteries();
        match batteries_res {
            Ok(batteries) => Some(
                batteries
                    .filter(|battery_res| battery_res.is_ok())
                    .map(|battery_res| {
                        let mut battery = battery_res.expect("This is not supposed to happen. The batteries should already be filtered at this point");
                        let _ = self.battery_manager.as_ref().unwrap().refresh(&mut battery); // This could fail and lead to weird behavior.
                                                                                              // Lets hope that doesn't happen
                        BatteryInfo {
                            charge:          f32::from(battery.state_of_charge()),
                            capacity_wh:     battery.energy_full().get::<watt_hour>(),
                            capacity_new_wh: battery.energy_full_design().get::<watt_hour>(),
                            health:          100.0 * f32::from(battery.state_of_health()),
                            voltage:         battery.voltage().get::<volt>(),
                            state:           battery.state(),
                            technology:      battery.technology(),
                            cycle_count:     battery.cycle_count(),
                            manufacturer:    battery.vendor().map(|s| s.to_string()),
                            model:           battery.model().map(|s| s.to_string()),
                        }
                    })
                    .collect(),
            ),
            Err(_) => None,
        }
    }

    // This is quite a complex function and I do not
    // see many advantages to refactoring it to if let
    pub fn network_information(&mut self) -> NetworkInfo {
        if let Some(networks) = self.networks.as_mut() {
            networks.refresh();
            networks.refresh_list();
        }

        let mut networks = match self.networks.as_ref() {
            Some(n) => n
                .list()
                .iter()
                .map(|(name, data)| Network {
                    name: name.to_string(),
                    received_recently: Some(data.received() as usize),
                    received_total: Some(data.total_received() as usize),
                    transmitted_recently: Some(data.transmitted() as usize),
                    transmitted_total: Some(data.total_transmitted() as usize),
                    packets_received_recently: Some(data.packets_received() as usize),
                    packets_received_total: Some(data.total_packets_received() as usize),
                    packets_transmitted_recently: Some(data.packets_transmitted() as usize),
                    packets_transmitted_total: Some(data.total_packets_transmitted() as usize),
                    mac_address: Some(data.mac_address()),
                    ..Default::default()
                })
                .collect::<Vec<Network>>(),
            None => vec![],
        };

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
                networks[network_index].ips = Some(interface.ips.iter().map(|ip| ip.ip()).collect());
                networks[network_index].flags = Some(network_flags);
            } else {
                networks.push(Network {
                    name: interface.name,
                    description: Some(interface.description),
                    index: Some(interface.index),
                    ips: Some(interface.ips.iter().map(|ip| ip.ip()).collect()),
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
        if let Some(sys) = self.system.as_mut() {
            sys.refresh_processes();
            Some(
                sys.processes()
                    .iter()
                    .map(|(pid, process)| ProcessInfo {
                        name:         process.name().to_string(),
                        path:         process.exe().map(|p| p.to_string_lossy().into_owned()),
                        memory_usage: process.memory() as usize,
                        swap_usage:   process.virtual_memory() as usize,
                        cpu_usage:    process.cpu_usage(),
                        run_time:     Duration::from_secs(process.run_time()),
                        pid:          *pid,
                        parent:       process.parent(),
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn kill_process(&self, pid: sysinfo::Pid) -> bool {
        match self.system.as_ref() {
            Some(sys) => sys.process(pid).map(|p| p.kill()).unwrap_or(false),
            None => false,
        }
    }

    pub fn get_process(&mut self, pid: sysinfo::Pid) -> Option<&sysinfo::Process> {
        match self.system.as_ref() {
            Some(sys) => sys.process(pid),
            None => None,
        }
    }

    pub fn component_information(&mut self) -> Option<Vec<ComponentInfo>> {
        if let Some(components) = self.components.as_mut() {
            components.refresh();
            components.refresh_list();
            Some(
                components
                    .list()
                    .iter()
                    .map(|component| ComponentInfo {
                        name:                 component.label().to_string(),
                        temperature:          component.temperature(),
                        critical_temperature: component.critical(),
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn print_type_of<T>(_: &T) {
        println!("{}", std::any::type_name::<T>())
    }

    pub fn display_information(&self) -> Option<Vec<DisplayInfo>> {
        display_info::DisplayInfo::all().ok().map(|monitors| {
            monitors
                .iter()
                .map(|monitor| DisplayInfo {
                    id:           monitor.id as usize,
                    size:         DisplaySize {
                        width:  monitor.width,
                        height: monitor.height,
                    },
                    scale_factor: monitor.scale_factor as f64,
                    rotation:     monitor.rotation as f64,
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
    crate::Manager::new().display_information();
}
