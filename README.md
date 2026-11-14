# NMEA Studio Pro (Rust)

A cross-platform **NMEA 0183 & NMEA 2000** simulator, traffic analyzer, and test studio inspired by **NemaStudio**, built natively in Rust with `eframe` / `egui` for **macOS, Linux, and Windows**.

---

## Key Features

### 1. Simulated Vessel, Helm & Sensor Desk
- **Interactive Graphical Gauges**:
  - **360° Compass & Course Rose**: Click or drag directly inside the compass dial to steer the vessel (`HDG` & `COG`), with live velocity and rate-of-turn (`ROT`) readouts.
  - **360° Wind Anemometer Rose**: Bow-up wind rose displaying Apparent Wind (`AWA`/`AWS` in cyan) and True Wind (`TWA`/`TWS` in orange), port/starboard close-hauled sectors, and drag-to-adjust wind direction.
  - **Echo Sounder Bathymetry & Artificial Horizon / Rudder Strip**: Live seabed profile, depth below transducer/surface/keel, sea temperature, pitch/roll horizon, and port/starboard rudder angle bar.
- **Full Sensor Suite**:
  - GNSS Position (`Lat`, `Lon`, `Alt`, `HDOP`, `Satellites`, `Fix Quality`)
  - Course & Speed (`SOG`, `STW`, `COG`, `HDT`, `HDM`, `ROT`, `Mag Variation & Deviation`)
  - Wind & Meteorology (`TWS`, `TWD`, computed `AWS`/`AWA`, Barometric Pressure, Air Temp, Relative Humidity, Dew Point)
  - Bathymetry (`DBT`, `DBS`, `DBK`, `DPT`, Water Temp `MTW`)
  - Propulsion, Electrical & Fluid Tanks (Engine `RPM`, Coolant Temp, Oil Pressure, Fuel Rate, Fuel/Water Tank Levels, Alternator & House Battery Voltage/Current)
- **Realistic Wave & Sensor Fluctuation**: Configurable multi-frequency wave oscillation and sensor jitter (GPS, Speed, Heading, Wind, Depth, Pitch/Roll).

### 2. Multi-Instrument Simulator (NMEA 0183 + NMEA 2000)
Configure multiple independent virtual marine instruments (GNSS Receiver, Gyro/Attitude Compass, Masthead Wind & Weather, Echo Sounder & Speed Log, Autopilot & Route Plotter, Engine/Rudder/Battery Gateway, or add custom instruments):
- **NMEA 0183 Sentences**:
  - `GGA`, `RMC`, `GLL`, `VTG`, `GSA`, `GSV`, `ZDA`
  - `HDT`, `HDG`, `HDM`, `ROT`
  - `MWV (Apparent & True)`, `MWD`, `VWR`, `VWT`, `MDA`, `XDR (Weather)`
  - `DPT`, `DBT`, `DBS`, `DBK`, `MTW`, `VHW`, `VLW`
  - `APB`, `RMB`, `BOD`, `BWC`, `XTE`, `WPL`, `RTE`
  - `RSA`, `RPM`, `XDR (Engine/Battery/Tanks)`
- **NMEA 2000 PGNs**:
  - `126992` (System Time), `127245` (Rudder), `127250` (Vessel Heading), `127251` (Rate of Turn), `127257` (Attitude), `127258` (Magnetic Variation)
  - `127488` (Engine Rapid Update), `127489` (Engine Dynamic - Fast-Packet), `127505` (Fluid Level), `127508` (Battery Status)
  - `128259` (Speed Water Referenced), `128267` (Water Depth), `128275` (Distance Log - Fast-Packet)
  - `129025` (Position Rapid Update), `129026` (COG & SOG Rapid Update), `129029` (GNSS Position Data - Fast-Packet)
  - `129283` (Cross-Track Error), `129284` (Navigation Data - Fast-Packet)
  - `130306` (Wind Data Apparent & True), `130310` / `130311` (Environmental Parameters), `130312` (Temperature), `130314` (Actual Pressure)
- **NMEA 2000 Wire Encapsulations over Serial / UDP / TCP**:
  - **Yacht Devices RAW ASCII (Single-Line)**: `HH:MM:SS.mmm T <29-bit CAN ID Hex> <B0> <B1> ...`
  - **Yacht Devices RAW ASCII (8-byte CAN Fast-Packet Frames)**: Fragments >8-byte PGNs using ISO 11783 / NMEA 2000 Fast-Packet sequence counters
  - **SeaSmart (`$PCDIN`)**: `$PCDIN,<PGN_HEX>,<TIMESTAMP_HEX>,<SRC_HEX>,<PAYLOAD_HEX>*<CS>`
  - **Actisense N2K ASCII**: `A<timestamp> <src><dst><prio> <pgn> <payload_hex>`
- **Fault Injection**: Intentionally corrupt XOR checksums, omit `*CS`, or omit `CRLF` to test parser resilience.

### 3. Multi-Port Transport Engine
- **Serial Port (`COM` / `/dev/tty*` / `/dev/cu.*`)**: Hardware & virtual serial port auto-discovery, configurable baud rate (`4800` to `230400` bps), data bits, parity, and stop bits.
- **UDP Unicast**: Bidirectional TX/RX with configurable destination IP:Port and local bind IP:Port.
- **UDP Multicast**: Full multicast group support (e.g. `239.192.0.1:60001` or `224.0.0.1`), configurable interface IP, Multicast TTL, and Loopback.
- **UDP Broadcast**: Subnet or `255.255.255.255` broadcast output.
- **TCP Server & TCP Client**: Multi-client TCP listener or outbound TCP client connection.

### 4. AIS & Radar ARPA Target Simulator
- Simulate multiple moving vessels around own ship with full kinematic updates and real-time **CPA / TCPA** collision alarm calculation.
- Generates ITU-R M.1371 6-bit armored `!AIVDM` & `!AIVDO` sentences (Class A Msg 1 & multi-fragment Msg 5; Class B Msg 18 & Msg 24), Radar ARPA `$RATTM`, and NMEA 2000 AIS PGNs (`129038` & `129039`).

### 5. Interactive Chart & Radar Plotter + Route Autopilot
- Pan and zoom 2D nautical chart & radar overlay with range rings, Lat/Lon graticule grid, own ship heading/COG vectors, wind vector, breadcrumb track trail, and AIS target vectors.
- Click on the chart to add waypoints or teleport own ship, or engage the **Route Autopilot** to automatically steer along active legs with Cross-Track Error (`XTE`) correction.

### 6. Log Recorder, Log Player & Custom Sentence / PGN Builder
- Record live NMEA 0183 and NMEA 2000 traffic to `.log`/`.nmea` files and replay logs at `0.5` to `100` lines/sec.
- Interactive NMEA 0183 checksum studio and NMEA 2000 custom PGN hex builder.
- Built-in **Live Traffic Monitor & Packet Inspector** that decodes clicked NMEA 0183 sentences and NMEA 2000 PGN frames into engineering units.

---

## Building & Running

```bash
# Run unit tests
cargo test

# Run the desktop application (Debug)
cargo run

# Build optimized release binary (macOS, Linux, Windows)
cargo build --release
```
