use serde::{Deserialize, Serialize};
use crate::model::route::Route;
use crate::model::vessel::VesselState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AisClass {
    ClassA,
    ClassB,
    ArpaOnly,
}

impl AisClass {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClassA => "AIS Class A (Msg 1/5 & PGN 129038/129794)",
            Self::ClassB => "AIS Class B (Msg 18/24 & PGN 129039/129809)",
            Self::ArpaOnly => "Radar ARPA Target ($RATTM / $RATLL)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavStatus {
    UnderWayUsingEngine = 0,
    AtAnchor = 1,
    NotUnderCommand = 2,
    RestrictedManoeuvrability = 3,
    ConstrainedByDraught = 4,
    Moored = 5,
    Aground = 6,
    EngagedInFishing = 7,
    UnderWaySailing = 8,
}

impl NavStatus {
    pub fn code(&self) -> u8 {
        *self as u8
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::UnderWayUsingEngine => "0 - Under way using engine",
            Self::AtAnchor => "1 - At anchor",
            Self::NotUnderCommand => "2 - Not under command",
            Self::RestrictedManoeuvrability => "3 - Restricted manoeuvrability",
            Self::ConstrainedByDraught => "4 - Constrained by draught",
            Self::Moored => "5 - Moored",
            Self::Aground => "6 - Aground",
            Self::EngagedInFishing => "7 - Engaged in fishing",
            Self::UnderWaySailing => "8 - Under way sailing",
        }
    }

    pub const ALL: [NavStatus; 9] = [
        Self::UnderWayUsingEngine,
        Self::AtAnchor,
        Self::NotUnderCommand,
        Self::RestrictedManoeuvrability,
        Self::ConstrainedByDraught,
        Self::Moored,
        Self::Aground,
        Self::EngagedInFishing,
        Self::UnderWaySailing,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetVessel {
    pub id: u32,
    pub enabled: bool,
    pub ais_class: AisClass,
    pub mmsi: u32,
    pub imo: u32,
    pub name: String,
    pub callsign: String,
    pub destination: String,
    pub ship_type: u8,
    pub nav_status: NavStatus,
    pub latitude: f64,
    pub longitude: f64,
    pub sog_knots: f64,
    pub cog_deg: f64,
    pub heading_deg: f64,
    pub rot_deg_per_min: f64,
    pub length_m: u16,
    pub beam_m: u16,
    pub draught_m: f32,
    pub report_interval_sec: f64,
    pub emit_arpa_ttm: bool,

    #[serde(skip)]
    pub time_since_dynamic_sec: f64,
    #[serde(skip)]
    pub time_since_static_sec: f64,
    #[serde(skip)]
    pub track_history: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Copy)]
pub struct CpaMetrics {
    pub range_nm: f64,
    pub bearing_true_deg: f64,
    pub bearing_rel_deg: f64,
    pub cpa_nm: f64,
    pub tcpa_min: f64, // positive = approaching CPA in future, negative = receding
}

impl TargetVessel {
    pub fn default_fleet() -> Vec<Self> {
        vec![
            Self {
                id: 1,
                enabled: true,
                ais_class: AisClass::ClassA,
                mmsi: 228041600,
                imo: 9451200,
                name: "CMA CGM BRETAGNE".to_string(),
                callsign: "FMBC".to_string(),
                destination: "FRBES".to_string(),
                ship_type: 70,
                nav_status: NavStatus::UnderWayUsingEngine,
                latitude: 48.3420,
                longitude: -4.5350,
                sog_knots: 12.4,
                cog_deg: 65.0,
                heading_deg: 64.0,
                rot_deg_per_min: 0.0,
                length_m: 185,
                beam_m: 28,
                draught_m: 8.4,
                report_interval_sec: 3.0,
                emit_arpa_ttm: true,
                time_since_dynamic_sec: 0.0,
                time_since_static_sec: 10.0,
                track_history: Vec::new(),
            },
            Self {
                id: 2,
                enabled: true,
                ais_class: AisClass::ClassB,
                mmsi: 227598120,
                imo: 0,
                name: "KORRIGAN II".to_string(),
                callsign: "FGY4210".to_string(),
                destination: "CAMARET".to_string(),
                ship_type: 36,
                nav_status: NavStatus::UnderWaySailing,
                latitude: 48.3680,
                longitude: -4.5120,
                sog_knots: 6.2,
                cog_deg: 195.0,
                heading_deg: 192.0,
                rot_deg_per_min: 0.0,
                length_m: 14,
                beam_m: 4,
                draught_m: 2.1,
                report_interval_sec: 5.0,
                emit_arpa_ttm: true,
                time_since_dynamic_sec: 1.5,
                time_since_static_sec: 12.0,
                track_history: Vec::new(),
            },
            Self {
                id: 3,
                enabled: true,
                ais_class: AisClass::ClassA,
                mmsi: 227006700,
                imo: 9214560,
                name: "ABEILLE BOURBON".to_string(),
                callsign: "FNSY".to_string(),
                destination: "BREST SAR".to_string(),
                ship_type: 52,
                nav_status: NavStatus::UnderWayUsingEngine,
                latitude: 48.3290,
                longitude: -4.5010,
                sog_knots: 14.0,
                cog_deg: 315.0,
                heading_deg: 315.0,
                rot_deg_per_min: 1.5,
                length_m: 80,
                beam_m: 16,
                draught_m: 6.5,
                report_interval_sec: 4.0,
                emit_arpa_ttm: true,
                time_since_dynamic_sec: 2.0,
                time_since_static_sec: 8.0,
                track_history: Vec::new(),
            },
        ]
    }

    pub fn step(&mut self, dt_sec: f64) {
        if !self.enabled || dt_sec <= 0.0 {
            return;
        }
        self.time_since_dynamic_sec += dt_sec;
        self.time_since_static_sec += dt_sec;

        if self.rot_deg_per_min.abs() > 0.01 {
            let d_hdg = (self.rot_deg_per_min / 60.0) * dt_sec;
            self.heading_deg = VesselState::normalize_deg(self.heading_deg + d_hdg);
            self.cog_deg = VesselState::normalize_deg(self.cog_deg + d_hdg);
        }

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
        }
    }

    pub fn compute_cpa(&self, own: &VesselState) -> CpaMetrics {
        let (range_nm, bearing_true_deg) = Route::distance_and_bearing_nm(
            own.latitude,
            own.longitude,
            self.latitude,
            self.longitude,
        );
        let bearing_rel_deg = VesselState::normalize_deg(bearing_true_deg - own.heading_true_deg);

        let brg_rad = bearing_true_deg.to_radians();
        let rx = range_nm * brg_rad.sin();
        let ry = range_nm * brg_rad.cos();

        let own_cog_rad = own.cog_deg.to_radians();
        let vx_own = own.sog_knots * own_cog_rad.sin();
        let vy_own = own.sog_knots * own_cog_rad.cos();

        let tgt_cog_rad = self.cog_deg.to_radians();
        let vx_tgt = self.sog_knots * tgt_cog_rad.sin();
        let vy_tgt = self.sog_knots * tgt_cog_rad.cos();

        let dvx = vx_tgt - vx_own;
        let dvy = vy_tgt - vy_own;
        let dv2 = dvx * dvx + dvy * dvy;

        if dv2 < 1e-6 {
            return CpaMetrics {
                range_nm,
                bearing_true_deg,
                bearing_rel_deg,
                cpa_nm: range_nm,
                tcpa_min: 0.0,
            };
        }

        let tcpa_hr = -(rx * dvx + ry * dvy) / dv2;
        let tcpa_min = tcpa_hr * 60.0;

        let cpa_x = rx + dvx * tcpa_hr.max(0.0);
        let cpa_y = ry + dvy * tcpa_hr.max(0.0);
        let cpa_nm = (cpa_x * cpa_x + cpa_y * cpa_y).sqrt();

        CpaMetrics {
            range_nm,
            bearing_true_deg,
            bearing_rel_deg,
            cpa_nm,
            tcpa_min,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ais_class_label() {
        assert!(AisClass::ClassA.label().contains("Class A"));
        assert!(AisClass::ClassB.label().contains("Class B"));
        assert!(AisClass::ArpaOnly.label().contains("ARPA"));
    }

    #[test]
    fn test_nav_status_code_and_label() {
        for (idx, status) in NavStatus::ALL.iter().enumerate() {
            assert_eq!(status.code(), idx as u8);
            assert!(!status.label().is_empty());
        }
    }

    #[test]
    fn test_default_fleet_and_step() {
        let mut fleet = TargetVessel::default_fleet();
        assert_eq!(fleet.len(), 3);

        let mut tgt = fleet.remove(0);
        let lat0 = tgt.latitude;
        tgt.enabled = false;
        tgt.step(10.0);
        assert_eq!(tgt.latitude, lat0);

        tgt.enabled = true;
        tgt.cog_deg = 0.0;
        tgt.heading_deg = 0.0;
        tgt.rot_deg_per_min = 6.0;
        tgt.sog_knots = 60.0;
        tgt.step(60.0);
        assert!(tgt.latitude > lat0);
        assert!((tgt.heading_deg - 6.0).abs() < 1e-6);
        assert!((tgt.time_since_dynamic_sec - 60.0).abs() < 1e-6);
    }

    #[test]
    fn test_compute_cpa_scenarios() {
        let mut own = VesselState::default();
        own.latitude = 0.0;
        own.longitude = 0.0;
        own.sog_knots = 10.0;
        own.cog_deg = 0.0; // Steaming North at 10 kn
        own.heading_true_deg = 0.0;

        let mut tgt = TargetVessel::default_fleet()[0].clone();
        // 1. Co-moving target (same velocity vector) 1 NM North -> dv2 == 0
        tgt.latitude = 1.0 / 60.0;
        tgt.longitude = 0.0;
        tgt.sog_knots = 10.0;
        tgt.cog_deg = 0.0;
        let cpa_same = tgt.compute_cpa(&own);
        assert!((cpa_same.range_nm - 1.0).abs() < 1e-5);
        assert!((cpa_same.cpa_nm - 1.0).abs() < 1e-5);
        assert_eq!(cpa_same.tcpa_min, 0.0);

        // 2. Head-on collision course: Target 2 NM North steaming South (180°) at 10 kn
        // Closing speed = 20 kn -> TCPA = 2 / 20 hr = 6.0 min, CPA = 0.0 NM
        tgt.latitude = 2.0 / 60.0;
        tgt.longitude = 0.0;
        tgt.sog_knots = 10.0;
        tgt.cog_deg = 180.0;
        let cpa_collision = tgt.compute_cpa(&own);
        assert!((cpa_collision.range_nm - 2.0).abs() < 1e-5);
        assert!(cpa_collision.cpa_nm < 1e-5);
        assert!((cpa_collision.tcpa_min - 6.0).abs() < 1e-5);

        // 3. Receding target: Target 2 NM North steaming North at 20 kn -> TCPA < 0
        tgt.cog_deg = 0.0;
        tgt.sog_knots = 20.0;
        let cpa_receding = tgt.compute_cpa(&own);
        assert!(cpa_receding.tcpa_min < 0.0);
        assert!((cpa_receding.cpa_nm - 2.0).abs() < 1e-5);
    }
}
