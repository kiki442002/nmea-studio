pub mod nmea0183;
pub mod nmea2000;

#[derive(Debug, Clone)]
pub struct DecodedPacketInfo {
    pub protocol: String,
    pub identifier: String,
    pub summary: String,
    pub checksum_valid: Option<bool>,
    pub fields: Vec<(String, String)>,
}

pub fn is_nmea2000_line(raw_line: &str) -> bool {
    let trimmed = raw_line.trim();
    trimmed.starts_with("$PCDIN,") || (!trimmed.starts_with('$') && !trimmed.starts_with('!'))
}

pub fn inspect_packet(raw_line: &str) -> DecodedPacketInfo {
    let trimmed = raw_line.trim();
    if is_nmea2000_line(trimmed) {
        nmea2000::inspect_nmea2000(trimmed)
    } else {
        nmea0183::inspect_nmea0183(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_nmea2000_line() {
        assert!(!is_nmea2000_line("$GPRMC,123519,A*00"));
        assert!(!is_nmea2000_line("!AIVDM,1,1,,A,13P,0*00"));
        assert!(is_nmea2000_line("$PCDIN,01F112,0008AF20,18,01*00"));
        assert!(is_nmea2000_line("12:35:19.120 T 09F8010C 80 79 D9 1C C0 A1 B2 FD"));
        assert!(is_nmea2000_line("A12345.678 12FF2 1F801 8079D91CC0A1B2FD"));
    }

    #[test]
    fn test_inspect_packet_routing() {
        let p0183 = inspect_packet("$HCHDT,243.5,T*29");
        assert_eq!(p0183.protocol, "NMEA 0183");
        assert_eq!(p0183.checksum_valid, Some(true));

        let pn2k_pcdin = inspect_packet("$PCDIN,01F112,0008AF20,18,0165A80000AEFFFC*59");
        assert!(pn2k_pcdin.protocol.contains("PCDIN"));
        assert_eq!(pn2k_pcdin.identifier, "PGN 127250");

        let pn2k_yd = inspect_packet("12:35:19.120 T 09F8010C 80 79 D9 1C C0 A1 B2 FD");
        assert!(pn2k_yd.protocol.contains("Yacht Devices"));
        assert_eq!(pn2k_yd.identifier, "PGN 129025");
    }
}
