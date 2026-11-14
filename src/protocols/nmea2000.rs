use chrono::{Timelike, Utc};
use crate::model::ais::{AisClass, TargetVessel};
use crate::model::instruments::{ErrorInjectionConfig, N2kWireFormat, Pgn2000Kind};
use crate::model::route::Route;
use crate::model::vessel::VesselState;
use crate::protocols::nmea0183::wrap_sentence;
use crate::protocols::DecodedPacketInfo;

/// Builds a 29-bit ISO 11783 / NMEA 2000 CAN Identifier
pub fn encode_can_id(priority: u8, pgn: u32, src: u8, dst: u8) -> u32 {
    let prio = (priority & 0x07) as u32;
    let dp = (pgn >> 16) & 0x03;
    let pf = (pgn >> 8) & 0xFF;
    let ps = if pf < 240 {
        (dst as u32) & 0xFF
    } else {
        pgn & 0xFF
    };
    (prio << 26) | (dp << 24) | (pf << 16) | (ps << 8) | (src as u32)
}

/// Extracts (priority, pgn, src, dst) from a 29-bit CAN Identifier
pub fn decode_can_id(can_id: u32) -> (u8, u32, u8, u8) {
    let prio = ((can_id >> 26) & 0x07) as u8;
    let dp = (can_id >> 24) & 0x03;
    let pf = (can_id >> 16) & 0xFF;
    let ps = (can_id >> 8) & 0xFF;
    let src = (can_id & 0xFF) as u8;

    let (pgn, dst) = if pf < 240 {
        ((dp << 16) | (pf << 8), ps as u8)
    } else {
        ((dp << 16) | (pf << 8) | ps, 0xFF)
    };
    (prio, pgn, src, dst)
}

pub fn format_n2k_frames(
    pgn: u32,
    priority: u8,
    src: u8,
    is_fast_packet: bool,
    fast_seq: u8,
    payload: &[u8],
    wire_format: N2kWireFormat,
    err: &ErrorInjectionConfig,
) -> Vec<String> {
    let now = Utc::now();
    let ts_yd = format!(
        "{:02}:{:02}:{:02}.{:03}",
        now.hour(),
        now.minute(),
        now.second(),
        (now.nanosecond() / 1_000_000) % 1000
    );
    let can_id = encode_can_id(priority, pgn, src, 0xFF);

    match wire_format {
        N2kWireFormat::YachtDevicesRawSingleLine => {
            let hex_bytes: Vec<String> = payload.iter().map(|b| format!("{:02X}", b)).collect();
            vec![format!("{} T {:08X} {}", ts_yd, can_id, hex_bytes.join(" "))]
        }
        N2kWireFormat::YachtDevicesRawFastPacket => {
            if !is_fast_packet && payload.len() <= 8 {
                let mut padded = payload.to_vec();
                while padded.len() < 8 {
                    padded.push(0xFF);
                }
                let hex_bytes: Vec<String> = padded.iter().map(|b| format!("{:02X}", b)).collect();
                vec![format!("{} T {:08X} {}", ts_yd, can_id, hex_bytes.join(" "))]
            } else {
                let mut frames = Vec::new();
                let seq_bits = (fast_seq & 0x07) << 5;
                let total_len = payload.len().min(223) as u8;

                let mut f0 = vec![seq_bits, total_len];
                for i in 0..6 {
                    f0.push(payload.get(i).copied().unwrap_or(0xFF));
                }
                let hex0: Vec<String> = f0.iter().map(|b| format!("{:02X}", b)).collect();
                frames.push(format!("{} T {:08X} {}", ts_yd, can_id, hex0.join(" ")));

                let mut offset = 6usize;
                let mut frame_idx = 1u8;
                while offset < payload.len() && frame_idx < 32 {
                    let mut fn_bytes = vec![seq_bits | frame_idx];
                    for i in 0..7 {
                        fn_bytes.push(payload.get(offset + i).copied().unwrap_or(0xFF));
                    }
                    let hexn: Vec<String> = fn_bytes.iter().map(|b| format!("{:02X}", b)).collect();
                    frames.push(format!("{} T {:08X} {}", ts_yd, can_id, hexn.join(" ")));
                    offset += 7;
                    frame_idx += 1;
                }
                frames
            }
        }
        N2kWireFormat::SeaSmartPcdin => {
            let ts_ms = (now.hour() * 3600 + now.minute() * 60 + now.second()) * 1000
                + (now.nanosecond() / 1_000_000);
            let hex_payload: String = payload.iter().map(|b| format!("{:02X}", b)).collect();
            let body = format!(
                "PCDIN,{:06X},{:08X},{:02X},{}",
                pgn, ts_ms, src, hex_payload
            );
            vec![wrap_sentence('$', &body, err)]
        }
        N2kWireFormat::ActisenseAscii => {
            let ts_sec = now.hour() * 3600 + now.minute() * 60 + now.second();
            let ms = (now.nanosecond() / 1_000_000) % 1000;
            let hex_payload: String = payload.iter().map(|b| format!("{:02X}", b)).collect();
            vec![format!(
                "A{:05}.{:03} {:02X}FF{:1X} {:05X} {}",
                ts_sec, ms, src, priority, pgn, hex_payload
            )]
        }
    }
}

pub fn build_pgn_payload(
    kind: Pgn2000Kind,
    sid: u8,
    vessel: &VesselState,
    route: &Route,
) -> Vec<u8> {
    let m = vessel.sampled_metrics();
    let now = Utc::now();
    let days_since_epoch = (now.timestamp() / 86_400).max(0) as u16;
    let secs_since_midnight = (now.num_seconds_from_midnight() as f64)
        + (now.nanosecond() as f64) / 1_000_000_000.0;
    let time_0001s = (secs_since_midnight * 10_000.0).round() as u32;

    let mut payload = Vec::with_capacity(64);

    match kind {
        Pgn2000Kind::Pgn126992SystemTime => {
            payload.push(sid);
            payload.push(0xF0);
            payload.extend_from_slice(&days_since_epoch.to_le_bytes());
            payload.extend_from_slice(&time_0001s.to_le_bytes());
        }
        Pgn2000Kind::Pgn127245Rudder => {
            payload.push(0);
            payload.push(0xFC);
            payload.extend_from_slice(&0x7FFFi16.to_le_bytes());
            let pos_rad = vessel.rudder_angle_deg.to_radians();
            let pos_raw = (pos_rad * 10_000.0).round().clamp(-32767.0, 32767.0) as i16;
            payload.extend_from_slice(&pos_raw.to_le_bytes());
            payload.extend_from_slice(&[0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn127250VesselHeading => {
            payload.push(sid);
            let hdg_rad = m.heading_true_deg.to_radians();
            let hdg_raw = (hdg_rad * 10_000.0).round().clamp(0.0, 65534.0) as u16;
            let dev_raw = (vessel.mag_dev_deg.to_radians() * 10_000.0).round() as i16;
            let var_raw = (vessel.mag_var_deg.to_radians() * 10_000.0).round() as i16;
            payload.extend_from_slice(&hdg_raw.to_le_bytes());
            payload.extend_from_slice(&dev_raw.to_le_bytes());
            payload.extend_from_slice(&var_raw.to_le_bytes());
            payload.push(0xFC);
        }
        Pgn2000Kind::Pgn127251RateOfTurn => {
            payload.push(sid);
            let rot_rad_s = (m.rot_deg_per_min / 60.0).to_radians();
            let rot_raw = (rot_rad_s * 32_000_000.0).round() as i32;
            payload.extend_from_slice(&rot_raw.to_le_bytes());
            payload.extend_from_slice(&[0xFF, 0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn127257Attitude => {
            payload.push(sid);
            let yaw_raw = (m.heading_true_deg.to_radians() * 10_000.0).round() as i16;
            let pitch_raw = (m.pitch_deg.to_radians() * 10_000.0).round() as i16;
            let roll_raw = (m.roll_deg.to_radians() * 10_000.0).round() as i16;
            payload.extend_from_slice(&yaw_raw.to_le_bytes());
            payload.extend_from_slice(&pitch_raw.to_le_bytes());
            payload.extend_from_slice(&roll_raw.to_le_bytes());
            payload.push(0xFF);
        }
        Pgn2000Kind::Pgn127258MagneticVariation => {
            payload.push(sid);
            payload.push(0xF1);
            payload.extend_from_slice(&days_since_epoch.to_le_bytes());
            let var_raw = (vessel.mag_var_deg.to_radians() * 10_000.0).round() as i16;
            payload.extend_from_slice(&var_raw.to_le_bytes());
            payload.extend_from_slice(&[0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn127488EngineRapid => {
            payload.push(0);
            let rpm_raw = (vessel.port_engine_rpm * 4.0).round().clamp(0.0, 65534.0) as u16;
            payload.extend_from_slice(&rpm_raw.to_le_bytes());
            payload.extend_from_slice(&0xFFFFu16.to_le_bytes());
            payload.push(0);
            payload.extend_from_slice(&[0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn127489EngineDynamic => {
            payload.push(0);
            let oil_press_raw = (vessel.engine_oil_pressure_kpa * 10.0).round() as u16;
            let oil_temp_raw =
                ((vessel.engine_coolant_temp_c + 8.0 + 273.15) * 10.0).round() as u16;
            let coolant_temp_raw = ((vessel.engine_coolant_temp_c + 273.15) * 100.0).round() as u16;
            let alt_v_raw = (vessel.alternator_voltage_v * 100.0).round() as i16;
            let fuel_rate_raw = (vessel.fuel_rate_lph * 10.0).round() as i16;
            let hours_sec = (vessel.engine_hours * 3600.0).round() as u32;
            payload.extend_from_slice(&oil_press_raw.to_le_bytes());
            payload.extend_from_slice(&oil_temp_raw.to_le_bytes());
            payload.extend_from_slice(&coolant_temp_raw.to_le_bytes());
            payload.extend_from_slice(&alt_v_raw.to_le_bytes());
            payload.extend_from_slice(&fuel_rate_raw.to_le_bytes());
            payload.extend_from_slice(&hours_sec.to_le_bytes());
            payload.extend_from_slice(&0xFFFFu16.to_le_bytes());
            payload.extend_from_slice(&0xFFFFu16.to_le_bytes());
            payload.push(0xFF);
            payload.extend_from_slice(&0u16.to_le_bytes());
            payload.extend_from_slice(&0u16.to_le_bytes());
            payload.push(50);
            payload.push(55);
        }
        Pgn2000Kind::Pgn127505FluidLevel => {
            payload.push(0x00);
            let level_raw = (vessel.fuel_tank_level_pct * 250.0).round() as i16;
            let cap_raw = (450.0 * 10.0) as u32;
            payload.extend_from_slice(&level_raw.to_le_bytes());
            payload.extend_from_slice(&cap_raw.to_le_bytes());
            payload.push(0xFF);
        }
        Pgn2000Kind::Pgn127508BatteryStatus => {
            payload.push(0);
            let v_raw = (vessel.house_battery_voltage_v * 100.0).round() as i16;
            let c_raw = (vessel.battery_current_a * 10.0).round() as i16;
            let t_raw = ((25.0_f64 + 273.15) * 100.0).round() as u16;
            payload.extend_from_slice(&v_raw.to_le_bytes());
            payload.extend_from_slice(&c_raw.to_le_bytes());
            payload.extend_from_slice(&t_raw.to_le_bytes());
            payload.push(sid);
        }
        Pgn2000Kind::Pgn128259SpeedWater => {
            payload.push(sid);
            let stw_ms = m.stw_knots * 0.514444;
            let sog_ms = m.sog_knots * 0.514444;
            let stw_raw = (stw_ms * 100.0).round().clamp(0.0, 65534.0) as u16;
            let sog_raw = (sog_ms * 100.0).round().clamp(0.0, 65534.0) as u16;
            payload.extend_from_slice(&stw_raw.to_le_bytes());
            payload.extend_from_slice(&sog_raw.to_le_bytes());
            payload.push(0);
            payload.extend_from_slice(&[0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn128267WaterDepth => {
            payload.push(sid);
            let depth_cm = (m.depth_transducer_m * 100.0).round().max(0.0) as u32;
            let offset_mm = (vessel.transducer_offset_m * 1000.0).round() as i16;
            payload.extend_from_slice(&depth_cm.to_le_bytes());
            payload.extend_from_slice(&offset_mm.to_le_bytes());
            payload.push(100);
        }
        Pgn2000Kind::Pgn128275DistanceLog => {
            payload.extend_from_slice(&days_since_epoch.to_le_bytes());
            payload.extend_from_slice(&time_0001s.to_le_bytes());
            let total_m = (vessel.log_total_nm * 1852.0).round() as u32;
            let trip_m = (vessel.log_trip_nm * 1852.0).round() as u32;
            payload.extend_from_slice(&total_m.to_le_bytes());
            payload.extend_from_slice(&trip_m.to_le_bytes());
        }
        Pgn2000Kind::Pgn129025PositionRapid => {
            let lat_raw = (m.latitude * 1e7).round() as i32;
            let lon_raw = (m.longitude * 1e7).round() as i32;
            payload.extend_from_slice(&lat_raw.to_le_bytes());
            payload.extend_from_slice(&lon_raw.to_le_bytes());
        }
        Pgn2000Kind::Pgn129026CogSogRapid => {
            payload.push(sid);
            payload.push(0xFC);
            let cog_raw = (m.cog_deg.to_radians() * 10_000.0)
                .round()
                .clamp(0.0, 65534.0) as u16;
            let sog_ms = m.sog_knots * 0.514444;
            let sog_raw = (sog_ms * 100.0).round().clamp(0.0, 65534.0) as u16;
            payload.extend_from_slice(&cog_raw.to_le_bytes());
            payload.extend_from_slice(&sog_raw.to_le_bytes());
            payload.extend_from_slice(&[0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn129029GnssPositionData => {
            payload.push(sid);
            payload.extend_from_slice(&days_since_epoch.to_le_bytes());
            payload.extend_from_slice(&time_0001s.to_le_bytes());
            let lat_i64 = (m.latitude * 1e16).round() as i64;
            let lon_i64 = (m.longitude * 1e16).round() as i64;
            let alt_i64 = (vessel.altitude_m * 1e6).round() as i64;
            payload.extend_from_slice(&lat_i64.to_le_bytes());
            payload.extend_from_slice(&lon_i64.to_le_bytes());
            payload.extend_from_slice(&alt_i64.to_le_bytes());
            payload.push(0x10 | (vessel.gps_fix_quality & 0x0F));
            payload.push(0xFC);
            payload.push(vessel.satellites_in_use);
            let hdop_raw = (vessel.hdop * 100.0).round() as i16;
            let pdop_raw = (vessel.hdop * 140.0).round() as i16;
            let geoid_raw = (47.2 * 100.0f64).round() as i32;
            payload.extend_from_slice(&hdop_raw.to_le_bytes());
            payload.extend_from_slice(&pdop_raw.to_le_bytes());
            payload.extend_from_slice(&geoid_raw.to_le_bytes());
            payload.push(0);
        }
        Pgn2000Kind::Pgn129283CrossTrackError => {
            let xte_m = route
                .active_leg_metrics(m.latitude, m.longitude, m.sog_knots, m.cog_deg)
                .map(|l| l.xte_nm * 1852.0)
                .unwrap_or(0.0);
            payload.push(sid);
            payload.push(0xC0);
            let xte_raw = (xte_m * 100.0).round() as i32;
            payload.extend_from_slice(&xte_raw.to_le_bytes());
            payload.extend_from_slice(&[0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn129284NavigationData => {
            if let Some(leg) =
                route.active_leg_metrics(m.latitude, m.longitude, m.sog_knots, m.cog_deg)
            {
                let dest_wp = &route.waypoints[leg.dest_idx];
                payload.push(sid);
                let dist_cm = (leg.distance_to_dest_nm * 1852.0 * 100.0).round() as u32;
                payload.extend_from_slice(&dist_cm.to_le_bytes());
                payload.push(0xC0);
                let brg_ptd_raw =
                    (leg.bearing_present_to_dest_true.to_radians() * 10_000.0).round() as u16;
                let brg_otd_raw =
                    (leg.bearing_origin_to_dest_true.to_radians() * 10_000.0).round() as u16;
                payload.extend_from_slice(&days_since_epoch.to_le_bytes());
                payload.extend_from_slice(&time_0001s.to_le_bytes());
                payload.extend_from_slice(&(leg.origin_idx as u32).to_le_bytes());
                payload.extend_from_slice(&(leg.dest_idx as u32).to_le_bytes());
                let wlat_raw = (dest_wp.latitude * 1e7).round() as i32;
                let wlon_raw = (dest_wp.longitude * 1e7).round() as i32;
                payload.extend_from_slice(&wlat_raw.to_le_bytes());
                payload.extend_from_slice(&wlon_raw.to_le_bytes());
                let vmg_raw = (leg.vmg_knots * 0.514444 * 100.0).round() as i16;
                payload.extend_from_slice(&vmg_raw.to_le_bytes());
                payload.extend_from_slice(&brg_otd_raw.to_le_bytes());
                payload.extend_from_slice(&brg_ptd_raw.to_le_bytes());
            }
        }
        Pgn2000Kind::Pgn130306WindDataApparent => {
            payload.push(sid);
            let aws_ms = m.wind_apparent_speed_knots * 0.514444;
            let spd_raw = (aws_ms * 100.0).round().clamp(0.0, 65534.0) as u16;
            let ang_raw = (m.wind_apparent_angle_deg.to_radians() * 10_000.0)
                .round()
                .clamp(0.0, 65534.0) as u16;
            payload.extend_from_slice(&spd_raw.to_le_bytes());
            payload.extend_from_slice(&ang_raw.to_le_bytes());
            payload.push(0xFA);
            payload.extend_from_slice(&[0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn130306WindDataTrue => {
            payload.push(sid);
            let tws_ms = m.wind_true_speed_knots * 0.514444;
            let spd_raw = (tws_ms * 100.0).round().clamp(0.0, 65534.0) as u16;
            let ang_raw = (m.wind_true_angle_deg.to_radians() * 10_000.0)
                .round()
                .clamp(0.0, 65534.0) as u16;
            payload.extend_from_slice(&spd_raw.to_le_bytes());
            payload.extend_from_slice(&ang_raw.to_le_bytes());
            payload.push(0xFB);
            payload.extend_from_slice(&[0xFF, 0xFF]);
        }
        Pgn2000Kind::Pgn130310EnvironmentalOutside => {
            payload.push(sid);
            let w_temp_raw = ((vessel.water_temp_c + 273.15) * 100.0).round() as u16;
            let a_temp_raw = ((vessel.air_temp_c + 273.15) * 100.0).round() as u16;
            let press_raw = (vessel.barometric_pressure_hpa).round() as u16;
            payload.extend_from_slice(&w_temp_raw.to_le_bytes());
            payload.extend_from_slice(&a_temp_raw.to_le_bytes());
            payload.extend_from_slice(&press_raw.to_le_bytes());
            payload.push(0xFF);
        }
        Pgn2000Kind::Pgn130311EnvironmentalParams => {
            payload.push(sid);
            payload.push(0x04);
            let a_temp_raw = ((vessel.air_temp_c + 273.15) * 100.0).round() as u16;
            let hum_raw = (vessel.relative_humidity_pct * 250.0).round() as i16;
            let press_raw = (vessel.barometric_pressure_hpa).round() as u16;
            payload.extend_from_slice(&a_temp_raw.to_le_bytes());
            payload.extend_from_slice(&hum_raw.to_le_bytes());
            payload.extend_from_slice(&press_raw.to_le_bytes());
        }
        Pgn2000Kind::Pgn130312Temperature => {
            payload.push(sid);
            payload.push(0);
            payload.push(0);
            let w_temp_raw = ((vessel.water_temp_c + 273.15) * 100.0).round() as u16;
            payload.extend_from_slice(&w_temp_raw.to_le_bytes());
            payload.extend_from_slice(&0xFFFFu16.to_le_bytes());
            payload.push(0xFF);
        }
        Pgn2000Kind::Pgn130314ActualPressure => {
            payload.push(sid);
            payload.push(0);
            payload.push(0);
            let press_raw = (vessel.barometric_pressure_hpa * 1000.0).round() as i32;
            payload.extend_from_slice(&press_raw.to_le_bytes());
            payload.push(0xFF);
        }
    }

    payload
}

pub fn generate_pgn_frames(
    kind: Pgn2000Kind,
    sid: u8,
    fast_seq: u8,
    src: u8,
    wire_format: N2kWireFormat,
    vessel: &VesselState,
    route: &Route,
    err: &ErrorInjectionConfig,
) -> Vec<String> {
    let payload = build_pgn_payload(kind, sid, vessel, route);
    if payload.is_empty() {
        return Vec::new();
    }
    format_n2k_frames(
        kind.pgn(),
        kind.default_priority(),
        src,
        kind.is_fast_packet(),
        fast_seq,
        &payload,
        wire_format,
        err,
    )
}

/// Generates NMEA 2000 AIS Position Report PGN 129038 (Class A) or PGN 129039 (Class B)
pub fn encode_ais_n2k_position(
    target: &TargetVessel,
    src: u8,
    fast_seq: u8,
    wire_format: N2kWireFormat,
) -> Vec<String> {
    let pgn = match target.ais_class {
        AisClass::ClassA => 129038,
        AisClass::ClassB => 129039,
        AisClass::ArpaOnly => return Vec::new(),
    };

    let mut payload = Vec::with_capacity(28);
    let msg_id = if pgn == 129038 { 1u8 } else { 18u8 };
    payload.push(msg_id);
    payload.extend_from_slice(&target.mmsi.to_le_bytes());
    let lon_raw = (target.longitude * 1e7).round() as i32;
    let lat_raw = (target.latitude * 1e7).round() as i32;
    payload.extend_from_slice(&lon_raw.to_le_bytes());
    payload.extend_from_slice(&lat_raw.to_le_bytes());
    payload.push(0x01);
    let cog_raw = (target.cog_deg.to_radians() * 10_000.0)
        .round()
        .clamp(0.0, 65534.0) as u16;
    let sog_raw = (target.sog_knots * 0.514444 * 100.0)
        .round()
        .clamp(0.0, 65534.0) as u16;
    payload.extend_from_slice(&cog_raw.to_le_bytes());
    payload.extend_from_slice(&sog_raw.to_le_bytes());
    payload.extend_from_slice(&[0x00, 0x00, 0x00]);
    let hdg_raw = (target.heading_deg.to_radians() * 10_000.0)
        .round()
        .clamp(0.0, 65534.0) as u16;
    payload.extend_from_slice(&hdg_raw.to_le_bytes());
    let rot_rad_s = (target.rot_deg_per_min / 60.0).to_radians();
    let rot_raw = (rot_rad_s * 32_000.0).round() as i16;
    payload.extend_from_slice(&rot_raw.to_le_bytes());
    payload.push(target.nav_status.code());
    payload.push(0xFF);

    format_n2k_frames(
        pgn,
        4,
        src,
        true,
        fast_seq,
        &payload,
        wire_format,
        &ErrorInjectionConfig::default(),
    )
}

// =========================================================================
// NMEA 2000 Decoder & Packet Inspector
// =========================================================================

pub fn inspect_nmea2000(raw_line: &str) -> DecodedPacketInfo {
    let line = raw_line.trim();

    // Case 1: SeaSmart $PCDIN,<pgn>,<ts>,<src>,<hex>*<cs>
    if line.starts_with("$PCDIN,") {
        let without_prefix = &line[1..];
        let (body, cs_valid) = if let Some((b, cs_str)) = without_prefix.rsplit_once('*') {
            let expected = crate::protocols::nmea0183::compute_checksum(b);
            let parsed = u8::from_str_radix(cs_str.trim(), 16).ok();
            (b, Some(parsed == Some(expected)))
        } else {
            (without_prefix, Some(false))
        };
        let parts: Vec<&str> = body.split(',').collect();
        if parts.len() >= 5 {
            let pgn = u32::from_str_radix(parts[1], 16).unwrap_or(0);
            let ts = parts[2];
            let src = u8::from_str_radix(parts[3], 16).unwrap_or(0);
            let bytes = parse_hex_stream(parts[4]);
            return decode_pgn_payload(
                "NMEA 2000 ($PCDIN)",
                pgn,
                3,
                src,
                0xFF,
                Some(ts),
                cs_valid,
                &bytes,
            );
        }
    }

    // Case 2: Actisense ASCII A<ts> <src><dst><prio> <pgn> <hex>
    if line.starts_with('A') && line.len() > 16 {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() >= 4 {
            let ts = &tokens[0][1..];
            let sdp = tokens[1];
            let src = if sdp.len() >= 2 {
                u8::from_str_radix(&sdp[0..2], 16).unwrap_or(0)
            } else {
                0
            };
            let prio = if sdp.len() >= 5 {
                u8::from_str_radix(&sdp[4..5], 16).unwrap_or(3)
            } else {
                3
            };
            let pgn = u32::from_str_radix(tokens[2], 16).unwrap_or(0);
            let hex_concat: String = tokens[3..].join("");
            let bytes = parse_hex_stream(&hex_concat);
            return decode_pgn_payload(
                "NMEA 2000 (Actisense)",
                pgn,
                prio,
                src,
                0xFF,
                Some(ts),
                None,
                &bytes,
            );
        }
    }

    // Case 3: Yacht Devices RAW: HH:MM:SS.mmm D/T/R <CAN_ID_HEX> <B0> <B1> ...
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() >= 4 && (tokens[1] == "T" || tokens[1] == "R" || tokens[1] == "D") {
        if let Ok(can_id) = u32::from_str_radix(tokens[2], 16) {
            let (prio, pgn, src, dst) = decode_can_id(can_id);
            let hex_concat: String = tokens[3..].join("");
            let bytes = parse_hex_stream(&hex_concat);
            return decode_pgn_payload(
                "NMEA 2000 (Yacht Devices RAW)",
                pgn,
                prio,
                src,
                dst,
                Some(tokens[0]),
                None,
                &bytes,
            );
        }
    }

    DecodedPacketInfo {
        protocol: "NMEA 2000".to_string(),
        identifier: "RAW".to_string(),
        summary: format!("Unrecognized frame: {}", line),
        checksum_valid: None,
        fields: vec![("Raw".to_string(), line.to_string())],
    }
}

pub fn parse_hex_stream(s: &str) -> Vec<u8> {
    let clean: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let mut out = Vec::with_capacity(clean.len() / 2);
    let mut i = 0;
    while i + 1 < clean.len() {
        if let Ok(b) = u8::from_str_radix(&clean[i..i + 2], 16) {
            out.push(b);
        }
        i += 2;
    }
    out
}

pub fn decode_pgn_payload(
    protocol_label: &str,
    pgn: u32,
    prio: u8,
    src: u8,
    dst: u8,
    timestamp: Option<&str>,
    checksum_valid: Option<bool>,
    bytes: &[u8],
) -> DecodedPacketInfo {
    let hex_str: Vec<String> = bytes.iter().map(|b| format!("{:02X}", b)).collect();
    let mut fields = vec![
        ("PGN".to_string(), format!("{} (0x{:05X})", pgn, pgn)),
        ("Priority".to_string(), prio.to_string()),
        ("Source Address".to_string(), format!("{} (0x{:02X})", src, src)),
        ("Destination".to_string(), format!("0x{:02X}", dst)),
        ("Payload Length".to_string(), format!("{} bytes", bytes.len())),
        ("Payload Hex".to_string(), hex_str.join(" ")),
    ];
    if let Some(ts) = timestamp {
        fields.insert(0, ("Timestamp".to_string(), ts.to_string()));
    }

    let summary = match pgn {
        126992 if bytes.len() >= 8 => {
            let days = u16::from_le_bytes([bytes[2], bytes[3]]);
            let secs = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as f64 * 1e-4;
            fields.push(("Days Since 1970-01-01".into(), days.to_string()));
            fields.push(("Seconds Since Midnight".into(), format!("{:.2} s", secs)));
            format!("PGN 126992 System Time: Day {}, {:.1}s UTC", days, secs)
        }
        129025 if bytes.len() >= 8 => {
            let lat = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64 * 1e-7;
            let lon = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as f64 * 1e-7;
            fields.push(("Latitude (deg)".into(), format!("{:.6}°", lat)));
            fields.push(("Longitude (deg)".into(), format!("{:.6}°", lon)));
            format!("PGN 129025 Position Rapid: {:.5}°, {:.5}°", lat, lon)
        }
        129026 if bytes.len() >= 6 => {
            let cog_rad = u16::from_le_bytes([bytes[2], bytes[3]]) as f64 * 1e-4;
            let sog_ms = u16::from_le_bytes([bytes[4], bytes[5]]) as f64 * 0.01;
            let cog_deg = cog_rad.to_degrees();
            let sog_kn = sog_ms / 0.514444;
            fields.push(("COG True (deg)".into(), format!("{:.1}°", cog_deg)));
            fields.push((
                "SOG (knots)".into(),
                format!("{:.2} kn ({:.2} m/s)", sog_kn, sog_ms),
            ));
            format!(
                "PGN 129026 COG/SOG Rapid: COG {:.1}°T, SOG {:.2} kn",
                cog_deg, sog_kn
            )
        }
        129029 if bytes.len() >= 34 => {
            let mut lat_b = [0u8; 8];
            let mut lon_b = [0u8; 8];
            lat_b.copy_from_slice(&bytes[7..15]);
            lon_b.copy_from_slice(&bytes[15..23]);
            let lat = i64::from_le_bytes(lat_b) as f64 * 1e-16;
            let lon = i64::from_le_bytes(lon_b) as f64 * 1e-16;
            let sats = bytes[33];
            fields.push(("Latitude (deg)".into(), format!("{:.6}°", lat)));
            fields.push(("Longitude (deg)".into(), format!("{:.6}°", lon)));
            fields.push(("Satellites".into(), sats.to_string()));
            format!("PGN 129029 GNSS Position: {:.5}°, {:.5}° ({} sats)", lat, lon, sats)
        }
        127250 if bytes.len() >= 7 => {
            let hdg_rad = u16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 1e-4;
            let dev_rad = i16::from_le_bytes([bytes[3], bytes[4]]) as f64 * 1e-4;
            let var_rad = i16::from_le_bytes([bytes[5], bytes[6]]) as f64 * 1e-4;
            let hdg_deg = hdg_rad.to_degrees();
            fields.push(("Heading (deg)".into(), format!("{:.1}°", hdg_deg)));
            fields.push(("Deviation (deg)".into(), format!("{:.1}°", dev_rad.to_degrees())));
            fields.push(("Variation (deg)".into(), format!("{:.1}°", var_rad.to_degrees())));
            format!("PGN 127250 Vessel Heading: {:.1}°", hdg_deg)
        }
        127251 if bytes.len() >= 5 => {
            let rot_raw = i32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as f64;
            let rot_rad_s = rot_raw / 32_000_000.0;
            let rot_deg_min = rot_rad_s.to_degrees() * 60.0;
            fields.push(("Rate of Turn".into(), format!("{:.1}°/min", rot_deg_min)));
            format!("PGN 127251 Rate of Turn: {:.1}°/min", rot_deg_min)
        }
        127257 if bytes.len() >= 7 => {
            let yaw = (i16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 1e-4).to_degrees();
            let pitch = (i16::from_le_bytes([bytes[3], bytes[4]]) as f64 * 1e-4).to_degrees();
            let roll = (i16::from_le_bytes([bytes[5], bytes[6]]) as f64 * 1e-4).to_degrees();
            fields.push(("Yaw (deg)".into(), format!("{:.1}°", yaw)));
            fields.push(("Pitch (deg)".into(), format!("{:.1}°", pitch)));
            fields.push(("Roll (deg)".into(), format!("{:.1}°", roll)));
            format!("PGN 127257 Attitude: Pitch {:.1}°, Roll {:.1}°", pitch, roll)
        }
        127258 if bytes.len() >= 6 => {
            let var_deg = (i16::from_le_bytes([bytes[4], bytes[5]]) as f64 * 1e-4).to_degrees();
            fields.push(("Magnetic Variation".into(), format!("{:+.1}°", var_deg)));
            format!("PGN 127258 Magnetic Variation: {:+.1}°", var_deg)
        }
        127245 if bytes.len() >= 6 => {
            let pos_rad = i16::from_le_bytes([bytes[4], bytes[5]]) as f64 * 1e-4;
            let pos_deg = pos_rad.to_degrees();
            fields.push(("Rudder Position (deg)".into(), format!("{:.1}°", pos_deg)));
            format!("PGN 127245 Rudder: {:.1}°", pos_deg)
        }
        127488 if bytes.len() >= 3 => {
            let inst = bytes[0];
            let rpm = u16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 0.25;
            fields.push(("Engine Instance".into(), inst.to_string()));
            fields.push(("Engine Speed (RPM)".into(), format!("{:.0} RPM", rpm)));
            format!("PGN 127488 Engine Rapid (Inst {}): {:.0} RPM", inst, rpm)
        }
        127489 if bytes.len() >= 11 => {
            let oil_kpa = u16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 0.1;
            let coolant_c = (u16::from_le_bytes([bytes[5], bytes[6]]) as f64 * 0.01) - 273.15;
            let alt_v = i16::from_le_bytes([bytes[7], bytes[8]]) as f64 * 0.01;
            let fuel_lph = i16::from_le_bytes([bytes[9], bytes[10]]) as f64 * 0.1;
            fields.push(("Oil Pressure".into(), format!("{:.1} kPa", oil_kpa)));
            fields.push(("Coolant Temp".into(), format!("{:.1} °C", coolant_c)));
            fields.push(("Alternator Voltage".into(), format!("{:.2} V", alt_v)));
            fields.push(("Fuel Rate".into(), format!("{:.1} L/h", fuel_lph)));
            format!(
                "PGN 127489 Engine Dynamic: {:.1}°C, {:.0} kPa, {:.2}V, {:.1} L/h",
                coolant_c, oil_kpa, alt_v, fuel_lph
            )
        }
        127505 if bytes.len() >= 7 => {
            let inst = bytes[0] & 0x0F;
            let fluid_type = (bytes[0] >> 4) & 0x0F;
            let level_pct = i16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 0.004;
            let cap_l = u32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]) as f64 * 0.1;
            fields.push((
                "Tank Instance / Type".into(),
                format!("Inst {}, Type {}", inst, fluid_type),
            ));
            fields.push(("Fluid Level (%)".into(), format!("{:.1}%", level_pct)));
            fields.push(("Capacity (L)".into(), format!("{:.1} L", cap_l)));
            format!(
                "PGN 127505 Fluid Level: {:.1}% ({:.0} L)",
                level_pct, cap_l
            )
        }
        127508 if bytes.len() >= 7 => {
            let inst = bytes[0];
            let v = i16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 0.01;
            let c = i16::from_le_bytes([bytes[3], bytes[4]]) as f64 * 0.1;
            fields.push(("Battery Instance".into(), inst.to_string()));
            fields.push(("Voltage (V)".into(), format!("{:.2} V", v)));
            fields.push(("Current (A)".into(), format!("{:.1} A", c)));
            format!("PGN 127508 Battery Status: {:.2} V, {:.1} A", v, c)
        }
        128259 if bytes.len() >= 5 => {
            let stw_ms = u16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 0.01;
            let stw_kn = stw_ms / 0.514444;
            fields.push((
                "Speed Water Referenced".into(),
                format!("{:.2} kn ({:.2} m/s)", stw_kn, stw_ms),
            ));
            format!("PGN 128259 Water Speed: {:.2} kn", stw_kn)
        }
        128267 if bytes.len() >= 7 => {
            let depth_m =
                u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as f64 * 0.01;
            let offset_m = i16::from_le_bytes([bytes[5], bytes[6]]) as f64 * 0.001;
            fields.push(("Depth Below Transducer (m)".into(), format!("{:.2} m", depth_m)));
            fields.push(("Transducer Offset (m)".into(), format!("{:.2} m", offset_m)));
            format!(
                "PGN 128267 Water Depth: {:.2} m (Offset {:+.2} m)",
                depth_m, offset_m
            )
        }
        128275 if bytes.len() >= 14 => {
            let total_nm =
                u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as f64 / 1852.0;
            let trip_nm =
                u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as f64 / 1852.0;
            fields.push(("Total Log (NM)".into(), format!("{:.1} NM", total_nm)));
            fields.push(("Trip Log (NM)".into(), format!("{:.2} NM", trip_nm)));
            format!(
                "PGN 128275 Distance Log: Total {:.1} NM, Trip {:.2} NM",
                total_nm, trip_nm
            )
        }
        130306 if bytes.len() >= 6 => {
            let spd_ms = u16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 0.01;
            let spd_kn = spd_ms / 0.514444;
            let ang_deg = (u16::from_le_bytes([bytes[3], bytes[4]]) as f64 * 1e-4).to_degrees();
            let ref_code = bytes[5] & 0x07;
            let ref_str = if ref_code == 2 { "Apparent" } else { "True" };
            fields.push((
                "Wind Speed".into(),
                format!("{:.1} kn ({:.2} m/s)", spd_kn, spd_ms),
            ));
            fields.push(("Wind Angle".into(), format!("{:.1}°", ang_deg)));
            fields.push(("Reference".into(), ref_str.into()));
            format!(
                "PGN 130306 Wind ({}): {:.1} kn @ {:.1}°",
                ref_str, spd_kn, ang_deg
            )
        }
        130310 if bytes.len() >= 7 => {
            let w_c = (u16::from_le_bytes([bytes[1], bytes[2]]) as f64 * 0.01) - 273.15;
            let a_c = (u16::from_le_bytes([bytes[3], bytes[4]]) as f64 * 0.01) - 273.15;
            let p_hpa = u16::from_le_bytes([bytes[5], bytes[6]]) as f64;
            fields.push(("Water Temperature".into(), format!("{:.1} °C", w_c)));
            fields.push(("Outside Air Temp".into(), format!("{:.1} °C", a_c)));
            fields.push(("Atmospheric Pressure".into(), format!("{:.1} hPa", p_hpa)));
            format!(
                "PGN 130310 Env: Water {:.1}°C, Air {:.1}°C, {:.0} hPa",
                w_c, a_c, p_hpa
            )
        }
        130311 if bytes.len() >= 8 => {
            let a_c = (u16::from_le_bytes([bytes[2], bytes[3]]) as f64 * 0.01) - 273.15;
            let hum = i16::from_le_bytes([bytes[4], bytes[5]]) as f64 * 0.004;
            let p_hpa = u16::from_le_bytes([bytes[6], bytes[7]]) as f64;
            fields.push(("Temperature".into(), format!("{:.1} °C", a_c)));
            fields.push(("Humidity".into(), format!("{:.1} %", hum)));
            fields.push(("Atmospheric Pressure".into(), format!("{:.1} hPa", p_hpa)));
            format!(
                "PGN 130311 Env: {:.1}°C, {:.0}% RH, {:.0} hPa",
                a_c, hum, p_hpa
            )
        }
        130312 if bytes.len() >= 5 => {
            let temp_c = (u16::from_le_bytes([bytes[3], bytes[4]]) as f64 * 0.01) - 273.15;
            fields.push(("Temperature".into(), format!("{:.1} °C", temp_c)));
            format!("PGN 130312 Temperature: {:.1} °C", temp_c)
        }
        130314 if bytes.len() >= 7 => {
            let press_hpa =
                i32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]) as f64 * 0.001;
            fields.push(("Actual Pressure".into(), format!("{:.1} hPa", press_hpa)));
            format!("PGN 130314 Actual Pressure: {:.1} hPa", press_hpa)
        }
        129283 if bytes.len() >= 6 => {
            let xte_m = i32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]) as f64 * 0.01;
            let xte_nm = xte_m / 1852.0;
            fields.push((
                "Cross-Track Error".into(),
                format!("{:.3} NM ({:.1} m)", xte_nm, xte_m),
            ));
            format!("PGN 129283 XTE: {:.3} NM ({:.1} m)", xte_nm, xte_m)
        }
        129284 if bytes.len() >= 28 => {
            let dist_m = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as f64 * 0.01;
            let dist_nm = dist_m / 1852.0;
            fields.push(("Distance to Waypoint".into(), format!("{:.2} NM", dist_nm)));
            format!("PGN 129284 Navigation Data: {:.2} NM to WPT", dist_nm)
        }
        129038 | 129039 if bytes.len() >= 13 => {
            let mmsi = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
            let lon = i32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as f64 * 1e-7;
            let lat = i32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]) as f64 * 1e-7;
            fields.push(("MMSI".into(), mmsi.to_string()));
            fields.push(("Latitude".into(), format!("{:.5}°", lat)));
            fields.push(("Longitude".into(), format!("{:.5}°", lon)));
            format!(
                "PGN {} AIS Position Report: MMSI {} @ {:.4}°, {:.4}°",
                pgn, mmsi, lat, lon
            )
        }
        _ => format!(
            "PGN {} (Src {}, {} bytes): {}",
            pgn,
            src,
            bytes.len(),
            hex_str.join(" ")
        ),
    };

    DecodedPacketInfo {
        protocol: protocol_label.to_string(),
        identifier: format!("PGN {}", pgn),
        summary,
        checksum_valid,
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_and_decode_can_id_pdu1_and_pdu2() {
        // PDU2 (broadcast, PF >= 240): PGN 129025 (0x1F801)
        let id_pdu2 = encode_can_id(2, 129025, 12, 0xFF);
        let (prio, pgn, src, dst) = decode_can_id(id_pdu2);
        assert_eq!(prio, 2);
        assert_eq!(pgn, 129025);
        assert_eq!(src, 12);
        assert_eq!(dst, 0xFF);

        // PDU1 (addressed, PF < 240): PGN 59392 (0x0E800, ISO Ack)
        let id_pdu1 = encode_can_id(6, 59392, 15, 42);
        let (prio1, pgn1, src1, dst1) = decode_can_id(id_pdu1);
        assert_eq!(prio1, 6);
        assert_eq!(pgn1, 59392);
        assert_eq!(src1, 15);
        assert_eq!(dst1, 42);
    }

    #[test]
    fn test_parse_hex_stream() {
        assert_eq!(parse_hex_stream("01 0A ff 42"), vec![0x01, 0x0A, 0xFF, 0x42]);
        assert_eq!(parse_hex_stream("010AFF42"), vec![0x01, 0x0A, 0xFF, 0x42]);
        assert!(parse_hex_stream("").is_empty());
    }

    #[test]
    fn test_build_and_decode_all_24_pgn_kinds_across_all_4_wire_formats() {
        let mut vessel = VesselState::default();
        vessel.fluctuation.enabled = false;
        let route = Route::default();
        let err = ErrorInjectionConfig::default();

        for kind in Pgn2000Kind::ALL {
            let payload = build_pgn_payload(kind, 5, &vessel, &route);
            assert!(
                !payload.is_empty(),
                "Expected non-empty payload for {:?}",
                kind
            );

            for wf in N2kWireFormat::ALL {
                let frames = generate_pgn_frames(kind, 5, 2, 18, wf, &vessel, &route, &err);
                assert!(
                    !frames.is_empty(),
                    "Expected frames for {:?} under {:?}",
                    kind,
                    wf
                );

                // Inspect the first frame
                let info = inspect_nmea2000(&frames[0]);
                assert_eq!(info.identifier, format!("PGN {}", kind.pgn()));
                if wf == N2kWireFormat::SeaSmartPcdin {
                    assert_eq!(info.checksum_valid, Some(true));
                }
            }
        }
    }

    #[test]
    fn test_encode_ais_n2k_position_all_classes() {
        let fleet = TargetVessel::default_fleet();
        // Class A -> PGN 129038
        let frames_a = encode_ais_n2k_position(
            &fleet[0],
            48,
            1,
            N2kWireFormat::YachtDevicesRawSingleLine,
        );
        assert_eq!(frames_a.len(), 1);
        let info_a = inspect_nmea2000(&frames_a[0]);
        assert_eq!(info_a.identifier, "PGN 129038");
        assert!(info_a.summary.contains(&fleet[0].mmsi.to_string()));

        // Class B -> PGN 129039
        let frames_b = encode_ais_n2k_position(&fleet[1], 48, 2, N2kWireFormat::SeaSmartPcdin);
        assert_eq!(frames_b.len(), 1);
        let info_b = inspect_nmea2000(&frames_b[0]);
        assert_eq!(info_b.identifier, "PGN 129039");
        assert_eq!(info_b.checksum_valid, Some(true));

        // ArpaOnly -> Empty
        let mut arpa = fleet[0].clone();
        arpa.ais_class = AisClass::ArpaOnly;
        assert!(
            encode_ais_n2k_position(&arpa, 48, 0, N2kWireFormat::YachtDevicesRawSingleLine)
                .is_empty()
        );
    }

    #[test]
    fn test_inspect_nmea2000_unrecognized_and_bad_checksum() {
        let bad_pcdin = inspect_nmea2000("$PCDIN,01F112,0008AF20,18,0165A80000AEFFFC*00");
        assert_eq!(bad_pcdin.checksum_valid, Some(false));

        let unknown = inspect_nmea2000("NOT_A_VALID_FRAME");
        assert_eq!(unknown.identifier, "RAW");
    }
}
