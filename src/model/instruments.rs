use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolMode {
    Nmea0183Only,
    Nmea2000Only,
    Dual0183And2000,
}

impl ProtocolMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Nmea0183Only => "NMEA 0183 Only",
            Self::Nmea2000Only => "NMEA 2000 Only",
            Self::Dual0183And2000 => "Dual (NMEA 0183 + NMEA 2000)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum N2kWireFormat {
    YachtDevicesRawSingleLine,
    YachtDevicesRawFastPacket,
    SeaSmartPcdin,
    ActisenseAscii,
}

impl N2kWireFormat {
    pub fn label(&self) -> &'static str {
        match self {
            Self::YachtDevicesRawSingleLine => "Yacht Devices RAW (hh:mm:ss.ddd D CAN_ID b0..bN)",
            Self::YachtDevicesRawFastPacket => "Yacht Devices RAW 8-byte CAN Fast-Packet Frames",
            Self::SeaSmartPcdin => "SeaSmart ($PCDIN,PGN,TS,SRC,DATA*CS)",
            Self::ActisenseAscii => "Actisense N2K ASCII (A<ts> <src><dst><prio> <pgn> <data>)",
        }
    }

    pub const ALL: [N2kWireFormat; 4] = [
        Self::YachtDevicesRawSingleLine,
        Self::YachtDevicesRawFastPacket,
        Self::SeaSmartPcdin,
        Self::ActisenseAscii,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sentence0183Kind {
    // GNSS / Position
    GGA,
    RMC,
    GLL,
    VTG,
    GSA,
    GSV,
    ZDA,
    // Heading & Attitude
    HDG,
    HDT,
    HDM,
    ROT,
    // Wind & Weather
    MWVApparent,
    MWVTrue,
    MWD,
    VWR,
    VWT,
    MDA,
    XDRWeather,
    // Depth, Speed & Water
    DPT,
    DBT,
    DBS,
    DBK,
    MTW,
    VHW,
    VLW,
    // Navigation & Autopilot
    APB,
    RMB,
    BOD,
    BWC,
    XTE,
    WPL,
    RTE,
    // Propulsion & Steering
    RSA,
    RPM,
    XDREngine,
}

impl Sentence0183Kind {
    pub const ALL: [Sentence0183Kind; 35] = [
        Self::GGA,
        Self::RMC,
        Self::GLL,
        Self::VTG,
        Self::GSA,
        Self::GSV,
        Self::ZDA,
        Self::HDG,
        Self::HDT,
        Self::HDM,
        Self::ROT,
        Self::MWVApparent,
        Self::MWVTrue,
        Self::MWD,
        Self::VWR,
        Self::VWT,
        Self::MDA,
        Self::XDRWeather,
        Self::DPT,
        Self::DBT,
        Self::DBS,
        Self::DBK,
        Self::MTW,
        Self::VHW,
        Self::VLW,
        Self::APB,
        Self::RMB,
        Self::BOD,
        Self::BWC,
        Self::XTE,
        Self::WPL,
        Self::RTE,
        Self::RSA,
        Self::RPM,
        Self::XDREngine,
    ];

    pub fn code(&self) -> &'static str {
        match self {
            Self::GGA => "GGA",
            Self::RMC => "RMC",
            Self::GLL => "GLL",
            Self::VTG => "VTG",
            Self::GSA => "GSA",
            Self::GSV => "GSV",
            Self::ZDA => "ZDA",
            Self::HDG => "HDG",
            Self::HDT => "HDT",
            Self::HDM => "HDM",
            Self::ROT => "ROT",
            Self::MWVApparent => "MWV (R)",
            Self::MWVTrue => "MWV (T)",
            Self::MWD => "MWD",
            Self::VWR => "VWR",
            Self::VWT => "VWT",
            Self::MDA => "MDA",
            Self::XDRWeather => "XDR (Env)",
            Self::DPT => "DPT",
            Self::DBT => "DBT",
            Self::DBS => "DBS",
            Self::DBK => "DBK",
            Self::MTW => "MTW",
            Self::VHW => "VHW",
            Self::VLW => "VLW",
            Self::APB => "APB",
            Self::RMB => "RMB",
            Self::BOD => "BOD",
            Self::BWC => "BWC",
            Self::XTE => "XTE",
            Self::WPL => "WPL",
            Self::RTE => "RTE",
            Self::RSA => "RSA",
            Self::RPM => "RPM",
            Self::XDREngine => "XDR (Eng)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::GGA => "Global Positioning System Fix Data",
            Self::RMC => "Recommended Minimum Specific GNSS Data",
            Self::GLL => "Geographic Position - Latitude/Longitude",
            Self::VTG => "Course Over Ground and Ground Speed",
            Self::GSA => "GNSS DOP and Active Satellites",
            Self::GSV => "GNSS Satellites in View",
            Self::ZDA => "Time & Date (UTC, Day, Month, Year)",
            Self::HDG => "Heading - Deviation & Variation",
            Self::HDT => "Heading - True",
            Self::HDM => "Heading - Magnetic",
            Self::ROT => "Rate Of Turn (deg/min)",
            Self::MWVApparent => "Wind Speed and Angle (Relative / Apparent)",
            Self::MWVTrue => "Wind Speed and Angle (Theoretical / True)",
            Self::MWD => "Wind Direction & Speed (True North)",
            Self::VWR => "Relative Wind Speed and Angle",
            Self::VWT => "True Wind Speed and Angle",
            Self::MDA => "Meteorological Composite (Pressure, Temp, Wind)",
            Self::XDRWeather => "Transducer Measurements (Baro, Air Temp, Humidity, Pitch/Roll)",
            Self::DPT => "Depth of Water (Transducer + Offset)",
            Self::DBT => "Depth Below Transducer (ft, m, fathoms)",
            Self::DBS => "Depth Below Surface",
            Self::DBK => "Depth Below Keel",
            Self::MTW => "Mean Temperature of Water",
            Self::VHW => "Water Speed and Heading",
            Self::VLW => "Distance Traveled through Water (Total & Trip)",
            Self::APB => "Autopilot Sentence 'B'",
            Self::RMB => "Recommended Minimum Navigation Information",
            Self::BOD => "Bearing - Origin to Destination",
            Self::BWC => "Bearing & Distance to Waypoint (Great Circle)",
            Self::XTE => "Cross-Track Error, Measured",
            Self::WPL => "Waypoint Location",
            Self::RTE => "Routes (Active Waypoint List)",
            Self::RSA => "Rudder Sensor Angle",
            Self::RPM => "Revolutions (Engine / Shaft RPM)",
            Self::XDREngine => "Transducer Measurements (Voltage, Fuel, Oil, Coolant)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pgn2000Kind {
    Pgn126992SystemTime,
    Pgn127245Rudder,
    Pgn127250VesselHeading,
    Pgn127251RateOfTurn,
    Pgn127257Attitude,
    Pgn127258MagneticVariation,
    Pgn127488EngineRapid,
    Pgn127489EngineDynamic,
    Pgn127505FluidLevel,
    Pgn127508BatteryStatus,
    Pgn128259SpeedWater,
    Pgn128267WaterDepth,
    Pgn128275DistanceLog,
    Pgn129025PositionRapid,
    Pgn129026CogSogRapid,
    Pgn129029GnssPositionData,
    Pgn129283CrossTrackError,
    Pgn129284NavigationData,
    Pgn130306WindDataApparent,
    Pgn130306WindDataTrue,
    Pgn130310EnvironmentalOutside,
    Pgn130311EnvironmentalParams,
    Pgn130312Temperature,
    Pgn130314ActualPressure,
}

impl Pgn2000Kind {
    pub const ALL: [Pgn2000Kind; 24] = [
        Self::Pgn126992SystemTime,
        Self::Pgn127245Rudder,
        Self::Pgn127250VesselHeading,
        Self::Pgn127251RateOfTurn,
        Self::Pgn127257Attitude,
        Self::Pgn127258MagneticVariation,
        Self::Pgn127488EngineRapid,
        Self::Pgn127489EngineDynamic,
        Self::Pgn127505FluidLevel,
        Self::Pgn127508BatteryStatus,
        Self::Pgn128259SpeedWater,
        Self::Pgn128267WaterDepth,
        Self::Pgn128275DistanceLog,
        Self::Pgn129025PositionRapid,
        Self::Pgn129026CogSogRapid,
        Self::Pgn129029GnssPositionData,
        Self::Pgn129283CrossTrackError,
        Self::Pgn129284NavigationData,
        Self::Pgn130306WindDataApparent,
        Self::Pgn130306WindDataTrue,
        Self::Pgn130310EnvironmentalOutside,
        Self::Pgn130311EnvironmentalParams,
        Self::Pgn130312Temperature,
        Self::Pgn130314ActualPressure,
    ];

    pub fn pgn(&self) -> u32 {
        match self {
            Self::Pgn126992SystemTime => 126992,
            Self::Pgn127245Rudder => 127245,
            Self::Pgn127250VesselHeading => 127250,
            Self::Pgn127251RateOfTurn => 127251,
            Self::Pgn127257Attitude => 127257,
            Self::Pgn127258MagneticVariation => 127258,
            Self::Pgn127488EngineRapid => 127488,
            Self::Pgn127489EngineDynamic => 127489,
            Self::Pgn127505FluidLevel => 127505,
            Self::Pgn127508BatteryStatus => 127508,
            Self::Pgn128259SpeedWater => 128259,
            Self::Pgn128267WaterDepth => 128267,
            Self::Pgn128275DistanceLog => 128275,
            Self::Pgn129025PositionRapid => 129025,
            Self::Pgn129026CogSogRapid => 129026,
            Self::Pgn129029GnssPositionData => 129029,
            Self::Pgn129283CrossTrackError => 129283,
            Self::Pgn129284NavigationData => 129284,
            Self::Pgn130306WindDataApparent => 130306,
            Self::Pgn130306WindDataTrue => 130306,
            Self::Pgn130310EnvironmentalOutside => 130310,
            Self::Pgn130311EnvironmentalParams => 130311,
            Self::Pgn130312Temperature => 130312,
            Self::Pgn130314ActualPressure => 130314,
        }
    }

    pub fn default_priority(&self) -> u8 {
        match self {
            Self::Pgn127245Rudder
            | Self::Pgn127250VesselHeading
            | Self::Pgn127251RateOfTurn
            | Self::Pgn127488EngineRapid
            | Self::Pgn128259SpeedWater
            | Self::Pgn129025PositionRapid
            | Self::Pgn129026CogSogRapid
            | Self::Pgn130306WindDataApparent
            | Self::Pgn130306WindDataTrue => 2,
            Self::Pgn126992SystemTime
            | Self::Pgn127257Attitude
            | Self::Pgn128267WaterDepth
            | Self::Pgn129029GnssPositionData
            | Self::Pgn129283CrossTrackError
            | Self::Pgn129284NavigationData => 3,
            _ => 6,
        }
    }

    pub fn is_fast_packet(&self) -> bool {
        matches!(
            self,
            Self::Pgn127489EngineDynamic
                | Self::Pgn128275DistanceLog
                | Self::Pgn129029GnssPositionData
                | Self::Pgn129284NavigationData
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pgn126992SystemTime => "126992 - System Time",
            Self::Pgn127245Rudder => "127245 - Rudder",
            Self::Pgn127250VesselHeading => "127250 - Vessel Heading",
            Self::Pgn127251RateOfTurn => "127251 - Rate of Turn",
            Self::Pgn127257Attitude => "127257 - Attitude (Yaw/Pitch/Roll)",
            Self::Pgn127258MagneticVariation => "127258 - Magnetic Variation",
            Self::Pgn127488EngineRapid => "127488 - Engine Parameters, Rapid Update",
            Self::Pgn127489EngineDynamic => "127489 - Engine Parameters, Dynamic (Fast-Packet)",
            Self::Pgn127505FluidLevel => "127505 - Fluid Level (Fuel / Water Tanks)",
            Self::Pgn127508BatteryStatus => "127508 - Battery Status",
            Self::Pgn128259SpeedWater => "128259 - Speed, Water Referenced",
            Self::Pgn128267WaterDepth => "128267 - Water Depth",
            Self::Pgn128275DistanceLog => "128275 - Distance Log (Fast-Packet)",
            Self::Pgn129025PositionRapid => "129025 - Position, Rapid Update",
            Self::Pgn129026CogSogRapid => "129026 - COG & SOG, Rapid Update",
            Self::Pgn129029GnssPositionData => "129029 - GNSS Position Data (Fast-Packet)",
            Self::Pgn129283CrossTrackError => "129283 - Cross-Track Error",
            Self::Pgn129284NavigationData => "129284 - Navigation Data (Fast-Packet)",
            Self::Pgn130306WindDataApparent => "130306 - Wind Data (Apparent)",
            Self::Pgn130306WindDataTrue => "130306 - Wind Data (True Ground/Boat)",
            Self::Pgn130310EnvironmentalOutside => {
                "130310 - Environmental Parameters (Water/Air/Baro)"
            }
            Self::Pgn130311EnvironmentalParams => {
                "130311 - Environmental Parameters (Temp/Humidity/Press)"
            }
            Self::Pgn130312Temperature => "130312 - Temperature (Sea / Air)",
            Self::Pgn130314ActualPressure => "130314 - Actual Pressure (Atmospheric)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentenceSchedule {
    pub kind: Sentence0183Kind,
    pub enabled: bool,
    pub interval_sec: f64,
    #[serde(skip)]
    pub elapsed_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgnSchedule {
    pub kind: Pgn2000Kind,
    pub enabled: bool,
    pub interval_sec: f64,
    #[serde(skip)]
    pub elapsed_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInjectionConfig {
    pub corrupt_checksum: bool,
    pub omit_checksum: bool,
    pub omit_crlf: bool,
}

impl Default for ErrorInjectionConfig {
    fn default() -> Self {
        Self {
            corrupt_checksum: false,
            omit_checksum: false,
            omit_crlf: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentTemplate {
    GpsGnss,
    GyroCompass,
    WindWeather,
    DepthSpeedLog,
    AutopilotNav,
    EngineBatteryRudder,
    CustomBlank,
}

impl InstrumentTemplate {
    pub const ALL: &'static [InstrumentTemplate] = &[
        Self::GpsGnss,
        Self::GyroCompass,
        Self::WindWeather,
        Self::DepthSpeedLog,
        Self::AutopilotNav,
        Self::EngineBatteryRudder,
        Self::CustomBlank,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::GpsGnss => "🌐 GPS / GNSS Receiver",
            Self::GyroCompass => "🎯 Gyro / Attitude Compass",
            Self::WindWeather => "💨 Wind & Weather Station",
            Self::DepthSpeedLog => "🌊 Echo Sounder (Depth) & Speed Log",
            Self::AutopilotNav => "☸ Autopilot & Route Plotter",
            Self::EngineBatteryRudder => "⚙ Engine, Rudder & Battery Gateway",
            Self::CustomBlank => "🛠 Custom Blank Instrument (All 0183 / N2K)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub id: String,
    pub name: String,
    pub category: String,
    pub enabled: bool,
    pub protocol_mode: ProtocolMode,
    pub talker_id: String,
    pub sentences_0183: Vec<SentenceSchedule>,
    pub n2k_source_addr: u8,
    pub n2k_wire_format: N2kWireFormat,
    pub pgns_2000: Vec<PgnSchedule>,
    pub error_injection: ErrorInjectionConfig,
    pub target_ports: Vec<String>,
    #[serde(skip)]
    pub fast_packet_seq: u8,
    #[serde(skip)]
    pub sid_counter: u8,
}

impl Instrument {
    /// Returns the dedicated pictogram for this instrument based on its category/ID/name.
    pub fn icon(&self) -> &'static str {
        let lower = format!("{} {} {}", self.id, self.category, self.name).to_lowercase();
        if lower.contains("gnss") || lower.contains("gps") || lower.contains("position") {
            "🌐"
        } else if lower.contains("gyro") || lower.contains("heading") || lower.contains("compass") {
            "🎯"
        } else if lower.contains("wind") || lower.contains("meteo") || lower.contains("weather") {
            "💨"
        } else if lower.contains("depth") || lower.contains("sounder") || lower.contains("log") {
            "🌊"
        } else if lower.contains("autopilot") || lower.contains("route") || lower.contains("nav") {
            "☸"
        } else if lower.contains("engine") || lower.contains("battery") || lower.contains("rudder") {
            "⚙"
        } else {
            "🛠"
        }
    }
    /// Returns current Sequence ID (0..=252) and wraps around to 0 after 252
    pub fn next_sid(&mut self) -> u8 {
        let s = self.sid_counter;
        self.sid_counter = if self.sid_counter >= 252 {
            0
        } else {
            self.sid_counter + 1
        };
        s
    }

    pub fn next_fast_seq(&mut self) -> u8 {
        let s = self.fast_packet_seq & 0x07;
        self.fast_packet_seq = (self.fast_packet_seq + 1) & 0x07;
        s
    }

    /// Creates a new unconfigured custom instrument containing the full catalog of
    /// 35 NMEA 0183 sentences and 24 NMEA 2000 PGNs (all unchecked by default)
    /// so the user can selectively enable whichever messages they need.
    pub fn new_custom(id_num: usize) -> Self {
        Self {
            id: format!("inst-custom-{}", id_num),
            name: format!("Custom Instrument #{}", id_num),
            category: "Custom".to_string(),
            enabled: true,
            protocol_mode: ProtocolMode::Nmea0183Only,
            talker_id: "II".to_string(),
            sentences_0183: Sentence0183Kind::ALL
                .iter()
                .map(|&kind| SentenceSchedule {
                    kind,
                    enabled: false,
                    interval_sec: 1.0,
                    elapsed_sec: 0.0,
                })
                .collect(),
            n2k_source_addr: (15 + (id_num as u8).saturating_mul(3)).min(250),
            n2k_wire_format: N2kWireFormat::YachtDevicesRawSingleLine,
            pgns_2000: Pgn2000Kind::ALL
                .iter()
                .map(|&kind| PgnSchedule {
                    kind,
                    enabled: false,
                    interval_sec: 1.0,
                    elapsed_sec: 0.0,
                })
                .collect(),
            error_injection: ErrorInjectionConfig::default(),
            target_ports: Vec::new(),
            fast_packet_seq: 0,
            sid_counter: 0,
        }
    }

    /// Creates an instrument from a NemaStudio-style device template (`GPS`, `Gyro`, `Wind`, `Depth`, `Autopilot`, `Engine`, or `CustomBlank`).
    pub fn from_template(template: InstrumentTemplate, id_num: usize) -> Self {
        let defaults = Self::default_instruments();
        match template {
            InstrumentTemplate::GpsGnss => {
                let mut inst = defaults[0].clone();
                inst.id = format!("inst-gnss-{}", id_num);
                if id_num > 1 {
                    inst.name = format!("GNSS / GPS Receiver #{}", id_num);
                }
                inst
            }
            InstrumentTemplate::GyroCompass => {
                let mut inst = defaults[1].clone();
                inst.id = format!("inst-gyro-{}", id_num);
                if id_num > 1 {
                    inst.name = format!("Gyro / Attitude Compass #{}", id_num);
                }
                inst
            }
            InstrumentTemplate::WindWeather => {
                let mut inst = defaults[2].clone();
                inst.id = format!("inst-wind-{}", id_num);
                if id_num > 1 {
                    inst.name = format!("Masthead Wind & Weather #{}", id_num);
                }
                inst
            }
            InstrumentTemplate::DepthSpeedLog => {
                let mut inst = defaults[3].clone();
                inst.id = format!("inst-depth-{}", id_num);
                if id_num > 1 {
                    inst.name = format!("Echo Sounder & Speed Log #{}", id_num);
                }
                inst
            }
            InstrumentTemplate::AutopilotNav => {
                let mut inst = defaults[4].clone();
                inst.id = format!("inst-nav-{}", id_num);
                if id_num > 1 {
                    inst.name = format!("Autopilot & Route Plotter #{}", id_num);
                }
                inst
            }
            InstrumentTemplate::EngineBatteryRudder => {
                let mut inst = defaults[5].clone();
                inst.id = format!("inst-engine-{}", id_num);
                if id_num > 1 {
                    inst.name = format!("Engine, Rudder & Battery #{}", id_num);
                }
                inst
            }
            InstrumentTemplate::CustomBlank => Self::new_custom(id_num),
        }
    }

    pub fn default_instruments() -> Vec<Self> {
        vec![
            Self {
                id: "inst-gnss".to_string(),
                name: "GNSS / GPS Receiver".to_string(),
                category: "Position & Time".to_string(),
                enabled: true,
                protocol_mode: ProtocolMode::Nmea0183Only,
                talker_id: "GP".to_string(),
                sentences_0183: vec![
                    SentenceSchedule { kind: Sentence0183Kind::RMC, enabled: true, interval_sec: 1.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::GGA, enabled: true, interval_sec: 1.0, elapsed_sec: 0.2 },
                    SentenceSchedule { kind: Sentence0183Kind::VTG, enabled: true, interval_sec: 1.0, elapsed_sec: 0.4 },
                    SentenceSchedule { kind: Sentence0183Kind::GLL, enabled: false, interval_sec: 1.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::GSA, enabled: true, interval_sec: 2.0, elapsed_sec: 0.6 },
                    SentenceSchedule { kind: Sentence0183Kind::GSV, enabled: false, interval_sec: 5.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::ZDA, enabled: true, interval_sec: 2.0, elapsed_sec: 0.8 },
                ],
                n2k_source_addr: 12,
                n2k_wire_format: N2kWireFormat::YachtDevicesRawSingleLine,
                pgns_2000: vec![
                    PgnSchedule { kind: Pgn2000Kind::Pgn129025PositionRapid, enabled: true, interval_sec: 0.5, elapsed_sec: 0.0 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn129026CogSogRapid, enabled: true, interval_sec: 0.5, elapsed_sec: 0.25 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn129029GnssPositionData, enabled: true, interval_sec: 1.0, elapsed_sec: 0.1 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn126992SystemTime, enabled: true, interval_sec: 1.0, elapsed_sec: 0.5 },
                ],
                error_injection: ErrorInjectionConfig::default(),
                target_ports: Vec::new(),
                fast_packet_seq: 0,
                sid_counter: 0,
            },
            Self {
                id: "inst-gyro".to_string(),
                name: "Gyro / Attitude Compass".to_string(),
                category: "Heading & Attitude".to_string(),
                enabled: true,
                protocol_mode: ProtocolMode::Nmea0183Only,
                talker_id: "HC".to_string(),
                sentences_0183: vec![
                    SentenceSchedule { kind: Sentence0183Kind::HDT, enabled: true, interval_sec: 0.5, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::HDG, enabled: true, interval_sec: 1.0, elapsed_sec: 0.2 },
                    SentenceSchedule { kind: Sentence0183Kind::HDM, enabled: false, interval_sec: 1.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::ROT, enabled: true, interval_sec: 1.0, elapsed_sec: 0.4 },
                ],
                n2k_source_addr: 24,
                n2k_wire_format: N2kWireFormat::YachtDevicesRawSingleLine,
                pgns_2000: vec![
                    PgnSchedule { kind: Pgn2000Kind::Pgn127250VesselHeading, enabled: true, interval_sec: 0.5, elapsed_sec: 0.1 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn127251RateOfTurn, enabled: true, interval_sec: 0.5, elapsed_sec: 0.3 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn127257Attitude, enabled: true, interval_sec: 1.0, elapsed_sec: 0.5 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn127258MagneticVariation, enabled: false, interval_sec: 5.0, elapsed_sec: 0.0 },
                ],
                error_injection: ErrorInjectionConfig::default(),
                target_ports: Vec::new(),
                fast_packet_seq: 0,
                sid_counter: 0,
            },
            Self {
                id: "inst-wind".to_string(),
                name: "Masthead Wind & Weather".to_string(),
                category: "Wind & Meteo".to_string(),
                enabled: true,
                protocol_mode: ProtocolMode::Nmea0183Only,
                talker_id: "WI".to_string(),
                sentences_0183: vec![
                    SentenceSchedule { kind: Sentence0183Kind::MWVApparent, enabled: true, interval_sec: 0.5, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::MWVTrue, enabled: true, interval_sec: 1.0, elapsed_sec: 0.3 },
                    SentenceSchedule { kind: Sentence0183Kind::MWD, enabled: true, interval_sec: 2.0, elapsed_sec: 0.6 },
                    SentenceSchedule { kind: Sentence0183Kind::VWR, enabled: false, interval_sec: 1.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::VWT, enabled: false, interval_sec: 1.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::MDA, enabled: true, interval_sec: 3.0, elapsed_sec: 0.9 },
                    SentenceSchedule { kind: Sentence0183Kind::XDRWeather, enabled: false, interval_sec: 2.0, elapsed_sec: 0.0 },
                ],
                n2k_source_addr: 35,
                n2k_wire_format: N2kWireFormat::SeaSmartPcdin,
                pgns_2000: vec![
                    PgnSchedule { kind: Pgn2000Kind::Pgn130306WindDataApparent, enabled: true, interval_sec: 0.5, elapsed_sec: 0.15 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn130306WindDataTrue, enabled: false, interval_sec: 1.0, elapsed_sec: 0.0 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn130310EnvironmentalOutside, enabled: true, interval_sec: 2.0, elapsed_sec: 0.4 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn130311EnvironmentalParams, enabled: true, interval_sec: 2.0, elapsed_sec: 0.8 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn130312Temperature, enabled: false, interval_sec: 2.0, elapsed_sec: 0.0 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn130314ActualPressure, enabled: false, interval_sec: 2.0, elapsed_sec: 0.0 },
                ],
                error_injection: ErrorInjectionConfig::default(),
                target_ports: Vec::new(),
                fast_packet_seq: 0,
                sid_counter: 0,
            },
            Self {
                id: "inst-depth-log".to_string(),
                name: "Echo Sounder & Speed Log".to_string(),
                category: "Depth & Water Speed".to_string(),
                enabled: true,
                protocol_mode: ProtocolMode::Nmea0183Only,
                talker_id: "SD".to_string(),
                sentences_0183: vec![
                    SentenceSchedule { kind: Sentence0183Kind::DPT, enabled: true, interval_sec: 1.0, elapsed_sec: 0.1 },
                    SentenceSchedule { kind: Sentence0183Kind::DBT, enabled: true, interval_sec: 1.0, elapsed_sec: 0.3 },
                    SentenceSchedule { kind: Sentence0183Kind::DBS, enabled: false, interval_sec: 1.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::DBK, enabled: false, interval_sec: 1.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::MTW, enabled: true, interval_sec: 2.0, elapsed_sec: 0.5 },
                    SentenceSchedule { kind: Sentence0183Kind::VHW, enabled: true, interval_sec: 1.0, elapsed_sec: 0.7 },
                    SentenceSchedule { kind: Sentence0183Kind::VLW, enabled: true, interval_sec: 3.0, elapsed_sec: 0.9 },
                ],
                n2k_source_addr: 42,
                n2k_wire_format: N2kWireFormat::YachtDevicesRawSingleLine,
                pgns_2000: vec![
                    PgnSchedule { kind: Pgn2000Kind::Pgn128267WaterDepth, enabled: true, interval_sec: 1.0, elapsed_sec: 0.2 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn128259SpeedWater, enabled: true, interval_sec: 1.0, elapsed_sec: 0.4 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn128275DistanceLog, enabled: true, interval_sec: 2.0, elapsed_sec: 0.6 },
                ],
                error_injection: ErrorInjectionConfig::default(),
                target_ports: Vec::new(),
                fast_packet_seq: 0,
                sid_counter: 0,
            },
            Self {
                id: "inst-nav-ap".to_string(),
                name: "Autopilot & Route Plotter".to_string(),
                category: "Navigation & Route".to_string(),
                enabled: true,
                protocol_mode: ProtocolMode::Nmea0183Only,
                talker_id: "EC".to_string(),
                sentences_0183: vec![
                    SentenceSchedule { kind: Sentence0183Kind::RMB, enabled: true, interval_sec: 1.0, elapsed_sec: 0.1 },
                    SentenceSchedule { kind: Sentence0183Kind::APB, enabled: true, interval_sec: 1.0, elapsed_sec: 0.3 },
                    SentenceSchedule { kind: Sentence0183Kind::XTE, enabled: true, interval_sec: 1.0, elapsed_sec: 0.5 },
                    SentenceSchedule { kind: Sentence0183Kind::BWC, enabled: true, interval_sec: 2.0, elapsed_sec: 0.7 },
                    SentenceSchedule { kind: Sentence0183Kind::BOD, enabled: true, interval_sec: 2.0, elapsed_sec: 0.9 },
                    SentenceSchedule { kind: Sentence0183Kind::WPL, enabled: false, interval_sec: 5.0, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::RTE, enabled: false, interval_sec: 5.0, elapsed_sec: 0.0 },
                ],
                n2k_source_addr: 50,
                n2k_wire_format: N2kWireFormat::SeaSmartPcdin,
                pgns_2000: vec![
                    PgnSchedule { kind: Pgn2000Kind::Pgn129283CrossTrackError, enabled: true, interval_sec: 1.0, elapsed_sec: 0.2 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn129284NavigationData, enabled: true, interval_sec: 1.0, elapsed_sec: 0.6 },
                ],
                error_injection: ErrorInjectionConfig::default(),
                target_ports: Vec::new(),
                fast_packet_seq: 0,
                sid_counter: 0,
            },
            Self {
                id: "inst-engine".to_string(),
                name: "Engine, Rudder & Battery Gateway".to_string(),
                category: "Propulsion & Electrical".to_string(),
                enabled: true,
                protocol_mode: ProtocolMode::Nmea0183Only,
                talker_id: "II".to_string(),
                sentences_0183: vec![
                    SentenceSchedule { kind: Sentence0183Kind::RSA, enabled: true, interval_sec: 0.5, elapsed_sec: 0.0 },
                    SentenceSchedule { kind: Sentence0183Kind::RPM, enabled: true, interval_sec: 1.0, elapsed_sec: 0.25 },
                    SentenceSchedule { kind: Sentence0183Kind::XDREngine, enabled: true, interval_sec: 2.0, elapsed_sec: 0.5 },
                ],
                n2k_source_addr: 64,
                n2k_wire_format: N2kWireFormat::YachtDevicesRawSingleLine,
                pgns_2000: vec![
                    PgnSchedule { kind: Pgn2000Kind::Pgn127245Rudder, enabled: true, interval_sec: 0.5, elapsed_sec: 0.1 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn127488EngineRapid, enabled: true, interval_sec: 0.5, elapsed_sec: 0.3 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn127489EngineDynamic, enabled: true, interval_sec: 1.5, elapsed_sec: 0.5 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn127505FluidLevel, enabled: true, interval_sec: 2.5, elapsed_sec: 0.7 },
                    PgnSchedule { kind: Pgn2000Kind::Pgn127508BatteryStatus, enabled: true, interval_sec: 2.0, elapsed_sec: 0.9 },
                ],
                error_injection: ErrorInjectionConfig::default(),
                target_ports: Vec::new(),
                fast_packet_seq: 0,
                sid_counter: 0,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_mode_and_wire_format_labels() {
        assert!(!ProtocolMode::Nmea0183Only.label().is_empty());
        assert!(!ProtocolMode::Nmea2000Only.label().is_empty());
        assert!(!ProtocolMode::Dual0183And2000.label().is_empty());

        for wf in N2kWireFormat::ALL {
            assert!(!wf.label().is_empty());
        }
    }

    #[test]
    fn test_sentence_0183_kind_code_and_description() {
        for kind in Sentence0183Kind::ALL {
            assert!(!kind.code().is_empty());
            assert!(!kind.description().is_empty());
        }
    }

    #[test]
    fn test_pgn_2000_kind_methods() {
        for pgn_kind in Pgn2000Kind::ALL {
            assert!(pgn_kind.pgn() >= 59392);
            let prio = pgn_kind.default_priority();
            assert!(prio <= 7);
            assert!(!pgn_kind.label().is_empty());
        }
        assert!(Pgn2000Kind::Pgn129029GnssPositionData.is_fast_packet());
        assert!(Pgn2000Kind::Pgn127489EngineDynamic.is_fast_packet());
        assert!(!Pgn2000Kind::Pgn129025PositionRapid.is_fast_packet());
    }

    #[test]
    fn test_error_injection_default() {
        let err = ErrorInjectionConfig::default();
        assert!(!err.corrupt_checksum);
        assert!(!err.omit_checksum);
        assert!(!err.omit_crlf);
    }

    #[test]
    fn test_instrument_next_sid_and_next_fast_seq_wrap() {
        let mut inst = Instrument::default_instruments().remove(0);
        inst.sid_counter = 251;
        assert_eq!(inst.next_sid(), 251);
        assert_eq!(inst.next_sid(), 252);
        // Must wrap around to 0 after 252
        assert_eq!(inst.next_sid(), 0);
        assert_eq!(inst.next_sid(), 1);

        inst.fast_packet_seq = 6;
        assert_eq!(inst.next_fast_seq(), 6);
        assert_eq!(inst.next_fast_seq(), 7);
        assert_eq!(inst.next_fast_seq(), 0);
    }

    #[test]
    fn test_default_instruments() {
        let insts = Instrument::default_instruments();
        assert_eq!(insts.len(), 6);
        for i in insts {
            assert!(i.enabled);
            assert!(!i.sentences_0183.is_empty());
            assert!(!i.pgns_2000.is_empty());
        }

        let custom = Instrument::new_custom(1);
        assert_eq!(custom.sentences_0183.len(), Sentence0183Kind::ALL.len());
        assert_eq!(custom.pgns_2000.len(), Pgn2000Kind::ALL.len());
        assert!(custom.sentences_0183.iter().all(|s| !s.enabled));
        assert!(custom.pgns_2000.iter().all(|p| !p.enabled));

        for (idx, &tmpl) in InstrumentTemplate::ALL.iter().enumerate() {
            assert!(!tmpl.label().is_empty());
            let inst = Instrument::from_template(tmpl, idx + 1);
            assert!(inst.enabled);
            assert!(!inst.icon().is_empty());
            assert!(!inst.sentences_0183.is_empty());
            assert!(!inst.pgns_2000.is_empty());
        }
        assert_eq!(Instrument::from_template(InstrumentTemplate::GpsGnss, 1).icon(), "🌐");
        assert_eq!(Instrument::from_template(InstrumentTemplate::GyroCompass, 1).icon(), "🎯");
        assert_eq!(Instrument::from_template(InstrumentTemplate::AutopilotNav, 1).icon(), "☸");
    }
}
