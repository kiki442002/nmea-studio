use chrono::{Datelike, Timelike, Utc};
use crate::model::ais::{AisClass, TargetVessel};
use crate::model::instruments::{ErrorInjectionConfig, Sentence0183Kind};
use crate::model::route::Route;
use crate::model::vessel::VesselState;
use crate::protocols::DecodedPacketInfo;

pub fn compute_checksum(payload: &str) -> u8 {
    let mut sum = 0u8;
    for b in payload.bytes() {
        sum ^= b;
    }
    sum
}

pub fn wrap_sentence(prefix: char, body: &str, err: &ErrorInjectionConfig) -> String {
    let mut cs = compute_checksum(body);
    if err.corrupt_checksum {
        cs ^= 0x5A;
    }
    if err.omit_checksum {
        format!("{}{}", prefix, body)
    } else {
        format!("{}{}*{:02X}", prefix, body, cs)
    }
}

pub fn format_lat_nmea(lat: f64) -> (String, char) {
    let hemi = if lat >= 0.0 { 'N' } else { 'S' };
    let abs_lat = lat.abs();
    let deg = abs_lat.floor() as u32;
    let min = (abs_lat - deg as f64) * 60.0;
    (format!("{:02}{:07.4}", deg, min), hemi)
}

pub fn format_lon_nmea(lon: f64) -> (String, char) {
    let hemi = if lon >= 0.0 { 'E' } else { 'W' };
    let abs_lon = lon.abs();
    let deg = abs_lon.floor() as u32;
    let min = (abs_lon - deg as f64) * 60.0;
    (format!("{:03}{:07.4}", deg, min), hemi)
}

pub fn generate_sentence(
    talker: &str,
    kind: Sentence0183Kind,
    vessel: &VesselState,
    route: &Route,
    err: &ErrorInjectionConfig,
) -> Vec<String> {
    let m = vessel.sampled_metrics();
    let now = Utc::now();
    let time_str = format!(
        "{:02}{:02}{:02}.{:02}",
        now.hour(),
        now.minute(),
        now.second(),
        (now.nanosecond() / 10_000_000) % 100
    );
    let date_str = format!(
        "{:02}{:02}{:02}",
        now.day(),
        now.month(),
        now.year() % 100
    );
    let (lat_s, lat_h) = format_lat_nmea(m.latitude);
    let (lon_s, lon_h) = format_lon_nmea(m.longitude);

    let mag_var_abs = vessel.mag_var_deg.abs();
    let mag_var_dir = if vessel.mag_var_deg >= 0.0 { 'E' } else { 'W' };
    let mag_dev_abs = vessel.mag_dev_deg.abs();
    let mag_dev_dir = if vessel.mag_dev_deg >= 0.0 { 'E' } else { 'W' };
    let cog_mag = VesselState::normalize_deg(m.cog_deg - vessel.mag_var_deg);
    let sog_kmh = m.sog_knots * 1.852;

    let mut bodies = Vec::new();

    match kind {
        Sentence0183Kind::GGA => {
            bodies.push(format!(
                "{}GGA,{},{},{},{},{},{},{:02},{:.1},{:.1},M,47.2,M,,",
                talker,
                time_str,
                lat_s,
                lat_h,
                lon_s,
                lon_h,
                vessel.gps_fix_quality,
                vessel.satellites_in_use,
                vessel.hdop,
                vessel.altitude_m
            ));
        }
        Sentence0183Kind::RMC => {
            bodies.push(format!(
                "{}RMC,{},A,{},{},{},{},{:.2},{:.1},{},{:.1},{},D",
                talker,
                time_str,
                lat_s,
                lat_h,
                lon_s,
                lon_h,
                m.sog_knots,
                m.cog_deg,
                date_str,
                mag_var_abs,
                mag_var_dir
            ));
        }
        Sentence0183Kind::GLL => {
            bodies.push(format!(
                "{}GLL,{},{},{},{},{},A,D",
                talker, lat_s, lat_h, lon_s, lon_h, time_str
            ));
        }
        Sentence0183Kind::VTG => {
            bodies.push(format!(
                "{}VTG,{:.1},T,{:.1},M,{:.2},N,{:.2},K,D",
                talker, m.cog_deg, cog_mag, m.sog_knots, sog_kmh
            ));
        }
        Sentence0183Kind::GSA => {
            bodies.push(format!(
                "{}GSA,A,3,02,05,09,12,17,19,23,25,28,31,,,{:.1},{:.1},1.1",
                talker,
                vessel.hdop * 1.4,
                vessel.hdop
            ));
        }
        Sentence0183Kind::GSV => {
            bodies.push(format!(
                "{}GSV,2,1,08,02,64,135,44,05,51,048,42,09,38,290,39,12,27,210,36",
                talker
            ));
            bodies.push(format!(
                "{}GSV,2,2,08,17,45,315,41,19,32,085,38,23,19,165,34,25,68,012,45",
                talker
            ));
        }
        Sentence0183Kind::ZDA => {
            bodies.push(format!(
                "{}ZDA,{},{:02},{:02},{:04},00,00",
                talker,
                time_str,
                now.day(),
                now.month(),
                now.year()
            ));
        }
        Sentence0183Kind::HDG => {
            let hdg_sensor = VesselState::normalize_deg(m.heading_mag_deg - vessel.mag_dev_deg);
            bodies.push(format!(
                "{}HDG,{:.1},{:.1},{},{:.1},{}",
                talker, hdg_sensor, mag_dev_abs, mag_dev_dir, mag_var_abs, mag_var_dir
            ));
        }
        Sentence0183Kind::HDT => {
            bodies.push(format!("{}HDT,{:.1},T", talker, m.heading_true_deg));
        }
        Sentence0183Kind::HDM => {
            bodies.push(format!("{}HDM,{:.1},M", talker, m.heading_mag_deg));
        }
        Sentence0183Kind::ROT => {
            bodies.push(format!("{}ROT,{:.1},A", talker, m.rot_deg_per_min));
        }
        Sentence0183Kind::MWVApparent => {
            bodies.push(format!(
                "{}MWV,{:.1},R,{:.1},N,A",
                talker, m.wind_apparent_angle_deg, m.wind_apparent_speed_knots
            ));
        }
        Sentence0183Kind::MWVTrue => {
            bodies.push(format!(
                "{}MWV,{:.1},T,{:.1},N,A",
                talker, m.wind_true_angle_deg, m.wind_true_speed_knots
            ));
        }
        Sentence0183Kind::MWD => {
            let twd_mag = VesselState::normalize_deg(m.wind_true_dir_deg - vessel.mag_var_deg);
            let tws_ms = m.wind_true_speed_knots * 0.514444;
            bodies.push(format!(
                "{}MWD,{:.1},T,{:.1},M,{:.1},N,{:.2},M",
                talker, m.wind_true_dir_deg, twd_mag, m.wind_true_speed_knots, tws_ms
            ));
        }
        Sentence0183Kind::VWR => {
            let (angle_lr, side) = if m.wind_apparent_angle_deg <= 180.0 {
                (m.wind_apparent_angle_deg, 'R')
            } else {
                (360.0 - m.wind_apparent_angle_deg, 'L')
            };
            let aws_ms = m.wind_apparent_speed_knots * 0.514444;
            let aws_kmh = m.wind_apparent_speed_knots * 1.852;
            bodies.push(format!(
                "{}VWR,{:.1},{},{:.1},N,{:.2},M,{:.1},K",
                talker, angle_lr, side, m.wind_apparent_speed_knots, aws_ms, aws_kmh
            ));
        }
        Sentence0183Kind::VWT => {
            let (angle_lr, side) = if m.wind_true_angle_deg <= 180.0 {
                (m.wind_true_angle_deg, 'R')
            } else {
                (360.0 - m.wind_true_angle_deg, 'L')
            };
            let tws_ms = m.wind_true_speed_knots * 0.514444;
            let tws_kmh = m.wind_true_speed_knots * 1.852;
            bodies.push(format!(
                "{}VWT,{:.1},{},{:.1},N,{:.2},M,{:.1},K",
                talker, angle_lr, side, m.wind_true_speed_knots, tws_ms, tws_kmh
            ));
        }
        Sentence0183Kind::MDA => {
            let bars = vessel.barometric_pressure_hpa / 1000.0;
            let inches_hg = vessel.barometric_pressure_hpa * 0.02953;
            let twd_mag = VesselState::normalize_deg(m.wind_true_dir_deg - vessel.mag_var_deg);
            let tws_ms = m.wind_true_speed_knots * 0.514444;
            bodies.push(format!(
                "{}MDA,{:.2},I,{:.4},B,{:.1},C,{:.1},C,{:.1},,{:.1},C,{:.1},T,{:.1},M,{:.1},N,{:.2},M",
                talker,
                inches_hg,
                bars,
                vessel.air_temp_c,
                vessel.water_temp_c,
                vessel.relative_humidity_pct,
                vessel.dew_point_c,
                m.wind_true_dir_deg,
                twd_mag,
                m.wind_true_speed_knots,
                tws_ms
            ));
        }
        Sentence0183Kind::XDRWeather => {
            let bars = vessel.barometric_pressure_hpa / 1000.0;
            bodies.push(format!(
                "{}XDR,P,{:.4},B,BARO,C,{:.1},C,AIRTEMP,H,{:.1},P,HUMID,A,{:.1},D,PITCH,A,{:.1},D,ROLL",
                talker,
                bars,
                vessel.air_temp_c,
                vessel.relative_humidity_pct,
                m.pitch_deg,
                m.roll_deg
            ));
        }
        Sentence0183Kind::DPT => {
            bodies.push(format!(
                "{}DPT,{:.1},{:.1},100.0",
                talker, m.depth_transducer_m, vessel.transducer_offset_m
            ));
        }
        Sentence0183Kind::DBT => {
            let feet = m.depth_transducer_m * 3.28084;
            let fathoms = m.depth_transducer_m * 0.546807;
            bodies.push(format!(
                "{}DBT,{:.1},f,{:.1},M,{:.1},F",
                talker, feet, m.depth_transducer_m, fathoms
            ));
        }
        Sentence0183Kind::DBS => {
            let feet = m.depth_surface_m * 3.28084;
            let fathoms = m.depth_surface_m * 0.546807;
            bodies.push(format!(
                "{}DBS,{:.1},f,{:.1},M,{:.1},F",
                talker, feet, m.depth_surface_m, fathoms
            ));
        }
        Sentence0183Kind::DBK => {
            let feet = m.depth_keel_m * 3.28084;
            let fathoms = m.depth_keel_m * 0.546807;
            bodies.push(format!(
                "{}DBK,{:.1},f,{:.1},M,{:.1},F",
                talker, feet, m.depth_keel_m, fathoms
            ));
        }
        Sentence0183Kind::MTW => {
            bodies.push(format!("{}MTW,{:.1},C", talker, vessel.water_temp_c));
        }
        Sentence0183Kind::VHW => {
            let stw_kmh = m.stw_knots * 1.852;
            bodies.push(format!(
                "{}VHW,{:.1},T,{:.1},M,{:.2},N,{:.2},K",
                talker, m.heading_true_deg, m.heading_mag_deg, m.stw_knots, stw_kmh
            ));
        }
        Sentence0183Kind::VLW => {
            bodies.push(format!(
                "{}VLW,{:.1},N,{:.2},N,{:.1},N,{:.2},N",
                talker,
                vessel.log_total_nm,
                vessel.log_trip_nm,
                vessel.log_total_nm,
                vessel.log_trip_nm
            ));
        }
        Sentence0183Kind::APB
        | Sentence0183Kind::RMB
        | Sentence0183Kind::BOD
        | Sentence0183Kind::BWC
        | Sentence0183Kind::XTE => {
            if let Some(leg) =
                route.active_leg_metrics(m.latitude, m.longitude, m.sog_knots, m.cog_deg)
            {
                let orig_wp = &route.waypoints[leg.origin_idx];
                let dest_wp = &route.waypoints[leg.dest_idx];
                let (dest_lat_s, dest_lat_h) = format_lat_nmea(dest_wp.latitude);
                let (dest_lon_s, dest_lon_h) = format_lon_nmea(dest_wp.longitude);
                let arr_char = if leg.is_arrived { 'A' } else { 'V' };
                let xte_abs = leg.xte_nm.abs().min(9.99);

                match kind {
                    Sentence0183Kind::XTE => {
                        bodies.push(format!(
                            "{}XTE,A,A,{:.2},{},N,D",
                            talker, xte_abs, leg.steer_direction
                        ));
                    }
                    Sentence0183Kind::APB => {
                        bodies.push(format!(
                            "{}APB,A,A,{:.2},{},N,{},{},{:.1},T,{},{:.1},T,{:.1},T,D",
                            talker,
                            xte_abs,
                            leg.steer_direction,
                            arr_char,
                            arr_char,
                            leg.bearing_origin_to_dest_true,
                            dest_wp.name,
                            leg.bearing_present_to_dest_true,
                            leg.bearing_present_to_dest_true
                        ));
                    }
                    Sentence0183Kind::RMB => {
                        bodies.push(format!(
                            "{}RMB,A,{:.2},{},{},{},{},{},{},{},{:.2},{:.1},{:.2},{},D",
                            talker,
                            xte_abs,
                            leg.steer_direction,
                            orig_wp.name,
                            dest_wp.name,
                            dest_lat_s,
                            dest_lat_h,
                            dest_lon_s,
                            dest_lon_h,
                            leg.distance_to_dest_nm,
                            leg.bearing_present_to_dest_true,
                            leg.vmg_knots,
                            arr_char
                        ));
                    }
                    Sentence0183Kind::BOD => {
                        let brg_od_mag = VesselState::normalize_deg(
                            leg.bearing_origin_to_dest_true - vessel.mag_var_deg,
                        );
                        bodies.push(format!(
                            "{}BOD,{:.1},T,{:.1},M,{},{}",
                            talker,
                            leg.bearing_origin_to_dest_true,
                            brg_od_mag,
                            dest_wp.name,
                            orig_wp.name
                        ));
                    }
                    Sentence0183Kind::BWC => {
                        let brg_pd_mag = VesselState::normalize_deg(
                            leg.bearing_present_to_dest_true - vessel.mag_var_deg,
                        );
                        bodies.push(format!(
                            "{}BWC,{},{},{},{},{},{:.1},T,{:.1},M,{:.2},N,{},D",
                            talker,
                            time_str,
                            dest_lat_s,
                            dest_lat_h,
                            dest_lon_s,
                            dest_lon_h,
                            leg.bearing_present_to_dest_true,
                            brg_pd_mag,
                            leg.distance_to_dest_nm,
                            dest_wp.name
                        ));
                    }
                    _ => {}
                }
            }
        }
        Sentence0183Kind::WPL => {
            for wp in &route.waypoints {
                let (wlat, wlat_h) = format_lat_nmea(wp.latitude);
                let (wlon, wlon_h) = format_lon_nmea(wp.longitude);
                bodies.push(format!(
                    "{}WPL,{},{},{},{},{}",
                    talker, wlat, wlat_h, wlon, wlon_h, wp.name
                ));
            }
        }
        Sentence0183Kind::RTE => {
            if !route.waypoints.is_empty() {
                let names: Vec<&str> = route.waypoints.iter().map(|w| w.name.as_str()).collect();
                bodies.push(format!(
                    "{}RTE,1,1,c,{},{}",
                    talker,
                    route.name,
                    names.join(",")
                ));
            }
        }
        Sentence0183Kind::RSA => {
            bodies.push(format!("{}RSA,{:.1},A,,V", talker, vessel.rudder_angle_deg));
        }
        Sentence0183Kind::RPM => {
            bodies.push(format!(
                "{}RPM,E,1,{:.0},100.0,A",
                talker, vessel.port_engine_rpm
            ));
            if vessel.dual_engine {
                bodies.push(format!(
                    "{}RPM,E,2,{:.0},100.0,A",
                    talker, vessel.stbd_engine_rpm
                ));
            }
        }
        Sentence0183Kind::XDREngine => {
            bodies.push(format!(
                "{}XDR,U,{:.2},V,BAT1,U,{:.2},V,ALT1,I,{:.1},A,BAT1,C,{:.1},C,ENG_TEMP,V,{:.1},P,FUEL_LVL",
                talker,
                vessel.house_battery_voltage_v,
                vessel.alternator_voltage_v,
                vessel.battery_current_a,
                vessel.engine_coolant_temp_c,
                vessel.fuel_tank_level_pct
            ));
        }
    }

    bodies
        .into_iter()
        .map(|b| wrap_sentence('$', &b, err))
        .collect()
}

// =========================================================================
// ITU-R M.1371 6-bit AIS Payload Bit-Packer (!AIVDM / !AIVDO)
// =========================================================================

pub(crate) struct BitWriter {
    bits: Vec<u8>,
}

impl BitWriter {
    pub(crate) fn new() -> Self {
        Self {
            bits: Vec::with_capacity(512),
        }
    }

    pub(crate) fn push_uint(&mut self, val: u64, num_bits: usize) {
        for i in (0..num_bits).rev() {
            let bit = ((val >> i) & 1) as u8;
            self.bits.push(bit);
        }
    }

    pub(crate) fn push_int(&mut self, val: i64, num_bits: usize) {
        let mask = if num_bits >= 64 {
            u64::MAX
        } else {
            (1u64 << num_bits) - 1
        };
        let uval = (val as u64) & mask;
        self.push_uint(uval, num_bits);
    }

    pub(crate) fn push_ais_text(&mut self, text: &str, num_chars: usize) {
        let upper = text.to_uppercase();
        let chars: Vec<char> = upper.chars().collect();
        for i in 0..num_chars {
            let c = chars.get(i).copied().unwrap_or('@');
            let b = c as u8;
            let six_bit = if (64..=95).contains(&b) {
                b - 64
            } else if (32..=63).contains(&b) {
                b
            } else {
                0 // '@'
            };
            self.push_uint(six_bit as u64, 6);
        }
    }

    pub(crate) fn to_armored_string(&self) -> (String, u8) {
        let rem = self.bits.len() % 6;
        let fill_bits = if rem == 0 { 0 } else { (6 - rem) as u8 };
        let total_bits = self.bits.len() + fill_bits as usize;
        let num_chars = total_bits / 6;
        let mut out = String::with_capacity(num_chars);

        for c_idx in 0..num_chars {
            let mut val = 0u8;
            for b_idx in 0..6 {
                let pos = c_idx * 6 + b_idx;
                let bit = self.bits.get(pos).copied().unwrap_or(0);
                val = (val << 1) | bit;
            }
            let ascii = if val < 40 { val + 48 } else { val + 56 };
            out.push(ascii as char);
        }

        (out, fill_bits)
    }
}

pub fn encode_ais_position_report(
    is_own_ship: bool,
    mmsi: u32,
    nav_status: u8,
    rot_deg_min: f64,
    sog_knots: f64,
    lat: f64,
    lon: f64,
    cog_deg: f64,
    hdg_deg: f64,
    class_b: bool,
) -> String {
    let mut bw = BitWriter::new();
    if !class_b {
        bw.push_uint(1, 6); // Message Type 1
        bw.push_uint(0, 2); // Repeat Indicator
        bw.push_uint(mmsi as u64, 30);
        bw.push_uint((nav_status & 0x0F) as u64, 4);

        let rot_ais = if rot_deg_min.abs() < 0.01 {
            0i64
        } else {
            let sign = if rot_deg_min >= 0.0 { 1.0 } else { -1.0 };
            (sign * 4.733 * rot_deg_min.abs().sqrt())
                .round()
                .clamp(-126.0, 126.0) as i64
        };
        bw.push_int(rot_ais, 8);

        let sog_10 = (sog_knots * 10.0).round().clamp(0.0, 1022.0) as u64;
        bw.push_uint(sog_10, 10);
        bw.push_uint(1, 1); // Position Accuracy = High

        let lon_i = (lon * 600_000.0).round() as i64;
        let lat_i = (lat * 600_000.0).round() as i64;
        bw.push_int(lon_i, 28);
        bw.push_int(lat_i, 27);

        let cog_10 = (VesselState::normalize_deg(cog_deg) * 10.0)
            .round()
            .clamp(0.0, 3599.0) as u64;
        bw.push_uint(cog_10, 12);

        let hdg_u = (VesselState::normalize_deg(hdg_deg).round() as u64).min(359);
        bw.push_uint(hdg_u, 9);
        bw.push_uint((Utc::now().second() as u64) & 0x3F, 6);
        bw.push_uint(0, 2); // Maneuver indicator
        bw.push_uint(0, 3); // Spare
        bw.push_uint(0, 1); // RAIM
        bw.push_uint(0, 19); // Radio status
    } else {
        bw.push_uint(18, 6); // Message Type 18
        bw.push_uint(0, 2);
        bw.push_uint(mmsi as u64, 30);
        bw.push_uint(0, 8);
        let sog_10 = (sog_knots * 10.0).round().clamp(0.0, 1022.0) as u64;
        bw.push_uint(sog_10, 10);
        bw.push_uint(1, 1);

        let lon_i = (lon * 600_000.0).round() as i64;
        let lat_i = (lat * 600_000.0).round() as i64;
        bw.push_int(lon_i, 28);
        bw.push_int(lat_i, 27);

        let cog_10 = (VesselState::normalize_deg(cog_deg) * 10.0)
            .round()
            .clamp(0.0, 3599.0) as u64;
        bw.push_uint(cog_10, 12);
        let hdg_u = (VesselState::normalize_deg(hdg_deg).round() as u64).min(359);
        bw.push_uint(hdg_u, 9);
        bw.push_uint((Utc::now().second() as u64) & 0x3F, 6);
        bw.push_uint(0, 2);
        bw.push_uint(1, 1);
        bw.push_uint(1, 1);
        bw.push_uint(1, 1);
        bw.push_uint(1, 1);
        bw.push_uint(0, 1);
        bw.push_uint(0, 1);
        bw.push_uint(0, 1);
        bw.push_uint(0, 20);
    }

    let (payload, fill) = bw.to_armored_string();
    let tag = if is_own_ship { "AIVDO" } else { "AIVDM" };
    let body = format!("{},1,1,,A,{},{}", tag, payload, fill);
    wrap_sentence('!', &body, &ErrorInjectionConfig::default())
}

pub fn encode_ais_static_report(target: &TargetVessel) -> Vec<String> {
    match target.ais_class {
        AisClass::ClassA => {
            let mut bw = BitWriter::new();
            bw.push_uint(5, 6);
            bw.push_uint(0, 2);
            bw.push_uint(target.mmsi as u64, 30);
            bw.push_uint(0, 2);
            bw.push_uint(target.imo as u64, 30);
            bw.push_ais_text(&target.callsign, 7);
            bw.push_ais_text(&target.name, 20);
            bw.push_uint(target.ship_type as u64, 8);
            let dim_bow = (target.length_m / 2) as u64;
            let dim_stern = (target.length_m - dim_bow as u16) as u64;
            let dim_port = (target.beam_m / 2) as u64;
            let dim_stbd = (target.beam_m - dim_port as u16) as u64;
            bw.push_uint(dim_bow, 9);
            bw.push_uint(dim_stern, 9);
            bw.push_uint(dim_port, 6);
            bw.push_uint(dim_stbd, 6);
            bw.push_uint(1, 4);
            bw.push_uint(0, 4);
            bw.push_uint(0, 5);
            bw.push_uint(24, 5);
            bw.push_uint(60, 6);
            let draught_10 = (target.draught_m * 10.0).round().clamp(0.0, 255.0) as u64;
            bw.push_uint(draught_10, 8);
            bw.push_ais_text(&target.destination, 20);
            bw.push_uint(0, 1);
            bw.push_uint(0, 1);

            let (payload, fill) = bw.to_armored_string();
            let split_idx = payload.len().min(60);
            let part1 = &payload[..split_idx];
            let part2 = &payload[split_idx..];
            let b1 = format!("AIVDM,2,1,3,A,{},0", part1);
            let b2 = format!("AIVDM,2,2,3,A,{},{}", part2, fill);
            vec![
                wrap_sentence('!', &b1, &ErrorInjectionConfig::default()),
                wrap_sentence('!', &b2, &ErrorInjectionConfig::default()),
            ]
        }
        AisClass::ClassB => {
            let mut bw = BitWriter::new();
            bw.push_uint(24, 6);
            bw.push_uint(0, 2);
            bw.push_uint(target.mmsi as u64, 30);
            bw.push_uint(0, 2);
            bw.push_ais_text(&target.name, 20);
            bw.push_uint(0, 8);
            let (payload, fill) = bw.to_armored_string();
            let body = format!("AIVDM,1,1,,B,{},{}", payload, fill);
            vec![wrap_sentence('!', &body, &ErrorInjectionConfig::default())]
        }
        AisClass::ArpaOnly => Vec::new(),
    }
}

pub fn encode_arpa_ttm(target: &TargetVessel, own: &VesselState) -> String {
    let cpa = target.compute_cpa(own);
    let now = Utc::now();
    let time_str = format!(
        "{:02}{:02}{:02}.{:02}",
        now.hour(),
        now.minute(),
        now.second(),
        (now.nanosecond() / 10_000_000) % 100
    );
    let body = format!(
        "RATTM,{:02},{:.2},{:.1},T,{:.1},{:.1},T,{:.2},{:.1},N,{},T,,,{}",
        target.id % 100,
        cpa.range_nm,
        cpa.bearing_true_deg,
        target.sog_knots,
        target.cog_deg,
        cpa.cpa_nm,
        cpa.tcpa_min,
        target.name,
        time_str
    );
    wrap_sentence('$', &body, &ErrorInjectionConfig::default())
}

// =========================================================================
// NMEA 0183 Decoder & Packet Inspector
// =========================================================================

pub fn inspect_nmea0183(raw_line: &str) -> DecodedPacketInfo {
    let line = raw_line.trim();
    if line.is_empty() {
        return DecodedPacketInfo {
            protocol: "NMEA 0183".to_string(),
            identifier: "EMPTY".to_string(),
            summary: "Empty line".to_string(),
            checksum_valid: None,
            fields: Vec::new(),
        };
    }

    let prefix = line.chars().next().unwrap_or('$');
    let without_prefix = if prefix == '$' || prefix == '!' {
        &line[1..]
    } else {
        line
    };

    let (body, checksum_valid, cs_field) =
        if let Some((b, cs_str)) = without_prefix.rsplit_once('*') {
            let expected = compute_checksum(b);
            let parsed = u8::from_str_radix(cs_str.trim(), 16).ok();
            let valid = parsed == Some(expected);
            (
                b,
                Some(valid),
                format!("*{} (Expected {:02X})", cs_str.trim(), expected),
            )
        } else {
            (without_prefix, Some(false), "Missing *CS".to_string())
        };

    let parts: Vec<&str> = body.split(',').collect();
    let header = parts.first().copied().unwrap_or("UNKNOWN");
    let (talker, formatter) = if header.len() >= 5 {
        (&header[..2], &header[2..])
    } else {
        ("--", header)
    };

    let mut fields = vec![
        ("Talker ID".to_string(), talker.to_string()),
        ("Sentence Formatter".to_string(), formatter.to_string()),
        ("Checksum".to_string(), cs_field),
    ];

    let summary = match formatter {
        "GGA" if parts.len() >= 10 => {
            fields.push(("UTC Time".into(), parts[1].into()));
            fields.push(("Latitude".into(), format!("{} {}", parts[2], parts[3])));
            fields.push(("Longitude".into(), format!("{} {}", parts[4], parts[5])));
            fields.push(("Fix Quality".into(), parts[6].into()));
            fields.push(("Satellites".into(), parts[7].into()));
            fields.push(("HDOP".into(), parts[8].into()));
            fields.push(("Altitude (m)".into(), parts[9].into()));
            format!(
                "GPS Fix: {} {}, {} {}, Sats: {}, Alt: {}m",
                parts[2], parts[3], parts[4], parts[5], parts[7], parts[9]
            )
        }
        "RMC" if parts.len() >= 10 => {
            fields.push(("UTC Time".into(), parts[1].into()));
            fields.push(("Status".into(), parts[2].into()));
            fields.push(("Latitude".into(), format!("{} {}", parts[3], parts[4])));
            fields.push(("Longitude".into(), format!("{} {}", parts[5], parts[6])));
            fields.push(("SOG (kn)".into(), parts[7].into()));
            fields.push(("COG (deg T)".into(), parts[8].into()));
            fields.push(("Date".into(), parts[9].into()));
            format!(
                "GNSS RMC: {} {}, {} {} | SOG {} kn, COG {}°",
                parts[3], parts[4], parts[5], parts[6], parts[7], parts[8]
            )
        }
        "VTG" if parts.len() >= 8 => {
            fields.push(("COG True (deg)".into(), parts[1].into()));
            fields.push(("COG Mag (deg)".into(), parts[3].into()));
            fields.push(("SOG (knots)".into(), parts[5].into()));
            fields.push(("SOG (km/h)".into(), parts[7].into()));
            format!(
                "Course & Ground Speed: COG {}°T, SOG {} kn",
                parts[1], parts[5]
            )
        }
        "HDT" if parts.len() >= 2 => {
            fields.push(("Heading True (deg)".into(), parts[1].into()));
            format!("True Heading: {}°T", parts[1])
        }
        "HDG" if parts.len() >= 5 => {
            fields.push(("Sensor Heading (deg)".into(), parts[1].into()));
            fields.push(("Deviation".into(), format!("{} {}", parts[2], parts[3])));
            fields.push((
                "Variation".into(),
                format!("{} {}", parts[4], parts.get(5).copied().unwrap_or("")),
            ));
            format!("Magnetic Heading: {}°", parts[1])
        }
        "ROT" if parts.len() >= 2 => {
            fields.push(("Rate of Turn (deg/min)".into(), parts[1].into()));
            format!("Rate of Turn: {}°/min", parts[1])
        }
        "MWV" if parts.len() >= 5 => {
            let ref_str = if parts[2] == "R" {
                "Apparent (R)"
            } else {
                "True (T)"
            };
            fields.push(("Wind Angle (deg)".into(), parts[1].into()));
            fields.push(("Reference".into(), ref_str.into()));
            fields.push(("Wind Speed".into(), format!("{} {}", parts[3], parts[4])));
            format!(
                "Wind ({}) Angle {}°, Speed {} {}",
                ref_str, parts[1], parts[3], parts[4]
            )
        }
        "DPT" if parts.len() >= 3 => {
            fields.push(("Depth Below Transducer (m)".into(), parts[1].into()));
            fields.push(("Offset (m)".into(), parts[2].into()));
            format!("Water Depth: {}m (Offset {}m)", parts[1], parts[2])
        }
        "DBT" if parts.len() >= 4 => {
            fields.push(("Depth (ft)".into(), parts[1].into()));
            fields.push(("Depth (m)".into(), parts[3].into()));
            format!("Depth Below Transducer: {}m ({}ft)", parts[3], parts[1])
        }
        "MTW" if parts.len() >= 2 => {
            fields.push(("Water Temperature (°C)".into(), parts[1].into()));
            format!("Water Temperature: {}°C", parts[1])
        }
        "VHW" if parts.len() >= 6 => {
            fields.push(("Heading True (deg)".into(), parts[1].into()));
            fields.push(("Water Speed (knots)".into(), parts[5].into()));
            format!(
                "Speed Through Water: {} kn, Heading {}°T",
                parts[5], parts[1]
            )
        }
        "RSA" if parts.len() >= 2 => {
            fields.push(("Rudder Angle (deg)".into(), parts[1].into()));
            format!("Rudder Angle: {}°", parts[1])
        }
        "RPM" if parts.len() >= 4 => {
            fields.push(("Source".into(), parts[1].into()));
            fields.push(("Engine #".into(), parts[2].into()));
            fields.push(("Speed (RPM)".into(), parts[3].into()));
            format!("Engine #{} Speed: {} RPM", parts[2], parts[3])
        }
        "APB" | "RMB" | "XTE" | "BWC" | "BOD" => {
            for (idx, p) in parts.iter().enumerate().skip(1) {
                fields.push((format!("Field {}", idx), (*p).to_string()));
            }
            format!("Navigation / Autopilot {} ({})", formatter, body)
        }
        "VDM" | "VDO" if parts.len() >= 6 => {
            fields.push(("Total Fragments".into(), parts[1].into()));
            fields.push(("Fragment #".into(), parts[2].into()));
            fields.push(("Channel".into(), parts[4].into()));
            fields.push(("6-bit Armored Payload".into(), parts[5].into()));
            format!(
                "AIS {} (Frag {}/{}, Ch {}) Payload: {}",
                formatter, parts[2], parts[1], parts[4], parts[5]
            )
        }
        "TTM" if parts.len() >= 12 => {
            fields.push(("Target #".into(), parts[1].into()));
            fields.push(("Target Distance (NM)".into(), parts[2].into()));
            fields.push(("Bearing True (deg)".into(), parts[3].into()));
            fields.push(("Target Speed (kn)".into(), parts[5].into()));
            fields.push(("Target Course (deg)".into(), parts[6].into()));
            fields.push(("CPA (NM)".into(), parts[8].into()));
            fields.push(("TCPA (min)".into(), parts[9].into()));
            fields.push(("Target Name".into(), parts[11].into()));
            format!(
                "ARPA Target #{}: {} | Rng {} NM, Brg {}°T, CPA {} NM, TCPA {} min",
                parts[1], parts[11], parts[2], parts[3], parts[8], parts[9]
            )
        }
        _ => {
            for (idx, p) in parts.iter().enumerate().skip(1) {
                fields.push((format!("Field {}", idx), (*p).to_string()));
            }
            format!("NMEA 0183 {}: {}", header, body)
        }
    };

    DecodedPacketInfo {
        protocol: "NMEA 0183".to_string(),
        identifier: header.to_string(),
        summary,
        checksum_valid,
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_checksum() {
        assert_eq!(compute_checksum(""), 0x00);
        assert_eq!(
            compute_checksum("GPGLL,5300.97914,N,00259.98174,E,125926,A"),
            0x28
        );
        assert_eq!(compute_checksum("HCHDT,243.5,T"), 0x29);
    }

    #[test]
    fn test_wrap_sentence_all_error_modes() {
        let normal = wrap_sentence('$', "HCHDT,243.5,T", &ErrorInjectionConfig::default());
        assert_eq!(normal, "$HCHDT,243.5,T*29");

        let corrupt = wrap_sentence(
            '$',
            "HCHDT,243.5,T",
            &ErrorInjectionConfig {
                corrupt_checksum: true,
                omit_checksum: false,
                omit_crlf: false,
            },
        );
        assert_eq!(corrupt, "$HCHDT,243.5,T*73");

        let omitted = wrap_sentence(
            '$',
            "HCHDT,243.5,T",
            &ErrorInjectionConfig {
                corrupt_checksum: false,
                omit_checksum: true,
                omit_crlf: false,
            },
        );
        assert_eq!(omitted, "$HCHDT,243.5,T");
    }

    #[test]
    fn test_format_lat_and_lon_nmea() {
        let (lat_n, h_n) = format_lat_nmea(48.358);
        assert_eq!(h_n, 'N');
        assert_eq!(lat_n, "4821.4800");

        let (lat_s, h_s) = format_lat_nmea(-33.85);
        assert_eq!(h_s, 'S');
        assert_eq!(lat_s, "3351.0000");

        let (lon_e, h_e) = format_lon_nmea(151.20);
        assert_eq!(h_e, 'E');
        assert_eq!(lon_e, "15112.0000");

        let (lon_w, h_w) = format_lon_nmea(-4.485);
        assert_eq!(h_w, 'W');
        assert_eq!(lon_w, "00429.1000");
    }

    #[test]
    fn test_generate_all_35_sentence_0183_kinds_and_inspect() {
        let mut vessel = VesselState::default();
        vessel.fluctuation.enabled = false;
        vessel.dual_engine = true;
        let route = Route::default();
        let err = ErrorInjectionConfig::default();

        for kind in Sentence0183Kind::ALL {
            let sentences = generate_sentence("GP", kind, &vessel, &route, &err);
            assert!(
                !sentences.is_empty(),
                "Expected non-empty sentences for {:?}",
                kind
            );
            for s in sentences {
                let info = inspect_nmea0183(&s);
                assert_eq!(info.checksum_valid, Some(true), "Failed on {}", s);
                assert_eq!(info.protocol, "NMEA 0183");
                assert!(!info.summary.is_empty());
            }
        }
    }

    #[test]
    fn test_bit_writer_primitives() {
        let mut bw = BitWriter::new();
        bw.push_uint(0, 6); // '0' (48)
        bw.push_uint(39, 6); // 'W' (87)
        bw.push_uint(40, 6); // '`' (96)
        bw.push_int(-1, 6); // 63 -> 'w' (119)
        let (armored, fill) = bw.to_armored_string();
        assert_eq!(fill, 0);
        assert_eq!(armored, "0W`w");

        let mut bw2 = BitWriter::new();
        bw2.push_ais_text("AB", 3); // Pads 3rd char with '@' (0)
        bw2.push_uint(1, 2); // 2 extra bits -> 4 fill bits
        let (arm2, fill2) = bw2.to_armored_string();
        assert_eq!(fill2, 4);
        assert_eq!(arm2.len(), 4);
    }

    #[test]
    fn test_ais_position_and_static_and_arpa_encoders() {
        let vessel = VesselState::default();
        let fleet = TargetVessel::default_fleet();

        // Class A target with positive and negative ROT
        let vdm_a = encode_ais_position_report(
            false,
            fleet[0].mmsi,
            0,
            -5.0,
            12.3,
            48.35,
            -4.50,
            120.0,
            119.0,
            false,
        );
        assert!(vdm_a.starts_with("!AIVDM,1,1,,A,"));
        let info_a = inspect_nmea0183(&vdm_a);
        assert_eq!(info_a.checksum_valid, Some(true));
        assert!(info_a.summary.contains("AIS VDM"));

        // Own ship Class B VDO
        let vdo_b = encode_ais_position_report(
            true,
            vessel.mmsi,
            0,
            0.0,
            7.8,
            vessel.latitude,
            vessel.longitude,
            vessel.cog_deg,
            vessel.heading_true_deg,
            true,
        );
        assert!(vdo_b.starts_with("!AIVDO,1,1,,A,"));
        assert_eq!(inspect_nmea0183(&vdo_b).checksum_valid, Some(true));

        // Static Class A (2 fragments)
        let stat_a = encode_ais_static_report(&fleet[0]);
        assert_eq!(stat_a.len(), 2);

        // Static Class B (1 fragment)
        let stat_b = encode_ais_static_report(&fleet[1]);
        assert_eq!(stat_b.len(), 1);

        // Static ArpaOnly (0 fragments)
        let mut arpa_t = fleet[0].clone();
        arpa_t.ais_class = AisClass::ArpaOnly;
        assert!(encode_ais_static_report(&arpa_t).is_empty());

        // ARPA TTM
        let ttm = encode_arpa_ttm(&fleet[0], &vessel);
        let ttm_info = inspect_nmea0183(&ttm);
        assert_eq!(ttm_info.checksum_valid, Some(true));
        assert!(ttm_info.summary.contains("ARPA Target"));
    }

    #[test]
    fn test_inspect_nmea0183_edge_cases() {
        let empty = inspect_nmea0183("   ");
        assert_eq!(empty.identifier, "EMPTY");
        assert!(empty.checksum_valid.is_none());

        let no_cs = inspect_nmea0183("$HCHDT,180.0,T");
        assert_eq!(no_cs.checksum_valid, Some(false));

        let bad_cs = inspect_nmea0183("$HCHDT,180.0,T*FF");
        assert_eq!(bad_cs.checksum_valid, Some(false));
    }
}
