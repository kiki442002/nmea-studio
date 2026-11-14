#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod model;
pub mod protocols;
pub mod recorder;
pub mod transport;
pub mod ui;

use eframe::egui;
use ui::NmeaStudioApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NMEA Studio - NMEA 0183 & NMEA 2000 Simulator & Analyzer")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "NMEA Studio",
        native_options,
        Box::new(|cc| Ok(Box::new(NmeaStudioApp::new(cc)))),
    )
}

#[cfg(test)]
mod tests {
    use crate::model::ais::{TargetVessel, AisClass};
    use crate::model::instruments::{ErrorInjectionConfig, N2kWireFormat, Pgn2000Kind, Sentence0183Kind};
    use crate::model::route::Route;
    use crate::model::vessel::VesselState;
    use crate::protocols::inspect_packet;
    use crate::protocols::nmea0183::{
        compute_checksum, encode_ais_position_report, encode_ais_static_report, encode_arpa_ttm,
        generate_sentence,
    };
    use crate::protocols::nmea2000::{
        decode_can_id, encode_can_id, generate_pgn_frames,
    };

    #[test]
    fn test_nmea0183_checksum_and_sentences() {
        let mut vessel = VesselState::default();
        vessel.fluctuation.enabled = false;
        let route = Route::default();
        let err = ErrorInjectionConfig::default();

        for kind in [
            Sentence0183Kind::GGA,
            Sentence0183Kind::RMC,
            Sentence0183Kind::GLL,
            Sentence0183Kind::VTG,
            Sentence0183Kind::HDT,
            Sentence0183Kind::HDG,
            Sentence0183Kind::MWVApparent,
            Sentence0183Kind::MWD,
            Sentence0183Kind::DPT,
            Sentence0183Kind::DBT,
            Sentence0183Kind::MTW,
            Sentence0183Kind::VHW,
            Sentence0183Kind::APB,
            Sentence0183Kind::RMB,
            Sentence0183Kind::XTE,
            Sentence0183Kind::RSA,
            Sentence0183Kind::RPM,
        ] {
            let lines = generate_sentence("GP", kind, &vessel, &route, &err);
            assert!(!lines.is_empty(), "No sentence generated for {:?}", kind);
            for line in lines {
                assert!(line.starts_with('$'));
                let dec = inspect_packet(&line);
                assert_eq!(dec.checksum_valid, Some(true), "Invalid checksum for {}", line);
            }
        }

        // Verify known XOR checksum
        let body = "GPGLL,5300.97914,N,00259.98174,E,125926,A";
        assert_eq!(compute_checksum(body), 0x28);
    }

    #[test]
    fn test_nmea2000_can_id_and_wire_formats() {
        let can_id = encode_can_id(2, 129025, 12, 0xFF);
        let (prio, pgn, src, _dst) = decode_can_id(can_id);
        assert_eq!(prio, 2);
        assert_eq!(pgn, 129025);
        assert_eq!(src, 12);

        let mut vessel = VesselState::default();
        vessel.fluctuation.enabled = false;
        let route = Route::default();
        let err = ErrorInjectionConfig::default();

        // Test Yacht Devices RAW Single-Line
        let yd_frames = generate_pgn_frames(
            Pgn2000Kind::Pgn129025PositionRapid,
            1,
            0,
            12,
            N2kWireFormat::YachtDevicesRawSingleLine,
            &vessel,
            &route,
            &err,
        );
        assert_eq!(yd_frames.len(), 1);
        let info = inspect_packet(&yd_frames[0]);
        assert!(info.identifier.contains("129025"));
        assert!(info.summary.contains("48.358"));

        // Test Fast-Packet fragmentation on PGN 129029 (43 bytes -> 7 CAN frames)
        let fp_frames = generate_pgn_frames(
            Pgn2000Kind::Pgn129029GnssPositionData,
            2,
            3,
            12,
            N2kWireFormat::YachtDevicesRawFastPacket,
            &vessel,
            &route,
            &err,
        );
        assert!(fp_frames.len() >= 6);

        // Test SeaSmart $PCDIN format
        let pcdin_frames = generate_pgn_frames(
            Pgn2000Kind::Pgn127250VesselHeading,
            1,
            0,
            24,
            N2kWireFormat::SeaSmartPcdin,
            &vessel,
            &route,
            &err,
        );
        assert_eq!(pcdin_frames.len(), 1);
        assert!(pcdin_frames[0].starts_with("$PCDIN,01F112,"));
        let pcdin_info = inspect_packet(&pcdin_frames[0]);
        assert_eq!(pcdin_info.checksum_valid, Some(true));
        assert!(pcdin_info.summary.contains("243.5"));
    }

    #[test]
    fn test_ais_and_arpa_encoding() {
        let vessel = VesselState::default();
        let fleet = TargetVessel::default_fleet();
        let tgt = &fleet[0];

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
        assert!(vdm.starts_with("!AIVDM,1,1,,A,"));
        assert_eq!(inspect_packet(&vdm).checksum_valid, Some(true));

        let statics = encode_ais_static_report(tgt);
        assert_eq!(statics.len(), 2); // Class A Msg 5 is 2 fragments
        assert!(statics[0].starts_with("!AIVDM,2,1,3,A,"));
        assert!(statics[1].starts_with("!AIVDM,2,2,3,A,"));

        let ttm = encode_arpa_ttm(tgt, &vessel);
        assert!(ttm.starts_with("$RATTM,"));
        assert_eq!(inspect_packet(&ttm).checksum_valid, Some(true));
    }
}
