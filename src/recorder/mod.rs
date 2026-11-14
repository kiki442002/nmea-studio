use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

pub struct LogRecorderPlayer {
    // Recorder state
    pub record_file_path: String,
    pub is_recording: bool,
    pub record_tx: bool,
    pub record_rx: bool,
    pub recorded_lines_count: usize,
    pub recorder_status: String,
    active_record_file: Option<File>,

    // Player state
    pub play_file_path: String,
    pub is_playing: bool,
    pub loop_playback: bool,
    pub lines_per_second: f64,
    pub loaded_lines: Vec<String>,
    pub current_line_idx: usize,
    pub player_status: String,
    accum_sec: f64,
}

impl Default for LogRecorderPlayer {
    fn default() -> Self {
        let mut s = Self {
            record_file_path: "nmea_session_capture.log".to_string(),
            is_recording: false,
            record_tx: true,
            record_rx: true,
            recorded_lines_count: 0,
            recorder_status: "Idle".to_string(),
            active_record_file: None,

            play_file_path: "sample_sea_trial.nmea".to_string(),
            is_playing: false,
            loop_playback: true,
            lines_per_second: 5.0,
            loaded_lines: Vec::new(),
            current_line_idx: 0,
            player_status: "Built-in sample log loaded (ready to play)".to_string(),
            accum_sec: 0.0,
        };
        s.load_builtin_sample();
        s
    }
}

impl LogRecorderPlayer {
    pub fn start_recording(&mut self, append: bool) {
        let mut opts = OpenOptions::new();
        opts.create(true).write(true);
        if append {
            opts.append(true);
        } else {
            opts.truncate(true);
        }
        match opts.open(&self.record_file_path) {
            Ok(f) => {
                self.active_record_file = Some(f);
                self.is_recording = true;
                if !append {
                    self.recorded_lines_count = 0;
                }
                self.recorder_status = format!("Recording to {}", self.record_file_path);
            }
            Err(e) => {
                self.is_recording = false;
                self.recorder_status = format!("Failed to open {}: {}", self.record_file_path, e);
            }
        }
    }

    pub fn stop_recording(&mut self) {
        self.active_record_file = None;
        self.is_recording = false;
        self.recorder_status = format!(
            "Stopped ({} lines written to {})",
            self.recorded_lines_count, self.record_file_path
        );
    }

    pub fn record_line(&mut self, line: &str, is_tx: bool) {
        if !self.is_recording {
            return;
        }
        if (is_tx && !self.record_tx) || (!is_tx && !self.record_rx) {
            return;
        }
        if let Some(ref mut f) = self.active_record_file {
            if writeln!(f, "{}", line).is_ok() {
                self.recorded_lines_count += 1;
            }
        }
    }

    pub fn load_file(&mut self) {
        match File::open(&self.play_file_path) {
            Ok(f) => {
                let reader = BufReader::new(f);
                let mut lines = Vec::new();
                for l in reader.lines().map_while(Result::ok) {
                    let t = l.trim().to_string();
                    if !t.is_empty() && !t.starts_with('#') {
                        lines.push(t);
                    }
                }
                self.current_line_idx = 0;
                self.player_status =
                    format!("Loaded {} lines from {}", lines.len(), self.play_file_path);
                self.loaded_lines = lines;
            }
            Err(e) => {
                self.player_status = format!("Error opening {}: {}", self.play_file_path, e);
            }
        }
    }

    pub fn load_builtin_sample(&mut self) {
        use crate::model::instruments::ErrorInjectionConfig;
        use crate::protocols::nmea0183::wrap_sentence;
        let err = ErrorInjectionConfig::default();
        self.loaded_lines = vec![
            wrap_sentence(
                '$',
                "GPRMC,123519.00,A,4821.4800,N,00429.1000,W,7.85,245.0,131126,1.8,W,D",
                &err,
            ),
            wrap_sentence(
                '$',
                "GPGGA,123519.00,4821.4800,N,00429.1000,W,2,10,0.8,2.4,M,47.2,M,,",
                &err,
            ),
            wrap_sentence('$', "HCHDT,243.5,T", &err),
            wrap_sentence('$', "WIMWV,38.4,R,19.2,N,A", &err),
            wrap_sentence('$', "SDDPT,24.5,0.8,100.0", &err),
            wrap_sentence('$', "SDMTW,15.6,C", &err),
            wrap_sentence('$', "IIVHW,243.5,T,245.3,M,7.50,N,13.89,K", &err),
            "12:35:19.120 T 09F8010C 80 79 D9 1C C0 A1 B2 FD".to_string(),
            "12:35:19.240 T 09F8020C 01 FC 6B A7 F2 01 FF FF".to_string(),
            wrap_sentence('$', "PCDIN,01F112,0008AF20,18,0165A80000AEFFFC", &err),
            wrap_sentence('!', "AIVDM,1,1,,A,13P<;J001lO`1=vL4c:8P4`0089;,0", &err),
            wrap_sentence(
                '$',
                "RATTM,01,1.42,248.5,T,12.4,065.0,T,0.18,4.2,N,CMA CGM BRETAGNE,T,,,123519.50",
                &err,
            ),
        ];
        self.current_line_idx = 0;
        self.player_status = format!(
            "Loaded built-in sea-trial log ({} NMEA 0183 & N2K frames)",
            self.loaded_lines.len()
        );
    }

    pub fn step_one(&mut self) -> Option<String> {
        if self.loaded_lines.is_empty() {
            return None;
        }
        if self.current_line_idx >= self.loaded_lines.len() {
            if self.loop_playback {
                self.current_line_idx = 0;
            } else {
                self.is_playing = false;
                return None;
            }
        }
        let line = self.loaded_lines[self.current_line_idx].clone();
        self.current_line_idx += 1;
        Some(line)
    }

    pub fn update(&mut self, dt_sec: f64) -> Vec<String> {
        if !self.is_playing || self.loaded_lines.is_empty() || dt_sec <= 0.0 {
            return Vec::new();
        }
        let interval = (1.0 / self.lines_per_second.max(0.1)).max(0.005);
        self.accum_sec += dt_sec;
        let mut out = Vec::new();
        while self.accum_sec >= interval {
            self.accum_sec -= interval;
            if let Some(line) = self.step_one() {
                out.push(line);
            } else {
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_player_default_and_builtin_sample() {
        let mut rp = LogRecorderPlayer::default();
        assert_eq!(rp.loaded_lines.len(), 12);
        assert_eq!(rp.current_line_idx, 0);

        for line in &rp.loaded_lines {
            let info = crate::protocols::inspect_packet(line);
            if let Some(valid) = info.checksum_valid {
                assert!(
                    valid,
                    "Built-in sample line has invalid checksum: {} ({})",
                    line, info.summary
                );
            }
        }

        let first = rp.step_one().unwrap();
        assert!(first.starts_with("$GPRMC"));
        assert_eq!(rp.current_line_idx, 1);
    }

    #[test]
    fn test_step_one_loop_and_non_loop() {
        let mut rp = LogRecorderPlayer::default();
        rp.loaded_lines = vec!["LINE1".to_string(), "LINE2".to_string()];
        rp.loop_playback = false;
        rp.is_playing = true;

        assert_eq!(rp.step_one().as_deref(), Some("LINE1"));
        assert_eq!(rp.step_one().as_deref(), Some("LINE2"));
        assert!(rp.step_one().is_none());
        assert!(!rp.is_playing);

        rp.loop_playback = true;
        assert_eq!(rp.step_one().as_deref(), Some("LINE1"));
    }

    #[test]
    fn test_update_playback_rate() {
        let mut rp = LogRecorderPlayer::default();
        rp.loaded_lines = vec!["A".into(), "B".into(), "C".into()];
        rp.lines_per_second = 10.0; // 0.1s per line
        rp.is_playing = true;

        let out = rp.update(0.25); // Should emit 2 lines (0.2s consumed, 0.05s remainder)
        assert_eq!(out, vec!["A", "B"]);
    }

    #[test]
    fn test_record_to_file_and_load_file() {
        let tmp_path_buf = std::env::temp_dir().join("nmea_studio_unit_test_capture.log");
        let tmp_path = tmp_path_buf.to_string_lossy().to_string();
        let mut rp = LogRecorderPlayer::default();
        rp.record_file_path = tmp_path.clone();
        rp.record_tx = true;
        rp.record_rx = false;

        rp.start_recording(false);
        assert!(rp.is_recording);
        rp.record_line("$HCHDT,100.0,T*20", true);
        rp.record_line("$IGNORED_RX*00", false);
        assert_eq!(rp.recorded_lines_count, 1);
        rp.stop_recording();
        assert!(!rp.is_recording);

        // Append 1 more line
        rp.start_recording(true);
        rp.record_line("$HCHDT,200.0,T*23", true);
        rp.stop_recording();
        assert_eq!(rp.recorded_lines_count, 2);

        // Load back via load_file
        rp.play_file_path = tmp_path.clone();
        rp.load_file();
        assert_eq!(
            rp.loaded_lines,
            vec!["$HCHDT,100.0,T*20", "$HCHDT,200.0,T*23"]
        );

        let _ = std::fs::remove_file(&tmp_path_buf);
    }
}
