# NMEA Studio — User Manual

Welcome to the official User Manual for **NMEA Studio**, a cross-platform **NMEA 0183 & NMEA 2000** marine simulator, traffic generator, and protocol analyzer for **macOS, Linux, and Windows**.

---

## Table of Contents

1. [Overview & Workspace Layout](#1-overview--workspace-layout)
2. [Default Empty Project, Presets & Saving/Loading (`.json`)](#2-default-empty-project-presets--savingloading-json)
3. [Adding & Configuring Devices (GPS, Gyro, AIS, etc.)](#3-adding--configuring-devices-gps-gyro-ais-etc)
4. [Port Routing & IEC 61162-450 UDP Multicast Standards](#4-port-routing--iec-61162-450-udp-multicast-standards)
5. [Helm Controls, Interactive Gauges & Sensors (`☸ Helm & Sensors`)](#5-helm-controls-interactive-gauges--sensors--helm--sensors)
6. [Nautical Chart, Radar ARPA & Route Autopilot (`🗺 Chart & Radar`)](#6-nautical-chart-radar-arpa--route-autopilot--chart--radar)
7. [AIS & ARPA Target Simulator (`🚢 AIS & ARPA`)](#7-ais--arpa-target-simulator--ais--arpa)
8. [Split Resizable Traffic Stream & Linked Packet Decoder](#8-split-resizable-traffic-stream--linked-packet-decoder)
9. [Stream Recorder & Log File Player (`⏺ Recorder & Player`)](#9-stream-recorder--log-file-player--recorder--player)
10. [Custom Sentence / PGN Studio & Fault Injection (`🔬 Custom Studio`)](#10-custom-sentence--pgn-studio--fault-injection--custom-studio)
11. [Supported NMEA 0183 Sentences & NMEA 2000 PGNs Reference](#11-supported-nmea-0183-sentences--nmea-2000-pgns-reference)

---

## 1. Overview & Workspace Layout

The **NMEA Studio** interface is organized into **4 resizable regions**:

1. **Top Telemetry & Simulation Ribbon**:
   - Real-time vessel summary (`Position`, `SOG`, `HDG`, `AWS`, `DBT`, and live `TX` / `RX` packet counters).
   - Simulation execution controls: **`⏸ Pause`** / **`▶ Run`**, single-step **`+1s`**, simulation time multiplier (**`Speed: 0.25x .. 20x`**), and **`Waves`** fluctuation toggle.
   - **Preset** selector and **`💾 Save...`** / **`📂 Load...`** project buttons.
2. **Left Object Explorer (`Object Explorer`)**:
   - A *NemaStudio*-style tree view displaying your **Own Ship**, **Instruments**, **Communication Ports**, **AIS & ARPA Targets**, and **Active Route**.
   - Quickly enable/disable any device or port via checkboxes, click an item to jump to its configuration tab, or click **`➕ Add Device / Instrument`** and **`➕ Add Port`** to add equipment on the fly.
3. **Central Multi-Tab Workspace**:
   - `☸ Helm & Sensors`: Interactive graphical gauges and full sensor controls.
   - `🗺 Chart & Radar`: 2D nautical chart, radar range rings, waypoint editor, and AIS target overlay.
   - `📡 Instruments`: Per-device configuration (NMEA 0183 talker ID and sentences, NMEA 2000 PGNs, transmit intervals, port routing, fault injection).
   - `🔌 Ports`: Serial, UDP Unicast, UDP Multicast (IEC 61162-450), UDP Broadcast, TCP Server, and TCP Client setup.
   - `🚢 AIS & ARPA`: Multi-vessel target fleet, CPA/TCPA collision alarms, and target kinematics editor.
   - `⏺ Recorder & Player`: Capture live traffic to `.log` files or replay recorded logs at adjustable rates.
   - `🔬 Custom Studio`: Interactive NMEA 0183 XOR checksum calculator and custom NMEA 2000 PGN hex builder.
4. **Bottom Split Console (`📡 Traffic Stream` + `🔬 Packet Decoder`)**:
   - Left pane: Real-time incoming (`RX`) and outgoing (`TX`) frame stream.
   - Right pane: Resizable, linked **Packet Decoder** dissecting the active or pinned frame into engineering units.

### Automatic System-Linked Day / Night High-Contrast Theme
**NMEA Studio** automatically tracks your operating system's light or dark appearance in real time (`ThemePreference::System`). Both **Day Mode** and **Night Mode** use ultra-high-contrast color palettes engineered for maximum readability in bright daylight and low-glare night bridge environments.

---

## 2. Default Empty Project, Presets & Saving/Loading (`.json`)

### Unconfigured Empty Default Project
When launched, **NMEA Studio** starts with **`0. Empty Project (Unconfigured Default)`**:
- No instruments, ports, or AIS targets are pre-populated, allowing you to construct your exact test topology from a clean slate without unwanted network traffic.

### Loading Built-in Scenarios
Use the **`Preset:`** dropdown in the top toolbar to switch between:
- **`0. Empty Project (Unconfigured Default)`**: Clean, unconfigured workspace.
- **`1. Coastal Cruiser (Default)`**: Full suite of navigation instruments, ports, route waypoints, and AIS targets.
- **`2. High-Speed Patrol Craft (28 kn)`**: Fast vessel scenario at 28 knots.
- **`3. Harbor Tug & Engine Test`**: Propulsion-focused vessel setup.

### Saving and Loading Projects (`💾 Save...` / `📂 Load...`)
1. Click **`💾 Save...`** in the top toolbar to open the interactive file browser modal.
2. Navigate through directories (`⬆ Parent Directory`, double-click folders, or click `🏠 Current Workspace`), enter a file name (e.g., `my_bridge_setup.json`), and click **`💾 Save Here`**.
3. Click **`📂 Load...`** to browse your disk, select any saved `.json` preset, and restore all instruments, ports, vessel state, AIS targets, and routes.

---

## 3. Adding & Configuring Devices (GPS, Gyro, AIS, etc.)

To add simulated marine devices to your project:

1. In the left **Object Explorer** or inside the **`📡 Instruments`** tab, click **`➕ Add Device / Instrument`**.
2. Select the desired device template from the menu:
   - **`🌐 GPS / GNSS Receiver`**: Emits `GGA`, `RMC`, `GLL`, `VTG`, `GSA`, `GSV`, `ZDA`.
   - **`🎯 Gyro / Heading & Attitude`**: Emits `HDT`, `HDG`, `HDM`, `ROT`.
   - **`💨 Wind & Weather Sensor`**: Emits `MWV (Apparent)`, `MWV (True)`, `MWD`, `VWR`, `VWT`, `MDA`, `XDR`.
   - **`🌊 Depth Sounder & Speed Log`**: Emits `DPT`, `DBT`, `DBS`, `DBK`, `MTW`, `VHW`, `VLW`.
   - **`☸ Autopilot & Route Navigation`**: Emits `APB`, `RMB`, `BOD`, `BWC`, `XTE`, `WPL`, `RTE`.
   - **`⚙ Engine, Rudder & Tanks`**: Emits `RSA`, `RPM`, `XDR (Engine/Battery/Tanks)`.
   - **`🛠 Custom Empty Instrument`**: Blank device where you enable only the specific sentences/PGNs you need.
   - **`🚢 AIS Transponder & Target (!AIVDM / !AIVDO)`**: Enables the AIS simulator and spawns an AIS vessel target.
   - **`📡 ARPA Radar Target ($RATTM)`**: Enables radar `$RATTM` output and spawns a radar target.

### Default NMEA 0183 Protocol Mode
- Every newly added device defaults to **`NMEA 0183 Only`** mode (`ProtocolMode::Nmea0183Only`).
- To enable **NMEA 2000** output on a device, select it in the **`📡 Instruments`** tab and change **`Protocol:`** to:
  - `NMEA 2000 Only (PGNs)`
  - or `Dual (NMEA 0183 + NMEA 2000)`

---

## 4. Port Routing & IEC 61162-450 UDP Multicast Standards

### Per-Device Output Port Selection
In the **`📡 Instruments`** tab (and in **`🚢 AIS & ARPA`**), every device features a **`🔌 Output Port Routing`** bar:
- **Assign existing ports**: Check one or more configured ports to route that device's output exclusively to those ports. *(If no specific port is checked, the device broadcasts to `All Active TX Ports`.)*
- **One-Click Port Creation**:
  - **`➕ Serial`**: Creates and binds a Serial Port (`COM1` or the first detected `/dev/cu.*` / `/dev/tty*` port at `4800` bps).
  - **`➕ UDP Unicast`**: Creates and binds a UDP Unicast port (`127.0.0.1:10110`).
  - **`➕ UDP Multicast (IEC)`**: Automatically creates and binds a **UDP Multicast** port pre-configured with the official **IEC 61162-450 (OneNet / LWE)** multicast IP address and UDP port matching that specific equipment type!

### Standard IEC 61162-450 UDP Multicast Groups
In the **IEC 61162-450** maritime Ethernet standard, IPv4 multicast addresses (`239.192.0.1` through `239.192.0.8`) and UDP destination ports (`60001` through `60008`) are standardized by equipment function. You can also select these presets manually in the **`🔌 Ports`** tab via the **`IEC 61162-450 Preset:`** dropdown:

| IEC 61162-450 Group | Multicast IP | UDP Port | Standard Equipment & Sentences |
| :--- | :--- | :--- | :--- |
| **`MISC`** | `239.192.0.1` | `60001` | Miscellaneous / Custom Equipment |
| **`TGTD`** | `239.192.0.2` | `60002` | AIS Targets (`!AIVDM`, `!AIVDO`) & Radar ARPA (`$RATTM`) |
| **`SATD`** | `239.192.0.3` | `60003` | Echo Sounder, Depth & Speed Log (`DPT`, `DBT`, `DBS`, `MTW`, `VHW`, `VLW`) |
| **`NAVD`** | `239.192.0.4` | `60004` | Navigation: GNSS/GPS, Gyrocompass, Wind & Weather, Autopilot |
| **`VDRD`** | `239.192.0.5` | `60005` | Voyage Data Recorder (VDR) |
| **`RCOM`** | `239.192.0.6` | `60006` | Radiocommunication Equipment |
| **`TIME`** | `239.192.0.7` | `60007` | UTC Time & Frequency Reference (`ZDA`) |
| **`PROP`** | `239.192.0.8` | `60008` | Propulsion, Engines, Rudder & Tanks (`RPM`, `RSA`, `XDR`) |

---

## 5. Helm Controls, Interactive Gauges & Sensors (`☸ Helm & Sensors`)

The **`☸ Helm & Sensors`** tab displays **3 interactive graphical gauges** at the top (each with a dedicated digital readout strip below the dial so text never overlaps graphics) and **4 sensor control panels** below:

1. **360° Compass & Course Rose (`COMPASS & COURSE ROSE`)**:
   - Click or drag anywhere inside the compass ring to steer the vessel's true heading (**`HDG`**) and course over ground (**`COG`**).
2. **360° Wind Anemometer Rose (`WIND ANEMOMETER ROSE`)**:
   - Displays Apparent Wind (**`AWA`/`AWS`**) and True Wind (**`TWA`/`TWS`**) relative to the bow. Drag inside the wind rose to rotate the True Wind Direction (**`TWD`**).
3. **Echo Sounder Bathymetry, Artificial Horizon & Rudder Strip (`BATHYMETRY / PITCH-ROLL / RUDDER`)**:
   - Shows a scrolling seabed depth profile, live pitch/roll horizon indicator, and port/starboard rudder angle bar.
4. **Sensor Control Panels**:
   - **🌐 GNSS Position, Course & Speed**: Vessel name/callsign, Latitude, Longitude, Altitude (`m`), `SOG`, `COG`, `Satellites`, and `HDOP`.
   - **🎯 Gyrocompass, Heading & Attitude (AHRS)**: True Heading (`HDT`), live computed Magnetic (`HDM`) and Compass (`HDG`) headings, Magnetic Variation and Deviation, Rate of Turn (`ROT`), Rudder Angle (`RSA`), `Pitch`, `Roll`, and Vertical `Heave`.
   - **🌊 Depth Sounder & Speed Log**: Depth below transducer (`DBT`), live computed depth below surface (`DBS`) and keel (`DBK`), transducer/keel offsets, Speed Through Water (`STW`), and Water Temperature (`MTW`).
   - **💨 Wind & Meteorology**: `TWS`, `TWD`, computed Apparent Wind (`AWS`/`AWA`), barometric pressure (`hPa`), air temperature, and relative humidity.
   - **⚙ Propulsion, Electrical & Wave Jitter**: Engine `RPM`, coolant temperature, oil pressure, fuel rate (`L/h`), fuel level (`%`), battery voltage (`V`), and realistic wave/sensor jitter amplitude.

---

## 6. Nautical Chart, Radar ARPA & Route Autopilot (`🗺 Chart & Radar`)

The **`🗺 Chart & Radar`** tab provides an interactive 2D navigation display combining a nautical chart plotter and radar overlay:

- **Panning & Zooming**:
  - **Mouse Wheel** (or `Zoom:` slider): Zoom in and out (`0.2x` to `30x`).
  - **Left-Click Drag**: Pan the chart freely (automatically unchecks `Follow Ship`; re-check `Follow Ship` or click `Center Own Ship` to lock back onto your vessel).
- **Chart Left-Click Modes (`Left-Click Action`)**:
  - **`Pan Chart`**: Drag to pan around the chart.
  - **`Add Waypoint`**: Click anywhere on the chart to append a new waypoint (`WP1`, `WP2`...) to the active route.
  - **`Teleport Own Ship`**: Instantly move your vessel to the clicked coordinates.
- **Route Autopilot (`Engage Route Autopilot`)**:
  - Check **`Engage Route Autopilot`** in the right-hand route panel (or select `Route Autopilot (Track Waypoints)` in `☸ Helm & Sensors`).
  - Own ship will automatically steer along the active route leg, compute Cross-Track Error (**`XTE`**), and sequence to the next waypoint upon reaching the arrival radius.

---

## 7. AIS & ARPA Target Simulator (`🚢 AIS & ARPA`)

In the **`🚢 AIS & ARPA`** tab:
1. Enable **`🚢 AIS / ARPA Simulator Active`** and select the desired output formats:
   - `Emit Own Ship !AIVDO` (own vessel position reports)
   - `Emit NMEA 0183 !AIVDM` (AIS Class A Msg 1 & 5; Class B Msg 18 & 24)
   - `Emit NMEA 2000 AIS PGNs (129038/129039)`
2. Click **`➕ Spawn Random Target`** to add a new moving vessel around own ship.
3. Select any target in the left fleet list to monitor its **Range**, **Bearing**, **CPA** (Closest Point of Approach), and **TCPA** (Time to CPA). A **`⚠ CPA ALARM`** badge is triggered whenever `CPA < 0.5 NM` and `0 < TCPA < 30 min`.
4. Use the **`🎯 Selected Target Editor`** on the right to:
   - Edit vessel name, callsign, `MMSI`, `IMO`, class (`Class A`, `Class B`, `ARPA Only`), navigation status, coordinates, `SOG`, and `COG`.
   - Click **`🎯 Aim Toward Own Ship (CPA Test)`** to put the target on a direct collision course with own ship for testing ECDIS/plotter collision alarms.
   - Click **`📡 Transmit AIS + ARPA Burst Now`** to immediately transmit position, static (Msg 5 / 24), and `$RATTM` sentences.

---

## 8. Split Resizable Traffic Stream & Linked Packet Decoder

The bottom dock is split into two distinct, independently resizable panels:
- **Left Panel: `📡 Traffic Stream`** (chronological table of incoming `RX` and outgoing `TX` frames).
- **Right Panel: `🔬 Packet Decoder`** (real-time protocol dissector and field inspector).

### Vertical & Horizontal Resizing
- **Vertical Resize**: Drag the top border of the bottom dock up or down to adjust the console height.
- **Horizontal Resize**: Drag the **vertical splitter bar** between **`📡 Traffic Stream`** and **`🔬 Packet Decoder`** left or right to resize both panels.
- **Column Resize**: Drag column dividers in the traffic table (`#Seq`, `Time`, `Dir`, `Proto`, `Source / Port`, `Raw Frame`) to customize column widths.

### Direct Visual Link Between Traffic Stream and Packet Decoder (`#Seq`)
Every frame is assigned a sequential identifier (**`#Seq`**) displayed in the first column of the Traffic Stream and in the header of the Packet Decoder:

1. **Live Follow Mode (`◎ LIVE FOLLOW #00012`)**:
   - By default, the Packet Decoder automatically tracks and decodes the newest frame matching your active filters.
   - The active row in `📡 Traffic Stream` is highlighted and marked with **`▶ #0012`**.
2. **Pinned Frame Mode (`📌 PINNED #00012`)**:
   - **Click any row** (or its `#Seq` button) in `📡 Traffic Stream` to pin that specific frame in the Packet Decoder.
   - The pinned row is highlighted with **`📌#0012`**, and the Packet Decoder's framed banner switches to **`📌 PINNED #00012`**, displaying:
     - Exact `#Seq`, timestamp, direction (`TX`/`RX`), source port/instrument, protocol badge, and checksum verification (`✔ CHECKSUM OK` or `✖ CHECKSUM INVALID`).
     - The full raw sentence/frame and decoded summary.
     - A 2-column table of all decoded engineering fields (decoded latitude/longitude, SOG, COG, MMSI, 6-bit AIS payload fields, NMEA 2000 PGN parameters, etc.).
   - Click **`▶ Follow Live`** in the Packet Decoder header (or click the pinned row again) to unpin and resume following live traffic.

### Stream Filtering & Pause
- Use **`⏸ Freeze`** / **`▶ Resume`** to pause scrolling, **`🗑 Clear`** to empty the buffer, the **`TX`**, **`RX`**, **`0183`**, **`N2K`** checkboxes, or the **`🔍`** search box (e.g., `RMC`, `AIVDM`, `129025`) to filter frames in real time.

---

## 9. Stream Recorder & Log File Player (`⏺ Recorder & Player`)

In the **`⏺ Recorder & Player`** tab:
- **Recording Traffic (`⏺ NMEA Stream Recorder`)**:
  - Enter an output file path (e.g., `nmea_capture.log`), choose whether to capture `Record Generated TX` and/or `Record Incoming RX`, and click **`⏺ Start Recording (Overwrite)`** or **`➕ Start Recording (Append)`**.
- **Replaying Logs (`▶ NMEA Log File Player`)**:
  - Load a `.log` / `.nmea` file via **`📂 Load File`** or load the built-in test dataset via **`Built-in Sample`**.
  - Adjust **`Playback Rate`** (`0.5` to `100 lines/s`), toggle **`Loop`**, and click **`▶ Play Log to Active Ports`** (or **`⏭ Step 1 Frame`** to step through one line at a time).

---

## 10. Custom Sentence / PGN Studio & Fault Injection (`🔬 Custom Studio`)

In the **`🔬 Custom Studio`** tab:
- **Left (`🔬 NMEA 0183 Custom Sentence Studio`)**:
  - Select the prefix (`$` or `!`) and enter any sentence body (without leading `$` or trailing `*CS`, e.g., `GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W`).
  - The hexadecimal XOR checksum (`*6A`) is computed live.
  - Check **`Inject Bad Checksum`** to intentionally corrupt the checksum and test receiver error handling.
  - Click **`📡 Transmit Sentence Once`** or enable **`Auto-Repeat every:`**.
- **Right (`⚡ NMEA 2000 Custom PGN Builder`)**:
  - Specify any **PGN Number** (`59392..=131071`), priority (`0..7`), source address (`0..253`), wire framing format (`Yacht Devices RAW`, `Fast-Packet`, `SeaSmart $PCDIN`, `Actisense ASCII`), and hex payload bytes.
  - Click **`⚡ Transmit PGN Now`** or enable periodic transmission.

---

## 11. Supported NMEA 0183 Sentences & NMEA 2000 PGNs Reference

### Supported NMEA 0183 Sentences
| Category | Sentence Formatters |
| :--- | :--- |
| **GNSS / GPS** | `GGA`, `RMC`, `GLL`, `VTG`, `GSA`, `GSV`, `ZDA` |
| **Heading & Attitude** | `HDT`, `HDG`, `HDM`, `ROT` |
| **Wind & Meteorology** | `MWV (Apparent)`, `MWV (True)`, `MWD`, `VWR`, `VWT`, `MDA`, `XDR (Weather)` |
| **Depth & Speed Log** | `DPT`, `DBT`, `DBS`, `DBK`, `MTW`, `VHW`, `VLW` |
| **Autopilot & Navigation** | `APB`, `RMB`, `BOD`, `BWC`, `XTE`, `WPL`, `RTE` |
| **Engine, Rudder & Tanks** | `RSA`, `RPM`, `XDR (Engine/Battery/Tanks)` |
| **AIS & Radar ARPA** | `!AIVDM` (Msg 1, 5, 18, 24), `!AIVDO`, `$RATTM` |

### Supported NMEA 2000 PGNs
| PGN | Official NMEA 2000 Parameter Group Name | Framing |
| :--- | :--- | :--- |
| **`126992`** | System Time | Single-Frame |
| **`127245`** | Rudder | Single-Frame |
| **`127250`** | Vessel Heading | Single-Frame |
| **`127251`** | Rate of Turn (ROT) | Single-Frame |
| **`127257`** | Attitude (Yaw, Pitch, Roll) | Single-Frame |
| **`127258`** | Magnetic Variation | Single-Frame |
| **`127488`** | Engine Parameters, Rapid Update | Single-Frame |
| **`127489`** | Engine Parameters, Dynamic | Fast-Packet (26 bytes) |
| **`127505`** | Fluid Level (Fuel / Water Tanks) | Single-Frame |
| **`127508`** | Battery Status | Single-Frame |
| **`128259`** | Speed, Water Referenced | Single-Frame |
| **`128267`** | Water Depth | Single-Frame |
| **`128275`** | Distance Log | Fast-Packet (14 bytes) |
| **`129025`** | Position, Rapid Update | Single-Frame |
| **`129026`** | COG & SOG, Rapid Update | Single-Frame |
| **`129029`** | GNSS Position Data | Fast-Packet (43 bytes) |
| **`129038`** | AIS Class A Position Report | Fast-Packet (27 bytes) |
| **`129039`** | AIS Class B Position Report | Fast-Packet (26 bytes) |
| **`129283`** | Cross-Track Error (XTE) | Single-Frame |
| **`129284`** | Navigation Data | Fast-Packet (34 bytes) |
| **`130306`** | Wind Data (Apparent & True) | Single-Frame |
| **`130310`** | Environmental Parameters (Water/Air/Pressure) | Single-Frame |
| **`130311`** | Environmental Parameters (Temp/Humidity/Pressure) | Single-Frame |
| **`130312`** | Temperature | Single-Frame |
| **`130314`** | Actual Pressure | Single-Frame |
