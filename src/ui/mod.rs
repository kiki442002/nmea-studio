pub mod gauges;
pub mod plotter;

use chrono::Local;
use egui::{Color32, Pos2, RichText, ScrollArea, Ui, Vec2};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use rand::Rng;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::model::ais::{AisClass, NavStatus, TargetVessel};
use crate::model::instruments::{
    ErrorInjectionConfig, Instrument, InstrumentTemplate, N2kWireFormat, ProtocolMode,
};
use crate::model::route::Waypoint;
use crate::model::vessel::SteeringMode;
use crate::model::StudioProject;
use crate::protocols::nmea0183::{
    compute_checksum, encode_ais_position_report, encode_ais_static_report, encode_arpa_ttm,
    generate_sentence, wrap_sentence,
};
use crate::protocols::nmea2000::{
    encode_ais_n2k_position, format_n2k_frames, generate_pgn_frames,
};
use crate::protocols::{inspect_packet, DecodedPacketInfo};
use crate::recorder::LogRecorderPlayer;
use crate::transport::{
    IecMulticastGroup, PortConfig, PortDirection, PortKind, TrafficDirection, TrafficLogEntry,
    TransportManager,
};
use crate::ui::gauges::{draw_compass_gauge, draw_depth_and_rudder_panel, draw_wind_gauge};
use crate::ui::plotter::PlotterState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTab {
    HelmAndSensors,
    ChartAndRadar,
    InstrumentsAndPgns,
    PortsAndNetwork,
    AisAndArpa,
    LogRecorderPlayer,
    CustomBuilder,
}

pub struct CustomBuilderState {
    // NMEA 0183 custom builder
    pub nmea0183_body: String,
    pub nmea0183_prefix: char,
    pub nmea0183_corrupt_cs: bool,
    pub nmea0183_periodic: bool,
    pub nmea0183_interval_sec: f64,
    pub nmea0183_elapsed: f64,

    // NMEA 2000 custom builder
    pub n2k_pgn: u32,
    pub n2k_priority: u8,
    pub n2k_source: u8,
    pub n2k_hex_payload: String,
    pub n2k_wire_format: N2kWireFormat,
    pub n2k_periodic: bool,
    pub n2k_interval_sec: f64,
    pub n2k_elapsed: f64,
}

impl Default for CustomBuilderState {
    fn default() -> Self {
        Self {
            nmea0183_body: "GPRMC,123519,A,4821.4800,N,00429.1000,W,007.8,245.0,131126,001.8,W,D"
                .to_string(),
            nmea0183_prefix: '$',
            nmea0183_corrupt_cs: false,
            nmea0183_periodic: false,
            nmea0183_interval_sec: 1.0,
            nmea0183_elapsed: 0.0,

            n2k_pgn: 129025,
            n2k_priority: 2,
            n2k_source: 15,
            n2k_hex_payload: "80 79 D9 1C C0 A1 B2 FD".to_string(),
            n2k_wire_format: N2kWireFormat::YachtDevicesRawSingleLine,
            n2k_periodic: false,
            n2k_interval_sec: 1.0,
            n2k_elapsed: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDialogMode {
    Closed,
    SaveProject,
    LoadProject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntryItem {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectFileDialog {
    pub mode: FileDialogMode,
    pub current_dir: String,
    pub file_name: String,
    pub entries: Vec<DirEntryItem>,
    pub error_message: String,
}

impl Default for ProjectFileDialog {
    fn default() -> Self {
        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .to_string_lossy()
            .to_string();
        let mut dlg = Self {
            mode: FileDialogMode::Closed,
            current_dir: cwd,
            file_name: "nmea_studio_preset.json".to_string(),
            entries: Vec::new(),
            error_message: String::new(),
        };
        dlg.refresh_entries();
        dlg
    }
}

impl ProjectFileDialog {
    pub fn open_save(&mut self, current_path: &str) {
        self.apply_initial_path(current_path);
        self.mode = FileDialogMode::SaveProject;
        self.error_message.clear();
        self.refresh_entries();
    }

    pub fn open_load(&mut self, current_path: &str) {
        self.apply_initial_path(current_path);
        self.mode = FileDialogMode::LoadProject;
        self.error_message.clear();
        self.refresh_entries();
    }

    fn apply_initial_path(&mut self, current_path: &str) {
        let p = Path::new(current_path);
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() && parent.exists() {
                self.current_dir = parent.to_string_lossy().to_string();
            }
        }
        if let Some(fname) = p.file_name() {
            if !fname.is_empty() {
                self.file_name = fname.to_string_lossy().to_string();
            }
        }
    }

    pub fn refresh_entries(&mut self) {
        self.entries.clear();
        let Ok(read_dir) = fs::read_dir(&self.current_dir) else {
            return;
        };
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let is_dir = path.is_dir();
            if is_dir {
                dirs.push(DirEntryItem {
                    name,
                    path: path.to_string_lossy().to_string(),
                    is_dir: true,
                });
            } else if name.to_lowercase().ends_with(".json") {
                files.push(DirEntryItem {
                    name,
                    path: path.to_string_lossy().to_string(),
                    is_dir: false,
                });
            }
        }
        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        self.entries.extend(dirs);
        self.entries.extend(files);
    }

    pub fn navigate_up(&mut self) {
        let p = Path::new(&self.current_dir);
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                self.current_dir = parent.to_string_lossy().to_string();
                self.refresh_entries();
            }
        }
    }

    pub fn navigate_to(&mut self, dir_path: &str) {
        if Path::new(dir_path).is_dir() {
            self.current_dir = dir_path.to_string();
            self.error_message.clear();
            self.refresh_entries();
        }
    }

    pub fn resolved_file_path(&self) -> String {
        let trimmed = self.file_name.trim();
        let fname = if self.mode == FileDialogMode::SaveProject
            && !trimmed.to_lowercase().ends_with(".json")
        {
            format!("{}.json", if trimmed.is_empty() { "preset" } else { trimmed })
        } else {
            trimmed.to_string()
        };
        Path::new(&self.current_dir)
            .join(fname)
            .to_string_lossy()
            .to_string()
    }
}

/// Optional helper to invoke the native OS file picker (macOS Finder via osascript, Linux zenity, Windows PowerShell)
pub fn pick_native_file_dialog(save: bool, initial_dir: &str, default_name: &str) -> Option<String> {
    if cfg!(target_os = "macos") {
        let script = if save {
            format!(
                "POSIX path of (choose file name with prompt \"Save NMEA Studio Preset (.json)\" default name \"{}\" default location (POSIX file \"{}\"))",
                default_name.replace('"', ""),
                initial_dir.replace('"', "")
            )
        } else {
            format!(
                "POSIX path of (choose file with prompt \"Load NMEA Studio Preset (.json)\" default location (POSIX file \"{}\"))",
                initial_dir.replace('"', "")
            )
        };
        if let Ok(out) = Command::new("osascript").arg("-e").arg(&script).output() {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    } else if cfg!(target_os = "linux") {
        let mut cmd = Command::new("zenity");
        cmd.arg("--file-selection")
            .arg("--title=NMEA Studio Preset")
            .arg("--file-filter=*.json");
        if save {
            cmd.arg("--save").arg("--confirm-overwrite");
        }
        if let Ok(out) = cmd.output() {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    } else if cfg!(target_os = "windows") {
        let dialog_class = if save {
            "System.Windows.Forms.SaveFileDialog"
        } else {
            "System.Windows.Forms.OpenFileDialog"
        };
        let ps = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object {}; $d.Filter = 'JSON Preset (*.json)|*.json|All Files (*.*)|*.*'; if ($d.ShowDialog() -eq 'OK') {{ Write-Output $d.FileName }}",
            dialog_class
        );
        if let Ok(out) = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(&ps)
            .output()
        {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    }
    None
}

pub struct NmeaStudioApp {
    pub project: StudioProject,
    pub project_file_path: String,
    pub project_status_msg: String,
    pub selected_preset_idx: usize,
    pub file_dialog: ProjectFileDialog,

    pub sim_running: bool,
    last_frame_instant: Instant,

    pub active_tab: WorkspaceTab,
    pub selected_instrument_idx: usize,
    pub selected_port_idx: usize,
    pub selected_ais_idx: usize,

    pub plotter: PlotterState,
    pub transport: TransportManager,
    pub recorder: LogRecorderPlayer,
    pub custom_builder: CustomBuilderState,

    // Traffic monitor state
    pub traffic_log: Vec<TrafficLogEntry>,
    pub max_traffic_entries: usize,
    pub next_seq_id: u64,
    pub monitor_paused: bool,
    pub filter_show_tx: bool,
    pub filter_show_rx: bool,
    pub filter_show_0183: bool,
    pub filter_show_n2k: bool,
    pub filter_search: String,
    pub selected_packet_seq: Option<u64>,
    pub selected_packet_decoded: Option<DecodedPacketInfo>,
    pub selected_packet_raw: String,
}

impl Default for NmeaStudioApp {
    fn default() -> Self {
        let project = StudioProject::default();
        let mut transport = TransportManager::default();
        transport.sync_ports(&project.ports);

        Self {
            project,
            project_file_path: "nmea_studio_preset.json".to_string(),
            project_status_msg: "Empty project ready".to_string(),
            selected_preset_idx: 0,
            file_dialog: ProjectFileDialog::default(),
            sim_running: true,
            last_frame_instant: Instant::now(),
            active_tab: WorkspaceTab::HelmAndSensors,
            selected_instrument_idx: 0,
            selected_port_idx: 0,
            selected_ais_idx: 0,
            plotter: PlotterState::default(),
            transport,
            recorder: LogRecorderPlayer::default(),
            custom_builder: CustomBuilderState::default(),
            traffic_log: Vec::with_capacity(2000),
            max_traffic_entries: 1500,
            next_seq_id: 0,
            monitor_paused: false,
            filter_show_tx: true,
            filter_show_rx: true,
            filter_show_0183: true,
            filter_show_n2k: true,
            filter_search: String::new(),
            selected_packet_seq: None,
            selected_packet_decoded: None,
            selected_packet_raw: String::new(),
        }
    }
}

impl NmeaStudioApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_studio_theme(&cc.egui_ctx);
        Self::default()
    }

    pub fn save_project_to_path(&mut self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.project)
            .map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(path, json).map_err(|e| format!("Save error: {}", e))?;
        self.project_file_path = path.to_string();
        self.project_status_msg = format!("Saved preset to {}", path);
        Ok(())
    }

    pub fn load_project_from_path(&mut self, path: &str) -> Result<(), String> {
        let json = fs::read_to_string(path).map_err(|e| format!("Read error: {}", e))?;
        let proj = serde_json::from_str::<StudioProject>(&json)
            .map_err(|e| format!("Parse error: {}", e))?;
        self.project = proj;
        self.transport.sync_ports(&self.project.ports);
        self.project_file_path = path.to_string();
        self.project_status_msg = format!("Loaded preset from {}", path);
        Ok(())
    }

    pub fn dispatch_generated_line(
        &mut self,
        source_name: &str,
        is_nmea2000: bool,
        target_ports: &[String],
        omit_crlf: bool,
        line: String,
    ) {
        let timestamp = Local::now().format("%H:%M:%S%.3f").to_string();
        let mut routed_port_names = Vec::new();

        for p in &self.project.ports {
            if !p.should_route(is_nmea2000, target_ports) {
                continue;
            }
            self.transport.send_line(&p.id, &line, omit_crlf);
            routed_port_names.push(p.name.clone());
        }

        self.recorder.record_line(&line, true);

        let port_label = if routed_port_names.is_empty() {
            "Internal Bus".to_string()
        } else if routed_port_names.len() == 1 {
            routed_port_names[0].clone()
        } else {
            format!("{} Ports", routed_port_names.len())
        };

        self.next_seq_id += 1;
        let entry = TrafficLogEntry {
            seq_id: self.next_seq_id,
            timestamp,
            port_id: "tx".to_string(),
            port_name: port_label,
            source_name: source_name.to_string(),
            direction: TrafficDirection::Tx,
            is_nmea2000,
            raw_line: line.clone(),
        };

        if self.selected_packet_seq.is_none() {
            self.selected_packet_decoded = Some(inspect_packet(&line));
            self.selected_packet_raw = line;
        }

        if !self.monitor_paused {
            self.traffic_log.push(entry);
            if self.traffic_log.len() > self.max_traffic_entries {
                let drain_count = self.traffic_log.len() - self.max_traffic_entries;
                self.traffic_log.drain(0..drain_count);
            }
        }
    }

    pub fn step_simulation(&mut self, dt_raw_sec: f64) {
        let dt = (dt_raw_sec * self.project.sim_speed_multiplier).clamp(0.0, 2.0);
        if dt <= 0.0 {
            return;
        }

        // 1. Route & Autopilot update
        let ap_bearing = self.project.route.step_navigation(
            self.project.vessel.latitude,
            self.project.vessel.longitude,
            self.project.vessel.sog_knots,
            self.project.vessel.cog_deg,
        );

        // 2. Own ship kinematics
        self.project.vessel.step(dt, ap_bearing);

        // 3. Step Instruments & collect due NMEA 0183 sentences and NMEA 2000 PGN frames
        let mut pending_out: Vec<(String, bool, Vec<String>, bool, String)> = Vec::new();

        for inst in &mut self.project.instruments {
            if !inst.enabled {
                continue;
            }
            let emit_0183 = matches!(
                inst.protocol_mode,
                ProtocolMode::Nmea0183Only | ProtocolMode::Dual0183And2000
            );
            let emit_2000 = matches!(
                inst.protocol_mode,
                ProtocolMode::Nmea2000Only | ProtocolMode::Dual0183And2000
            );

            if emit_0183 {
                for sched in &mut inst.sentences_0183 {
                    if !sched.enabled {
                        continue;
                    }
                    sched.elapsed_sec += dt_raw_sec;
                    if sched.elapsed_sec >= sched.interval_sec.max(0.05) {
                        sched.elapsed_sec = 0.0;
                        let lines = generate_sentence(
                            &inst.talker_id,
                            sched.kind,
                            &self.project.vessel,
                            &self.project.route,
                            &inst.error_injection,
                        );
                        for l in lines {
                            pending_out.push((
                                inst.name.clone(),
                                false,
                                inst.target_ports.clone(),
                                inst.error_injection.omit_crlf,
                                l,
                            ));
                        }
                    }
                }
            }

            if emit_2000 {
                let mut due_pgns = Vec::new();
                for psched in &mut inst.pgns_2000 {
                    if !psched.enabled {
                        continue;
                    }
                    psched.elapsed_sec += dt_raw_sec;
                    if psched.elapsed_sec >= psched.interval_sec.max(0.05) {
                        psched.elapsed_sec = 0.0;
                        due_pgns.push(psched.kind);
                    }
                }
                for pkind in due_pgns {
                    let sid = inst.next_sid();
                    let fseq = inst.next_fast_seq();
                    let frames = generate_pgn_frames(
                        pkind,
                        sid,
                        fseq,
                        inst.n2k_source_addr,
                        inst.n2k_wire_format,
                        &self.project.vessel,
                        &self.project.route,
                        &inst.error_injection,
                    );
                    for f in frames {
                        pending_out.push((
                            inst.name.clone(),
                            true,
                            inst.target_ports.clone(),
                            inst.error_injection.omit_crlf,
                            f,
                        ));
                    }
                }
            }
        }

        // 4. Step AIS & ARPA Targets
        if self.project.ais_config.enabled {
            let ais_ports = self.project.ais_config.target_ports.clone();
            if self.project.ais_config.emit_own_ship_vdo {
                self.project.ais_config.vdo_elapsed_sec += dt_raw_sec;
                if self.project.ais_config.vdo_elapsed_sec
                    >= self.project.ais_config.vdo_interval_sec.max(0.5)
                {
                    self.project.ais_config.vdo_elapsed_sec = 0.0;
                    let vdo = encode_ais_position_report(
                        true,
                        self.project.vessel.mmsi,
                        0,
                        self.project.vessel.rot_deg_per_min,
                        self.project.vessel.sog_knots,
                        self.project.vessel.latitude,
                        self.project.vessel.longitude,
                        self.project.vessel.cog_deg,
                        self.project.vessel.heading_true_deg,
                        false,
                    );
                    pending_out.push((
                        "Own Ship AIS (!AIVDO)".to_string(),
                        false,
                        ais_ports.clone(),
                        false,
                        vdo,
                    ));
                }
            }

            for tgt in &mut self.project.ais_targets {
                if !tgt.enabled {
                    continue;
                }
                tgt.step(dt);
                if tgt.time_since_dynamic_sec >= tgt.report_interval_sec.max(0.5) {
                    tgt.time_since_dynamic_sec = 0.0;

                    if self.project.ais_config.emit_nmea0183_vdm
                        && tgt.ais_class != AisClass::ArpaOnly
                    {
                        let vdm = encode_ais_position_report(
                            false,
                            tgt.mmsi,
                            tgt.nav_status.code(),
                            tgt.rot_deg_per_min,
                            tgt.sog_knots,
                            tgt.latitude,
                            tgt.longitude,
                            tgt.cog_deg,
                            tgt.heading_deg,
                            tgt.ais_class == AisClass::ClassB,
                        );
                        pending_out.push((
                            format!("AIS: {}", tgt.name),
                            false,
                            ais_ports.clone(),
                            false,
                            vdm,
                        ));
                    }

                    if tgt.emit_arpa_ttm || tgt.ais_class == AisClass::ArpaOnly {
                        let ttm = encode_arpa_ttm(tgt, &self.project.vessel);
                        pending_out.push((
                            format!("ARPA: {}", tgt.name),
                            false,
                            ais_ports.clone(),
                            false,
                            ttm,
                        ));
                    }

                    if self.project.ais_config.emit_nmea2000_pgns
                        && tgt.ais_class != AisClass::ArpaOnly
                    {
                        self.project.ais_config.fast_seq =
                            (self.project.ais_config.fast_seq + 1) & 0x07;
                        let frames = encode_ais_n2k_position(
                            tgt,
                            self.project.ais_config.n2k_source_addr,
                            self.project.ais_config.fast_seq,
                            self.project.ais_config.n2k_wire_format,
                        );
                        for f in frames {
                            pending_out.push((
                                format!("AIS N2K: {}", tgt.name),
                                true,
                                ais_ports.clone(),
                                false,
                                f,
                            ));
                        }
                    }
                }

                // Emit static AIS report (Msg 5 / Msg 24) every 15 seconds
                if tgt.time_since_static_sec >= 15.0 {
                    tgt.time_since_static_sec = 0.0;
                    if self.project.ais_config.emit_nmea0183_vdm {
                        for s in encode_ais_static_report(tgt) {
                            pending_out.push((
                                format!("AIS Static: {}", tgt.name),
                                false,
                                ais_ports.clone(),
                                false,
                                s,
                            ));
                        }
                    }
                }
            }
        }

        // 5. Custom Builder periodic transmitters
        if self.custom_builder.nmea0183_periodic {
            self.custom_builder.nmea0183_elapsed += dt_raw_sec;
            if self.custom_builder.nmea0183_elapsed
                >= self.custom_builder.nmea0183_interval_sec.max(0.1)
            {
                self.custom_builder.nmea0183_elapsed = 0.0;
                let err = ErrorInjectionConfig {
                    corrupt_checksum: self.custom_builder.nmea0183_corrupt_cs,
                    omit_checksum: false,
                    omit_crlf: false,
                };
                let line = wrap_sentence(
                    self.custom_builder.nmea0183_prefix,
                    &self.custom_builder.nmea0183_body,
                    &err,
                );
                pending_out.push(("Custom 0183".to_string(), false, Vec::new(), false, line));
            }
        }

        if self.custom_builder.n2k_periodic {
            self.custom_builder.n2k_elapsed += dt_raw_sec;
            if self.custom_builder.n2k_elapsed >= self.custom_builder.n2k_interval_sec.max(0.1) {
                self.custom_builder.n2k_elapsed = 0.0;
                let payload = parse_hex_bytes(&self.custom_builder.n2k_hex_payload);
                let frames = format_n2k_frames(
                    self.custom_builder.n2k_pgn,
                    self.custom_builder.n2k_priority,
                    self.custom_builder.n2k_source,
                    payload.len() > 8,
                    0,
                    &payload,
                    self.custom_builder.n2k_wire_format,
                    &ErrorInjectionConfig::default(),
                );
                for f in frames {
                    pending_out.push(("Custom N2K".to_string(), true, Vec::new(), false, f));
                }
            }
        }

        for (src, is_n2k, ports, omit_crlf, line) in pending_out {
            self.dispatch_generated_line(&src, is_n2k, &ports, omit_crlf, line);
        }
    }
}

pub fn parse_hex_bytes(s: &str) -> Vec<u8> {
    crate::protocols::nmea2000::parse_hex_stream(s)
}

pub fn matches_traffic_filter(
    e: &TrafficLogEntry,
    show_tx: bool,
    show_rx: bool,
    show_0183: bool,
    show_n2k: bool,
    search_upper: &str,
) -> bool {
    if e.direction == TrafficDirection::Tx && !show_tx {
        return false;
    }
    if e.direction == TrafficDirection::Rx && !show_rx {
        return false;
    }
    if e.is_nmea2000 && !show_n2k {
        return false;
    }
    if !e.is_nmea2000 && !show_0183 {
        return false;
    }
    if !search_upper.is_empty()
        && !e.raw_line.to_uppercase().contains(search_upper)
        && !e.source_name.to_uppercase().contains(search_upper)
    {
        return false;
    }
    true
}

/// Generates the NMEA Studio application icon (RGBA8 pixel buffer) at the requested square resolution.
/// Renders an anti-aliased nautical radar & 8-point compass rose badge inside a rounded squircle.
pub fn generate_app_logo_rgba(size: u32) -> egui::IconData {
    let size = size.max(16);
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;
    let half = s * 0.5;

    // Helper: signed distance to a 2D line segment (ax, ay) -> (bx, by)
    let dist_to_segment = |px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32| -> f32 {
        let vx = bx - ax;
        let vy = by - ay;
        let wx = px - ax;
        let wy = py - ay;
        let len_sq = vx * vx + vy * vy;
        let t = if len_sq > 1e-6 {
            ((wx * vx + wy * vy) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dx = px - (ax + t * vx);
        let dy = py - (ay + t * vy);
        (dx * dx + dy * dy).sqrt()
    };

    // Helper: point-in-triangle with anti-aliased edge distance
    let triangle_coverage =
        |px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32| -> f32 {
            let e0 = (px - bx) * (ay - by) - (ax - bx) * (py - by);
            let e1 = (px - cx) * (by - cy) - (bx - cx) * (py - cy);
            let e2 = (px - ax) * (cy - ay) - (cx - ax) * (py - ay);
            let inside = (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0)
                || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0);
            let d0 = dist_to_segment(px, py, ax, ay, bx, by);
            let d1 = dist_to_segment(px, py, bx, by, cx, cy);
            let d2 = dist_to_segment(px, py, cx, cy, ax, ay);
            let min_d = d0.min(d1).min(d2);
            let signed_d = if inside { min_d } else { -min_d };
            (signed_d * half + 0.5).clamp(0.0, 1.0)
        };

    let blend = |dst: &mut [u8; 4], sr: f32, sg: f32, sb: f32, sa: f32| {
        let sa = sa.clamp(0.0, 1.0);
        if sa <= 0.0 {
            return;
        }
        let da = dst[3] as f32 / 255.0;
        let out_a = sa + da * (1.0 - sa);
        if out_a > 1e-5 {
            let dr = dst[0] as f32 / 255.0;
            let dg = dst[1] as f32 / 255.0;
            let db = dst[2] as f32 / 255.0;
            let or = (sr * sa + dr * da * (1.0 - sa)) / out_a;
            let og = (sg * sa + dg * da * (1.0 - sa)) / out_a;
            let ob = (sb * sa + db * da * (1.0 - sa)) / out_a;
            dst[0] = (or.clamp(0.0, 1.0) * 255.0).round() as u8;
            dst[1] = (og.clamp(0.0, 1.0) * 255.0).round() as u8;
            dst[2] = (ob.clamp(0.0, 1.0) * 255.0).round() as u8;
            dst[3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    };

    for y in 0..size {
        for x in 0..size {
            // Normalized coordinates in [-1.0, 1.0], +x right, +y down
            let nx = ((x as f32 + 0.5) - half) / half;
            let ny = ((y as f32 + 0.5) - half) / half;
            let r = (nx * nx + ny * ny).sqrt();

            // Rounded squircle SDF
            let box_half = 0.88;
            let corner_r = 0.24;
            let qx = nx.abs() - (box_half - corner_r);
            let qy = ny.abs() - (box_half - corner_r);
            let squircle_sdf =
                (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - corner_r;

            let aa = 2.0 / s;
            let bg_alpha = ((0.0 - squircle_sdf) / aa + 0.5).clamp(0.0, 1.0);
            if bg_alpha <= 0.0 {
                continue;
            }

            let mut pix = [0u8; 4];

            // 1. Deep Nautical Midnight-Blue radial background gradient
            let vignette = (1.0 - (r * 0.72).clamp(0.0, 1.0)).powf(1.2);
            let bg_r = 0.02 + 0.06 * vignette;
            let bg_g = 0.07 + 0.16 * vignette;
            let bg_b = 0.14 + 0.28 * vignette;
            blend(&mut pix, bg_r, bg_g, bg_b, bg_alpha);

            // 2. Outer squircle metallic cyan rim
            let rim_dist = (squircle_sdf + 0.025).abs();
            let rim_alpha = ((0.022 - rim_dist) / aa).clamp(0.0, 1.0) * bg_alpha;
            blend(&mut pix, 0.20, 0.78, 0.96, rim_alpha * 0.85);

            // 3. Radar sweep glow in top-right sector (-75° to 0° in math angle -> NE quadrant)
            if r < 0.74 {
                let bearing_deg = nx.atan2(-ny).to_degrees().rem_euclid(360.0);
                if bearing_deg <= 72.0 {
                    let sweep_t = (bearing_deg / 72.0).powf(1.5);
                    let radial_fade = (1.0 - (r / 0.74)).clamp(0.0, 1.0).powf(0.6);
                    blend(
                        &mut pix,
                        0.08,
                        0.88,
                        0.72,
                        sweep_t * radial_fade * 0.34 * bg_alpha,
                    );
                }
            }

            // 4. Concentric radar range rings (r = 0.74, 0.52, 0.30)
            for &(ring_r, thick, alpha_mul) in &[
                (0.74_f32, 0.016_f32, 0.90_f32),
                (0.52_f32, 0.011_f32, 0.50_f32),
                (0.30_f32, 0.010_f32, 0.40_f32),
            ] {
                let d_ring = (r - ring_r).abs();
                let a_ring = ((thick - d_ring) / aa).clamp(0.0, 1.0) * alpha_mul * bg_alpha;
                blend(&mut pix, 0.22, 0.82, 1.0, a_ring);
            }

            // 5. Compass degree ticks along the outer radar ring (every 30 degrees)
            if (0.66..=0.75).contains(&r) {
                let deg = nx.atan2(-ny).to_degrees().rem_euclid(360.0);
                let nearest_30 = (deg / 30.0).round() * 30.0;
                let angle_rad = nearest_30.to_radians();
                let tx0 = angle_rad.sin() * 0.67;
                let ty0 = -angle_rad.cos() * 0.67;
                let tx1 = angle_rad.sin() * 0.74;
                let ty1 = -angle_rad.cos() * 0.74;
                let d_tick = dist_to_segment(nx, ny, tx0, ty0, tx1, ty1);
                let a_tick = ((0.012 - d_tick) / aa).clamp(0.0, 1.0) * 0.75 * bg_alpha;
                blend(&mut pix, 0.55, 0.92, 1.0, a_tick);
            }

            // 6. Subtle crosshair lines
            if r < 0.74 {
                let d_cross = nx.abs().min(ny.abs());
                let a_cross = ((0.008 - d_cross) / aa).clamp(0.0, 1.0) * 0.28 * bg_alpha;
                blend(&mut pix, 0.30, 0.85, 1.0, a_cross);
            }

            // 7. Intercardinal 4-point star (NE, SE, SW, NW) at 45°, length 0.44
            for &deg in &[45.0_f32, 135.0, 225.0, 315.0] {
                let rad = deg.to_radians();
                let tip_x = rad.sin() * 0.45;
                let tip_y = -rad.cos() * 0.45;
                let left_rad = (deg - 90.0).to_radians();
                let right_rad = (deg + 90.0).to_radians();
                let w = 0.085;
                let cov_l = triangle_coverage(
                    nx,
                    ny,
                    0.0,
                    0.0,
                    tip_x,
                    tip_y,
                    left_rad.sin() * w,
                    -left_rad.cos() * w,
                );
                let cov_r = triangle_coverage(
                    nx,
                    ny,
                    0.0,
                    0.0,
                    tip_x,
                    tip_y,
                    right_rad.sin() * w,
                    -right_rad.cos() * w,
                );
                blend(&mut pix, 0.18, 0.68, 0.86, cov_l * 0.85 * bg_alpha);
                blend(&mut pix, 0.09, 0.42, 0.62, cov_r * 0.85 * bg_alpha);
            }

            // 8. Principal 4-point Cardinal Compass Rose (N, E, S, W), length 0.68
            // East, South, West pointers in dual-tone cyan/white & deep teal
            for &deg in &[90.0_f32, 180.0, 270.0] {
                let rad = deg.to_radians();
                let tip_x = rad.sin() * 0.66;
                let tip_y = -rad.cos() * 0.66;
                let left_rad = (deg - 45.0).to_radians();
                let right_rad = (deg + 45.0).to_radians();
                let w = 0.145;
                let cov_l = triangle_coverage(
                    nx,
                    ny,
                    0.0,
                    0.0,
                    tip_x,
                    tip_y,
                    left_rad.sin() * w,
                    -left_rad.cos() * w,
                );
                let cov_r = triangle_coverage(
                    nx,
                    ny,
                    0.0,
                    0.0,
                    tip_x,
                    tip_y,
                    right_rad.sin() * w,
                    -right_rad.cos() * w,
                );
                blend(&mut pix, 0.88, 0.97, 1.00, cov_l * bg_alpha);
                blend(&mut pix, 0.18, 0.62, 0.84, cov_r * bg_alpha);
            }

            // North pointer (0°) in high-visibility Marine Coral-Orange / Gold so North stands out!
            {
                let tip_x = 0.0;
                let tip_y = -0.68;
                let left_rad = (-45.0_f32).to_radians();
                let right_rad = (45.0_f32).to_radians();
                let w = 0.155;
                let cov_l = triangle_coverage(
                    nx,
                    ny,
                    0.0,
                    0.0,
                    tip_x,
                    tip_y,
                    left_rad.sin() * w,
                    -left_rad.cos() * w,
                );
                let cov_r = triangle_coverage(
                    nx,
                    ny,
                    0.0,
                    0.0,
                    tip_x,
                    tip_y,
                    right_rad.sin() * w,
                    -right_rad.cos() * w,
                );
                blend(&mut pix, 1.00, 0.46, 0.18, cov_l * bg_alpha);
                blend(&mut pix, 0.86, 0.24, 0.08, cov_r * bg_alpha);
            }

            // 9. Radar AIS target blip in the NE sweep sector (at (0.37, -0.32))
            let blip_d = ((nx - 0.37).powi(2) + (ny + 0.32).powi(2)).sqrt();
            let blip_glow = ((0.085 - blip_d) / 0.085).clamp(0.0, 1.0).powi(2);
            blend(&mut pix, 0.20, 1.0, 0.55, blip_glow * 0.75 * bg_alpha);
            let blip_core = ((0.036 - blip_d) / aa).clamp(0.0, 1.0);
            blend(&mut pix, 0.75, 1.0, 0.88, blip_core * bg_alpha);

            // 10. Center compass hub ring & jewel
            let hub_outer = ((0.095 - r) / aa).clamp(0.0, 1.0);
            blend(&mut pix, 0.05, 0.14, 0.24, hub_outer * bg_alpha);
            let hub_inner = ((0.062 - r) / aa).clamp(0.0, 1.0);
            blend(&mut pix, 0.30, 0.90, 1.00, hub_inner * bg_alpha);
            let hub_dot = ((0.025 - r) / aa).clamp(0.0, 1.0);
            blend(&mut pix, 1.0, 1.0, 1.0, hub_dot * bg_alpha);

            let idx = ((y * size + x) * 4) as usize;
            rgba[idx..idx + 4].copy_from_slice(&pix);
        }
    }

    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

/// Draws a compact vector NMEA Studio compass-radar emblem directly inside the UI toolbar.
pub fn draw_app_logo_badge(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect);
    let c = rect.center();
    let r = size * 0.46;

    // Rounded badge background
    painter.rect_filled(
        rect.shrink(1.0),
        egui::CornerRadius::same((size * 0.22) as u8),
        Color32::from_rgb(10, 28, 48),
    );
    painter.rect_stroke(
        rect.shrink(1.0),
        egui::CornerRadius::same((size * 0.22) as u8),
        egui::Stroke::new(1.2, Color32::from_rgb(50, 195, 245)),
        egui::StrokeKind::Inside,
    );

    // Outer radar ring
    painter.circle_stroke(
        c,
        r * 0.80,
        egui::Stroke::new(1.0, Color32::from_rgb(45, 175, 225)),
    );

    // South / East / West compass points
    let w = r * 0.22;
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(c.x, c.y + r * 0.78),
            Pos2::new(c.x - w, c.y),
            Pos2::new(c.x + w, c.y),
        ],
        Color32::from_rgb(210, 240, 255),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(c.x - r * 0.75, c.y),
            Pos2::new(c.x, c.y - w * 0.85),
            Pos2::new(c.x, c.y + w * 0.85),
        ],
        Color32::from_rgb(75, 195, 240),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(c.x + r * 0.75, c.y),
            Pos2::new(c.x, c.y - w * 0.85),
            Pos2::new(c.x, c.y + w * 0.85),
        ],
        Color32::from_rgb(75, 195, 240),
        egui::Stroke::NONE,
    ));

    // North pointer in bright coral-orange
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(c.x, c.y - r * 0.82),
            Pos2::new(c.x + w, c.y),
            Pos2::new(c.x - w, c.y),
        ],
        Color32::from_rgb(255, 110, 45),
        egui::Stroke::NONE,
    ));

    // Center hub
    painter.circle_filled(c, r * 0.18, Color32::from_rgb(80, 230, 255));
}

/// Encodes an `egui::IconData` (RGBA8) into a standard PNG image byte buffer (using uncompressed zlib blocks).
pub fn encode_rgba_to_png(icon: &egui::IconData) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    fn write_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(data);
        let mut crc_buf = Vec::with_capacity(4 + data.len());
        crc_buf.extend_from_slice(tag);
        crc_buf.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_buf).to_be_bytes());
    }

    let w = icon.width;
    let h = icon.height;
    let mut raw_scanlines = Vec::with_capacity(((w * 4 + 1) * h) as usize);
    for y in 0..h {
        raw_scanlines.push(0u8); // Filter type 0 (None)
        let row_start = (y * w * 4) as usize;
        let row_end = row_start + (w * 4) as usize;
        raw_scanlines.extend_from_slice(&icon.rgba[row_start..row_end]);
    }

    // Wrap raw_scanlines in valid zlib stream (stored blocks of up to 65535 bytes + Adler32)
    let mut zlib = Vec::with_capacity(raw_scanlines.len() + 64);
    zlib.push(0x78);
    zlib.push(0x01);
    let chunks: Vec<&[u8]> = raw_scanlines.chunks(65_535).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i + 1 == chunks.len();
        zlib.push(if is_last { 0x01 } else { 0x00 });
        let len = chunk.len() as u16;
        let nlen = !len;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&nlen.to_le_bytes());
        zlib.extend_from_slice(chunk);
    }
    // Compute Adler-32 checksum
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in &raw_scanlines {
        s1 = (s1 + b as u32) % 65_521;
        s2 = (s2 + s1) % 65_521;
    }
    let adler = (s2 << 16) | s1;
    zlib.extend_from_slice(&adler.to_be_bytes());

    let mut png = Vec::with_capacity(zlib.len() + 128);
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]); // PNG signature

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &zlib);
    write_chunk(&mut png, b"IEND", &[]);
    png
}

/// Encodes a multi-resolution Windows `.ico` file from `generate_app_logo_rgba` for the given sizes (e.g. `&[16, 32, 48, 64, 256]`).
pub fn encode_app_logo_ico(sizes: &[u32]) -> Vec<u8> {
    let mut payloads: Vec<(u32, Vec<u8>)> = Vec::with_capacity(sizes.len());
    for &sz in sizes {
        let icon = generate_app_logo_rgba(sz);
        // 40-byte BITMAPINFOHEADER + bottom-up BGRA pixels + 32-bit aligned 1-bit AND mask
        let w = sz as i32;
        let h = (sz * 2) as i32; // doubled height in ICO DIB header
        let xor_bytes = (sz * sz * 4) as usize;
        let mask_row_bytes = ((sz + 31) / 32 * 4) as usize;
        let and_bytes = mask_row_bytes * (sz as usize);
        let mut dib = Vec::with_capacity(40 + xor_bytes + and_bytes);

        dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
        dib.extend_from_slice(&w.to_le_bytes()); // biWidth
        dib.extend_from_slice(&h.to_le_bytes()); // biHeight
        dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
        dib.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
        dib.extend_from_slice(&(xor_bytes as u32).to_le_bytes()); // biSizeImage
        dib.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
        dib.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
        dib.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        dib.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

        // Bottom-up BGRA pixel rows
        for y in (0..sz).rev() {
            for x in 0..sz {
                let idx = ((y * sz + x) * 4) as usize;
                let r = icon.rgba[idx];
                let g = icon.rgba[idx + 1];
                let b = icon.rgba[idx + 2];
                let a = icon.rgba[idx + 3];
                dib.extend_from_slice(&[b, g, r, a]);
            }
        }

        // Bottom-up 1-bit AND transparency mask (1 = transparent when alpha == 0)
        for y in (0..sz).rev() {
            let mut row_mask = vec![0u8; mask_row_bytes];
            for x in 0..sz {
                let idx = ((y * sz + x) * 4) as usize;
                if icon.rgba[idx + 3] == 0 {
                    row_mask[(x / 8) as usize] |= 0x80 >> (x % 8);
                }
            }
            dib.extend_from_slice(&row_mask);
        }

        payloads.push((sz, dib));
    }

    let count = payloads.len() as u16;
    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // Type = 1 (ICO)
    ico.extend_from_slice(&count.to_le_bytes()); // Image count

    let mut offset = 6 + (payloads.len() as u32) * 16;
    for (sz, data) in &payloads {
        let dim_byte = if *sz >= 256 { 0u8 } else { *sz as u8 };
        ico.push(dim_byte); // width
        ico.push(dim_byte); // height
        ico.push(0); // color count
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // color planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        ico.extend_from_slice(&(data.len() as u32).to_le_bytes()); // size
        ico.extend_from_slice(&offset.to_le_bytes()); // offset
        offset += data.len() as u32;
    }

    for (_, data) in payloads {
        ico.extend_from_slice(&data);
    }
    ico
}

/// Ultra-high-contrast semantic color palette that adapts automatically to the OS theme (Day Mode / Night Mode).
#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub dark: bool,
    pub accent: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub positive: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub ais_cyan: Color32,
    pub nmea0183_teal: Color32,
    pub card_fill: Color32,
}

impl ThemePalette {
    pub fn from_ui(ui: &Ui) -> Self {
        Self::for_dark(ui.visuals().dark_mode)
    }

    pub fn for_dark(dark: bool) -> Self {
        if dark {
            Self {
                dark: true,
                accent: Color32::from_rgb(80, 225, 255),
                text_primary: Color32::from_rgb(255, 255, 255),
                text_secondary: Color32::from_rgb(235, 245, 255),
                text_muted: Color32::from_rgb(200, 220, 245),
                positive: Color32::from_rgb(45, 255, 130),
                warning: Color32::from_rgb(255, 215, 40),
                danger: Color32::from_rgb(255, 75, 75),
                ais_cyan: Color32::from_rgb(70, 245, 255),
                nmea0183_teal: Color32::from_rgb(100, 255, 205),
                card_fill: Color32::from_rgb(12, 18, 28),
            }
        } else {
            Self {
                dark: false,
                accent: Color32::from_rgb(0, 72, 145),
                text_primary: Color32::from_rgb(0, 0, 0),
                text_secondary: Color32::from_rgb(10, 15, 28),
                text_muted: Color32::from_rgb(25, 35, 52),
                positive: Color32::from_rgb(0, 105, 35),
                warning: Color32::from_rgb(150, 55, 0),
                danger: Color32::from_rgb(180, 0, 10),
                ais_cyan: Color32::from_rgb(0, 88, 118),
                nmea0183_teal: Color32::from_rgb(0, 96, 80),
                card_fill: Color32::from_rgb(255, 255, 255),
            }
        }
    }
}

pub fn configure_studio_theme(ctx: &egui::Context) {
    // Keep Day/Night theme automatically linked to the OS system theme in real time
    ctx.set_theme(egui::ThemePreference::System);

    // 1. Ultra-high-contrast Night Mode (Dark) visuals
    let mut dark = egui::Visuals::dark();
    dark.override_text_color = Some(Color32::from_rgb(255, 255, 255));
    dark.window_fill = Color32::from_rgb(6, 9, 15);
    dark.panel_fill = Color32::from_rgb(8, 12, 20);
    dark.faint_bg_color = Color32::from_rgb(16, 24, 38);
    dark.extreme_bg_color = Color32::from_rgb(2, 4, 8);
    dark.widgets.noninteractive.bg_fill = Color32::from_rgb(14, 20, 32);
    dark.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.2, Color32::from_rgb(65, 90, 125));
    dark.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.2, Color32::from_rgb(255, 255, 255));
    dark.widgets.inactive.bg_fill = Color32::from_rgb(24, 36, 54);
    dark.widgets.inactive.bg_stroke = egui::Stroke::new(1.2, Color32::from_rgb(90, 125, 170));
    dark.widgets.inactive.fg_stroke = egui::Stroke::new(1.3, Color32::from_rgb(255, 255, 255));
    dark.widgets.hovered.bg_fill = Color32::from_rgb(38, 60, 92);
    dark.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, Color32::from_rgb(130, 200, 255));
    dark.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, Color32::from_rgb(255, 255, 255));
    dark.widgets.active.bg_fill = Color32::from_rgb(0, 110, 195);
    dark.widgets.active.bg_stroke = egui::Stroke::new(1.8, Color32::from_rgb(160, 230, 255));
    dark.widgets.active.fg_stroke = egui::Stroke::new(1.6, Color32::from_rgb(255, 255, 255));
    dark.selection.bg_fill = Color32::from_rgb(0, 105, 190);
    dark.selection.stroke = egui::Stroke::new(1.5, Color32::from_rgb(255, 255, 255));
    ctx.set_visuals_of(egui::Theme::Dark, dark);

    // 2. Ultra-high-contrast Day Mode (Light) visuals
    let mut light = egui::Visuals::light();
    light.override_text_color = Some(Color32::from_rgb(0, 0, 0));
    light.window_fill = Color32::from_rgb(255, 255, 255);
    light.panel_fill = Color32::from_rgb(245, 248, 252);
    light.faint_bg_color = Color32::from_rgb(226, 234, 244);
    light.extreme_bg_color = Color32::from_rgb(255, 255, 255);
    light.widgets.noninteractive.bg_fill = Color32::from_rgb(255, 255, 255);
    light.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.2, Color32::from_rgb(100, 116, 139));
    light.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.3, Color32::from_rgb(0, 0, 0));
    light.widgets.inactive.bg_fill = Color32::from_rgb(226, 234, 244);
    light.widgets.inactive.bg_stroke = egui::Stroke::new(1.3, Color32::from_rgb(71, 85, 105));
    light.widgets.inactive.fg_stroke = egui::Stroke::new(1.4, Color32::from_rgb(0, 0, 0));
    light.widgets.hovered.bg_fill = Color32::from_rgb(195, 215, 240);
    light.widgets.hovered.bg_stroke = egui::Stroke::new(1.6, Color32::from_rgb(0, 75, 145));
    light.widgets.hovered.fg_stroke = egui::Stroke::new(1.6, Color32::from_rgb(0, 0, 0));
    light.widgets.active.bg_fill = Color32::from_rgb(155, 205, 250);
    light.widgets.active.bg_stroke = egui::Stroke::new(1.8, Color32::from_rgb(0, 60, 125));
    light.widgets.active.fg_stroke = egui::Stroke::new(1.6, Color32::from_rgb(0, 0, 0));
    light.selection.bg_fill = Color32::from_rgb(165, 212, 255);
    light.selection.stroke = egui::Stroke::new(1.8, Color32::from_rgb(0, 50, 115));
    ctx.set_visuals_of(egui::Theme::Light, light);
}

impl eframe::App for NmeaStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt_raw = now.duration_since(self.last_frame_instant).as_secs_f64().min(0.25);
        self.last_frame_instant = now;

        // 1. Poll incoming RX packets from active Serial / UDP / TCP workers
        let rx_entries = self.transport.poll_events(&mut self.next_seq_id);
        for rx in rx_entries {
            self.recorder.record_line(&rx.raw_line, false);
            if !self.monitor_paused {
                self.traffic_log.push(rx);
            }
        }

        // 2. Step Log Player if playing
        let played_lines = self.recorder.update(dt_raw);
        for pline in played_lines {
            let is_n2k = pline.starts_with("$PCDIN,")
                || (!pline.starts_with('$') && !pline.starts_with('!'));
            self.dispatch_generated_line("Log Player", is_n2k, &[], false, pline);
        }

        // 3. Step live simulation if running
        if self.sim_running {
            self.step_simulation(dt_raw);
        }

        // Request continuous repaint for smooth simulation and RX polling (~30 FPS)
        ctx.request_repaint_after(std::time::Duration::from_millis(33));

        // =====================================================================
        // TOP TOOLBAR RIBBON (Stable single-line layout that never jumps height)
        // =====================================================================
        egui::TopBottomPanel::top("top_ribbon").show(ctx, |ui| {
            let pal = ThemePalette::from_ui(ui);
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                draw_app_logo_badge(ui, 22.0);
                ui.heading(
                    RichText::new("NMEA Studio")
                        .color(pal.accent)
                        .strong(),
                );
                ui.separator();

                let run_btn_text = if self.sim_running {
                    RichText::new("⏸ Pause").color(pal.warning).strong()
                } else {
                    RichText::new("▶ Start").color(pal.positive).strong()
                };
                if ui.button(run_btn_text).clicked() {
                    self.sim_running = !self.sim_running;
                }
                if ui.button("+1s").clicked() {
                    self.step_simulation(1.0);
                }

                ui.label("Speed:");
                ui.add(
                    egui::DragValue::new(&mut self.project.sim_speed_multiplier)
                        .speed(0.1)
                        .range(0.25..=20.0)
                        .suffix("x"),
                );

                ui.checkbox(&mut self.project.vessel.fluctuation.enabled, "Waves");

                ui.separator();
                ui.label("Preset:");
                egui::ComboBox::from_id_salt("preset_selector")
                    .width(185.0)
                    .selected_text(StudioProject::preset_names()[self.selected_preset_idx])
                    .show_ui(ui, |ui| {
                        for (idx, name) in StudioProject::preset_names().iter().enumerate() {
                            if ui
                                .selectable_value(&mut self.selected_preset_idx, idx, *name)
                                .clicked()
                            {
                                self.project = StudioProject::load_preset(idx);
                                self.transport.sync_ports(&self.project.ports);
                                self.project_status_msg = format!("Loaded preset: {}", name);
                            }
                        }
                    });

                if ui.button("💾 Save...").clicked() {
                    self.file_dialog.open_save(&self.project_file_path);
                }
                if ui.button("📂 Load...").clicked() {
                    self.file_dialog.open_load(&self.project_file_path);
                }

                ui.separator();
                ui.add(
                    egui::Label::new(
                        RichText::new(&self.project_status_msg)
                            .small()
                            .strong()
                            .color(pal.text_secondary),
                    )
                    .truncate(),
                );
            });
            ui.add_space(2.0);
        });

        // Modal dialog window for saving or loading a project/preset (.json)
        self.draw_project_file_dialog(ctx);

        // =====================================================================
        // BOTTOM TRAFFIC MONITOR & PACKET INSPECTOR DOCK
        // =====================================================================
        egui::TopBottomPanel::bottom("bottom_traffic_dock")
            .resizable(true)
            .default_height(240.0)
            .min_height(110.0)
            .max_height(550.0)
            .show(ctx, |ui| {
                self.draw_traffic_monitor_and_inspector(ui);
            });

        // =====================================================================
        // LEFT OBJECT EXPLORER DOCK (NemaStudio Tree Style)
        // =====================================================================
        egui::SidePanel::left("left_explorer_dock")
            .resizable(true)
            .default_width(255.0)
            .min_width(160.0)
            .max_width(480.0)
            .show(ctx, |ui| {
                self.draw_left_explorer(ui);
            });

        // =====================================================================
        // CENTRAL MULTI-TAB WORKSPACE
        // =====================================================================
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(
                    &mut self.active_tab,
                    WorkspaceTab::HelmAndSensors,
                    "☸ Helm & Sensors",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    WorkspaceTab::ChartAndRadar,
                    "🗺 Chart & Radar",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    WorkspaceTab::InstrumentsAndPgns,
                    "📡 Instruments",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    WorkspaceTab::PortsAndNetwork,
                    "🔌 Ports",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    WorkspaceTab::AisAndArpa,
                    "🚢 AIS & ARPA",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    WorkspaceTab::LogRecorderPlayer,
                    "⏺ Recorder & Player",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    WorkspaceTab::CustomBuilder,
                    "🔬 Custom Studio",
                );
            });
            ui.separator();

            match self.active_tab {
                WorkspaceTab::HelmAndSensors => self.draw_helm_and_sensors_tab(ui),
                WorkspaceTab::ChartAndRadar => self.draw_chart_and_radar_tab(ui),
                WorkspaceTab::InstrumentsAndPgns => self.draw_instruments_tab(ui),
                WorkspaceTab::PortsAndNetwork => self.draw_ports_tab(ui),
                WorkspaceTab::AisAndArpa => self.draw_ais_tab(ui),
                WorkspaceTab::LogRecorderPlayer => self.draw_recorder_tab(ui),
                WorkspaceTab::CustomBuilder => self.draw_custom_builder_tab(ui),
            }
        });
    }
}

impl NmeaStudioApp {
    fn draw_project_file_dialog(&mut self, ctx: &egui::Context) {
        if self.file_dialog.mode == FileDialogMode::Closed {
            return;
        }

        let is_save = self.file_dialog.mode == FileDialogMode::SaveProject;
        let title = if is_save {
            "💾 Save Project / Preset (.json)"
        } else {
            "📂 Load Project / Preset (.json)"
        };

        let mut close_requested = false;
        let mut trigger_confirm = false;
        let mut navigate_to_dir: Option<String> = None;
        let mut navigate_up = false;

        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .default_height(420.0)
            .min_width(360.0)
            .min_height(280.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let pal = ThemePalette::from_ui(ui);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Quick Locations:");
                    if let Ok(cwd) = std::env::current_dir() {
                        if ui.small_button("📁 Workspace").clicked() {
                            navigate_to_dir = Some(cwd.to_string_lossy().to_string());
                        }
                    }
                    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))
                    {
                        if ui.small_button("🏠 Home").clicked() {
                            navigate_to_dir = Some(home.clone());
                        }
                        let desktop = Path::new(&home).join("Desktop");
                        if desktop.is_dir() && ui.small_button("Desktop").clicked() {
                            navigate_to_dir = Some(desktop.to_string_lossy().to_string());
                        }
                        let docs = Path::new(&home).join("Documents");
                        if docs.is_dir() && ui.small_button("Documents").clicked() {
                            navigate_to_dir = Some(docs.to_string_lossy().to_string());
                        }
                    }

                    ui.separator();
                    if ui
                        .button("📂 Browse via Native OS Window...")
                        .clicked()
                    {
                        if let Some(chosen) = pick_native_file_dialog(
                            is_save,
                            &self.file_dialog.current_dir,
                            &self.file_dialog.file_name,
                        ) {
                            let res = if is_save {
                                self.save_project_to_path(&chosen)
                            } else {
                                self.load_project_from_path(&chosen)
                            };
                            match res {
                                Ok(()) => close_requested = true,
                                Err(e) => self.file_dialog.error_message = e,
                            }
                        }
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("⬆ Parent").clicked() {
                        navigate_up = true;
                    }
                    ui.label("Folder:");
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.file_dialog.current_dir)
                            .desired_width((ui.available_width() - 48.0).max(120.0)),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.file_dialog.refresh_entries();
                    }
                    if ui.button("🔄").clicked() {
                        self.file_dialog.refresh_entries();
                    }
                });

                if is_save {
                    ui.horizontal(|ui| {
                        ui.label("Preset Title:");
                        ui.text_edit_singleline(&mut self.project.project_name);
                    });
                }

                ui.add_space(4.0);
                egui::Frame::group(ui.style())
                    .fill(pal.card_fill)
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("file_dialog_entries_scroll")
                            .max_height(200.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if self.file_dialog.entries.is_empty() {
                                    ui.label(
                                        RichText::new(
                                            "No subdirectories or .json preset files in this folder.",
                                        )
                                        .color(pal.text_muted),
                                    );
                                }
                                for item in &self.file_dialog.entries {
                                    if item.is_dir {
                                        if ui
                                            .selectable_label(
                                                false,
                                                RichText::new(format!("📁 {}/", item.name))
                                                    .color(pal.accent),
                                            )
                                            .clicked()
                                        {
                                            navigate_to_dir = Some(item.path.clone());
                                        }
                                    } else {
                                        let is_sel = self.file_dialog.file_name == item.name;
                                        let resp = ui.selectable_label(
                                            is_sel,
                                            RichText::new(format!("📄 {}", item.name))
                                                .color(pal.text_primary)
                                                .monospace(),
                                        );
                                        if resp.clicked() {
                                            self.file_dialog.file_name = item.name.clone();
                                        }
                                        if resp.double_clicked() {
                                            self.file_dialog.file_name = item.name.clone();
                                            trigger_confirm = true;
                                        }
                                    }
                                }
                            });
                    });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("File Name (.json):");
                    let resp = ui.text_edit_singleline(&mut self.file_dialog.file_name);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        trigger_confirm = true;
                    }
                });

                let full_target = self.file_dialog.resolved_file_path();
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("Target Path: {}", full_target))
                            .small()
                            .monospace()
                            .color(pal.text_muted),
                    )
                    .truncate(),
                );

                if !self.file_dialog.error_message.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("⚠ {}", self.file_dialog.error_message))
                                .color(pal.danger),
                        )
                        .truncate(),
                    );
                }

                ui.separator();
                ui.horizontal(|ui| {
                    let confirm_label = if is_save {
                        "💾 Save Preset Here"
                    } else {
                        "📂 Load Selected Preset"
                    };
                    if ui.button(RichText::new(confirm_label).strong()).clicked() {
                        trigger_confirm = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });

        if navigate_up {
            self.file_dialog.navigate_up();
        }
        if let Some(dir) = navigate_to_dir {
            self.file_dialog.navigate_to(&dir);
        }
        if trigger_confirm {
            let target = self.file_dialog.resolved_file_path();
            let res = if is_save {
                self.save_project_to_path(&target)
            } else {
                self.load_project_from_path(&target)
            };
            match res {
                Ok(()) => close_requested = true,
                Err(e) => self.file_dialog.error_message = e,
            }
        }
        if close_requested {
            self.file_dialog.mode = FileDialogMode::Closed;
        }
    }

    pub fn add_instrument_from_template(&mut self, template: InstrumentTemplate) {
        let new_id = self.project.instruments.len() + 1;
        let inst = Instrument::from_template(template, new_id);
        self.project_status_msg = format!("Added {}", inst.name);
        self.project.instruments.push(inst);
        self.selected_instrument_idx = self.project.instruments.len() - 1;
        self.active_tab = WorkspaceTab::InstrumentsAndPgns;
    }

    pub fn add_ais_or_arpa_device(&mut self, arpa_only: bool) {
        self.project.ais_config.enabled = true;
        if !arpa_only {
            self.project.ais_config.emit_own_ship_vdo = true;
            self.project.ais_config.emit_nmea0183_vdm = true;
        }
        self.spawn_random_ais_target();
        if let Some(last) = self.project.ais_targets.last_mut() {
            if arpa_only {
                last.ais_class = AisClass::ArpaOnly;
                last.emit_arpa_ttm = true;
                last.name = format!("RADAR TARGET {:02}", last.id);
                self.project_status_msg = format!("Added ARPA Radar Target ({})", last.name);
            } else {
                self.project_status_msg = format!("Added AIS Target ({})", last.name);
            }
        }
        self.selected_ais_idx = self.project.ais_targets.len().saturating_sub(1);
        self.active_tab = WorkspaceTab::AisAndArpa;
    }

    pub fn create_port_config_of_kind(ports_len: usize, kind: PortKind, prefix: Option<&str>) -> PortConfig {
        let n = ports_len + 1;
        let mut p = PortConfig::default_ports()[0].clone();
        p.id = format!("port-custom-{}", n);
        p.kind = kind;
        p.enabled = true;
        let kind_short = match kind {
            PortKind::UdpUnicast => "UDP Unicast",
            PortKind::UdpMulticast => "UDP Multicast",
            PortKind::UdpBroadcast => "UDP Broadcast",
            PortKind::Serial => "Serial Port",
            PortKind::TcpServer => "TCP Server",
            PortKind::TcpClient => "TCP Client",
        };
        if kind == PortKind::UdpMulticast {
            let iec_grp = IecMulticastGroup::for_device_name(prefix.unwrap_or(""));
            p.multicast_group = iec_grp.ip().to_string();
            p.multicast_port = iec_grp.port();
            p.remote_host = iec_grp.ip().to_string();
            p.remote_port = iec_grp.port();
            p.local_bind_port = iec_grp.port();
            p.name = if let Some(pref) = prefix {
                format!("{} - {} ({})", pref, kind_short, iec_grp.code())
            } else {
                format!("{} #{} ({})", kind_short, n, iec_grp.code())
            };
        } else {
            p.name = if let Some(pref) = prefix {
                format!("{} - {}", pref, kind_short)
            } else {
                format!("{} #{}", kind_short, n)
            };
            if kind == PortKind::Serial {
                p.serial_port_name = if cfg!(target_os = "windows") {
                    format!("COM{}", n + 2)
                } else if cfg!(target_os = "macos") {
                    "/dev/cu.usbserial-0001".to_string()
                } else {
                    format!("/dev/ttyUSB{}", n - 1)
                };
                p.baud_rate = 4800;
            } else {
                p.remote_port = 10110 + (n as u16) - 1;
                p.local_bind_port = 10210 + (n as u16) - 1;
            }
        }
        p
    }

    pub fn add_port_of_kind(&mut self, kind: PortKind) {
        let mut p = Self::create_port_config_of_kind(self.project.ports.len(), kind, None);
        p.enabled = false;
        self.project_status_msg = format!("Added {}", p.name);
        self.project.ports.push(p);
        self.transport.sync_ports(&self.project.ports);
        self.selected_port_idx = self.project.ports.len() - 1;
        self.active_tab = WorkspaceTab::PortsAndNetwork;
    }

    fn draw_device_port_selector(
        ui: &mut Ui,
        id_salt: &str,
        device_name: &str,
        target_ports: &mut Vec<String>,
        ports: &mut Vec<PortConfig>,
        serial_list: &[String],
        transport: &mut TransportManager,
    ) {
        let pal = ThemePalette::from_ui(ui);
        let mut sync_needed = false;
        let mut new_port_kind: Option<PortKind> = None;

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("🔌 Output Port:");

                // Determine current selection label
                let current_label = if target_ports.is_empty() {
                    if ports.is_empty() {
                        "No Port Configured (Add Serial / UDP below)".to_string()
                    } else {
                        format!("All Active Ports ({})", ports.len())
                    }
                } else if target_ports.len() == 1 {
                    if let Some(p) = ports.iter().find(|p| p.id == target_ports[0]) {
                        format!("{} ({})", p.name, p.summary_endpoint())
                    } else {
                        "All Active Ports".to_string()
                    }
                } else {
                    format!("Multiple Selected Ports ({})", target_ports.len())
                };

                egui::ComboBox::from_id_salt(format!("{}_port_sel", id_salt))
                    .selected_text(current_label)
                    .width(270.0)
                    .show_ui(ui, |ui| {
                        let is_all = target_ports.is_empty();
                        if ui
                            .selectable_label(is_all, "All Active Ports (Broadcast to all)")
                            .clicked()
                        {
                            target_ports.clear();
                        }
                        if !ports.is_empty() {
                            ui.separator();
                            ui.label(
                                RichText::new("Select Single Port:")
                                    .small()
                                    .strong()
                                    .color(pal.text_secondary),
                            );
                            for p in ports.iter() {
                                let is_single =
                                    target_ports.len() == 1 && target_ports[0] == p.id;
                                let label = format!(
                                    "{} [{} - {}]",
                                    p.name,
                                    p.kind.label(),
                                    p.summary_endpoint()
                                );
                                if ui.selectable_label(is_single, label).clicked() {
                                    target_ports.clear();
                                    target_ports.push(p.id.clone());
                                }
                            }
                            if ports.len() > 1 {
                                ui.separator();
                                ui.label(
                                    RichText::new("Or Route to Multiple Ports:")
                                        .small()
                                        .strong()
                                        .color(pal.text_secondary),
                                );
                                for p in ports.iter() {
                                    let mut checked = target_ports.contains(&p.id);
                                    if ui
                                        .checkbox(&mut checked, format!("{} ({})", p.name, p.kind.label()))
                                        .changed()
                                    {
                                        if checked {
                                            if !target_ports.contains(&p.id) {
                                                target_ports.push(p.id.clone());
                                            }
                                        } else {
                                            target_ports.retain(|id| id != &p.id);
                                        }
                                    }
                                }
                            }
                        }
                    });

                ui.separator();
                ui.label(RichText::new("Assign New:").small().strong());
                if ui.small_button("➕ Serial").clicked() {
                    new_port_kind = Some(PortKind::Serial);
                }
                if ui.small_button("➕ UDP Unicast").clicked() {
                    new_port_kind = Some(PortKind::UdpUnicast);
                }
                if ui.small_button("➕ UDP Multicast").clicked() {
                    new_port_kind = Some(PortKind::UdpMulticast);
                }
                ui.menu_button("➕ More", |ui| {
                    if ui.button("UDP Broadcast").clicked() {
                        new_port_kind = Some(PortKind::UdpBroadcast);
                        ui.close_menu();
                    }
                    if ui.button("TCP Server").clicked() {
                        new_port_kind = Some(PortKind::TcpServer);
                        ui.close_menu();
                    }
                    if ui.button("TCP Client").clicked() {
                        new_port_kind = Some(PortKind::TcpClient);
                        ui.close_menu();
                    }
                });
            });

            if let Some(kind) = new_port_kind {
                let p = Self::create_port_config_of_kind(ports.len(), kind, Some(device_name));
                target_ports.clear();
                target_ports.push(p.id.clone());
                ports.push(p);
                sync_needed = true;
            }

            // Inline editor for the port(s) used by this device so the user can configure
            // Serial / UDP Unicast / UDP Multicast right on the device panel
            let active_port_idx: Option<usize> = if target_ports.len() == 1 {
                ports.iter().position(|p| p.id == target_ports[0])
            } else if target_ports.is_empty() && ports.len() == 1 {
                Some(0)
            } else {
                None
            };

            if let Some(p_idx) = active_port_idx {
                if let Some(p) = ports.get_mut(p_idx) {
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .checkbox(&mut p.enabled, RichText::new("Port Active").strong())
                            .changed()
                        {
                            sync_needed = true;
                        }
                        ui.label("Type:");
                        egui::ComboBox::from_id_salt(format!("{}_inline_kind", id_salt))
                            .selected_text(p.kind.label())
                            .show_ui(ui, |ui| {
                                for k in [
                                    PortKind::Serial,
                                    PortKind::UdpUnicast,
                                    PortKind::UdpMulticast,
                                    PortKind::UdpBroadcast,
                                    PortKind::TcpServer,
                                    PortKind::TcpClient,
                                ] {
                                    if ui.selectable_value(&mut p.kind, k, k.label()).changed() {
                                        sync_needed = true;
                                    }
                                }
                            });

                        match p.kind {
                            PortKind::Serial => {
                                ui.label("Serial Port:");
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut p.serial_port_name)
                                            .desired_width(135.0),
                                    )
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                                egui::ComboBox::from_id_salt(format!("{}_inline_ser", id_salt))
                                    .selected_text("Detected")
                                    .show_ui(ui, |ui| {
                                        if serial_list.is_empty() {
                                            ui.label("No hardware serial ports detected");
                                        }
                                        for sp in serial_list {
                                            if ui
                                                .selectable_value(
                                                    &mut p.serial_port_name,
                                                    sp.clone(),
                                                    sp,
                                                )
                                                .changed()
                                            {
                                                sync_needed = true;
                                            }
                                        }
                                    });
                                ui.label("Baud:");
                                egui::ComboBox::from_id_salt(format!("{}_inline_baud", id_salt))
                                    .selected_text(format!("{} bps", p.baud_rate))
                                    .show_ui(ui, |ui| {
                                        for baud in
                                            [4800u32, 9600, 19200, 38400, 57600, 115200, 230400]
                                        {
                                            if ui
                                                .selectable_value(
                                                    &mut p.baud_rate,
                                                    baud,
                                                    format!("{} bps", baud),
                                                )
                                                .changed()
                                            {
                                                sync_needed = true;
                                            }
                                        }
                                    });
                            }
                            PortKind::UdpUnicast | PortKind::UdpBroadcast => {
                                ui.label("Target IP:");
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut p.remote_host)
                                            .desired_width(110.0),
                                    )
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                                ui.label("UDP Port:");
                                if ui
                                    .add(egui::DragValue::new(&mut p.remote_port).range(1..=65535))
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                            }
                            PortKind::UdpMulticast => {
                                ui.label("IEC 61162-450 Group:");
                                egui::ComboBox::from_id_salt(format!("{}_iec_mcast_grp", id_salt))
                                    .selected_text(format!("{}:{}", p.multicast_group, p.multicast_port))
                                    .show_ui(ui, |ui| {
                                        for grp in IecMulticastGroup::ALL {
                                            let is_sel = p.multicast_group == grp.ip()
                                                && p.multicast_port == grp.port();
                                            if ui.selectable_label(is_sel, grp.label()).clicked() {
                                                p.multicast_group = grp.ip().to_string();
                                                p.multicast_port = grp.port();
                                                sync_needed = true;
                                            }
                                        }
                                    });
                                ui.label("Multicast IP:");
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut p.multicast_group)
                                            .desired_width(105.0),
                                    )
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                                ui.label("UDP Port:");
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut p.multicast_port)
                                            .range(1..=65535),
                                    )
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                            }
                            PortKind::TcpServer => {
                                ui.label("Listen IP:");
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut p.local_bind_host)
                                            .desired_width(105.0),
                                    )
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                                ui.label("Port:");
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut p.local_bind_port)
                                            .range(1..=65535),
                                    )
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                            }
                            PortKind::TcpClient => {
                                ui.label("Server IP:");
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut p.remote_host)
                                            .desired_width(105.0),
                                    )
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                                ui.label("Port:");
                                if ui
                                    .add(egui::DragValue::new(&mut p.remote_port).range(1..=65535))
                                    .changed()
                                {
                                    sync_needed = true;
                                }
                            }
                        }
                    });
                }
            }
        });

        if sync_needed {
            transport.sync_ports(ports);
        }
    }

    fn draw_left_explorer(&mut self, ui: &mut Ui) {
        let pal = ThemePalette::from_ui(ui);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        ScrollArea::vertical()
            .id_salt("left_explorer_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                ui.add_space(4.0);
                // 1. Own Ship Summary Card
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.strong("🚢 Own Vessel");
                        if ui.small_button("Helm").clicked() {
                            self.active_tab = WorkspaceTab::HelmAndSensors;
                        }
                        if ui.small_button("Chart").clicked() {
                            self.active_tab = WorkspaceTab::ChartAndRadar;
                        }
                    });
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "{} (MMSI {})",
                                self.project.vessel.vessel_name, self.project.vessel.mmsi
                            ))
                            .small()
                            .strong()
                            .color(pal.accent),
                        )
                        .truncate(),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "Lat: {:+09.5}°  Lon: {:+010.5}°",
                                self.project.vessel.latitude, self.project.vessel.longitude
                            ))
                            .monospace()
                            .size(11.0),
                        )
                        .truncate(),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "SOG: {:4.1} kn   COG: {:05.1}°T",
                                self.project.vessel.sog_knots, self.project.vessel.cog_deg
                            ))
                            .monospace()
                            .size(11.0),
                        )
                        .truncate(),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "HDG: {:05.1}°T   DBT: {:4.1} m",
                                self.project.vessel.heading_true_deg,
                                self.project.vessel.depth_below_transducer_m
                            ))
                            .monospace()
                            .size(11.0),
                        )
                        .truncate(),
                    );
                });

                ui.add_space(6.0);

                // 2. Simulated Instruments Tree
                egui::CollapsingHeader::new(RichText::new("📡 Simulated Instruments").strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        let mut clicked_idx = None;
                        let ports_snap = self.project.ports.clone();
                        for (idx, inst) in self.project.instruments.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut inst.enabled, "");
                                let badge = match inst.protocol_mode {
                                    ProtocolMode::Nmea0183Only => "[0183]",
                                    ProtocolMode::Nmea2000Only => "[N2K]",
                                    ProtocolMode::Dual0183And2000 => "[DUAL]",
                                };
                                let is_sel = self.selected_instrument_idx == idx
                                    && self.active_tab == WorkspaceTab::InstrumentsAndPgns;
                                if ui
                                    .selectable_label(
                                        is_sel,
                                        format!("{} {} {}", inst.icon(), inst.name, badge),
                                    )
                                    .clicked()
                                {
                                    clicked_idx = Some(idx);
                                }
                            });
                            if !inst.target_ports.is_empty() {
                                let port_names: Vec<String> = inst
                                    .target_ports
                                    .iter()
                                    .filter_map(|pid| {
                                        ports_snap
                                            .iter()
                                            .find(|p| &p.id == pid)
                                            .map(|p| p.summary_endpoint())
                                    })
                                    .collect();
                                if !port_names.is_empty() {
                                    ui.indent(&inst.id, |ui| {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(port_names.join(", "))
                                                    .small()
                                                    .color(pal.text_muted),
                                            )
                                            .truncate(),
                                        );
                                    });
                                }
                            }
                        }
                        if let Some(i) = clicked_idx {
                            self.selected_instrument_idx = i;
                            self.active_tab = WorkspaceTab::InstrumentsAndPgns;
                        }
                        let mut chosen_tmpl: Option<InstrumentTemplate> = None;
                        let mut chosen_ais: Option<bool> = None;
                        ui.menu_button("➕ Add Device / Instrument", |ui| {
                            for &tmpl in InstrumentTemplate::ALL {
                                if ui.button(tmpl.label()).clicked() {
                                    chosen_tmpl = Some(tmpl);
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui
                                .button("🚢 AIS Transponder & Target (!AIVDM / !AIVDO)")
                                .clicked()
                            {
                                chosen_ais = Some(false);
                                ui.close_menu();
                            }
                            if ui.button("📡 ARPA Radar Target ($RATTM)").clicked() {
                                chosen_ais = Some(true);
                                ui.close_menu();
                            }
                        });
                        if let Some(tmpl) = chosen_tmpl {
                            self.add_instrument_from_template(tmpl);
                        }
                        if let Some(arpa_only) = chosen_ais {
                            self.add_ais_or_arpa_device(arpa_only);
                        }
                    });

                ui.add_space(6.0);

                // 3. Communication Ports Tree
                egui::CollapsingHeader::new(RichText::new("🔌 Communication Ports").strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        let mut ports_changed = false;
                        let mut clicked_port = None;
                        for (idx, p) in self.project.ports.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut p.enabled, "").changed() {
                                    ports_changed = true;
                                }
                                let st = self.transport.stats.get(&p.id);
                                let is_open = st.map(|s| s.is_open).unwrap_or(false);
                                let dot_col = if p.enabled && is_open {
                                    pal.positive
                                } else if p.enabled {
                                    pal.warning
                                } else {
                                    pal.text_muted
                                };
                                ui.label(RichText::new("◎").color(dot_col).strong());
                                let is_sel = self.selected_port_idx == idx
                                    && self.active_tab == WorkspaceTab::PortsAndNetwork;
                                if ui.selectable_label(is_sel, &p.name).clicked() {
                                    clicked_port = Some(idx);
                                }
                            });
                            ui.indent(&p.id, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(p.summary_endpoint())
                                            .small()
                                            .color(pal.text_muted),
                                    )
                                    .truncate(),
                                );
                            });
                        }
                        if ports_changed {
                            self.transport.sync_ports(&self.project.ports);
                        }
                        if let Some(i) = clicked_port {
                            self.selected_port_idx = i;
                            self.active_tab = WorkspaceTab::PortsAndNetwork;
                        }
                        let mut chosen_port_kind: Option<PortKind> = None;
                        ui.menu_button("➕ Add Port", |ui| {
                            for k in PortKind::ALL {
                                if ui.button(k.label()).clicked() {
                                    chosen_port_kind = Some(k);
                                    ui.close_menu();
                                }
                            }
                        });
                        if let Some(k) = chosen_port_kind {
                            self.add_port_of_kind(k);
                        }
                    });

                ui.add_space(6.0);

                // 4. AIS & ARPA Targets Tree
                egui::CollapsingHeader::new(RichText::new("🚢 AIS & ARPA Fleet").strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        ui.checkbox(
                            &mut self.project.ais_config.enabled,
                            "Enable AIS / ARPA Engine",
                        );
                        let mut clicked_ais = None;
                        for (idx, tgt) in self.project.ais_targets.iter_mut().enumerate() {
                            let cpa = tgt.compute_cpa(&self.project.vessel);
                            let is_alarm =
                                cpa.cpa_nm < 0.5 && cpa.tcpa_min > 0.0 && cpa.tcpa_min < 30.0;
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut tgt.enabled, "");
                                let is_sel = self.selected_ais_idx == idx
                                    && self.active_tab == WorkspaceTab::AisAndArpa;
                                let label_text = if is_alarm {
                                    RichText::new(format!("⚠ {} ({:.1}NM)", tgt.name, cpa.range_nm))
                                        .color(pal.danger)
                                } else {
                                    RichText::new(format!("{} ({:.1}NM)", tgt.name, cpa.range_nm))
                                };
                                if ui.selectable_label(is_sel, label_text).clicked() {
                                    clicked_ais = Some(idx);
                                }
                            });
                        }
                        if let Some(i) = clicked_ais {
                            self.selected_ais_idx = i;
                            self.active_tab = WorkspaceTab::AisAndArpa;
                        }
                        if ui.small_button("➕ Spawn Random AIS Vessel").clicked() {
                            self.spawn_random_ais_target();
                            self.selected_ais_idx = self.project.ais_targets.len() - 1;
                            self.active_tab = WorkspaceTab::AisAndArpa;
                        }
                    });
            });
    }

    fn draw_helm_and_sensors_tab(&mut self, ui: &mut Ui) {
        let m = self.project.vessel.sampled_metrics();
        ScrollArea::vertical()
            .id_salt("helm_and_sensors_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Top Row: Visual Instrument Gauges (Compass Rose, Wind Rose, Echo Sounder & Rudder/Attitude)
                ui.columns(3, |cols| {
                    let gauge_sz = (cols[0].available_width() - 12.0).clamp(135.0, 210.0);
                    cols[0].vertical_centered(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new("🎯 Compass & Course Rose (Drag)").strong(),
                            )
                            .truncate(),
                        );
                        draw_compass_gauge(
                            ui,
                            gauge_sz,
                            &mut self.project.vessel.heading_true_deg,
                            &mut self.project.vessel.cog_deg,
                            m.sog_knots,
                            m.rot_deg_per_min,
                        );
                    });

                    cols[1].vertical_centered(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new("💨 Wind Anemometer Rose (Drag)").strong(),
                            )
                            .truncate(),
                        );
                        draw_wind_gauge(
                            ui,
                            gauge_sz,
                            &mut self.project.vessel.wind_true_dir_deg,
                            m.wind_true_speed_knots,
                            m.wind_apparent_angle_deg,
                            m.wind_apparent_speed_knots,
                            m.wind_true_angle_deg,
                            m.heading_true_deg,
                        );
                    });

                    cols[2].vertical(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new("🌊 Bathymetry, Attitude & Rudder").strong(),
                            )
                            .truncate(),
                        );
                        draw_depth_and_rudder_panel(
                            ui,
                            m.depth_transducer_m,
                            self.project.vessel.transducer_offset_m,
                            self.project.vessel.water_temp_c,
                            self.project.vessel.rudder_angle_deg,
                            m.pitch_deg,
                            m.roll_deg,
                            self.project.vessel.sim_elapsed_sec,
                        );
                        ui.add_space(6.0);
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Steering:");
                                egui::ComboBox::from_id_salt("steer_mode_cb")
                                    .selected_text(self.project.vessel.steering_mode.label())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.project.vessel.steering_mode,
                                            SteeringMode::ManualHeading,
                                            SteeringMode::ManualHeading.label(),
                                        );
                                        ui.selectable_value(
                                            &mut self.project.vessel.steering_mode,
                                            SteeringMode::TurnRate,
                                            SteeringMode::TurnRate.label(),
                                        );
                                        ui.selectable_value(
                                            &mut self.project.vessel.steering_mode,
                                            SteeringMode::FollowRoute,
                                            SteeringMode::FollowRoute.label(),
                                        );
                                    });
                            });
                        });
                    });
                });

                ui.separator();

                // Second Row: Detailed Sensor Parameter Sliders in 2 Columns
                ui.columns(2, |cols| {
                    // LEFT COLUMN: 1. GNSS, 2. Gyro / Heading & Attitude, 3. Depth & Speed Log
                    cols[0].vertical(|ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("🌐 GNSS Position, Course & Speed");
                            egui::Grid::new("gnss_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Vessel Name / Callsign:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut self.project.vessel.vessel_name,
                                            )
                                            .desired_width(120.0),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut self.project.vessel.callsign,
                                            )
                                            .desired_width(70.0),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Latitude (°N+ / °S-):");
                                    ui.add(
                                        egui::DragValue::new(&mut self.project.vessel.latitude)
                                            .speed(0.0005)
                                            .range(-89.9..=89.9)
                                            .fixed_decimals(6),
                                    );
                                    ui.end_row();

                                    ui.label("Longitude (°E+ / °W-):");
                                    ui.add(
                                        egui::DragValue::new(&mut self.project.vessel.longitude)
                                            .speed(0.0005)
                                            .range(-180.0..=180.0)
                                            .fixed_decimals(6),
                                    );
                                    ui.end_row();

                                    ui.label("Altitude / Satellites / HDOP:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.altitude_m,
                                            )
                                            .speed(0.2)
                                            .prefix("Alt ")
                                            .suffix(" m"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.satellites_in_use,
                                            )
                                            .range(0..=32)
                                            .prefix("Sats "),
                                        );
                                        ui.add(
                                            egui::DragValue::new(&mut self.project.vessel.hdop)
                                                .speed(0.05)
                                                .range(0.4..=10.0)
                                                .prefix("HDOP "),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Speed Over Ground (SOG):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.sog_knots,
                                            0.0..=45.0,
                                        )
                                        .suffix(" kn"),
                                    );
                                    ui.end_row();

                                    ui.label("Course Over Ground (COG):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.cog_deg,
                                            0.0..=359.9,
                                        )
                                        .suffix(" °T"),
                                    );
                                    ui.end_row();
                                });
                        });

                        ui.add_space(8.0);

                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("🎯 Gyrocompass, Heading & Attitude (AHRS)");
                            let hdm = self.project.vessel.heading_magnetic_deg();
                            let hdg_comp = (hdm - self.project.vessel.mag_dev_deg).rem_euclid(360.0);
                            egui::Grid::new("gyro_ahrs_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("True Heading (HDT):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.heading_true_deg,
                                            0.0..=359.9,
                                        )
                                        .suffix(" °T"),
                                    );
                                    ui.end_row();

                                    ui.label("Computed Magnetic / Compass:");
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(format!(
                                                "HDM: {:05.1}°M  |  HDG: {:05.1}°C",
                                                hdm, hdg_comp
                                            ))
                                            .monospace(),
                                        )
                                        .truncate(),
                                    );
                                    ui.end_row();

                                    ui.label("Magnetic Variation / Dev:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.mag_var_deg,
                                            )
                                            .speed(0.1)
                                            .prefix("Var: ")
                                            .suffix("°"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.mag_dev_deg,
                                            )
                                            .speed(0.1)
                                            .prefix("Dev: ")
                                            .suffix("°"),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Rate of Turn (ROT):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.rot_deg_per_min,
                                            -60.0..=60.0,
                                        )
                                        .suffix(" °/min"),
                                    );
                                    ui.end_row();

                                    ui.label("Rudder Angle (RSA):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.rudder_angle_deg,
                                            -35.0..=35.0,
                                        )
                                        .suffix(" °"),
                                    );
                                    ui.end_row();

                                    ui.label("Pitch (Bow Up+ / Down-):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.pitch_deg,
                                            -25.0..=25.0,
                                        )
                                        .suffix(" °"),
                                    );
                                    ui.end_row();

                                    ui.label("Roll (Stbd+ / Port-):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.roll_deg,
                                            -35.0..=35.0,
                                        )
                                        .suffix(" °"),
                                    );
                                    ui.end_row();

                                    ui.label("Vertical Heave:");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.heave_m,
                                            -5.0..=5.0,
                                        )
                                        .suffix(" m"),
                                    );
                                    ui.end_row();
                                });
                        });

                        ui.add_space(8.0);

                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("🌊 Depth Sounder & Speed Log");
                            egui::Grid::new("depth_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Depth Below Transducer (DBT):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.depth_below_transducer_m,
                                            0.5..=250.0,
                                        )
                                        .logarithmic(true)
                                        .suffix(" m"),
                                    );
                                    ui.end_row();

                                    ui.label("Computed Surface / Keel:");
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(format!(
                                                "DBS: {:5.1} m  |  DBK: {:5.1} m",
                                                m.depth_surface_m, m.depth_keel_m
                                            ))
                                            .monospace(),
                                        )
                                        .truncate(),
                                    );
                                    ui.end_row();

                                    ui.label("Transducer / Keel Offset:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.transducer_offset_m,
                                            )
                                            .speed(0.05)
                                            .prefix("Xdcr: ")
                                            .suffix(" m"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.keel_offset_m,
                                            )
                                            .speed(0.05)
                                            .prefix("Keel: ")
                                            .suffix(" m"),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Speed Through Water (STW):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.stw_knots,
                                            0.0..=45.0,
                                        )
                                        .suffix(" kn"),
                                    );
                                    ui.end_row();

                                    ui.label("Water Temperature (MTW):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.water_temp_c,
                                            -2.0..=38.0,
                                        )
                                        .suffix(" °C"),
                                    );
                                    ui.end_row();
                                });
                        });
                    });

                    // RIGHT COLUMN: Wind/Meteo, Propulsion/Electrical, and Fluctuation
                    cols[1].vertical(|ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("💨 Wind & Meteorological Station");
                            egui::Grid::new("wind_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("True Wind Speed (TWS):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.wind_true_speed_knots,
                                            0.0..=75.0,
                                        )
                                        .suffix(" kn"),
                                    );
                                    ui.end_row();

                                    ui.label("True Wind Direction (TWD):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.wind_true_dir_deg,
                                            0.0..=359.0,
                                        )
                                        .suffix(" °T"),
                                    );
                                    ui.end_row();

                                    ui.label("Computed Apparent Wind:");
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(format!(
                                                "AWS: {:4.1} kn @ AWA: {:05.1}° (TWA: {:05.1}°)",
                                                m.wind_apparent_speed_knots,
                                                m.wind_apparent_angle_deg,
                                                m.wind_true_angle_deg
                                            ))
                                            .monospace(),
                                        )
                                        .truncate(),
                                    );
                                    ui.end_row();

                                    ui.label("Barometric Pressure:");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.barometric_pressure_hpa,
                                            950.0..=1050.0,
                                        )
                                        .suffix(" hPa"),
                                    );
                                    ui.end_row();

                                    ui.label("Outside Air Temp / RH:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.air_temp_c,
                                            )
                                            .speed(0.2)
                                            .suffix(" °C"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.relative_humidity_pct,
                                            )
                                            .speed(0.5)
                                            .range(10.0..=100.0)
                                            .suffix(" %RH"),
                                        );
                                    });
                                    ui.end_row();
                                });
                        });

                        ui.add_space(8.0);

                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("⚙ Propulsion, Battery & Fluid Tanks (N2K & XDR)");
                            egui::Grid::new("eng_grid")
                                .num_columns(2)
                                .spacing([12.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("Engine Speed (RPM):");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.project.vessel.port_engine_rpm,
                                            0.0..=4500.0,
                                        )
                                        .suffix(" RPM"),
                                    );
                                    ui.end_row();

                                    ui.label("Coolant Temp / Oil Press:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.engine_coolant_temp_c,
                                            )
                                            .speed(0.5)
                                            .suffix(" °C"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.engine_oil_pressure_kpa,
                                            )
                                            .speed(2.0)
                                            .suffix(" kPa"),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Fuel Rate / Tank Level:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.fuel_rate_lph,
                                            )
                                            .speed(0.5)
                                            .suffix(" L/h"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.fuel_tank_level_pct,
                                            )
                                            .speed(0.5)
                                            .range(0.0..=100.0)
                                            .suffix(" %"),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Battery Voltage / Current:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.house_battery_voltage_v,
                                            )
                                            .speed(0.05)
                                            .suffix(" V"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.project.vessel.battery_current_a,
                                            )
                                            .speed(0.2)
                                            .suffix(" A"),
                                        );
                                    });
                                    ui.end_row();
                                });
                        });

                        ui.add_space(8.0);

                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.heading("🌊 Sensor Fluctuation / Wave Noise");
                                ui.checkbox(
                                    &mut self.project.vessel.fluctuation.enabled,
                                    "Active",
                                );
                            });
                            egui::Grid::new("fluct_grid")
                                .num_columns(2)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label("Heading / Speed Jitter:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self
                                                    .project
                                                    .vessel
                                                    .fluctuation
                                                    .heading_jitter_deg,
                                            )
                                            .speed(0.05)
                                            .suffix("°"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self
                                                    .project
                                                    .vessel
                                                    .fluctuation
                                                    .speed_jitter_knots,
                                            )
                                            .speed(0.02)
                                            .suffix(" kn"),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Wave Pitch/Roll Amp & Period:");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self
                                                    .project
                                                    .vessel
                                                    .fluctuation
                                                    .pitch_roll_wave_amp_deg,
                                            )
                                            .speed(0.2)
                                            .suffix("°"),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self
                                                    .project
                                                    .vessel
                                                    .fluctuation
                                                    .wave_period_sec,
                                            )
                                            .speed(0.2)
                                            .suffix(" s"),
                                        );
                                    });
                                    ui.end_row();
                                });
                        });
                    });
                });
            });
    }

    fn draw_chart_and_radar_tab(&mut self, ui: &mut Ui) {
        StripBuilder::new(ui)
            .size(Size::remainder().at_least(180.0))
            .size(Size::initial(295.0).at_least(210.0))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    ui.set_clip_rect(ui.max_rect());
                    self.plotter.draw(
                        ui,
                        &mut self.project.vessel,
                        &mut self.project.route,
                        &mut self.project.ais_targets,
                    );
                });
                strip.cell(|ui| {
                    ui.set_clip_rect(ui.max_rect());
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        ui.heading("📍 Active Route & Waypoints");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.project.route.active, "Route Active");
                            ui.checkbox(&mut self.project.route.loop_route, "Loop Route");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Route Name:");
                            ui.text_edit_singleline(&mut self.project.route.name);
                        });
                        if ui.button("☸ Engage Autopilot on Route").clicked() {
                            self.project.route.active = true;
                            self.project.vessel.steering_mode = SteeringMode::FollowRoute;
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("➕ Add Waypoint").clicked() {
                                let n = self.project.route.waypoints.len() + 1;
                                self.project.route.waypoints.push(Waypoint {
                                    id: format!("WP{}", n),
                                    name: format!("WPT-{:02}", n),
                                    latitude: self.project.vessel.latitude - 0.01 * (n as f64),
                                    longitude: self.project.vessel.longitude - 0.02 * (n as f64),
                                    arrival_radius_nm: 0.10,
                                });
                            }
                            if ui.button("Next Leg").clicked()
                                && self.project.route.waypoints.len() >= 2
                            {
                                self.project.route.active_leg_to_idx =
                                    (self.project.route.active_leg_to_idx + 1)
                                        .clamp(1, self.project.route.waypoints.len() - 1);
                            }
                        });

                        ui.add_space(4.0);
                        ScrollArea::vertical()
                            .id_salt("route_wpts_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let mut remove_idx = None;
                                let mut activate_idx = None;
                                for (idx, wp) in self.project.route.waypoints.iter_mut().enumerate()
                                {
                                    let is_dest = self.project.route.active
                                        && self.project.route.active_leg_to_idx == idx;
                                    egui::Frame::group(ui.style()).show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.strong(format!("#{}", idx + 1));
                                            ui.add(
                                                egui::TextEdit::singleline(&mut wp.name)
                                                    .desired_width(110.0),
                                            );
                                            if idx >= 1 {
                                                let btn_txt =
                                                    if is_dest { "Active" } else { "Go To" };
                                                if ui.small_button(btn_txt).clicked() {
                                                    activate_idx = Some(idx);
                                                }
                                            }
                                            if ui.small_button("🗑").clicked() {
                                                remove_idx = Some(idx);
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.add(
                                                egui::DragValue::new(&mut wp.latitude)
                                                    .speed(0.0005)
                                                    .prefix("Lat: ")
                                                    .fixed_decimals(5),
                                            );
                                            ui.add(
                                                egui::DragValue::new(&mut wp.longitude)
                                                    .speed(0.0005)
                                                    .prefix("Lon: ")
                                                    .fixed_decimals(5),
                                            );
                                        });
                                    });
                                }
                                if let Some(i) = activate_idx {
                                    self.project.route.active_leg_to_idx = i;
                                    self.project.route.active = true;
                                }
                                if let Some(i) = remove_idx {
                                    self.project.route.waypoints.remove(i);
                                    if self.project.route.active_leg_to_idx
                                        >= self.project.route.waypoints.len()
                                        && !self.project.route.waypoints.is_empty()
                                    {
                                        self.project.route.active_leg_to_idx =
                                            (self.project.route.waypoints.len() - 1).max(1);
                                    }
                                }
                            });
                    });
                });
            });
    }

    fn draw_instruments_tab(&mut self, ui: &mut Ui) {
        let pal = ThemePalette::from_ui(ui);
        let mut chosen_tmpl: Option<InstrumentTemplate> = None;
        let mut chosen_ais: Option<bool> = None;

        // Top selector & NemaStudio-style Add Device menu
        ui.horizontal_wrapped(|ui| {
            for (i, inst) in self.project.instruments.iter().enumerate() {
                if ui
                    .selectable_label(
                        self.selected_instrument_idx == i,
                        format!("{} {}", inst.icon(), inst.name),
                    )
                    .clicked()
                {
                    self.selected_instrument_idx = i;
                }
            }
            ui.menu_button("➕ Add Simulated Device / Instrument", |ui| {
                for &tmpl in InstrumentTemplate::ALL {
                    if ui.button(tmpl.label()).clicked() {
                        chosen_tmpl = Some(tmpl);
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui
                    .button("🚢 AIS Transponder & Target (!AIVDM / !AIVDO)")
                    .clicked()
                {
                    chosen_ais = Some(false);
                    ui.close_menu();
                }
                if ui.button("📡 ARPA Radar Target ($RATTM)").clicked() {
                    chosen_ais = Some(true);
                    ui.close_menu();
                }
            });
        });
        ui.separator();

        if self.project.instruments.is_empty() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.heading("📡 Add a Simulated NMEA 0183 / NMEA 2000 Device");
                ui.label(
                    "Select a device type below to add it to your project (just like in NemaStudio):",
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for &tmpl in InstrumentTemplate::ALL {
                        if ui.button(tmpl.label()).clicked() {
                            chosen_tmpl = Some(tmpl);
                        }
                    }
                    if ui
                        .button("🚢 AIS Transponder & Target (!AIVDM / !AIVDO)")
                        .clicked()
                    {
                        chosen_ais = Some(false);
                    }
                    if ui.button("📡 ARPA Radar Target ($RATTM)").clicked() {
                        chosen_ais = Some(true);
                    }
                });
            });
            if let Some(tmpl) = chosen_tmpl {
                self.add_instrument_from_template(tmpl);
            }
            if let Some(arpa_only) = chosen_ais {
                self.add_ais_or_arpa_device(arpa_only);
            }
            return;
        }
        if let Some(tmpl) = chosen_tmpl {
            self.add_instrument_from_template(tmpl);
        }
        if let Some(arpa_only) = chosen_ais {
            self.add_ais_or_arpa_device(arpa_only);
            return;
        }
        let idx = self
            .selected_instrument_idx
            .min(self.project.instruments.len() - 1);
        self.selected_instrument_idx = idx;

        let mut manual_tx: Vec<(String, bool, Vec<String>, bool, String)> = Vec::new();
        let mut delete_inst = false;
        let vessel_snap = self.project.vessel.clone();
        let route_snap = self.project.route.clone();
        let serial_list = self.transport.available_serial_ports.clone();

        if let Some(inst) = self.project.instruments.get_mut(self.selected_instrument_idx) {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut inst.enabled, "Instrument Enabled");
                    ui.label("Name:");
                    ui.add(egui::TextEdit::singleline(&mut inst.name).desired_width(180.0));
                    ui.label("Protocol Mode:");
                    egui::ComboBox::from_id_salt("inst_proto_mode")
                        .selected_text(inst.protocol_mode.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut inst.protocol_mode,
                                ProtocolMode::Nmea0183Only,
                                ProtocolMode::Nmea0183Only.label(),
                            );
                            ui.selectable_value(
                                &mut inst.protocol_mode,
                                ProtocolMode::Nmea2000Only,
                                ProtocolMode::Nmea2000Only.label(),
                            );
                            ui.selectable_value(
                                &mut inst.protocol_mode,
                                ProtocolMode::Dual0183And2000,
                                ProtocolMode::Dual0183And2000.label(),
                            );
                        });
                    if ui.small_button("🗑 Delete Instrument").clicked() {
                        delete_inst = true;
                    }
                });
            });

            ui.add_space(4.0);
            let inst_name_for_port = inst.name.clone();
            Self::draw_device_port_selector(
                ui,
                &format!("inst_{}", self.selected_instrument_idx),
                &inst_name_for_port,
                &mut inst.target_ports,
                &mut self.project.ports,
                &serial_list,
                &mut self.transport,
            );

            ui.add_space(6.0);

            let draw_0183_panel = |ui: &mut Ui,
                                   inst: &mut Instrument,
                                   manual_tx: &mut Vec<(String, bool, Vec<String>, bool, String)>| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.heading("📄 NMEA 0183 Configuration");
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Talker ID (2 chars):");
                        ui.add(
                            egui::TextEdit::singleline(&mut inst.talker_id).desired_width(45.0),
                        );
                        for preset_t in ["GP", "GN", "HC", "WI", "SD", "II", "EC", "AG"] {
                            if ui.small_button(preset_t).clicked() {
                                inst.talker_id = preset_t.to_string();
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Fault Injection:");
                        ui.checkbox(
                            &mut inst.error_injection.corrupt_checksum,
                            "Corrupt *CS",
                        );
                        ui.checkbox(&mut inst.error_injection.omit_checksum, "Omit *CS");
                        ui.checkbox(&mut inst.error_injection.omit_crlf, "Omit CRLF");
                    });
                    ui.separator();

                    ScrollArea::vertical()
                        .id_salt("inst_0183_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for sched in &mut inst.sentences_0183 {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut sched.enabled, "");
                                    ui.strong(format!("${}{}", inst.talker_id, sched.kind.code()));
                                    ui.add(
                                        egui::DragValue::new(&mut sched.interval_sec)
                                            .speed(0.1)
                                            .range(0.1..=60.0)
                                            .suffix(" s"),
                                    );
                                    if ui.small_button("Send Now").clicked() {
                                        let lines = generate_sentence(
                                            &inst.talker_id,
                                            sched.kind,
                                            &vessel_snap,
                                            &route_snap,
                                            &inst.error_injection,
                                        );
                                        for l in lines {
                                            manual_tx.push((
                                                inst.name.clone(),
                                                false,
                                                inst.target_ports.clone(),
                                                inst.error_injection.omit_crlf,
                                                l,
                                            ));
                                        }
                                    }
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(sched.kind.description())
                                                .small()
                                                .color(pal.text_muted),
                                        )
                                        .truncate(),
                                    );
                                });
                            }
                        });
                });
            };

            let draw_2000_panel = |ui: &mut Ui,
                                   inst: &mut Instrument,
                                   manual_tx: &mut Vec<(String, bool, Vec<String>, bool, String)>| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.heading("⚡ NMEA 2000 PGN Configuration");
                    ui.horizontal(|ui| {
                        ui.label("CAN Source Address (0..253):");
                        ui.add(
                            egui::DragValue::new(&mut inst.n2k_source_addr).range(0..=253),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Wire Framing:");
                        egui::ComboBox::from_id_salt("inst_n2k_wire")
                            .selected_text(inst.n2k_wire_format.label())
                            .show_ui(ui, |ui| {
                                for wf in N2kWireFormat::ALL {
                                    ui.selectable_value(
                                        &mut inst.n2k_wire_format,
                                        wf,
                                        wf.label(),
                                    );
                                }
                            });
                    });
                    ui.separator();

                    let inst_name = inst.name.clone();
                    let src_addr = inst.n2k_source_addr;
                    let wf = inst.n2k_wire_format;
                    let err_inj = inst.error_injection.clone();
                    let t_ports = inst.target_ports.clone();

                    ScrollArea::vertical()
                        .id_salt("inst_2000_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for psched in &mut inst.pgns_2000 {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut psched.enabled, "");
                                    ui.add(
                                        egui::DragValue::new(&mut psched.interval_sec)
                                            .speed(0.1)
                                            .range(0.1..=60.0)
                                            .suffix(" s"),
                                    );
                                    if ui.small_button("Send Now").clicked() {
                                        let frames = generate_pgn_frames(
                                            psched.kind,
                                            0,
                                            0,
                                            src_addr,
                                            wf,
                                            &vessel_snap,
                                            &route_snap,
                                            &err_inj,
                                        );
                                        for f in frames {
                                            manual_tx.push((
                                                inst_name.clone(),
                                                true,
                                                t_ports.clone(),
                                                err_inj.omit_crlf,
                                                f,
                                            ));
                                        }
                                    }
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(psched.kind.label())
                                                .strong(),
                                        )
                                        .truncate(),
                                    );
                                });
                            }
                        });
                });
            };

            match inst.protocol_mode {
                ProtocolMode::Nmea0183Only => {
                    draw_0183_panel(ui, inst, &mut manual_tx);
                }
                ProtocolMode::Nmea2000Only => {
                    draw_2000_panel(ui, inst, &mut manual_tx);
                }
                ProtocolMode::Dual0183And2000 => {
                    ui.columns(2, |cols| {
                        cols[0].vertical(|ui| {
                            draw_0183_panel(ui, inst, &mut manual_tx);
                        });
                        cols[1].vertical(|ui| {
                            draw_2000_panel(ui, inst, &mut manual_tx);
                        });
                    });
                }
            }
        }

        if delete_inst && idx < self.project.instruments.len() {
            self.project.instruments.remove(idx);
            self.selected_instrument_idx = 0;
        }

        for (src, is_n2k, ports, omit_crlf, line) in manual_tx {
            self.dispatch_generated_line(&src, is_n2k, &ports, omit_crlf, line);
        }
    }

    fn draw_ports_tab(&mut self, ui: &mut Ui) {
        let pal = ThemePalette::from_ui(ui);
        let mut chosen_port_kind: Option<PortKind> = None;

        ui.horizontal_wrapped(|ui| {
            for (idx, p) in self.project.ports.iter().enumerate() {
                if ui
                    .selectable_label(self.selected_port_idx == idx, format!("🔌 {}", p.name))
                    .clicked()
                {
                    self.selected_port_idx = idx;
                }
            }
            ui.menu_button("➕ Add Communication Port", |ui| {
                for k in PortKind::ALL {
                    if ui.button(k.label()).clicked() {
                        chosen_port_kind = Some(k);
                        ui.close_menu();
                    }
                }
            });
            if ui.button("🔄 Refresh Serial Ports").clicked() {
                self.transport.refresh_serial_ports();
            }
        });
        ui.separator();

        if self.project.ports.is_empty() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.heading("🔌 Add a Communication Port");
                ui.label("Select a transport protocol below to add a port to your project:");
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for k in PortKind::ALL {
                        if ui.button(k.label()).clicked() {
                            chosen_port_kind = Some(k);
                        }
                    }
                });
            });
            if let Some(k) = chosen_port_kind {
                self.add_port_of_kind(k);
            }
            return;
        }
        if let Some(k) = chosen_port_kind {
            self.add_port_of_kind(k);
        }
        let idx = self.selected_port_idx.min(self.project.ports.len() - 1);
        self.selected_port_idx = idx;

        let mut changed = false;
        let mut delete_current = false;
        let serial_list = self.transport.available_serial_ports.clone();

        ScrollArea::vertical()
            .id_salt("ports_tab_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(p) = self.project.ports.get_mut(idx) {
                    let st = self.transport.stats.get(&p.id).cloned().unwrap_or_default();

                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .checkbox(&mut p.enabled, RichText::new("Port Active").strong())
                                .changed()
                            {
                                changed = true;
                            }
                            if ui.small_button("🗑 Delete Port").clicked() {
                                delete_current = true;
                            }
                            ui.separator();
                            let status_col = if st.is_open {
                                pal.positive
                            } else {
                                pal.warning
                            };
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "Status: {}  |  TX: {} pkts ({} B)  |  RX: {} pkts ({} B)",
                                        st.status_message,
                                        st.tx_packets,
                                        st.tx_bytes,
                                        st.rx_packets,
                                        st.rx_bytes
                                    ))
                                    .monospace()
                                    .strong()
                                    .color(status_col),
                                )
                                .truncate(),
                            );
                        });

                        ui.add_space(6.0);

                        egui::Grid::new("port_cfg_grid")
                            .num_columns(2)
                            .spacing([14.0, 8.0])
                            .show(ui, |ui| {
                                ui.label("Port Name:");
                                if ui.text_edit_singleline(&mut p.name).changed() {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("Transport Type:");
                                egui::ComboBox::from_id_salt("port_kind_cb")
                                    .selected_text(p.kind.label())
                                    .show_ui(ui, |ui| {
                                        for k in PortKind::ALL {
                                            if ui
                                                .selectable_value(&mut p.kind, k, k.label())
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        }
                                    });
                                ui.end_row();

                                ui.label("Direction:");
                                egui::ComboBox::from_id_salt("port_dir_cb")
                                    .selected_text(p.direction.label())
                                    .show_ui(ui, |ui| {
                                        for d in [
                                            PortDirection::Bidirectional,
                                            PortDirection::TxOnly,
                                            PortDirection::RxOnly,
                                        ] {
                                            if ui
                                                .selectable_value(&mut p.direction, d, d.label())
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        }
                                    });
                                ui.end_row();

                                ui.label("Protocols Routed:");
                                ui.horizontal_wrapped(|ui| {
                                    ui.checkbox(
                                        &mut p.pass_nmea0183,
                                        "NMEA 0183 ($GP..., !AIVDM)",
                                    );
                                    ui.checkbox(
                                        &mut p.pass_nmea2000,
                                        "NMEA 2000 (Yacht Devices RAW / $PCDIN / Actisense)",
                                    );
                                });
                                ui.end_row();

                                match p.kind {
                                    PortKind::UdpUnicast | PortKind::UdpBroadcast => {
                                        ui.label("Destination IP / Host:");
                                        if ui.text_edit_singleline(&mut p.remote_host).changed() {
                                            changed = true;
                                        }
                                        ui.end_row();

                                        ui.label("Destination UDP Port:");
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut p.remote_port)
                                                    .range(1..=65535),
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        ui.end_row();

                                        ui.label("Local Bind IP & Port (for RX):");
                                        ui.horizontal(|ui| {
                                            if ui
                                                .text_edit_singleline(&mut p.local_bind_host)
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut p.local_bind_port)
                                                        .range(0..=65535),
                                                )
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        });
                                        ui.end_row();
                                    }
                                    PortKind::UdpMulticast => {
                                        ui.label("IEC 61162-450 Standard Group:");
                                        egui::ComboBox::from_id_salt("port_tab_iec_mcast_cb")
                                            .selected_text(format!(
                                                "{}:{}",
                                                p.multicast_group, p.multicast_port
                                            ))
                                            .show_ui(ui, |ui| {
                                                for grp in IecMulticastGroup::ALL {
                                                    let is_sel = p.multicast_group == grp.ip()
                                                        && p.multicast_port == grp.port();
                                                    if ui
                                                        .selectable_label(is_sel, grp.label())
                                                        .clicked()
                                                    {
                                                        p.multicast_group = grp.ip().to_string();
                                                        p.multicast_port = grp.port();
                                                        changed = true;
                                                    }
                                                }
                                            });
                                        ui.end_row();

                                        ui.label("Multicast Group IP (224.0.0.0 - 239.255.255.255):");
                                        ui.horizontal(|ui| {
                                            if ui
                                                .text_edit_singleline(&mut p.multicast_group)
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui.small_button("224.0.0.1").clicked() {
                                                p.multicast_group = "224.0.0.1".to_string();
                                                changed = true;
                                            }
                                        });
                                        ui.end_row();

                                        ui.label("Multicast UDP Port:");
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut p.multicast_port)
                                                    .range(1..=65535),
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        ui.end_row();

                                        ui.label("Local Interface IP:");
                                        if ui
                                            .text_edit_singleline(&mut p.multicast_interface)
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        ui.end_row();

                                        ui.label("Multicast Options:");
                                        ui.horizontal(|ui| {
                                            ui.label("TTL:");
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut p.multicast_ttl)
                                                        .range(1..=128),
                                                )
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .checkbox(
                                                    &mut p.multicast_loopback,
                                                    "Enable Loopback",
                                                )
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        });
                                        ui.end_row();
                                    }
                                    PortKind::Serial => {
                                        ui.label("Serial Device (COM / /dev/tty...):");
                                        ui.horizontal(|ui| {
                                            if ui
                                                .text_edit_singleline(&mut p.serial_port_name)
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            egui::ComboBox::from_id_salt("detected_serial_cb")
                                                .selected_text("Detected Ports")
                                                .show_ui(ui, |ui| {
                                                    if serial_list.is_empty() {
                                                        ui.label(
                                                            "No hardware serial ports detected",
                                                        );
                                                    }
                                                    for sp in &serial_list {
                                                        if ui
                                                            .selectable_value(
                                                                &mut p.serial_port_name,
                                                                sp.clone(),
                                                                sp,
                                                            )
                                                            .changed()
                                                        {
                                                            changed = true;
                                                        }
                                                    }
                                                });
                                        });
                                        ui.end_row();

                                        ui.label("Baud Rate (bps):");
                                        ui.horizontal_wrapped(|ui| {
                                            for baud in [
                                                4800u32, 9600, 19200, 38400, 57600, 115200, 230400,
                                            ] {
                                                if ui
                                                    .selectable_value(
                                                        &mut p.baud_rate,
                                                        baud,
                                                        baud.to_string(),
                                                    )
                                                    .changed()
                                                {
                                                    changed = true;
                                                }
                                            }
                                        });
                                        ui.end_row();

                                        ui.label("Framing (Data / Parity / Stop):");
                                        ui.horizontal_wrapped(|ui| {
                                            if ui
                                                .selectable_value(&mut p.data_bits, 8, "8 Data")
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .selectable_value(&mut p.data_bits, 7, "7 Data")
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .checkbox(&mut p.parity_none, "No Parity (N)")
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .selectable_value(&mut p.stop_bits, 1, "1 Stop")
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .selectable_value(&mut p.stop_bits, 2, "2 Stop")
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        });
                                        ui.end_row();
                                    }
                                    PortKind::TcpServer => {
                                        ui.label("Listen Bind Address & Port:");
                                        ui.horizontal(|ui| {
                                            if ui
                                                .text_edit_singleline(&mut p.local_bind_host)
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut p.local_bind_port)
                                                        .range(1..=65535),
                                                )
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        });
                                        ui.end_row();
                                    }
                                    PortKind::TcpClient => {
                                        ui.label("Remote Server Host & Port:");
                                        ui.horizontal(|ui| {
                                            if ui
                                                .text_edit_singleline(&mut p.remote_host)
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut p.remote_port)
                                                        .range(1..=65535),
                                                )
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        });
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                }
            });

        if delete_current && idx < self.project.ports.len() {
            self.project.ports.remove(idx);
            self.selected_port_idx = 0;
            changed = true;
        }
        if changed {
            self.transport.sync_ports(&self.project.ports);
        }
    }

    pub fn spawn_random_ais_target(&mut self) {
        let mut rng = rand::thread_rng();
        let next_id = self.project.ais_targets.len() as u32 + 1;
        let d_lat = rng.gen_range(-0.035..0.035);
        let d_lon = rng.gen_range(-0.045..0.045);
        let cog = rng.gen_range(0.0..360.0f64).round();
        let sog = (rng.gen_range(4.0..19.0f64) * 10.0).round() / 10.0;
        let mmsi = rng.gen_range(227_000_000..250_999_999);

        self.project.ais_targets.push(TargetVessel {
            id: next_id,
            enabled: true,
            ais_class: if next_id % 2 == 0 {
                AisClass::ClassB
            } else {
                AisClass::ClassA
            },
            mmsi,
            imo: 9000000 + next_id * 137,
            name: format!("VESSEL {:02}", next_id),
            callsign: format!("FV{:03}", next_id),
            destination: "BREST".to_string(),
            ship_type: 70,
            nav_status: NavStatus::UnderWayUsingEngine,
            latitude: self.project.vessel.latitude + d_lat,
            longitude: self.project.vessel.longitude + d_lon,
            sog_knots: sog,
            cog_deg: cog,
            heading_deg: cog,
            rot_deg_per_min: 0.0,
            length_m: 95,
            beam_m: 16,
            draught_m: 5.5,
            report_interval_sec: 3.0,
            emit_arpa_ttm: true,
            time_since_dynamic_sec: 0.0,
            time_since_static_sec: 0.0,
            track_history: Vec::new(),
        });
    }

    fn draw_ais_tab(&mut self, ui: &mut Ui) {
        let pal = ThemePalette::from_ui(ui);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.project.ais_config.enabled,
                    "🚢 AIS / ARPA Simulator Active",
                );
                ui.checkbox(
                    &mut self.project.ais_config.emit_own_ship_vdo,
                    "Emit Own Ship !AIVDO",
                );
                ui.checkbox(
                    &mut self.project.ais_config.emit_nmea0183_vdm,
                    "Emit NMEA 0183 !AIVDM",
                );
                ui.checkbox(
                    &mut self.project.ais_config.emit_nmea2000_pgns,
                    "Emit NMEA 2000 AIS PGNs (129038/129039)",
                );
                if ui.button("➕ Spawn Random Target").clicked() {
                    self.spawn_random_ais_target();
                    self.selected_ais_idx = self.project.ais_targets.len() - 1;
                }
            });
        });

        ui.add_space(4.0);
        let serial_list = self.transport.available_serial_ports.clone();
        Self::draw_device_port_selector(
            ui,
            "ais_arpa_dev",
            "AIS / ARPA",
            &mut self.project.ais_config.target_ports,
            &mut self.project.ports,
            &serial_list,
            &mut self.transport,
        );

        ui.add_space(6.0);

        // Fleet overview table + Target Editor
        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                ui.heading("🚢 Target Fleet & CPA/TCPA Status");
                ScrollArea::vertical()
                    .id_salt("ais_fleet_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (idx, tgt) in self.project.ais_targets.iter_mut().enumerate() {
                            let cpa = tgt.compute_cpa(&self.project.vessel);
                            let is_alarm =
                                cpa.cpa_nm < 0.5 && cpa.tcpa_min > 0.0 && cpa.tcpa_min < 30.0;
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut tgt.enabled, "");
                                    if ui
                                        .selectable_label(
                                            self.selected_ais_idx == idx,
                                            RichText::new(format!(
                                                "🚢 {} (MMSI {})",
                                                tgt.name, tgt.mmsi
                                            ))
                                            .strong(),
                                        )
                                        .clicked()
                                    {
                                        self.selected_ais_idx = idx;
                                    }
                                    if is_alarm {
                                        ui.label(
                                            RichText::new("⚠ CPA ALARM")
                                                .color(pal.danger)
                                                .strong(),
                                        );
                                    }
                                });
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!(
                                            "Rng: {:5.2} NM | Brg: {:05.1}°T | SOG: {:4.1} kn | COG: {:05.1}°T",
                                            cpa.range_nm,
                                            cpa.bearing_true_deg,
                                            tgt.sog_knots,
                                            tgt.cog_deg
                                        ))
                                        .monospace(),
                                    )
                                    .truncate(),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!(
                                            "CPA: {:5.2} NM | TCPA: {:+5.1} min | {}",
                                            cpa.cpa_nm,
                                            cpa.tcpa_min,
                                            tgt.ais_class.label()
                                        ))
                                        .monospace(),
                                    )
                                    .truncate(),
                                );
                            });
                        }
                    });
            });

            cols[1].vertical(|ui| {
                ui.heading("🎯 Selected Target Editor");
                if self.project.ais_targets.is_empty() {
                    ui.label("No AIS targets configured.");
                    return;
                }
                let idx = self
                    .selected_ais_idx
                    .min(self.project.ais_targets.len() - 1);
                self.selected_ais_idx = idx;

                let own_lat = self.project.vessel.latitude;
                let own_lon = self.project.vessel.longitude;
                let mut burst_target: Option<TargetVessel> = None;

                ScrollArea::vertical()
                    .id_salt("ais_target_editor_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(tgt) = self.project.ais_targets.get_mut(idx) {
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                egui::Grid::new("ais_edit_grid")
                                    .num_columns(2)
                                    .spacing([12.0, 6.0])
                                    .show(ui, |ui| {
                                        ui.label("Vessel Name / Callsign:");
                                        ui.horizontal(|ui| {
                                            ui.add(
                                                egui::TextEdit::singleline(&mut tgt.name)
                                                    .desired_width(120.0),
                                            );
                                            ui.add(
                                                egui::TextEdit::singleline(&mut tgt.callsign)
                                                    .desired_width(70.0),
                                            );
                                        });
                                        ui.end_row();

                                        ui.label("MMSI / IMO:");
                                        ui.horizontal(|ui| {
                                            ui.add(egui::DragValue::new(&mut tgt.mmsi));
                                            ui.add(
                                                egui::DragValue::new(&mut tgt.imo).prefix("IMO "),
                                            );
                                        });
                                        ui.end_row();

                                        ui.label("Target Class:");
                                        egui::ComboBox::from_id_salt("ais_class_cb")
                                            .selected_text(tgt.ais_class.label())
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut tgt.ais_class,
                                                    AisClass::ClassA,
                                                    AisClass::ClassA.label(),
                                                );
                                                ui.selectable_value(
                                                    &mut tgt.ais_class,
                                                    AisClass::ClassB,
                                                    AisClass::ClassB.label(),
                                                );
                                                ui.selectable_value(
                                                    &mut tgt.ais_class,
                                                    AisClass::ArpaOnly,
                                                    AisClass::ArpaOnly.label(),
                                                );
                                            });
                                        ui.end_row();

                                        ui.label("Navigation Status:");
                                        egui::ComboBox::from_id_salt("ais_nav_status_cb")
                                            .selected_text(tgt.nav_status.label())
                                            .show_ui(ui, |ui| {
                                                for ns in NavStatus::ALL {
                                                    ui.selectable_value(
                                                        &mut tgt.nav_status,
                                                        ns,
                                                        ns.label(),
                                                    );
                                                }
                                            });
                                        ui.end_row();

                                        ui.label("Latitude / Longitude:");
                                        ui.horizontal(|ui| {
                                            ui.add(
                                                egui::DragValue::new(&mut tgt.latitude)
                                                    .speed(0.0005)
                                                    .fixed_decimals(5),
                                            );
                                            ui.add(
                                                egui::DragValue::new(&mut tgt.longitude)
                                                    .speed(0.0005)
                                                    .fixed_decimals(5),
                                            );
                                        });
                                        ui.end_row();

                                        ui.label("SOG (knots):");
                                        ui.add(
                                            egui::Slider::new(&mut tgt.sog_knots, 0.0..=40.0)
                                                .suffix(" kn"),
                                        );
                                        ui.end_row();

                                        ui.label("COG / Heading (°T):");
                                        ui.horizontal(|ui| {
                                            if ui
                                                .add(
                                                    egui::Slider::new(
                                                        &mut tgt.cog_deg,
                                                        0.0..=359.0,
                                                    )
                                                    .suffix("°"),
                                                )
                                                .changed()
                                            {
                                                tgt.heading_deg = tgt.cog_deg;
                                            }
                                        });
                                        ui.end_row();

                                        ui.label("Report Interval:");
                                        ui.add(
                                            egui::Slider::new(
                                                &mut tgt.report_interval_sec,
                                                0.5..=30.0,
                                            )
                                            .suffix(" s"),
                                        );
                                        ui.end_row();

                                        ui.label("Radar $RATTM:");
                                        ui.checkbox(&mut tgt.emit_arpa_ttm, "Emit $RATTM");
                                        ui.end_row();
                                    });

                                ui.separator();
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("🎯 Aim Toward Own Ship (CPA Test)").clicked() {
                                        let (_dist, brg) =
                                            crate::model::route::Route::distance_and_bearing_nm(
                                                tgt.latitude,
                                                tgt.longitude,
                                                own_lat,
                                                own_lon,
                                            );
                                        tgt.cog_deg = brg;
                                        tgt.heading_deg = brg;
                                    }
                                    if ui.button("📡 Transmit AIS + ARPA Burst Now").clicked() {
                                        burst_target = Some(tgt.clone());
                                    }
                                });
                            });
                        }
                    });

                if let Some(t) = burst_target {
                    let ais_ports = self.project.ais_config.target_ports.clone();
                    let vdm = encode_ais_position_report(
                        false,
                        t.mmsi,
                        t.nav_status.code(),
                        t.rot_deg_per_min,
                        t.sog_knots,
                        t.latitude,
                        t.longitude,
                        t.cog_deg,
                        t.heading_deg,
                        t.ais_class == AisClass::ClassB,
                    );
                    self.dispatch_generated_line(
                        &format!("AIS: {}", t.name),
                        false,
                        &ais_ports,
                        false,
                        vdm,
                    );
                    for s in encode_ais_static_report(&t) {
                        self.dispatch_generated_line(
                            &format!("AIS Static: {}", t.name),
                            false,
                            &ais_ports,
                            false,
                            s,
                        );
                    }
                    let ttm = encode_arpa_ttm(&t, &self.project.vessel);
                    self.dispatch_generated_line(
                        &format!("ARPA: {}", t.name),
                        false,
                        &ais_ports,
                        false,
                        ttm,
                    );
                }
            });
        });
    }

    fn draw_recorder_tab(&mut self, ui: &mut Ui) {
        let pal = ThemePalette::from_ui(ui);
        ScrollArea::vertical()
            .id_salt("recorder_tab_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.columns(2, |cols| {
                    // Left: Stream Recorder
                    cols[0].vertical(|ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("⏺ NMEA Stream Recorder");
                            ui.horizontal(|ui| {
                                ui.label("Output Log File:");
                                ui.text_edit_singleline(&mut self.recorder.record_file_path);
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.recorder.record_tx, "Record Generated TX");
                                ui.checkbox(&mut self.recorder.record_rx, "Record Incoming RX");
                            });
                            ui.horizontal_wrapped(|ui| {
                                if !self.recorder.is_recording {
                                    if ui.button("⏺ Start Recording (Overwrite)").clicked() {
                                        self.recorder.start_recording(false);
                                    }
                                    if ui.button("➕ Start Recording (Append)").clicked() {
                                        self.recorder.start_recording(true);
                                    }
                                } else if ui.button("⏹ Stop Recording").clicked() {
                                    self.recorder.stop_recording();
                                }
                            });
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "Status: {} | Recorded Lines: {}",
                                        self.recorder.recorder_status,
                                        self.recorder.recorded_lines_count
                                    ))
                                    .color(pal.accent),
                                )
                                .truncate(),
                            );
                        });
                    });

                    // Right: Log File Player
                    cols[1].vertical(|ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("▶ NMEA Log File Player");
                            ui.horizontal(|ui| {
                                ui.label("Log File Path:");
                                ui.text_edit_singleline(&mut self.recorder.play_file_path);
                                if ui.button("📂 Load File").clicked() {
                                    self.recorder.load_file();
                                }
                                if ui.button("Built-in Sample").clicked() {
                                    self.recorder.load_builtin_sample();
                                }
                            });

                            ui.horizontal_wrapped(|ui| {
                                let play_lbl = if self.recorder.is_playing {
                                    "⏸ Pause Playback"
                                } else {
                                    "▶ Play Log to Active Ports"
                                };
                                if ui.button(play_lbl).clicked() {
                                    self.recorder.is_playing = !self.recorder.is_playing;
                                }
                                if ui.button("⏭ Step 1 Frame").clicked() {
                                    if let Some(line) = self.recorder.step_one() {
                                        let is_n2k = line.starts_with("$PCDIN,")
                                            || (!line.starts_with('$') && !line.starts_with('!'));
                                        self.dispatch_generated_line(
                                            "Log Player",
                                            is_n2k,
                                            &[],
                                            false,
                                            line,
                                        );
                                    }
                                }
                                ui.checkbox(&mut self.recorder.loop_playback, "Loop");
                            });

                            ui.horizontal(|ui| {
                                ui.label("Playback Rate:");
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.recorder.lines_per_second,
                                        0.5..=100.0,
                                    )
                                    .suffix(" lines/s")
                                    .logarithmic(true),
                                );
                            });

                            if !self.recorder.loaded_lines.is_empty() {
                                let max_idx = self.recorder.loaded_lines.len().saturating_sub(1);
                                ui.horizontal(|ui| {
                                    ui.label("Position:");
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.recorder.current_line_idx,
                                            0..=max_idx,
                                        )
                                        .suffix(format!(" / {}", self.recorder.loaded_lines.len())),
                                    );
                                });
                            }
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&self.recorder.player_status)
                                        .small()
                                        .color(pal.accent),
                                )
                                .truncate(),
                            );
                        });
                    });
                });
            });
    }

    fn draw_custom_builder_tab(&mut self, ui: &mut Ui) {
        let pal = ThemePalette::from_ui(ui);
        let mut to_send: Vec<(String, bool, String)> = Vec::new();

        ScrollArea::vertical()
            .id_salt("custom_builder_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.columns(2, |cols| {
                    // Left: Custom NMEA 0183 Sentence Studio & Checksum Calculator
                    cols[0].vertical(|ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("🔬 NMEA 0183 Custom Sentence Studio");
                            ui.label("Sentence body (without leading $/! or trailing *CS):");
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt("custom_0183_prefix")
                                    .width(45.0)
                                    .selected_text(self.custom_builder.nmea0183_prefix.to_string())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.custom_builder.nmea0183_prefix,
                                            '$',
                                            "$",
                                        );
                                        ui.selectable_value(
                                            &mut self.custom_builder.nmea0183_prefix,
                                            '!',
                                            "!",
                                        );
                                    });
                                ui.text_edit_singleline(&mut self.custom_builder.nmea0183_body);
                            });

                            let exact_cs = compute_checksum(&self.custom_builder.nmea0183_body);
                            let err = ErrorInjectionConfig {
                                corrupt_checksum: self.custom_builder.nmea0183_corrupt_cs,
                                omit_checksum: false,
                                omit_crlf: false,
                            };
                            let full_sentence = wrap_sentence(
                                self.custom_builder.nmea0183_prefix,
                                &self.custom_builder.nmea0183_body,
                                &err,
                            );

                            ui.horizontal_wrapped(|ui| {
                                ui.label("XOR Checksum:");
                                ui.strong(
                                    RichText::new(format!("✔ *{:02X} (dec {})", exact_cs, exact_cs))
                                        .color(pal.positive),
                                );
                                ui.checkbox(
                                    &mut self.custom_builder.nmea0183_corrupt_cs,
                                    "Inject Bad Checksum",
                                );
                            });

                            ui.label("Wire Preview:");
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&full_sentence)
                                        .monospace()
                                        .color(pal.text_primary),
                                )
                                .truncate(),
                            );

                            ui.horizontal_wrapped(|ui| {
                                if ui.button("📡 Transmit Sentence Once").clicked() {
                                    to_send.push((
                                        "Custom 0183".to_string(),
                                        false,
                                        full_sentence.clone(),
                                    ));
                                }
                                ui.checkbox(
                                    &mut self.custom_builder.nmea0183_periodic,
                                    "Auto-Repeat every:",
                                );
                                ui.add(
                                    egui::DragValue::new(
                                        &mut self.custom_builder.nmea0183_interval_sec,
                                    )
                                    .speed(0.1)
                                    .range(0.1..=60.0)
                                    .suffix(" s"),
                                );
                            });
                        });
                    });

                    // Right: Custom NMEA 2000 PGN Frame Studio
                    cols[1].vertical(|ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.heading("⚡ NMEA 2000 Custom PGN Builder");
                            egui::Grid::new("custom_n2k_grid")
                                .num_columns(2)
                                .spacing([10.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("PGN Number:");
                                    ui.add(
                                        egui::DragValue::new(&mut self.custom_builder.n2k_pgn)
                                            .range(59392..=131071),
                                    );
                                    ui.end_row();

                                    ui.label("Priority (0..7) / Source (0..253):");
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.custom_builder.n2k_priority,
                                            )
                                            .range(0..=7)
                                            .prefix("Prio "),
                                        );
                                        ui.add(
                                            egui::DragValue::new(
                                                &mut self.custom_builder.n2k_source,
                                            )
                                            .range(0..=253)
                                            .prefix("Src "),
                                        );
                                    });
                                    ui.end_row();

                                    ui.label("Framing Format:");
                                    egui::ComboBox::from_id_salt("custom_n2k_wf")
                                        .selected_text(self.custom_builder.n2k_wire_format.label())
                                        .show_ui(ui, |ui| {
                                            for wf in N2kWireFormat::ALL {
                                                ui.selectable_value(
                                                    &mut self.custom_builder.n2k_wire_format,
                                                    wf,
                                                    wf.label(),
                                                );
                                            }
                                        });
                                    ui.end_row();

                                    ui.label("Hex Payload Bytes:");
                                    ui.text_edit_singleline(
                                        &mut self.custom_builder.n2k_hex_payload,
                                    );
                                    ui.end_row();
                                });

                            let payload = parse_hex_bytes(&self.custom_builder.n2k_hex_payload);
                            let preview_frames = format_n2k_frames(
                                self.custom_builder.n2k_pgn,
                                self.custom_builder.n2k_priority,
                                self.custom_builder.n2k_source,
                                payload.len() > 8,
                                0,
                                &payload,
                                self.custom_builder.n2k_wire_format,
                                &ErrorInjectionConfig::default(),
                            );

                            ui.label("Generated Frame Preview:");
                            ScrollArea::vertical()
                                .id_salt("custom_n2k_preview_scroll")
                                .max_height(48.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    for pf in &preview_frames {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(pf)
                                                    .monospace()
                                                    .color(pal.text_primary),
                                            )
                                            .truncate(),
                                        );
                                    }
                                });

                            ui.horizontal_wrapped(|ui| {
                                if ui.button("⚡ Transmit PGN Now").clicked() {
                                    for pf in preview_frames {
                                        to_send.push(("Custom N2K".to_string(), true, pf));
                                    }
                                }
                                ui.checkbox(
                                    &mut self.custom_builder.n2k_periodic,
                                    "Auto-Repeat every:",
                                );
                                ui.add(
                                    egui::DragValue::new(
                                        &mut self.custom_builder.n2k_interval_sec,
                                    )
                                    .speed(0.1)
                                    .range(0.1..=60.0)
                                    .suffix(" s"),
                                );
                            });
                        });
                    });
                });
            });

        for (src, is_n2k, line) in to_send {
            self.dispatch_generated_line(&src, is_n2k, &[], false, line);
        }
    }

    fn draw_traffic_monitor_and_inspector(&mut self, ui: &mut Ui) {
        let pal = ThemePalette::from_ui(ui);
        let search_upper = self.filter_search.trim().to_uppercase();
        let filtered: Vec<TrafficLogEntry> = self
            .traffic_log
            .iter()
            .filter(|e| {
                matches_traffic_filter(
                    e,
                    self.filter_show_tx,
                    self.filter_show_rx,
                    self.filter_show_0183,
                    self.filter_show_n2k,
                    &search_upper,
                )
            })
            .cloned()
            .collect();

        // Determine the active linked entry (either pinned by user click, or live-following latest)
        let is_pinned = self.selected_packet_seq.is_some();
        let active_entry: Option<TrafficLogEntry> = if let Some(pinned_seq) = self.selected_packet_seq {
            self.traffic_log
                .iter()
                .find(|e| e.seq_id == pinned_seq)
                .cloned()
        } else {
            filtered.last().cloned()
        };

        // Keep decoded packet synced when following latest or when pinned entry is found
        if !is_pinned {
            if let Some(ref latest) = active_entry {
                if self.selected_packet_raw != latest.raw_line {
                    self.selected_packet_raw = latest.raw_line.clone();
                    self.selected_packet_decoded = Some(inspect_packet(&latest.raw_line));
                }
            }
        }

        let active_seq = self
            .selected_packet_seq
            .or_else(|| active_entry.as_ref().map(|e| e.seq_id));

        let mut newly_selected: Option<Option<(u64, String)>> = None;

        // =====================================================================
        // RIGHT RESIZABLE PANEL: PACKET DECODER & INSPECTOR
        // Users can drag the vertical splitter between the Traffic Console and
        // the Packet Decoder, as well as the top edge of the bottom dock.
        // =====================================================================
        egui::SidePanel::right("packet_decoder_split_panel")
            .resizable(true)
            .default_width(500.0)
            .width_range(260.0..=920.0)
            .show_inside(ui, |ui| {
                ui.set_clip_rect(ui.max_rect());
                egui::Frame::group(ui.style())
                    .fill(pal.card_fill)
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());

                        // Decoder Header Bar with explicit link state
                        ui.horizontal(|ui| {
                            ui.strong("🔬 Packet Decoder");
                            ui.separator();
                            if let Some(seq) = active_seq {
                                if is_pinned {
                                    ui.label(
                                        RichText::new(format!("📌 PINNED #{:05}", seq))
                                            .color(pal.warning)
                                            .monospace()
                                            .strong(),
                                    );
                                    if ui.small_button("▶ Follow Live").clicked() {
                                        newly_selected = Some(None);
                                    }
                                } else {
                                    ui.label(
                                        RichText::new(format!("◎ LIVE FOLLOW #{:05}", seq))
                                            .color(pal.positive)
                                            .monospace()
                                            .strong(),
                                    );
                                }
                            } else {
                                ui.label(
                                    RichText::new("◎ No Frame Selected")
                                        .color(pal.text_muted)
                                        .small(),
                                );
                            }
                        });

                        ui.add_space(2.0);

                        if let Some(ref dec) = self.selected_packet_decoded {
                            // Dedicated Linked Frame Banner connecting the Traffic Console row to the Decoder
                            let link_border_col = if is_pinned {
                                pal.warning
                            } else {
                                pal.accent
                            };
                            egui::Frame::group(ui.style())
                                .stroke(egui::Stroke::new(1.5, link_border_col))
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width());
                                    // Metadata link line (#Seq, Timestamp, Direction, Source, Protocol, Checksum)
                                    ui.horizontal_wrapped(|ui| {
                                        if let Some(ref entry) = active_entry {
                                            let dir_str = match entry.direction {
                                                TrafficDirection::Tx => "TX",
                                                TrafficDirection::Rx => "RX",
                                            };
                                            ui.label(
                                                RichText::new(format!(
                                                    "#{:05}  |  {}  |  {}  |  {}",
                                                    entry.seq_id,
                                                    entry.timestamp,
                                                    dir_str,
                                                    entry.source_name
                                                ))
                                                .small()
                                                .monospace()
                                                .color(pal.text_secondary),
                                            );
                                            ui.separator();
                                        }
                                        ui.label(
                                            RichText::new(&dec.protocol)
                                                .small()
                                                .color(pal.accent)
                                                .strong(),
                                        );
                                        if let Some(valid) = dec.checksum_valid {
                                            if valid {
                                                ui.label(
                                                    RichText::new("✔ CHECKSUM OK")
                                                        .color(pal.positive)
                                                        .small()
                                                        .strong(),
                                                );
                                            } else {
                                                ui.label(
                                                    RichText::new("✖ CHECKSUM INVALID")
                                                        .color(pal.danger)
                                                        .small()
                                                        .strong(),
                                                );
                                            }
                                        }
                                    });

                                    // Raw frame line
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&self.selected_packet_raw)
                                                .monospace()
                                                .strong()
                                                .color(pal.text_primary),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(&self.selected_packet_raw);

                                    // Decoded summary
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&dec.summary)
                                                .small()
                                                .color(pal.warning)
                                                .strong(),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(&dec.summary);
                                });

                            ui.add_space(2.0);

                            // Scrollable decoded fields table
                            ScrollArea::vertical()
                                .id_salt("inspector_fields_scroll")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    egui::Grid::new("decoded_fields_grid")
                                        .striped(true)
                                        .num_columns(2)
                                        .spacing([14.0, 4.0])
                                        .show(ui, |ui| {
                                            for (k, v) in &dec.fields {
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(k)
                                                            .color(pal.text_secondary)
                                                            .strong(),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(v)
                                                            .monospace()
                                                            .color(pal.text_primary),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.end_row();
                                            }
                                        });
                                });
                        } else {
                            ui.label(
                                "Click any frame in the Traffic Console on the left to pin and decode its fields.",
                            );
                        }
                    });
            });

        // =====================================================================
        // LEFT PANEL: LIVE TRAFFIC STREAM CONSOLE
        // =====================================================================
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                ui.set_clip_rect(ui.max_rect());
                egui::Frame::group(ui.style())
                    .fill(pal.card_fill)
                    .show(ui, |ui| {
                        // Traffic Stream Toolbar
                        ui.horizontal(|ui| {
                            ui.strong("📡 Traffic Stream");
                            let pause_txt = if self.monitor_paused {
                                RichText::new("▶ Resume").color(pal.positive).strong()
                            } else {
                                RichText::new("⏸ Freeze").color(pal.warning).strong()
                            };
                            if ui.button(pause_txt).clicked() {
                                self.monitor_paused = !self.monitor_paused;
                            }
                            if ui.button("🗑 Clear").clicked() {
                                self.traffic_log.clear();
                            }
                            ui.separator();
                            ui.checkbox(&mut self.filter_show_tx, "TX");
                            ui.checkbox(&mut self.filter_show_rx, "RX");
                            ui.checkbox(&mut self.filter_show_0183, "0183");
                            ui.checkbox(&mut self.filter_show_n2k, "N2K");
                            ui.separator();
                            ui.label("🔍");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.filter_search)
                                    .hint_text("Filter (RMC, AIVDM, 129025...)")
                                    .desired_width(145.0),
                            );
                        });
                        ui.separator();

                        let text_height =
                            egui::TextStyle::Monospace.resolve(ui.style()).size + 4.0;

                        TableBuilder::new(ui)
                            .striped(true)
                            .resizable(true)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(Column::initial(64.0).at_least(52.0).clip(true)) // #Seq (link indicator)
                            .column(Column::initial(78.0).at_least(60.0).clip(true)) // Time
                            .column(Column::initial(34.0).at_least(28.0).clip(true)) // Dir
                            .column(Column::initial(42.0).at_least(36.0).clip(true)) // Proto
                            .column(Column::initial(110.0).at_least(60.0).clip(true)) // Source
                            .column(Column::remainder().clip(true)) // Raw Frame
                            .stick_to_bottom(!self.monitor_paused && !is_pinned)
                            .header(18.0, |mut header| {
                                header.col(|ui| {
                                    ui.strong("#Seq");
                                });
                                header.col(|ui| {
                                    ui.strong("Time");
                                });
                                header.col(|ui| {
                                    ui.strong("Dir");
                                });
                                header.col(|ui| {
                                    ui.strong("Proto");
                                });
                                header.col(|ui| {
                                    ui.strong("Source / Port");
                                });
                                header.col(|ui| {
                                    ui.strong("Raw Frame (Click to Pin in Decoder)");
                                });
                            })
                            .body(|body| {
                                body.rows(text_height, filtered.len(), |mut row| {
                                    let entry = &filtered[row.index()];
                                    let is_active = active_seq == Some(entry.seq_id);
                                    row.set_selected(is_active);

                                    row.col(|ui| {
                                        let (seq_txt, seq_col) = if is_active {
                                            if is_pinned {
                                                (
                                                    format!("📌#{:04}", entry.seq_id % 10000),
                                                    pal.warning,
                                                )
                                            } else {
                                                (
                                                    format!("▶ #{:04}", entry.seq_id % 10000),
                                                    pal.accent,
                                                )
                                            }
                                        } else {
                                            (
                                                format!("  #{:04}", entry.seq_id % 10000),
                                                pal.text_muted,
                                            )
                                        };
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new(seq_txt)
                                                        .monospace()
                                                        .color(seq_col)
                                                        .strong(),
                                                )
                                                .frame(false)
                                                .truncate(),
                                            )
                                            .clicked()
                                        {
                                            if self.selected_packet_seq == Some(entry.seq_id) {
                                                newly_selected = Some(None);
                                            } else {
                                                newly_selected = Some(Some((
                                                    entry.seq_id,
                                                    entry.raw_line.clone(),
                                                )));
                                            }
                                        }
                                    });
                                    row.col(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&entry.timestamp).monospace(),
                                            )
                                            .truncate(),
                                        );
                                    });
                                    row.col(|ui| {
                                        let (txt, col) = match entry.direction {
                                            TrafficDirection::Tx => ("TX", pal.accent),
                                            TrafficDirection::Rx => ("RX", pal.positive),
                                        };
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(txt).color(col).monospace().strong(),
                                            )
                                            .truncate(),
                                        );
                                    });
                                    row.col(|ui| {
                                        let (txt, col) = if entry.is_nmea2000 {
                                            ("N2K", pal.warning)
                                        } else {
                                            ("0183", pal.nmea0183_teal)
                                        };
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(txt).color(col).monospace().strong(),
                                            )
                                            .truncate(),
                                        );
                                    });
                                    row.col(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&entry.source_name)
                                                    .small()
                                                    .color(pal.text_secondary),
                                            )
                                            .truncate(),
                                        );
                                    });
                                    row.col(|ui| {
                                        let line_col = if entry.is_nmea2000 {
                                            pal.warning
                                        } else if entry.raw_line.starts_with('!') {
                                            pal.ais_cyan
                                        } else {
                                            pal.text_primary
                                        };
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new(&entry.raw_line)
                                                        .monospace()
                                                        .color(line_col),
                                                )
                                                .selected(is_active)
                                                .frame(is_active)
                                                .truncate(),
                                            )
                                            .clicked()
                                        {
                                            if self.selected_packet_seq == Some(entry.seq_id) {
                                                newly_selected = Some(None);
                                            } else {
                                                newly_selected = Some(Some((
                                                    entry.seq_id,
                                                    entry.raw_line.clone(),
                                                )));
                                            }
                                        }
                                    });
                                });
                            });
                    });
            });

        if let Some(sel_change) = newly_selected {
            match sel_change {
                Some((seq, raw)) => {
                    self.selected_packet_seq = Some(seq);
                    self.selected_packet_decoded = Some(inspect_packet(&raw));
                    self.selected_packet_raw = raw;
                }
                None => {
                    self.selected_packet_seq = None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_bytes_and_matches_traffic_filter() {
        assert_eq!(parse_hex_bytes("AA BB 01"), vec![0xAA, 0xBB, 0x01]);

        let tx_0183 = TrafficLogEntry {
            seq_id: 1,
            timestamp: "12:00:00.000".into(),
            port_id: "p1".into(),
            port_name: "UDP".into(),
            source_name: "GNSS".into(),
            direction: TrafficDirection::Tx,
            is_nmea2000: false,
            raw_line: "$GPRMC,123519,A*00".into(),
        };

        assert!(matches_traffic_filter(&tx_0183, true, true, true, true, ""));
        assert!(!matches_traffic_filter(&tx_0183, false, true, true, true, ""));
        assert!(!matches_traffic_filter(&tx_0183, true, true, false, true, ""));
        assert!(matches_traffic_filter(&tx_0183, true, true, true, true, "RMC"));
        assert!(!matches_traffic_filter(&tx_0183, true, true, true, true, "129025"));
    }

    #[test]
    fn test_app_step_simulation_and_dispatch_and_ais_spawn() {
        let mut app = NmeaStudioApp::default();
        // Verify default project starts completely empty / unconfigured
        assert!(app.project.instruments.is_empty());
        assert!(app.project.ports.is_empty());
        assert!(app.project.ais_targets.is_empty());
        assert!(app.project.route.waypoints.is_empty());

        // Verify adding individual devices & ports from an empty project (NemaStudio style)
        app.add_instrument_from_template(InstrumentTemplate::GpsGnss);
        app.add_instrument_from_template(InstrumentTemplate::GyroCompass);
        app.add_instrument_from_template(InstrumentTemplate::WindWeather);
        assert_eq!(app.project.instruments.len(), 3);
        // Verify all added devices default to NMEA 0183 Only
        for inst in &app.project.instruments {
            assert_eq!(inst.protocol_mode, ProtocolMode::Nmea0183Only);
        }
        app.add_ais_or_arpa_device(false);
        app.add_ais_or_arpa_device(true);
        assert!(app.project.ais_config.enabled);
        assert!(!app.project.ais_config.emit_nmea2000_pgns);
        assert_eq!(app.project.ais_targets.len(), 2);

        // Verify per-device port creation & selection (Serial, UDP Unicast, UDP Multicast)
        let p_ser = NmeaStudioApp::create_port_config_of_kind(0, PortKind::Serial, Some("GPS"));
        assert_eq!(p_ser.kind, PortKind::Serial);
        assert_eq!(p_ser.baud_rate, 4800);
        let p_uni = NmeaStudioApp::create_port_config_of_kind(1, PortKind::UdpUnicast, Some("Gyro"));
        assert_eq!(p_uni.kind, PortKind::UdpUnicast);
        let p_mcast = NmeaStudioApp::create_port_config_of_kind(2, PortKind::UdpMulticast, Some("AIS"));
        assert_eq!(p_mcast.kind, PortKind::UdpMulticast);

        app.add_port_of_kind(PortKind::UdpUnicast);
        app.add_port_of_kind(PortKind::UdpMulticast);
        assert_eq!(app.project.ports.len(), 2);
        // Assign first instrument to first port specifically
        let first_port_id = app.project.ports[0].id.clone();
        app.project.instruments[0].target_ports = vec![first_port_id.clone()];
        assert_eq!(app.project.instruments[0].target_ports, vec![first_port_id]);

        // Load Preset #1 (Brest Approaches Dual 0183 + N2K)
        app.project = StudioProject::load_preset(1);
        for p in &mut app.project.ports {
            p.enabled = false;
        }
        app.transport.sync_ports(&app.project.ports);

        let initial_ais_count = app.project.ais_targets.len();
        app.spawn_random_ais_target();
        assert_eq!(app.project.ais_targets.len(), initial_ais_count + 1);

        // Enable custom periodic 0183 & N2K builders as well
        app.custom_builder.nmea0183_periodic = true;
        app.custom_builder.n2k_periodic = true;

        // Step simulation by 2.0 seconds -> should generate 0183, N2K, AIS, ARPA, and Custom lines
        app.step_simulation(2.0);
        assert!(!app.traffic_log.is_empty());
        assert!(app.traffic_log.iter().any(|e| !e.is_nmea2000));
        assert!(app.traffic_log.iter().any(|e| e.is_nmea2000));
        assert!(app.selected_packet_decoded.is_some());
    }

    #[test]
    fn test_project_file_dialog_and_save_load_roundtrip() {
        let mut app = NmeaStudioApp::default();
        app.project.project_name = "Custom User Preset".to_string();
        app.project.vessel.vessel_name = "OCEAN EXPLORER".to_string();
        app.project.instruments.push(Instrument::new_custom(1));

        let tmp_file = std::env::temp_dir().join("unit_test_preset_no_ext");
        let tmp_dir = tmp_file
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let expected_json = tmp_file
            .with_extension("json")
            .to_string_lossy()
            .to_string();

        let mut dlg = ProjectFileDialog::default();
        dlg.open_save(&tmp_file.to_string_lossy());
        assert_eq!(dlg.mode, FileDialogMode::SaveProject);
        assert_eq!(dlg.current_dir, tmp_dir);
        // Should automatically append .json in SaveProject mode
        assert_eq!(dlg.resolved_file_path(), expected_json);

        let save_path = dlg.resolved_file_path();
        app.save_project_to_path(&save_path).unwrap();

        dlg.open_load(&save_path);
        assert_eq!(dlg.mode, FileDialogMode::LoadProject);
        assert!(dlg
            .entries
            .iter()
            .any(|e| e.name == "unit_test_preset_no_ext.json"));

        dlg.navigate_up();
        dlg.navigate_to(&tmp_dir);
        assert_eq!(dlg.current_dir, tmp_dir);

        let mut app2 = NmeaStudioApp::default();
        assert!(app2.project.instruments.is_empty());
        app2.load_project_from_path(&save_path).unwrap();
        assert_eq!(app2.project.project_name, "Custom User Preset");
        assert_eq!(app2.project.vessel.vessel_name, "OCEAN EXPLORER");
        assert_eq!(app2.project.instruments.len(), 1);

        let _ = std::fs::remove_file(&save_path);
    }

    #[test]
    fn test_headless_ui_rendering_all_tabs_and_panels() {
        let ctx = egui::Context::default();
        configure_studio_theme(&ctx);
        assert_eq!(
            ctx.options(|opt| opt.theme_preference),
            egui::ThemePreference::System
        );

        let dark_pal = ThemePalette::for_dark(true);
        let light_pal = ThemePalette::for_dark(false);
        assert_ne!(dark_pal.card_fill, light_pal.card_fill);
        assert_ne!(dark_pal.text_primary, light_pal.text_primary);

        let mut app = NmeaStudioApp::default();
        // Test rendering both with empty project + open file dialog AND with loaded preset #1
        app.file_dialog.open_save("/tmp/test_preset.json");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.draw_project_file_dialog(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                app.draw_instruments_tab(ui);
                app.draw_ports_tab(ui);
            });
        });
        app.file_dialog.mode = FileDialogMode::Closed;

        app.project = StudioProject::load_preset(1);
        for p in &mut app.project.ports {
            p.enabled = false;
        }
        app.transport.sync_ports(&app.project.ports);
        app.step_simulation(1.0);

        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            ctx.set_theme(theme);
            for tab in [
                WorkspaceTab::HelmAndSensors,
                WorkspaceTab::ChartAndRadar,
                WorkspaceTab::InstrumentsAndPgns,
                WorkspaceTab::PortsAndNetwork,
                WorkspaceTab::AisAndArpa,
                WorkspaceTab::LogRecorderPlayer,
                WorkspaceTab::CustomBuilder,
            ] {
                app.active_tab = tab;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::SidePanel::left("test_left").show(ctx, |ui| {
                        app.draw_left_explorer(ui);
                    });
                    egui::TopBottomPanel::bottom("test_bottom").show(ctx, |ui| {
                        app.draw_traffic_monitor_and_inspector(ui);
                    });
                    egui::CentralPanel::default().show(ctx, |ui| match app.active_tab {
                        WorkspaceTab::HelmAndSensors => app.draw_helm_and_sensors_tab(ui),
                        WorkspaceTab::ChartAndRadar => app.draw_chart_and_radar_tab(ui),
                        WorkspaceTab::InstrumentsAndPgns => app.draw_instruments_tab(ui),
                        WorkspaceTab::PortsAndNetwork => app.draw_ports_tab(ui),
                        WorkspaceTab::AisAndArpa => app.draw_ais_tab(ui),
                        WorkspaceTab::LogRecorderPlayer => app.draw_recorder_tab(ui),
                        WorkspaceTab::CustomBuilder => app.draw_custom_builder_tab(ui),
                    });
                });
            }
        }

        ctx.fonts(|fonts| {
            let font_id = egui::FontId::proportional(14.0);
            let missing_job = egui::text::LayoutJob::simple_singleline(
                "\u{10FFFF}".to_string(),
                font_id.clone(),
                egui::Color32::WHITE,
            );
            let missing_uv = fonts.layout_job(missing_job).rows[0].glyphs[0].uv_rect;

            let mut all_chars = std::collections::BTreeSet::new();
            for file in [
                include_str!("../main.rs"),
                include_str!("../model/mod.rs"),
                include_str!("../model/vessel.rs"),
                include_str!("../model/ais.rs"),
                include_str!("../model/instruments.rs"),
                include_str!("../model/route.rs"),
                include_str!("../transport/mod.rs"),
                include_str!("../recorder/mod.rs"),
                include_str!("../protocols/mod.rs"),
                include_str!("../protocols/nmea0183.rs"),
                include_str!("../protocols/nmea2000.rs"),
                include_str!("mod.rs"),
                include_str!("gauges.rs"),
                include_str!("plotter.rs"),
            ] {
                for ch in file.chars() {
                    if !ch.is_ascii() && ch != '\u{FE0F}' && ch != '\u{10FFFF}' {
                        all_chars.insert(ch);
                    }
                }
            }
            let s: String = all_chars.iter().copied().collect();
            let job = egui::text::LayoutJob::simple_singleline(
                s,
                font_id.clone(),
                egui::Color32::WHITE,
            );
            let galley = fonts.layout_job(job);
            let mut missing = Vec::new();
            for row in &galley.rows {
                for g in &row.glyphs {
                    if g.uv_rect.min == missing_uv.min && g.uv_rect.max == missing_uv.max {
                        missing.push((g.chr, g.chr as u32));
                    }
                }
            }
            assert!(
                missing.is_empty(),
                "Found pictograms missing from egui font atlas: {:?}",
                missing
            );
        });
    }

    #[test]
    fn test_generate_app_logo_rgba_and_icon_formats() {
        let icon64 = generate_app_logo_rgba(64);
        assert_eq!(icon64.width, 64);
        assert_eq!(icon64.height, 64);
        assert_eq!(icon64.rgba.len(), 64 * 64 * 4);
        // Center pixel must be opaque
        let center_idx = ((32 * 64 + 32) * 4) as usize;
        assert_eq!(icon64.rgba[center_idx + 3], 255);
        // Extreme corner pixel (0, 0) outside rounded squircle must be transparent
        assert_eq!(icon64.rgba[3], 0);

        let png_bytes = encode_rgba_to_png(&icon64);
        assert!(png_bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));

        let ico_bytes = encode_app_logo_ico(&[16, 32, 48, 64, 256]);
        assert_eq!(&ico_bytes[0..6], &[0, 0, 1, 0, 5, 0]);

        if std::env::var("GENERATE_ASSETS").is_ok() {
            let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
            let _ = fs::create_dir_all(&assets_dir);
            let icon512 = generate_app_logo_rgba(512);
            fs::write(assets_dir.join("icon.png"), encode_rgba_to_png(&icon512)).unwrap();
            fs::write(assets_dir.join("icon.ico"), &ico_bytes).unwrap();

            let iconset_dir = assets_dir.join("AppIcon.iconset");
            let _ = fs::create_dir_all(&iconset_dir);
            for &(name, sz) in &[
                ("icon_16x16.png", 16u32),
                ("icon_16x16@2x.png", 32),
                ("icon_32x32.png", 32),
                ("icon_32x32@2x.png", 64),
                ("icon_128x128.png", 128),
                ("icon_128x128@2x.png", 256),
                ("icon_256x256.png", 256),
                ("icon_256x256@2x.png", 512),
                ("icon_512x512.png", 512),
                ("icon_512x512@2x.png", 1024),
            ] {
                let ic = generate_app_logo_rgba(sz);
                fs::write(iconset_dir.join(name), encode_rgba_to_png(&ic)).unwrap();
            }
        }
    }
}
