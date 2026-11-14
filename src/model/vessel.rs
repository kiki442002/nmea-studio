use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SteeringMode {
    ManualHeading,
    TurnRate,
    FollowRoute,
}

impl SteeringMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ManualHeading => "Manual Course / Heading",
            Self::TurnRate => "Rate of Turn (ROT / Rudder)",
            Self::FollowRoute => "Autopilot (Follow Active Route)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FluctuationConfig {
    pub enabled: bool,
    pub gps_jitter_meters: f64,
    pub speed_jitter_knots: f64,
    pub heading_jitter_deg: f64,
    pub wind_speed_jitter_knots: f64,
    pub wind_angle_jitter_deg: f64,
    pub depth_jitter_meters: f64,
    pub pitch_roll_wave_amp_deg: f64,
    pub wave_period_sec: f64,
}

impl Default for FluctuationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gps_jitter_meters: 0.6,
            speed_jitter_knots: 0.12,
            heading_jitter_deg: 0.4,
            wind_speed_jitter_knots: 0.5,
            wind_angle_jitter_deg: 2.0,
            depth_jitter_meters: 0.15,
            pitch_roll_wave_amp_deg: 2.5,
            wave_period_sec: 6.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VesselState {
    // Identification
    pub vessel_name: String,
    pub callsign: String,
    pub mmsi: u32,
    pub imo: u32,

    // Position & Navigation
    pub latitude: f64,         // Degrees (-90..90, +N)
    pub longitude: f64,        // Degrees (-180..180, +E)
    pub altitude_m: f64,       // Meters above MSL
    pub sog_knots: f64,        // Speed Over Ground (knots)
    pub stw_knots: f64,        // Speed Through Water (knots)
    pub cog_deg: f64,          // Course Over Ground (0..360 true)
    pub heading_true_deg: f64, // True Heading (0..360)
    pub mag_var_deg: f64,      // Magnetic variation (+East, -West)
    pub mag_dev_deg: f64,      // Compass deviation (+East, -West)
    pub rot_deg_per_min: f64,  // Rate of Turn (deg/min, +starboard)
    pub rudder_angle_deg: f64, // Rudder angle (deg, +starboard)
    pub steering_mode: SteeringMode,
    pub command_heading_deg: f64,

    // Attitude
    pub pitch_deg: f64, // +Bow up
    pub roll_deg: f64,  // +Starboard down
    pub heave_m: f64,   // Vertical heave in meters

    // Hydro / Bathymetry & Log
    pub depth_below_transducer_m: f64,
    pub transducer_offset_m: f64, // + means transducer is below waterline, - means above keel
    pub keel_offset_m: f64,       // Distance from transducer to keel
    pub water_temp_c: f64,
    pub log_total_nm: f64,
    pub log_trip_nm: f64,
    pub current_set_deg: f64,
    pub current_drift_knots: f64,

    // Wind & Weather
    pub wind_true_speed_knots: f64,
    pub wind_true_dir_deg: f64, // True wind direction (coming from, 0..360 true)
    pub air_temp_c: f64,
    pub barometric_pressure_hpa: f64,
    pub relative_humidity_pct: f64,
    pub dew_point_c: f64,

    // Propulsion, Electrical & Tanks (NMEA 2000 & 0183 XDR/RPM)
    pub port_engine_rpm: f64,
    pub stbd_engine_rpm: f64,
    pub dual_engine: bool,
    pub engine_coolant_temp_c: f64,
    pub engine_oil_pressure_kpa: f64,
    pub engine_hours: f64,
    pub fuel_rate_lph: f64,
    pub alternator_voltage_v: f64,
    pub house_battery_voltage_v: f64,
    pub battery_current_a: f64,
    pub battery_soc_pct: f64,
    pub fuel_tank_level_pct: f64,
    pub fresh_water_level_pct: f64,

    // GNSS Quality
    pub gps_fix_quality: u8, // 1 = GPS, 2 = DGPS, 4 = RTK
    pub satellites_in_use: u8,
    pub hdop: f64,

    // Dynamic fluctuation / noise
    pub fluctuation: FluctuationConfig,

    // Track trail history: (lat, lon)
    #[serde(skip)]
    pub track_history: Vec<(f64, f64)>,
    #[serde(skip)]
    pub sim_elapsed_sec: f64,
    #[serde(skip)]
    pub last_track_record_sec: f64,
}

impl Default for VesselState {
    fn default() -> Self {
        Self {
            vessel_name: "AURORA N2K".to_string(),
            callsign: "FAUR2".to_string(),
            mmsi: 227123450,
            imo: 9812345,

            latitude: 48.3580,
            longitude: -4.4850,
            altitude_m: 2.4,
            sog_knots: 7.8,
            stw_knots: 7.5,
            cog_deg: 245.0,
            heading_true_deg: 243.5,
            mag_var_deg: -1.8,
            mag_dev_deg: 0.3,
            rot_deg_per_min: 0.0,
            rudder_angle_deg: 0.0,
            steering_mode: SteeringMode::ManualHeading,
            command_heading_deg: 243.5,

            pitch_deg: 0.8,
            roll_deg: -1.2,
            heave_m: 0.0,

            depth_below_transducer_m: 24.5,
            transducer_offset_m: 0.8,
            keel_offset_m: 1.5,
            water_temp_c: 15.6,
            log_total_nm: 1428.4,
            log_trip_nm: 18.2,
            current_set_deg: 95.0,
            current_drift_knots: 0.4,

            wind_true_speed_knots: 14.5,
            wind_true_dir_deg: 290.0,
            air_temp_c: 17.8,
            barometric_pressure_hpa: 1016.4,
            relative_humidity_pct: 74.0,
            dew_point_c: 13.1,

            port_engine_rpm: 1850.0,
            stbd_engine_rpm: 1850.0,
            dual_engine: false,
            engine_coolant_temp_c: 82.5,
            engine_oil_pressure_kpa: 345.0,
            engine_hours: 412.6,
            fuel_rate_lph: 14.2,
            alternator_voltage_v: 14.1,
            house_battery_voltage_v: 13.4,
            battery_current_a: 8.5,
            battery_soc_pct: 92.0,
            fuel_tank_level_pct: 78.0,
            fresh_water_level_pct: 65.0,

            gps_fix_quality: 2,
            satellites_in_use: 10,
            hdop: 0.8,

            fluctuation: FluctuationConfig::default(),
            track_history: Vec::new(),
            sim_elapsed_sec: 0.0,
            last_track_record_sec: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SampledVesselMetrics {
    pub latitude: f64,
    pub longitude: f64,
    pub sog_knots: f64,
    pub stw_knots: f64,
    pub cog_deg: f64,
    pub heading_true_deg: f64,
    pub heading_mag_deg: f64,
    pub rot_deg_per_min: f64,
    pub depth_transducer_m: f64,
    pub depth_surface_m: f64,
    pub depth_keel_m: f64,
    pub wind_true_speed_knots: f64,
    pub wind_true_dir_deg: f64,
    pub wind_true_angle_deg: f64,
    pub wind_apparent_speed_knots: f64,
    pub wind_apparent_angle_deg: f64,
    pub pitch_deg: f64,
    pub roll_deg: f64,
}

impl VesselState {
    pub fn normalize_deg(deg: f64) -> f64 {
        let mut d = deg % 360.0;
        if d < 0.0 {
            d += 360.0;
        }
        d
    }

    pub fn signed_angle_diff(target: f64, current: f64) -> f64 {
        let mut diff = (target - current) % 360.0;
        if diff > 180.0 {
            diff -= 360.0;
        } else if diff < -180.0 {
            diff += 360.0;
        }
        diff
    }

    pub fn heading_magnetic(&self) -> f64 {
        Self::normalize_deg(self.heading_true_deg - self.mag_var_deg)
    }

    pub fn cog_magnetic(&self) -> f64 {
        Self::normalize_deg(self.cog_deg - self.mag_var_deg)
    }

    /// Computes apparent wind speed (knots), apparent wind angle (0..360 relative to bow, starboard +),
    /// and true wind angle (0..360 relative to bow).
    pub fn compute_apparent_wind(&self, tws: f64, twd: f64, hdg: f64, sog: f64) -> (f64, f64, f64) {
        let twa = Self::normalize_deg(twd - hdg);
        let twa_rad = twa.to_radians();

        let wx = tws * twa_rad.cos() + sog;
        let wy = tws * twa_rad.sin();

        let aws = (wx * wx + wy * wy).sqrt();
        let mut awa = wy.atan2(wx).to_degrees();
        if awa < 0.0 {
            awa += 360.0;
        }
        (aws, awa, twa)
    }

    /// Returns instantaneous metrics with optional realistic wave/sensor jitter applied
    pub fn sampled_metrics(&self) -> SampledVesselMetrics {
        let t = self.sim_elapsed_sec;
        let (lat_j, lon_j, spd_j, hdg_j, ws_j, wa_j, dep_j, pitch_w, roll_w) =
            if self.fluctuation.enabled {
                let omega = 2.0 * PI / self.fluctuation.wave_period_sec.max(1.0);
                let s1 = (t * omega).sin();
                let c1 = (t * omega * 0.73).cos();
                let s2 = (t * 2.13).sin() * (t * 0.87).cos();

                let lat_m = self.fluctuation.gps_jitter_meters * s2 * 0.5;
                let lon_m = self.fluctuation.gps_jitter_meters * c1 * 0.5;
                let dlat = lat_m / 111_320.0;
                let dlon = lon_m / (111_320.0 * self.latitude.to_radians().cos().abs().max(0.05));

                (
                    dlat,
                    dlon,
                    self.fluctuation.speed_jitter_knots * s2,
                    self.fluctuation.heading_jitter_deg * s1,
                    self.fluctuation.wind_speed_jitter_knots * s2,
                    self.fluctuation.wind_angle_jitter_deg * c1,
                    self.fluctuation.depth_jitter_meters * s1,
                    self.fluctuation.pitch_roll_wave_amp_deg * 0.6 * s1,
                    self.fluctuation.pitch_roll_wave_amp_deg * c1,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            };

        let latitude = (self.latitude + lat_j).clamp(-89.999, 89.999);
        let longitude = ((self.longitude + lon_j + 180.0).rem_euclid(360.0)) - 180.0;
        let sog_knots = (self.sog_knots + spd_j).max(0.0);
        let stw_knots = (self.stw_knots + spd_j * 0.8).max(0.0);
        let heading_true_deg = Self::normalize_deg(self.heading_true_deg + hdg_j);
        let heading_mag_deg = Self::normalize_deg(heading_true_deg - self.mag_var_deg);
        let cog_deg = Self::normalize_deg(self.cog_deg + hdg_j * 0.7);
        let depth_transducer_m = (self.depth_below_transducer_m + dep_j).max(0.5);
        let depth_surface_m = (depth_transducer_m + self.transducer_offset_m).max(0.5);
        let depth_keel_m = (depth_transducer_m - self.keel_offset_m).max(0.0);

        let wind_true_speed_knots = (self.wind_true_speed_knots + ws_j).max(0.0);
        let wind_true_dir_deg = Self::normalize_deg(self.wind_true_dir_deg + wa_j);
        let (aws, awa, twa) = self.compute_apparent_wind(
            wind_true_speed_knots,
            wind_true_dir_deg,
            heading_true_deg,
            sog_knots,
        );

        SampledVesselMetrics {
            latitude,
            longitude,
            sog_knots,
            stw_knots,
            cog_deg,
            heading_true_deg,
            heading_mag_deg,
            rot_deg_per_min: self.rot_deg_per_min,
            depth_transducer_m,
            depth_surface_m,
            depth_keel_m,
            wind_true_speed_knots,
            wind_true_dir_deg,
            wind_true_angle_deg: twa,
            wind_apparent_speed_knots: aws,
            wind_apparent_angle_deg: awa,
            pitch_deg: self.pitch_deg + pitch_w,
            roll_deg: self.roll_deg + roll_w,
        }
    }

    /// Advance vessel kinematics by `dt_sec` seconds
    pub fn step(&mut self, dt_sec: f64, autopilot_target_bearing: Option<f64>) {
        if dt_sec <= 0.0 {
            return;
        }
        self.sim_elapsed_sec += dt_sec;

        // 1. Update heading & ROT based on steering mode
        match self.steering_mode {
            SteeringMode::ManualHeading => {
                let d_hdg = (self.rot_deg_per_min / 60.0) * dt_sec;
                if self.rot_deg_per_min.abs() > 0.01 {
                    self.heading_true_deg = Self::normalize_deg(self.heading_true_deg + d_hdg);
                    self.cog_deg = Self::normalize_deg(self.cog_deg + d_hdg);
                }
            }
            SteeringMode::TurnRate => {
                if self.rudder_angle_deg.abs() > 0.1 {
                    self.rot_deg_per_min = (self.rudder_angle_deg
                        * 1.2
                        * (self.sog_knots.max(1.0) / 6.0))
                        .clamp(-60.0, 60.0);
                }
                let d_hdg = (self.rot_deg_per_min / 60.0) * dt_sec;
                self.heading_true_deg = Self::normalize_deg(self.heading_true_deg + d_hdg);
                self.cog_deg = Self::normalize_deg(self.heading_true_deg);
            }
            SteeringMode::FollowRoute => {
                if let Some(target_brg) = autopilot_target_bearing {
                    self.command_heading_deg = target_brg;
                }
                let err = Self::signed_angle_diff(self.command_heading_deg, self.heading_true_deg);
                let desired_rot = (err * 1.8).clamp(-25.0, 25.0);
                self.rot_deg_per_min = desired_rot;
                self.rudder_angle_deg = (desired_rot / 1.5).clamp(-30.0, 30.0);
                let d_hdg = (self.rot_deg_per_min / 60.0) * dt_sec;
                self.heading_true_deg = Self::normalize_deg(self.heading_true_deg + d_hdg);
                self.cog_deg = Self::normalize_deg(self.heading_true_deg);
            }
        }

        // 2. Advance position using SOG and COG
        let dist_nm = self.sog_knots * (dt_sec / 3600.0);
        if dist_nm > 0.0 {
            let cog_rad = self.cog_deg.to_radians();
            let d_north_nm = dist_nm * cog_rad.cos();
            let d_east_nm = dist_nm * cog_rad.sin();

            let d_lat = d_north_nm / 60.0;
            let cos_lat = self.latitude.to_radians().cos().abs().max(0.01);
            let d_lon = d_east_nm / (60.0 * cos_lat);

            self.latitude = (self.latitude + d_lat).clamp(-89.9, 89.9);
            self.longitude = ((self.longitude + d_lon + 180.0).rem_euclid(360.0)) - 180.0;

            self.log_total_nm += dist_nm;
            self.log_trip_nm += dist_nm;
            self.engine_hours += dt_sec / 3600.0;
        }

        // 3. Record track history every ~2 seconds of simulation
        if self.track_history.is_empty()
            || (self.sim_elapsed_sec - self.last_track_record_sec) >= 2.0
        {
            self.last_track_record_sec = self.sim_elapsed_sec;
            self.track_history.push((self.latitude, self.longitude));
            if self.track_history.len() > 600 {
                self.track_history.remove(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steering_mode_label() {
        assert!(!SteeringMode::ManualHeading.label().is_empty());
        assert!(!SteeringMode::TurnRate.label().is_empty());
        assert!(!SteeringMode::FollowRoute.label().is_empty());
    }

    #[test]
    fn test_fluctuation_config_default() {
        let cfg = FluctuationConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.wave_period_sec > 0.0);
        assert!(cfg.gps_jitter_meters > 0.0);
    }

    #[test]
    fn test_vessel_state_default() {
        let v = VesselState::default();
        assert_eq!(v.vessel_name, "AURORA N2K");
        assert!(v.latitude > -90.0 && v.latitude < 90.0);
        assert!(v.longitude > -180.0 && v.longitude < 180.0);
    }

    #[test]
    fn test_normalize_deg() {
        assert!((VesselState::normalize_deg(0.0) - 0.0).abs() < 1e-9);
        assert!((VesselState::normalize_deg(360.0) - 0.0).abs() < 1e-9);
        assert!((VesselState::normalize_deg(450.0) - 90.0).abs() < 1e-9);
        assert!((VesselState::normalize_deg(-90.0) - 270.0).abs() < 1e-9);
        assert!((VesselState::normalize_deg(-725.0) - 355.0).abs() < 1e-9);
    }

    #[test]
    fn test_signed_angle_diff() {
        assert!((VesselState::signed_angle_diff(10.0, 350.0) - 20.0).abs() < 1e-9);
        assert!((VesselState::signed_angle_diff(350.0, 10.0) - (-20.0)).abs() < 1e-9);
        assert!((VesselState::signed_angle_diff(90.0, 0.0) - 90.0).abs() < 1e-9);
        assert!((VesselState::signed_angle_diff(0.0, 90.0) - (-90.0)).abs() < 1e-9);
    }

    #[test]
    fn test_heading_and_cog_magnetic() {
        let mut v = VesselState::default();
        v.heading_true_deg = 100.0;
        v.cog_deg = 105.0;
        v.mag_var_deg = -5.0; // 5 deg West -> Magnetic = True - (-5) = +5
        assert!((v.heading_magnetic() - 105.0).abs() < 1e-9);
        assert!((v.cog_magnetic() - 110.0).abs() < 1e-9);
    }

    #[test]
    fn test_compute_apparent_wind() {
        let v = VesselState::default();
        // Headwind: TWD = 0, HDG = 0, TWS = 15, SOG = 5 => AWS = 20, AWA = 0, TWA = 0
        let (aws, awa, twa) = v.compute_apparent_wind(15.0, 0.0, 0.0, 5.0);
        assert!((aws - 20.0).abs() < 1e-6);
        assert!(awa.abs() < 1e-6 || (awa - 360.0).abs() < 1e-6);
        assert!(twa.abs() < 1e-6);

        // Tailwind: TWD = 180, HDG = 0, TWS = 15, SOG = 5 => AWS = 10, AWA = 180, TWA = 180
        let (aws2, awa2, twa2) = v.compute_apparent_wind(15.0, 180.0, 0.0, 5.0);
        assert!((aws2 - 10.0).abs() < 1e-6);
        assert!((awa2 - 180.0).abs() < 1e-6);
        assert!((twa2 - 180.0).abs() < 1e-6);
    }

    #[test]
    fn test_sampled_metrics_with_and_without_fluctuation() {
        let mut v = VesselState::default();
        v.fluctuation.enabled = false;
        let m0 = v.sampled_metrics();
        assert!((m0.latitude - v.latitude).abs() < 1e-9);
        assert!((m0.longitude - v.longitude).abs() < 1e-9);
        assert!((m0.sog_knots - v.sog_knots).abs() < 1e-9);
        assert!(
            (m0.depth_surface_m - (v.depth_below_transducer_m + v.transducer_offset_m)).abs()
                < 1e-9
        );
        assert!((m0.depth_keel_m - (v.depth_below_transducer_m - v.keel_offset_m)).abs() < 1e-9);

        v.fluctuation.enabled = true;
        v.sim_elapsed_sec = 1.23;
        let m1 = v.sampled_metrics();
        assert!((m1.latitude - v.latitude).abs() < 0.01);
        assert!((m1.pitch_deg - v.pitch_deg).abs() <= v.fluctuation.pitch_roll_wave_amp_deg + 0.1);
    }

    #[test]
    fn test_step_all_steering_modes_and_kinematics() {
        let mut v = VesselState::default();
        let initial_lat = v.latitude;
        // Zero/negative dt does nothing
        v.step(0.0, None);
        assert_eq!(v.latitude, initial_lat);

        // ManualHeading heading North at 60 knots for 60 seconds = 1 NM = 1/60 deg latitude
        v.steering_mode = SteeringMode::ManualHeading;
        v.cog_deg = 0.0;
        v.heading_true_deg = 0.0;
        v.sog_knots = 60.0;
        v.rot_deg_per_min = 0.0;
        let start_total = v.log_total_nm;
        v.step(60.0, None);
        assert!((v.latitude - (initial_lat + 1.0 / 60.0)).abs() < 1e-6);
        assert!((v.log_total_nm - (start_total + 1.0)).abs() < 1e-6);
        assert_eq!(v.track_history.len(), 1);

        // TurnRate mode with starboard rudder
        v.steering_mode = SteeringMode::TurnRate;
        v.rudder_angle_deg = 10.0;
        v.sog_knots = 6.0;
        let hdg_before = v.heading_true_deg;
        v.step(10.0, None);
        assert!(v.heading_true_deg > hdg_before);

        // FollowRoute mode steering toward 090 deg
        v.steering_mode = SteeringMode::FollowRoute;
        v.heading_true_deg = 45.0;
        v.step(5.0, Some(90.0));
        assert!(v.heading_true_deg > 45.0);
        assert!((v.command_heading_deg - 90.0).abs() < 1e-6);
    }
}
