use serde::{Deserialize, Serialize};
use crate::model::vessel::VesselState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: String,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub arrival_radius_nm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    pub name: String,
    pub waypoints: Vec<Waypoint>,
    pub active_leg_to_idx: usize, // index of destination waypoint (>= 1 if route has >= 2 wpts)
    pub active: bool,
    pub loop_route: bool,
    pub reverse: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ActiveLegMetrics {
    pub origin_idx: usize,
    pub dest_idx: usize,
    pub bearing_origin_to_dest_true: f64,
    pub bearing_present_to_dest_true: f64,
    pub distance_to_dest_nm: f64,
    pub xte_nm: f64, // + means ship is right of track, steer left (L); - means ship is left of track, steer right (R)
    pub steer_direction: char, // 'L' or 'R'
    pub vmg_knots: f64,
    pub is_arrived: bool,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            id: "RTE-01".to_string(),
            name: "Brest Approach Channel".to_string(),
            waypoints: vec![
                Waypoint {
                    id: "WP1".to_string(),
                    name: "GOULET-E".to_string(),
                    latitude: 48.3580,
                    longitude: -4.4850,
                    arrival_radius_nm: 0.10,
                },
                Waypoint {
                    id: "WP2".to_string(),
                    name: "MENGAM".to_string(),
                    latitude: 48.3385,
                    longitude: -4.5680,
                    arrival_radius_nm: 0.10,
                },
                Waypoint {
                    id: "WP3".to_string(),
                    name: "PORTZIC".to_string(),
                    latitude: 48.3210,
                    longitude: -4.6450,
                    arrival_radius_nm: 0.12,
                },
                Waypoint {
                    id: "WP4".to_string(),
                    name: "TOULBROC".to_string(),
                    latitude: 48.3080,
                    longitude: -4.7120,
                    arrival_radius_nm: 0.12,
                },
            ],
            active_leg_to_idx: 1,
            active: true,
            loop_route: true,
            reverse: false,
        }
    }
}

impl Route {
    /// Calculates distance in nautical miles and true bearing (0..360) from (lat1, lon1) to (lat2, lon2)
    pub fn distance_and_bearing_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> (f64, f64) {
        let d_lat_nm = (lat2 - lat1) * 60.0;
        let mean_lat_rad = ((lat1 + lat2) * 0.5).to_radians();
        let d_lon_nm = (lon2 - lon1) * 60.0 * mean_lat_rad.cos();

        let dist_nm = (d_lat_nm * d_lat_nm + d_lon_nm * d_lon_nm).sqrt();
        let brg_deg = VesselState::normalize_deg(d_lon_nm.atan2(d_lat_nm).to_degrees());
        (dist_nm, brg_deg)
    }

    pub fn active_leg_metrics(
        &self,
        ship_lat: f64,
        ship_lon: f64,
        sog_knots: f64,
        cog_deg: f64,
    ) -> Option<ActiveLegMetrics> {
        if self.waypoints.is_empty() {
            return None;
        }
        if self.waypoints.len() == 1 {
            let wp = &self.waypoints[0];
            let (dist, brg) =
                Self::distance_and_bearing_nm(ship_lat, ship_lon, wp.latitude, wp.longitude);
            let angle_diff = (cog_deg - brg).to_radians();
            let vmg = sog_knots * angle_diff.cos();
            return Some(ActiveLegMetrics {
                origin_idx: 0,
                dest_idx: 0,
                bearing_origin_to_dest_true: brg,
                bearing_present_to_dest_true: brg,
                distance_to_dest_nm: dist,
                xte_nm: 0.0,
                steer_direction: 'R',
                vmg_knots: vmg,
                is_arrived: dist <= wp.arrival_radius_nm,
            });
        }

        let dest_idx = self.active_leg_to_idx.clamp(1, self.waypoints.len() - 1);
        let origin_idx = dest_idx - 1;
        let origin = &self.waypoints[origin_idx];
        let dest = &self.waypoints[dest_idx];

        let (_leg_dist, brg_od) = Self::distance_and_bearing_nm(
            origin.latitude,
            origin.longitude,
            dest.latitude,
            dest.longitude,
        );
        let (dist_to_dest, brg_pd) =
            Self::distance_and_bearing_nm(ship_lat, ship_lon, dest.latitude, dest.longitude);
        let (dist_from_orig, brg_op) =
            Self::distance_and_bearing_nm(origin.latitude, origin.longitude, ship_lat, ship_lon);

        // Cross-track error: positive if ship is to starboard (right) of leg origin->dest
        let angle_diff_rad = VesselState::signed_angle_diff(brg_op, brg_od).to_radians();
        let xte_nm = dist_from_orig * angle_diff_rad.sin();
        let steer_direction = if xte_nm >= 0.0 { 'L' } else { 'R' };

        let vmg_angle_rad = VesselState::signed_angle_diff(cog_deg, brg_pd).to_radians();
        let vmg_knots = sog_knots * vmg_angle_rad.cos();

        Some(ActiveLegMetrics {
            origin_idx,
            dest_idx,
            bearing_origin_to_dest_true: brg_od,
            bearing_present_to_dest_true: brg_pd,
            distance_to_dest_nm: dist_to_dest,
            xte_nm,
            steer_direction,
            vmg_knots,
            is_arrived: dist_to_dest <= dest.arrival_radius_nm,
        })
    }

    /// Updates active leg if vessel has entered arrival circle of destination waypoint.
    /// Returns the autopilot target course (including XTE correction) if `self.active` is true.
    pub fn step_navigation(
        &mut self,
        ship_lat: f64,
        ship_lon: f64,
        sog_knots: f64,
        cog_deg: f64,
    ) -> Option<f64> {
        if !self.active || self.waypoints.is_empty() {
            return None;
        }
        let metrics = self.active_leg_metrics(ship_lat, ship_lon, sog_knots, cog_deg)?;
        if metrics.is_arrived && self.waypoints.len() >= 2 {
            if self.active_leg_to_idx + 1 < self.waypoints.len() {
                self.active_leg_to_idx += 1;
            } else if self.loop_route {
                self.waypoints.reverse();
                self.active_leg_to_idx = 1;
            }
        }

        let updated = self.active_leg_metrics(ship_lat, ship_lon, sog_knots, cog_deg)?;
        let xte_corr = (-updated.xte_nm * 45.0).clamp(-25.0, 25.0);
        Some(VesselState::normalize_deg(
            updated.bearing_present_to_dest_true + xte_corr,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_default() {
        let r = Route::default();
        assert_eq!(r.waypoints.len(), 4);
        assert_eq!(r.active_leg_to_idx, 1);
        assert!(r.active);
    }

    #[test]
    fn test_distance_and_bearing_nm() {
        // 1 degree North = 60 NM at bearing 0°
        let (d_n, b_n) = Route::distance_and_bearing_nm(0.0, 0.0, 1.0, 0.0);
        assert!((d_n - 60.0).abs() < 1e-6);
        assert!(b_n.abs() < 1e-6 || (b_n - 360.0).abs() < 1e-6);

        // 1 degree East at equator = 60 NM at bearing 90°
        let (d_e, b_e) = Route::distance_and_bearing_nm(0.0, 0.0, 0.0, 1.0);
        assert!((d_e - 60.0).abs() < 1e-6);
        assert!((b_e - 90.0).abs() < 1e-6);

        // 1 degree South = 60 NM at bearing 180°
        let (d_s, b_s) = Route::distance_and_bearing_nm(0.0, 0.0, -1.0, 0.0);
        assert!((d_s - 60.0).abs() < 1e-6);
        assert!((b_s - 180.0).abs() < 1e-6);
    }

    #[test]
    fn test_active_leg_metrics_cases() {
        let mut r = Route::default();
        r.waypoints.clear();
        assert!(r.active_leg_metrics(48.0, -4.0, 8.0, 0.0).is_none());

        // Single waypoint route
        r.waypoints.push(Waypoint {
            id: "W1".into(),
            name: "NORTH".into(),
            latitude: 1.0,
            longitude: 0.0,
            arrival_radius_nm: 0.5,
        });
        let m1 = r.active_leg_metrics(0.0, 0.0, 10.0, 0.0).unwrap();
        assert_eq!(m1.origin_idx, 0);
        assert_eq!(m1.dest_idx, 0);
        assert!((m1.distance_to_dest_nm - 60.0).abs() < 1e-5);
        assert!((m1.vmg_knots - 10.0).abs() < 1e-5);
        assert!(!m1.is_arrived);

        // Two-waypoint route: origin (0,0) -> dest (1,0) due North
        r.waypoints.insert(
            0,
            Waypoint {
                id: "W0".into(),
                name: "ORIG".into(),
                latitude: 0.0,
                longitude: 0.0,
                arrival_radius_nm: 0.5,
            },
        );
        r.active_leg_to_idx = 1;

        // Ship is East (right) of track at (0.5, +0.01) -> XTE > 0, steer 'L'
        let m_right = r.active_leg_metrics(0.5, 0.01, 10.0, 0.0).unwrap();
        assert!(m_right.xte_nm > 0.0);
        assert_eq!(m_right.steer_direction, 'L');

        // Ship is West (left) of track at (0.5, -0.01) -> XTE < 0, steer 'R'
        let m_left = r.active_leg_metrics(0.5, -0.01, 10.0, 0.0).unwrap();
        assert!(m_left.xte_nm < 0.0);
        assert_eq!(m_left.steer_direction, 'R');
    }

    #[test]
    fn test_step_navigation_sequencing_and_loop() {
        let mut r = Route::default();
        r.active = false;
        assert!(r.step_navigation(48.358, -4.485, 8.0, 245.0).is_none());

        r.active = true;
        r.active_leg_to_idx = 1;
        // Position exactly at WP2 -> should advance active_leg_to_idx to 2
        let wp2_lat = r.waypoints[1].latitude;
        let wp2_lon = r.waypoints[1].longitude;
        let brg = r.step_navigation(wp2_lat, wp2_lon, 8.0, 245.0);
        assert!(brg.is_some());
        assert_eq!(r.active_leg_to_idx, 2);

        // Advance to final waypoint WP4 with loop_route = true -> reverses waypoints and sets active_leg_to_idx = 1
        r.active_leg_to_idx = 3;
        let wp4_lat = r.waypoints[3].latitude;
        let wp4_lon = r.waypoints[3].longitude;
        let _ = r.step_navigation(wp4_lat, wp4_lon, 8.0, 245.0);
        assert_eq!(r.active_leg_to_idx, 1);
        assert_eq!(r.waypoints[0].id, "WP4");
    }
}
