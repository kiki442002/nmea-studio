pub mod ais;
pub mod instruments;
pub mod route;
pub mod vessel;

use serde::{Deserialize, Serialize};
use crate::model::ais::TargetVessel;
use crate::model::instruments::{Instrument, N2kWireFormat};
use crate::model::route::Route;
use crate::model::vessel::{SteeringMode, VesselState};
use crate::transport::PortConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AisTransponderConfig {
    pub enabled: bool,
    pub emit_own_ship_vdo: bool,
    pub vdo_interval_sec: f64,
    pub emit_nmea0183_vdm: bool,
    pub emit_nmea2000_pgns: bool,
    pub n2k_wire_format: N2kWireFormat,
    pub n2k_source_addr: u8,
    #[serde(default)]
    pub target_ports: Vec<String>,
    #[serde(skip)]
    pub vdo_elapsed_sec: f64,
    #[serde(skip)]
    pub fast_seq: u8,
}

impl Default for AisTransponderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            emit_own_ship_vdo: true,
            vdo_interval_sec: 3.0,
            emit_nmea0183_vdm: true,
            emit_nmea2000_pgns: false,
            n2k_wire_format: N2kWireFormat::YachtDevicesRawSingleLine,
            n2k_source_addr: 48,
            target_ports: Vec::new(),
            vdo_elapsed_sec: 0.0,
            fast_seq: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioProject {
    pub project_name: String,
    pub vessel: VesselState,
    pub route: Route,
    pub instruments: Vec<Instrument>,
    pub ais_targets: Vec<TargetVessel>,
    pub ais_config: AisTransponderConfig,
    pub ports: Vec<PortConfig>,
    pub sim_speed_multiplier: f64,
}

impl Default for StudioProject {
    /// Returns a completely blank/unconfigured project by default.
    fn default() -> Self {
        Self::empty()
    }
}

impl StudioProject {
    /// Creates a completely blank project with no pre-configured instruments, ports, route waypoints, or AIS targets.
    pub fn empty() -> Self {
        let mut vessel = VesselState::default();
        vessel.vessel_name = "MY VESSEL".to_string();
        vessel.callsign = "CALL01".to_string();
        vessel.sog_knots = 0.0;
        vessel.stw_knots = 0.0;
        vessel.heading_true_deg = 0.0;
        vessel.cog_deg = 0.0;
        vessel.wind_true_speed_knots = 0.0;
        vessel.wind_true_dir_deg = 0.0;
        vessel.port_engine_rpm = 0.0;
        vessel.stbd_engine_rpm = 0.0;
        vessel.fuel_rate_lph = 0.0;
        vessel.fluctuation.enabled = false;

        let route = Route {
            id: "RTE01".to_string(),
            name: "New Route".to_string(),
            active: false,
            loop_route: false,
            reverse: false,
            active_leg_to_idx: 1,
            waypoints: Vec::new(),
        };

        let ais_config = AisTransponderConfig {
            enabled: false,
            emit_own_ship_vdo: false,
            vdo_interval_sec: 3.0,
            emit_nmea0183_vdm: false,
            emit_nmea2000_pgns: false,
            n2k_wire_format: N2kWireFormat::YachtDevicesRawSingleLine,
            n2k_source_addr: 48,
            target_ports: Vec::new(),
            vdo_elapsed_sec: 0.0,
            fast_seq: 0,
        };

        Self {
            project_name: "Empty Project (Unconfigured)".to_string(),
            vessel,
            route,
            instruments: Vec::new(),
            ais_targets: Vec::new(),
            ais_config,
            ports: Vec::new(),
            sim_speed_multiplier: 1.0,
        }
    }

    /// Creates the pre-configured Brest Approaches Dual 0183 + NMEA 2000 demonstration project.
    pub fn configured_brest_suite() -> Self {
        Self {
            project_name: "Brest Approaches - NMEA 0183 & NMEA 2000 Suite".to_string(),
            vessel: VesselState::default(),
            route: Route::default(),
            instruments: Instrument::default_instruments(),
            ais_targets: TargetVessel::default_fleet(),
            ais_config: AisTransponderConfig::default(),
            ports: PortConfig::default_ports(),
            sim_speed_multiplier: 1.0,
        }
    }

    pub fn preset_names() -> &'static [&'static str] {
        &[
            "Default: Empty Project (Unconfigured)",
            "Brest Approaches (Dual 0183 + N2K)",
            "Heavy Weather Sailing (High Wind & Wave Fluctuation)",
            "Busy Shipping Lane (Close-Quarters AIS & ARPA Targets)",
            "Twin-Engine Motor Yacht (Full N2K Propulsion & Tanks)",
        ]
    }

    pub fn load_preset(index: usize) -> Self {
        match index {
            0 => Self::empty(),
            1 => Self::configured_brest_suite(),
            2 => {
                let mut proj = Self::configured_brest_suite();
                proj.project_name = "Heavy Weather Sailing".to_string();
                proj.vessel.vessel_name = "VALKYRIE".to_string();
                proj.vessel.sog_knots = 9.4;
                proj.vessel.stw_knots = 8.9;
                proj.vessel.wind_true_speed_knots = 34.0;
                proj.vessel.wind_true_dir_deg = 275.0;
                proj.vessel.barometric_pressure_hpa = 992.5;
                proj.vessel.pitch_deg = 4.5;
                proj.vessel.roll_deg = -14.0;
                proj.vessel.fluctuation.enabled = true;
                proj.vessel.fluctuation.pitch_roll_wave_amp_deg = 7.5;
                proj.vessel.fluctuation.wave_period_sec = 4.8;
                proj.vessel.fluctuation.wind_speed_jitter_knots = 3.2;
                proj.vessel.steering_mode = SteeringMode::FollowRoute;
                proj
            }
            3 => {
                let mut proj = Self::configured_brest_suite();
                proj.project_name = "Busy Shipping Lane - AIS & ARPA Stress Test".to_string();
                proj.vessel.sog_knots = 11.5;
                proj.vessel.stw_knots = 11.2;
                let mut t4 = proj.ais_targets[0].clone();
                t4.id = 4;
                t4.mmsi = 235091420;
                t4.name = "BRITANNIA EXPRESS".to_string();
                t4.callsign = "2BEX4".to_string();
                t4.latitude = proj.vessel.latitude - 0.015;
                t4.longitude = proj.vessel.longitude - 0.025;
                t4.sog_knots = 18.5;
                t4.cog_deg = 45.0;
                t4.heading_deg = 45.0;
                t4.report_interval_sec = 2.0;
                proj.ais_targets.push(t4);

                let mut t5 = proj.ais_targets[1].clone();
                t5.id = 5;
                t5.mmsi = 244810230;
                t5.name = "ROTTERDAM TRADER".to_string();
                t5.callsign = "PBRT".to_string();
                t5.latitude = proj.vessel.latitude + 0.018;
                t5.longitude = proj.vessel.longitude - 0.020;
                t5.sog_knots = 13.0;
                t5.cog_deg = 135.0;
                t5.heading_deg = 135.0;
                t5.report_interval_sec = 2.5;
                proj.ais_targets.push(t5);
                proj
            }
            4 => {
                let mut proj = Self::configured_brest_suite();
                proj.project_name = "Twin-Engine Motor Yacht - NMEA 2000 Suite".to_string();
                proj.vessel.vessel_name = "SOLARIS 68".to_string();
                proj.vessel.dual_engine = true;
                proj.vessel.port_engine_rpm = 2450.0;
                proj.vessel.stbd_engine_rpm = 2465.0;
                proj.vessel.sog_knots = 21.5;
                proj.vessel.stw_knots = 21.2;
                proj.vessel.fuel_rate_lph = 68.4;
                proj.vessel.steering_mode = SteeringMode::FollowRoute;
                proj
            }
            _ => Self::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ais_transponder_config_default() {
        let cfg = AisTransponderConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.emit_own_ship_vdo);
        assert!(cfg.emit_nmea0183_vdm);
        assert!(!cfg.emit_nmea2000_pgns);
        assert!(cfg.target_ports.is_empty());
    }

    #[test]
    fn test_studio_project_default_is_empty_and_presets() {
        let names = StudioProject::preset_names();
        assert_eq!(names.len(), 5);

        // Default / Preset 0 must be completely empty/unconfigured
        let def = StudioProject::default();
        assert!(def.instruments.is_empty());
        assert!(def.ports.is_empty());
        assert!(def.ais_targets.is_empty());
        assert!(def.route.waypoints.is_empty());
        assert!(!def.ais_config.enabled);

        let p0 = StudioProject::load_preset(0);
        assert!(p0.instruments.is_empty());

        let p1 = StudioProject::load_preset(1);
        assert_eq!(p1.ais_targets.len(), 3);
        assert_eq!(p1.instruments.len(), 6);

        let p2 = StudioProject::load_preset(2);
        assert_eq!(p2.vessel.vessel_name, "VALKYRIE");
        assert_eq!(p2.vessel.steering_mode, SteeringMode::FollowRoute);

        let p3 = StudioProject::load_preset(3);
        assert_eq!(p3.ais_targets.len(), 5);

        let p4 = StudioProject::load_preset(4);
        assert!(p4.vessel.dual_engine);
        assert_eq!(p4.vessel.vessel_name, "SOLARIS 68");
    }
}
