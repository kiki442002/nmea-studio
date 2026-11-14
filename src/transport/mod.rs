use chrono::Local;
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::protocols::is_nmea2000_line;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortKind {
    UdpUnicast,
    UdpMulticast,
    UdpBroadcast,
    Serial,
    TcpServer,
    TcpClient,
}

impl PortKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::UdpUnicast => "🎯 UDP Unicast",
            Self::UdpMulticast => "📡 UDP Multicast (IEC 61162-450)",
            Self::UdpBroadcast => "📢 UDP Broadcast",
            Self::Serial => "🔌 Serial Port (COM / tty)",
            Self::TcpServer => "🖥 TCP Server (Listener)",
            Self::TcpClient => "🔗 TCP Client",
        }
    }

    pub const ALL: [PortKind; 6] = [
        Self::UdpUnicast,
        Self::UdpMulticast,
        Self::UdpBroadcast,
        Self::Serial,
        Self::TcpServer,
        Self::TcpClient,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    TxOnly,
    RxOnly,
    Bidirectional,
}

impl PortDirection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TxOnly => "TX Only (Output)",
            Self::RxOnly => "RX Only (Input Listen)",
            Self::Bidirectional => "TX + RX (Bidirectional)",
        }
    }

    pub fn can_tx(&self) -> bool {
        matches!(self, Self::TxOnly | Self::Bidirectional)
    }

    pub fn can_rx(&self) -> bool {
        matches!(self, Self::RxOnly | Self::Bidirectional)
    }
}

/// Official IEC 61162-450 (NMEA over IP / LWE) UDP Multicast Transmission Groups
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IecMulticastGroup {
    Misc, // 239.192.0.1 : 60001 - Miscellaneous / Weather / Wind
    Tgtd, // 239.192.0.2 : 60002 - Target Data (AIS, Radar ARPA)
    Satd, // 239.192.0.3 : 60003 - High-update / Gyro / Attitude / Heading
    Navd, // 239.192.0.4 : 60004 - Navigation (GPS/GNSS, Speed Log, Depth Sounder)
    Vdrd, // 239.192.0.5 : 60005 - VDR (Voyage Data Recorder)
    Rcom, // 239.192.0.6 : 60006 - Radio Communication
    Time, // 239.192.0.7 : 60007 - Time Data
    Prop, // 239.192.0.8 : 60008 - Propulsion, Engine & Autopilot / Rudder
}

impl IecMulticastGroup {
    pub const ALL: [IecMulticastGroup; 8] = [
        Self::Misc,
        Self::Tgtd,
        Self::Satd,
        Self::Navd,
        Self::Vdrd,
        Self::Rcom,
        Self::Time,
        Self::Prop,
    ];

    pub fn code(&self) -> &'static str {
        match self {
            Self::Misc => "MISC",
            Self::Tgtd => "TGTD",
            Self::Satd => "SATD",
            Self::Navd => "NAVD",
            Self::Vdrd => "VDRD",
            Self::Rcom => "RCOM",
            Self::Time => "TIME",
            Self::Prop => "PROP",
        }
    }

    pub fn ip(&self) -> &'static str {
        match self {
            Self::Misc => "239.192.0.1",
            Self::Tgtd => "239.192.0.2",
            Self::Satd => "239.192.0.3",
            Self::Navd => "239.192.0.4",
            Self::Vdrd => "239.192.0.5",
            Self::Rcom => "239.192.0.6",
            Self::Time => "239.192.0.7",
            Self::Prop => "239.192.0.8",
        }
    }

    pub fn port(&self) -> u16 {
        match self {
            Self::Misc => 60001,
            Self::Tgtd => 60002,
            Self::Satd => 60003,
            Self::Navd => 60004,
            Self::Vdrd => 60005,
            Self::Rcom => 60006,
            Self::Time => 60007,
            Self::Prop => 60008,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Misc => "MISC - 239.192.0.1:60001 (Wind / Meteo / Misc)",
            Self::Tgtd => "TGTD - 239.192.0.2:60002 (AIS / Radar ARPA Targets)",
            Self::Satd => "SATD - 239.192.0.3:60003 (Gyro / Heading / Attitude)",
            Self::Navd => "NAVD - 239.192.0.4:60004 (GPS / GNSS / Depth / Speed)",
            Self::Vdrd => "VDRD - 239.192.0.5:60005 (Voyage Data Recorder)",
            Self::Rcom => "RCOM - 239.192.0.6:60006 (Radio Communication)",
            Self::Time => "TIME - 239.192.0.7:60007 (Time Reference)",
            Self::Prop => "PROP - 239.192.0.8:60008 (Propulsion / Engine / Autopilot)",
        }
    }

    /// Selects the standard IEC 61162-450 multicast group based on the device name/category
    pub fn for_device_name(name: &str) -> Self {
        let upper = name.to_ascii_uppercase();
        if upper.contains("AIS") || upper.contains("ARPA") || upper.contains("RADAR") {
            Self::Tgtd
        } else if upper.contains("GYRO") || upper.contains("COMPASS") || upper.contains("HEADING") {
            Self::Satd
        } else if upper.contains("GPS")
            || upper.contains("GNSS")
            || upper.contains("DEPTH")
            || upper.contains("SPEED")
            || upper.contains("SOUNDER")
            || upper.contains("NAV")
        {
            Self::Navd
        } else if upper.contains("ENGINE")
            || upper.contains("BATTERY")
            || upper.contains("AUTOPILOT")
            || upper.contains("RUDDER")
            || upper.contains("PROP")
        {
            Self::Prop
        } else {
            Self::Misc
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: PortKind,
    pub direction: PortDirection,
    pub pass_nmea0183: bool,
    pub pass_nmea2000: bool,

    // Network settings
    pub remote_host: String,
    pub remote_port: u16,
    pub local_bind_host: String,
    pub local_bind_port: u16,

    // Multicast settings
    pub multicast_group: String,
    pub multicast_port: u16,
    pub multicast_interface: String,
    pub multicast_ttl: u32,
    pub multicast_loopback: bool,

    // Serial settings
    pub serial_port_name: String,
    pub baud_rate: u32,
    pub data_bits: u8, // 7 or 8
    pub parity_none: bool,
    pub stop_bits: u8, // 1 or 2
}

impl PortConfig {
    pub fn default_ports() -> Vec<Self> {
        vec![
            Self {
                id: "port-udp-uni".to_string(),
                name: "OpenCPN / Local UDP Unicast".to_string(),
                enabled: true,
                kind: PortKind::UdpUnicast,
                direction: PortDirection::Bidirectional,
                pass_nmea0183: true,
                pass_nmea2000: true,
                remote_host: "127.0.0.1".to_string(),
                remote_port: 10110,
                local_bind_host: "0.0.0.0".to_string(),
                local_bind_port: 10111,
                multicast_group: "239.192.0.1".to_string(),
                multicast_port: 60001,
                multicast_interface: "0.0.0.0".to_string(),
                multicast_ttl: 1,
                multicast_loopback: true,
                serial_port_name: "/dev/ttyUSB0".to_string(),
                baud_rate: 38400,
                data_bits: 8,
                parity_none: true,
                stop_bits: 1,
            },
            Self {
                id: "port-udp-mcast".to_string(),
                name: "Marine Multicast Bus (239.192.0.1:60001)".to_string(),
                enabled: true,
                kind: PortKind::UdpMulticast,
                direction: PortDirection::TxOnly,
                pass_nmea0183: true,
                pass_nmea2000: true,
                remote_host: "239.192.0.1".to_string(),
                remote_port: 60001,
                local_bind_host: "0.0.0.0".to_string(),
                local_bind_port: 60001,
                multicast_group: "239.192.0.1".to_string(),
                multicast_port: 60001,
                multicast_interface: "0.0.0.0".to_string(),
                multicast_ttl: 1,
                multicast_loopback: true,
                serial_port_name: "/dev/ttyUSB0".to_string(),
                baud_rate: 38400,
                data_bits: 8,
                parity_none: true,
                stop_bits: 1,
            },
            Self {
                id: "port-serial-1".to_string(),
                name: "RS-422 / USB Serial Gateway".to_string(),
                enabled: false,
                kind: PortKind::Serial,
                direction: PortDirection::Bidirectional,
                pass_nmea0183: true,
                pass_nmea2000: true,
                remote_host: "127.0.0.1".to_string(),
                remote_port: 10110,
                local_bind_host: "0.0.0.0".to_string(),
                local_bind_port: 0,
                multicast_group: "239.192.0.1".to_string(),
                multicast_port: 60001,
                multicast_interface: "0.0.0.0".to_string(),
                multicast_ttl: 1,
                multicast_loopback: true,
                serial_port_name: if cfg!(target_os = "windows") {
                    "COM3".to_string()
                } else if cfg!(target_os = "macos") {
                    "/dev/cu.usbserial-0001".to_string()
                } else {
                    "/dev/ttyUSB0".to_string()
                },
                baud_rate: 38400,
                data_bits: 8,
                parity_none: true,
                stop_bits: 1,
            },
        ]
    }

    pub fn summary_endpoint(&self) -> String {
        match self.kind {
            PortKind::UdpUnicast => format!(
                "UDP {} -> {}:{}",
                self.local_bind_port, self.remote_host, self.remote_port
            ),
            PortKind::UdpMulticast => format!(
                "MCAST {}:{} (TTL {})",
                self.multicast_group, self.multicast_port, self.multicast_ttl
            ),
            PortKind::UdpBroadcast => format!("BCAST {}:{}", self.remote_host, self.remote_port),
            PortKind::Serial => format!("{} @ {} bps", self.serial_port_name, self.baud_rate),
            PortKind::TcpServer => {
                format!("TCP Listen {}:{}", self.local_bind_host, self.local_bind_port)
            }
            PortKind::TcpClient => format!("TCP Connect {}:{}", self.remote_host, self.remote_port),
        }
    }

    /// Checks whether a generated message should be transmitted on this port
    pub fn should_route(&self, is_nmea2000: bool, target_ports: &[String]) -> bool {
        if !self.enabled || !self.direction.can_tx() {
            return false;
        }
        if is_nmea2000 && !self.pass_nmea2000 {
            return false;
        }
        if !is_nmea2000 && !self.pass_nmea0183 {
            return false;
        }
        if !target_ports.is_empty() && !target_ports.contains(&self.id) {
            return false;
        }
        true
    }
}

pub fn format_wire_message(line: &str, omit_crlf: bool) -> String {
    if omit_crlf {
        line.to_string()
    } else {
        format!("{}\r\n", line)
    }
}

/// Extracts complete newline-delimited lines from a streaming receive buffer, leaving any partial trailing data in `rx_accum`.
pub fn extract_complete_lines(rx_accum: &mut String) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(pos) = rx_accum.find('\n') {
        let line = rx_accum[..pos].trim().to_string();
        rx_accum.drain(..=pos);
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficDirection {
    Tx,
    Rx,
}

#[derive(Debug, Clone)]
pub struct TrafficLogEntry {
    pub seq_id: u64,
    pub timestamp: String,
    pub port_id: String,
    pub port_name: String,
    pub source_name: String,
    pub direction: TrafficDirection,
    pub is_nmea2000: bool,
    pub raw_line: String,
}

#[derive(Debug, Clone, Default)]
pub struct PortStats {
    pub tx_packets: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub rx_bytes: u64,
    pub is_open: bool,
    pub status_message: String,
}

enum WorkerEvent {
    RxLine {
        port_id: String,
        port_name: String,
        line: String,
    },
    Status {
        port_id: String,
        is_open: bool,
        message: String,
    },
    TxSent {
        port_id: String,
        bytes: usize,
    },
}

struct ActiveWorker {
    config_snapshot: String,
    stop_flag: Arc<AtomicBool>,
    tx_sender: Sender<String>,
}

pub struct TransportManager {
    workers: HashMap<String, ActiveWorker>,
    event_tx: Sender<WorkerEvent>,
    event_rx: Receiver<WorkerEvent>,
    pub stats: HashMap<String, PortStats>,
    pub available_serial_ports: Vec<String>,
}

impl Default for TransportManager {
    fn default() -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let mut mgr = Self {
            workers: HashMap::new(),
            event_tx,
            event_rx,
            stats: HashMap::new(),
            available_serial_ports: Vec::new(),
        };
        mgr.refresh_serial_ports();
        mgr
    }
}

impl TransportManager {
    pub fn refresh_serial_ports(&mut self) {
        let mut names = Vec::new();
        if let Ok(ports) = serialport::available_ports() {
            for p in ports {
                names.push(p.port_name);
            }
        }
        names.sort();
        self.available_serial_ports = names;
    }

    pub fn sync_ports(&mut self, configs: &[PortConfig]) {
        let active_ids: Vec<String> = configs
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.id.clone())
            .collect();

        let existing_ids: Vec<String> = self.workers.keys().cloned().collect();
        for id in existing_ids {
            if !active_ids.contains(&id) {
                if let Some(w) = self.workers.remove(&id) {
                    w.stop_flag.store(true, Ordering::Relaxed);
                }
                if let Some(st) = self.stats.get_mut(&id) {
                    st.is_open = false;
                    st.status_message = "Disabled".to_string();
                }
            }
        }

        for cfg in configs {
            self.stats.entry(cfg.id.clone()).or_default();
            if !cfg.enabled {
                continue;
            }
            let snap = serde_json::to_string(cfg).unwrap_or_default();
            let needs_restart = match self.workers.get(&cfg.id) {
                Some(w) => w.config_snapshot != snap,
                None => true,
            };

            if needs_restart {
                if let Some(old) = self.workers.remove(&cfg.id) {
                    old.stop_flag.store(true, Ordering::Relaxed);
                }
                let stop_flag = Arc::new(AtomicBool::new(false));
                let (tx_sender, tx_receiver) = mpsc::channel::<String>();
                let ev_tx = self.event_tx.clone();
                let cfg_clone = cfg.clone();
                let stop_clone = stop_flag.clone();

                thread::spawn(move || {
                    run_port_worker(cfg_clone, stop_clone, tx_receiver, ev_tx);
                });

                self.workers.insert(
                    cfg.id.clone(),
                    ActiveWorker {
                        config_snapshot: snap,
                        stop_flag,
                        tx_sender,
                    },
                );
            }
        }
    }

    pub fn send_line(&mut self, port_id: &str, line: &str, omit_crlf: bool) {
        if let Some(worker) = self.workers.get(port_id) {
            let formatted = format_wire_message(line, omit_crlf);
            let _ = worker.tx_sender.send(formatted);
        }
    }

    pub fn poll_events(&mut self, next_seq: &mut u64) -> Vec<TrafficLogEntry> {
        let mut received = Vec::new();
        while let Ok(ev) = self.event_rx.try_recv() {
            match ev {
                WorkerEvent::Status {
                    port_id,
                    is_open,
                    message,
                } => {
                    let st = self.stats.entry(port_id).or_default();
                    st.is_open = is_open;
                    st.status_message = message;
                }
                WorkerEvent::TxSent { port_id, bytes } => {
                    let st = self.stats.entry(port_id).or_default();
                    st.tx_packets += 1;
                    st.tx_bytes += bytes as u64;
                }
                WorkerEvent::RxLine {
                    port_id,
                    port_name,
                    line,
                } => {
                    let trimmed = line.trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let st = self.stats.entry(port_id.clone()).or_default();
                    st.rx_packets += 1;
                    st.rx_bytes += line.len() as u64;

                    let is_n2k = is_nmea2000_line(&trimmed);
                    *next_seq += 1;
                    received.push(TrafficLogEntry {
                        seq_id: *next_seq,
                        timestamp: Local::now().format("%H:%M:%S%.3f").to_string(),
                        port_id,
                        port_name: port_name.clone(),
                        source_name: format!("RX ({})", port_name),
                        direction: TrafficDirection::Rx,
                        is_nmea2000: is_n2k,
                        raw_line: trimmed,
                    });
                }
            }
        }
        received
    }
}

fn run_port_worker(
    cfg: PortConfig,
    stop_flag: Arc<AtomicBool>,
    tx_rx: Receiver<String>,
    ev_tx: Sender<WorkerEvent>,
) {
    match cfg.kind {
        PortKind::UdpUnicast | PortKind::UdpBroadcast => {
            run_udp_worker(cfg, stop_flag, tx_rx, ev_tx);
        }
        PortKind::UdpMulticast => {
            run_multicast_worker(cfg, stop_flag, tx_rx, ev_tx);
        }
        PortKind::Serial => {
            run_serial_worker(cfg, stop_flag, tx_rx, ev_tx);
        }
        PortKind::TcpClient => {
            run_tcp_client_worker(cfg, stop_flag, tx_rx, ev_tx);
        }
        PortKind::TcpServer => {
            run_tcp_server_worker(cfg, stop_flag, tx_rx, ev_tx);
        }
    }
}

fn run_udp_worker(
    cfg: PortConfig,
    stop: Arc<AtomicBool>,
    tx_rx: Receiver<String>,
    ev_tx: Sender<WorkerEvent>,
) {
    let bind_addr = if cfg.direction.can_rx() {
        format!("{}:{}", cfg.local_bind_host, cfg.local_bind_port)
    } else {
        "0.0.0.0:0".to_string()
    };

    let sock = match UdpSocket::bind(&bind_addr).or_else(|_| UdpSocket::bind("0.0.0.0:0")) {
        Ok(s) => s,
        Err(e) => {
            let _ = ev_tx.send(WorkerEvent::Status {
                port_id: cfg.id,
                is_open: false,
                message: format!("UDP bind error: {}", e),
            });
            return;
        }
    };

    if cfg.kind == PortKind::UdpBroadcast {
        let _ = sock.set_broadcast(true);
    }
    let _ = sock.set_read_timeout(Some(Duration::from_millis(25)));
    let target_addr = format!("{}:{}", cfg.remote_host, cfg.remote_port);

    let _ = ev_tx.send(WorkerEvent::Status {
        port_id: cfg.id.clone(),
        is_open: true,
        message: format!("Active ({})", target_addr),
    });

    let mut rx_buf = [0u8; 4096];
    while !stop.load(Ordering::Relaxed) {
        while let Ok(msg) = tx_rx.try_recv() {
            if cfg.direction.can_tx() {
                if let Ok(sent) = sock.send_to(msg.as_bytes(), &target_addr) {
                    let _ = ev_tx.send(WorkerEvent::TxSent {
                        port_id: cfg.id.clone(),
                        bytes: sent,
                    });
                }
            }
        }

        if cfg.direction.can_rx() {
            if let Ok((n, _src)) = sock.recv_from(&mut rx_buf) {
                let text = String::from_utf8_lossy(&rx_buf[..n]);
                for line in text.lines() {
                    let _ = ev_tx.send(WorkerEvent::RxLine {
                        port_id: cfg.id.clone(),
                        port_name: cfg.name.clone(),
                        line: line.to_string(),
                    });
                }
            }
        } else {
            thread::sleep(Duration::from_millis(20));
        }
    }
}

fn run_multicast_worker(
    cfg: PortConfig,
    stop: Arc<AtomicBool>,
    tx_rx: Receiver<String>,
    ev_tx: Sender<WorkerEvent>,
) {
    let mcast_ip: Ipv4Addr = match cfg.multicast_group.parse() {
        Ok(ip) => ip,
        Err(e) => {
            let _ = ev_tx.send(WorkerEvent::Status {
                port_id: cfg.id,
                is_open: false,
                message: format!("Invalid multicast IP: {}", e),
            });
            return;
        }
    };
    let iface_ip: Ipv4Addr = cfg
        .multicast_interface
        .parse()
        .unwrap_or(Ipv4Addr::UNSPECIFIED);

    let socket = match Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)) {
        Ok(s) => s,
        Err(e) => {
            let _ = ev_tx.send(WorkerEvent::Status {
                port_id: cfg.id,
                is_open: false,
                message: format!("Socket error: {}", e),
            });
            return;
        }
    };

    let _ = socket.set_reuse_address(true);
    #[cfg(unix)]
    let _ = socket.set_reuse_port(true);

    let bind_port = if cfg.direction.can_rx() {
        cfg.multicast_port
    } else {
        0
    };
    let bind_sockaddr: SocketAddr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, bind_port));
    if let Err(e) = socket.bind(&bind_sockaddr.into()) {
        let _ = ev_tx.send(WorkerEvent::Status {
            port_id: cfg.id,
            is_open: false,
            message: format!("Multicast bind error: {}", e),
        });
        return;
    }

    let _ = socket.set_multicast_ttl_v4(cfg.multicast_ttl);
    let _ = socket.set_multicast_loop_v4(cfg.multicast_loopback);
    let _ = socket.set_multicast_if_v4(&iface_ip);
    if cfg.direction.can_rx() {
        let _ = socket.join_multicast_v4(&mcast_ip, &iface_ip);
    }

    let udp_sock: UdpSocket = socket.into();
    let _ = udp_sock.set_read_timeout(Some(Duration::from_millis(25)));
    let dest_addr = SocketAddr::from((mcast_ip, cfg.multicast_port));

    let _ = ev_tx.send(WorkerEvent::Status {
        port_id: cfg.id.clone(),
        is_open: true,
        message: format!("Multicast Active ({})", dest_addr),
    });

    let mut rx_buf = [0u8; 4096];
    while !stop.load(Ordering::Relaxed) {
        while let Ok(msg) = tx_rx.try_recv() {
            if cfg.direction.can_tx() {
                if let Ok(sent) = udp_sock.send_to(msg.as_bytes(), dest_addr) {
                    let _ = ev_tx.send(WorkerEvent::TxSent {
                        port_id: cfg.id.clone(),
                        bytes: sent,
                    });
                }
            }
        }

        if cfg.direction.can_rx() {
            if let Ok((n, _)) = udp_sock.recv_from(&mut rx_buf) {
                let text = String::from_utf8_lossy(&rx_buf[..n]);
                for line in text.lines() {
                    let _ = ev_tx.send(WorkerEvent::RxLine {
                        port_id: cfg.id.clone(),
                        port_name: cfg.name.clone(),
                        line: line.to_string(),
                    });
                }
            }
        } else {
            thread::sleep(Duration::from_millis(20));
        }
    }
}

fn run_serial_worker(
    cfg: PortConfig,
    stop: Arc<AtomicBool>,
    tx_rx: Receiver<String>,
    ev_tx: Sender<WorkerEvent>,
) {
    let builder = serialport::new(&cfg.serial_port_name, cfg.baud_rate)
        .data_bits(if cfg.data_bits == 7 {
            serialport::DataBits::Seven
        } else {
            serialport::DataBits::Eight
        })
        .parity(if cfg.parity_none {
            serialport::Parity::None
        } else {
            serialport::Parity::Even
        })
        .stop_bits(if cfg.stop_bits == 2 {
            serialport::StopBits::Two
        } else {
            serialport::StopBits::One
        })
        .timeout(Duration::from_millis(25));

    let mut port = match builder.open() {
        Ok(p) => p,
        Err(e) => {
            let _ = ev_tx.send(WorkerEvent::Status {
                port_id: cfg.id,
                is_open: false,
                message: format!("Serial error: {}", e),
            });
            return;
        }
    };

    let _ = ev_tx.send(WorkerEvent::Status {
        port_id: cfg.id.clone(),
        is_open: true,
        message: format!("Opened {} @ {}", cfg.serial_port_name, cfg.baud_rate),
    });

    let mut rx_accum = String::new();
    let mut buf = [0u8; 512];

    while !stop.load(Ordering::Relaxed) {
        while let Ok(msg) = tx_rx.try_recv() {
            if cfg.direction.can_tx() {
                if let Ok(()) = port.write_all(msg.as_bytes()) {
                    let _ = ev_tx.send(WorkerEvent::TxSent {
                        port_id: cfg.id.clone(),
                        bytes: msg.len(),
                    });
                }
            }
        }

        if cfg.direction.can_rx() {
            if let Ok(n) = port.read(&mut buf) {
                if n > 0 {
                    rx_accum.push_str(&String::from_utf8_lossy(&buf[..n]));
                    for line in extract_complete_lines(&mut rx_accum) {
                        let _ = ev_tx.send(WorkerEvent::RxLine {
                            port_id: cfg.id.clone(),
                            port_name: cfg.name.clone(),
                            line,
                        });
                    }
                }
            }
        } else {
            thread::sleep(Duration::from_millis(20));
        }
    }
}

fn run_tcp_client_worker(
    cfg: PortConfig,
    stop: Arc<AtomicBool>,
    tx_rx: Receiver<String>,
    ev_tx: Sender<WorkerEvent>,
) {
    let addr = format!("{}:{}", cfg.remote_host, cfg.remote_port);
    let mut stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(e) => {
            let _ = ev_tx.send(WorkerEvent::Status {
                port_id: cfg.id,
                is_open: false,
                message: format!("TCP connect failed: {}", e),
            });
            return;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(25)));
    let _ = stream.set_nodelay(true);

    let _ = ev_tx.send(WorkerEvent::Status {
        port_id: cfg.id.clone(),
        is_open: true,
        message: format!("Connected to {}", addr),
    });

    let mut rx_accum = String::new();
    let mut buf = [0u8; 1024];

    while !stop.load(Ordering::Relaxed) {
        while let Ok(msg) = tx_rx.try_recv() {
            if cfg.direction.can_tx() {
                if stream.write_all(msg.as_bytes()).is_ok() {
                    let _ = ev_tx.send(WorkerEvent::TxSent {
                        port_id: cfg.id.clone(),
                        bytes: msg.len(),
                    });
                }
            }
        }

        if cfg.direction.can_rx() {
            if let Ok(n) = stream.read(&mut buf) {
                if n > 0 {
                    rx_accum.push_str(&String::from_utf8_lossy(&buf[..n]));
                    for line in extract_complete_lines(&mut rx_accum) {
                        let _ = ev_tx.send(WorkerEvent::RxLine {
                            port_id: cfg.id.clone(),
                            port_name: cfg.name.clone(),
                            line,
                        });
                    }
                }
            }
        } else {
            thread::sleep(Duration::from_millis(20));
        }
    }
}

fn run_tcp_server_worker(
    cfg: PortConfig,
    stop: Arc<AtomicBool>,
    tx_rx: Receiver<String>,
    ev_tx: Sender<WorkerEvent>,
) {
    let bind_addr = format!("{}:{}", cfg.local_bind_host, cfg.local_bind_port);
    let listener = match TcpListener::bind(&bind_addr) {
        Ok(l) => l,
        Err(e) => {
            let _ = ev_tx.send(WorkerEvent::Status {
                port_id: cfg.id,
                is_open: false,
                message: format!("TCP bind failed: {}", e),
            });
            return;
        }
    };
    let _ = listener.set_nonblocking(true);

    let _ = ev_tx.send(WorkerEvent::Status {
        port_id: cfg.id.clone(),
        is_open: true,
        message: format!("Listening on {} (0 clients)", bind_addr),
    });

    let mut clients: Vec<TcpStream> = Vec::new();

    while !stop.load(Ordering::Relaxed) {
        if let Ok((client, _addr)) = listener.accept() {
            let _ = client.set_nonblocking(true);
            let _ = client.set_nodelay(true);
            if cfg.direction.can_rx() {
                if let Ok(reader_stream) = client.try_clone() {
                    let _ = reader_stream.set_nonblocking(false);
                    let _ = reader_stream.set_read_timeout(Some(Duration::from_millis(100)));
                    let ev_clone = ev_tx.clone();
                    let stop_c = stop.clone();
                    let pid = cfg.id.clone();
                    let pname = cfg.name.clone();
                    thread::spawn(move || {
                        let mut reader = BufReader::new(reader_stream);
                        let mut line = String::new();
                        while !stop_c.load(Ordering::Relaxed) {
                            line.clear();
                            if let Ok(n) = reader.read_line(&mut line) {
                                if n == 0 {
                                    break;
                                }
                                let _ = ev_clone.send(WorkerEvent::RxLine {
                                    port_id: pid.clone(),
                                    port_name: pname.clone(),
                                    line: line.trim().to_string(),
                                });
                            }
                        }
                    });
                }
            }
            clients.push(client);
            let _ = ev_tx.send(WorkerEvent::Status {
                port_id: cfg.id.clone(),
                is_open: true,
                message: format!("Listening on {} ({} clients)", bind_addr, clients.len()),
            });
        }

        while let Ok(msg) = tx_rx.try_recv() {
            if cfg.direction.can_tx() && !clients.is_empty() {
                let bytes = msg.as_bytes();
                let mut sent_any = false;
                clients.retain_mut(|c| {
                    if c.write_all(bytes).is_ok() {
                        sent_any = true;
                        true
                    } else {
                        false
                    }
                });
                if sent_any {
                    let _ = ev_tx.send(WorkerEvent::TxSent {
                        port_id: cfg.id.clone(),
                        bytes: bytes.len(),
                    });
                }
            }
        }

        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_kind_and_direction_methods() {
        for k in PortKind::ALL {
            assert!(!k.label().is_empty());
        }
        assert!(PortDirection::TxOnly.can_tx());
        assert!(!PortDirection::TxOnly.can_rx());
        assert!(!PortDirection::RxOnly.can_tx());
        assert!(PortDirection::RxOnly.can_rx());
        assert!(PortDirection::Bidirectional.can_tx());
        assert!(PortDirection::Bidirectional.can_rx());
        assert!(!PortDirection::Bidirectional.label().is_empty());

        for grp in IecMulticastGroup::ALL {
            assert!(!grp.code().is_empty());
            assert!(grp.ip().starts_with("239.192.0."));
            assert!((60001..=60008).contains(&grp.port()));
            assert!(!grp.label().is_empty());
        }
        assert_eq!(IecMulticastGroup::for_device_name("AIS / ARPA"), IecMulticastGroup::Tgtd);
        assert_eq!(IecMulticastGroup::for_device_name("GPS / GNSS"), IecMulticastGroup::Navd);
        assert_eq!(IecMulticastGroup::for_device_name("Gyro & Compass"), IecMulticastGroup::Satd);
        assert_eq!(IecMulticastGroup::for_device_name("Engine & Autopilot"), IecMulticastGroup::Prop);
        assert_eq!(IecMulticastGroup::for_device_name("Wind & Weather"), IecMulticastGroup::Misc);
    }

    #[test]
    fn test_port_config_summary_and_should_route() {
        let mut p = PortConfig::default_ports().remove(0);
        for k in PortKind::ALL {
            p.kind = k;
            assert!(!p.summary_endpoint().is_empty());
        }

        p.enabled = true;
        p.direction = PortDirection::Bidirectional;
        p.pass_nmea0183 = true;
        p.pass_nmea2000 = false;

        assert!(p.should_route(false, &[]));
        assert!(!p.should_route(true, &[]));
        assert!(p.should_route(false, &[p.id.clone()]));
        assert!(!p.should_route(false, &["other-port".to_string()]));

        p.enabled = false;
        assert!(!p.should_route(false, &[]));
    }

    #[test]
    fn test_format_wire_message_and_extract_complete_lines() {
        assert_eq!(format_wire_message("$HCHDT,1.0,T*00", false), "$HCHDT,1.0,T*00\r\n");
        assert_eq!(format_wire_message("$HCHDT,1.0,T*00", true), "$HCHDT,1.0,T*00");

        let mut buf = "$GPRMC,1*00\r\n\n$GPGGA,2*00\r\n$PARTIAL".to_string();
        let extracted = extract_complete_lines(&mut buf);
        assert_eq!(extracted, vec!["$GPRMC,1*00", "$GPGGA,2*00"]);
        assert_eq!(buf, "$PARTIAL");
    }

    #[test]
    fn test_transport_manager_udp_loopback() {
        let mut mgr = TransportManager::default();
        let mut p = PortConfig::default_ports().remove(0);
        p.id = "test-udp-loop".to_string();
        p.enabled = true;
        p.kind = PortKind::UdpUnicast;
        p.direction = PortDirection::Bidirectional;
        p.local_bind_host = "127.0.0.1".to_string();
        p.local_bind_port = 19876;
        p.remote_host = "127.0.0.1".to_string();
        p.remote_port = 19876;

        mgr.sync_ports(&[p.clone()]);
        thread::sleep(Duration::from_millis(40));

        mgr.send_line("test-udp-loop", "$HCHDT,243.5,T*2B", false);
        thread::sleep(Duration::from_millis(60));

        let mut seq = 0;
        let rx = mgr.poll_events(&mut seq);
        // Disable worker cleanly
        mgr.sync_ports(&[]);

        if let Some(st) = mgr.stats.get("test-udp-loop") {
            if st.tx_packets > 0 && !rx.is_empty() {
                assert_eq!(rx[0].raw_line, "$HCHDT,243.5,T*2B");
                assert!(!rx[0].is_nmea2000);
            }
        }
    }
}
