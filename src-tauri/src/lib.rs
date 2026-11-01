use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use sysinfo::{Pid, System};
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessBrief {
    pub pid: u32,
    pub name: String,
    pub memory_mb: u64,
    pub exe_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub top_memory_processes: Vec<ProcessBrief>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListeningPort {
    pub port: u16,
    pub protocol: String,
    pub pid: u32,
    pub process_name: String,
    pub exe_path: Option<String>,
    pub is_dev_port: bool,
}

pub struct AppState {
    pub sys: Mutex<System>,
}

fn is_dev_port_number(port: u16) -> bool {
    matches!(
        port,
        3000 | 3001 | 3002 | 3003 | 4000 | 4200 | 5000 | 5173 | 5174 | 5175 | 5432 | 3306 | 6379 | 5672 | 8000 | 8001 | 8080 | 8081 | 8443 | 8888 | 9000 | 9090 | 27017 | 1025 | 1433
    )
}

fn pid_to_u32(pid: Pid) -> u32 {
    let s = pid.to_string();
    s.parse::<u32>().unwrap_or(0)
}

#[tauri::command]
fn get_system_stats(state: tauri::State<'_, AppState>) -> SystemStats {
    let mut sys = state.sys.lock().unwrap();
    sys.refresh_cpu();
    sys.refresh_memory();
    sys.refresh_processes();

    let cpu_usage_percent = sys.global_cpu_info().cpu_usage();
    let memory_used_mb = sys.used_memory() / (1024 * 1024);
    let memory_total_mb = sys.total_memory() / (1024 * 1024);

    let mut procs: Vec<ProcessBrief> = sys
        .processes()
        .iter()
        .map(|(pid, proc_)| ProcessBrief {
            pid: pid_to_u32(*pid),
            name: proc_.name().to_string(),
            memory_mb: proc_.memory() / (1024 * 1024),
            exe_path: proc_.exe().map(|p| p.to_string_lossy().to_string()),
        })
        .collect();

    procs.sort_by(|a, b| b.memory_mb.cmp(&a.memory_mb));
    procs.truncate(5);

    SystemStats {
        cpu_usage_percent,
        memory_used_mb,
        memory_total_mb,
        top_memory_processes: procs,
    }
}

#[tauri::command]
fn get_active_ports(state: tauri::State<'_, AppState>) -> Vec<ListeningPort> {
    let mut sys = state.sys.lock().unwrap();
    sys.refresh_processes();

    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP;

    let mut result = Vec::new();
    if let Ok(sockets) = get_sockets_info(af_flags, proto_flags) {
        for socket in sockets {
            if let ProtocolSocketInfo::Tcp(tcp_info) = socket.protocol_socket_info {
                if tcp_info.state == TcpState::Listen {
                    let port = tcp_info.local_port;
                    let pid = socket.associated_pids.first().copied().unwrap_or(0);

                    let (process_name, exe_path) = if pid > 0 {
                        let sys_pid = Pid::from(pid as usize);
                        if let Some(proc_) = sys.process(sys_pid) {
                            (
                                proc_.name().to_string(),
                                proc_.exe().map(|p| p.to_string_lossy().to_string()),
                            )
                        } else {
                            ("Unknown".to_string(), None)
                        }
                    } else {
                        ("System / Unknown".to_string(), None)
                    };

                    let protocol = match tcp_info.local_addr {
                        std::net::IpAddr::V4(_) => "TCP",
                        std::net::IpAddr::V6(_) => "TCP6",
                    }
                    .to_string();

                    let is_dev_port = is_dev_port_number(port);

                    result.push(ListeningPort {
                        port,
                        protocol,
                        pid,
                        process_name,
                        exe_path,
                        is_dev_port,
                    });
                }
            }
        }
    }

    result.sort_by(|a, b| {
        b.is_dev_port
            .cmp(&a.is_dev_port)
            .then_with(|| a.port.cmp(&b.port))
    });

    result.dedup_by(|a, b| a.port == b.port && a.pid == b.pid);

    result
}

#[tauri::command]
fn kill_process(state: tauri::State<'_, AppState>, pid: u32) -> Result<bool, String> {
    let mut sys = state.sys.lock().unwrap();
    let sys_pid = Pid::from(pid as usize);
    sys.refresh_processes();

    if let Some(process) = sys.process(sys_pid) {
        if process.kill() {
            Ok(true)
        } else {
            Err(format!("Failed to terminate process PID {}", pid))
        }
    } else {
        Err(format!("Process PID {} not found", pid))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            sys: Mutex::new(System::new_all()),
        })
        .invoke_handler(tauri::generate_handler![
            get_system_stats,
            get_active_ports,
            kill_process
        ])
        .setup(|app| {
            let tray_icon = app.default_window_icon().cloned();

            let mut tray_builder = TrayIconBuilder::new().tooltip("Port Sentinel & Dev Monitor");

            if let Some(icon) = tray_icon {
                tray_builder = tray_builder.icon(icon);
            }

            tray_builder
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            if is_visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.move_window(Position::TrayBottomCenter);
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            let main_window = app.get_webview_window("main");
            if let Some(window) = main_window {
                let w_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = w_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
