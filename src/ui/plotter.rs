use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use crate::model::ais::TargetVessel;
use crate::model::route::{Route, Waypoint};
use crate::model::vessel::VesselState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotterTool {
    PanAndInspect,
    AddWaypoint,
    MoveOwnShip,
}

pub struct PlotterState {
    pub center_lat: f64,
    pub center_lon: f64,
    pub range_nm: f64, // Half-height of viewport in Nautical Miles
    pub follow_own_ship: bool,
    pub show_range_rings: bool,
    pub show_grid: bool,
    pub show_wind_vectors: bool,
    pub show_ais_labels: bool,
    pub predictor_minutes: f64,
    pub active_tool: PlotterTool,
    pub selected_ais_id: Option<u32>,
}

impl Default for PlotterState {
    fn default() -> Self {
        Self {
            center_lat: 48.3580,
            center_lon: -4.4850,
            range_nm: 3.0,
            follow_own_ship: true,
            show_range_rings: true,
            show_grid: true,
            show_wind_vectors: true,
            show_ais_labels: true,
            predictor_minutes: 6.0,
            active_tool: PlotterTool::PanAndInspect,
            selected_ais_id: None,
        }
    }
}

pub fn grid_step_for_range(range_nm: f64) -> f64 {
    if range_nm > 15.0 {
        0.2
    } else if range_nm > 4.0 {
        0.05
    } else if range_nm > 1.2 {
        0.02
    } else {
        0.005
    }
}

impl PlotterState {
    pub fn latlon_to_screen(&self, rect: Rect, lat: f64, lon: f64) -> Pos2 {
        let px_per_nm = (rect.height() * 0.5) / (self.range_nm.max(0.05) as f32);
        let d_north_nm = (lat - self.center_lat) * 60.0;
        let cos_lat = self.center_lat.to_radians().cos().abs().max(0.05);
        let d_east_nm = (lon - self.center_lon) * 60.0 * cos_lat;

        let c = rect.center();
        Pos2::new(
            c.x + (d_east_nm as f32) * px_per_nm,
            c.y - (d_north_nm as f32) * px_per_nm,
        )
    }

    pub fn screen_to_latlon(&self, rect: Rect, pos: Pos2) -> (f64, f64) {
        let px_per_nm = (rect.height() * 0.5) / (self.range_nm.max(0.05) as f32);
        let c = rect.center();
        let d_east_nm = ((pos.x - c.x) / px_per_nm) as f64;
        let d_north_nm = ((c.y - pos.y) / px_per_nm) as f64;

        let cos_lat = self.center_lat.to_radians().cos().abs().max(0.05);
        let lat = (self.center_lat + d_north_nm / 60.0).clamp(-89.9, 89.9);
        let lon = self.center_lon + d_east_nm / (60.0 * cos_lat);
        (lat, lon)
    }

    pub fn heading_vec(deg: f64) -> Vec2 {
        let rad = (deg as f32 - 90.0).to_radians();
        Vec2::new(rad.cos(), rad.sin())
    }

    pub fn draw(
        &mut self,
        ui: &mut Ui,
        vessel: &mut VesselState,
        route: &mut Route,
        ais_targets: &mut [TargetVessel],
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut self.active_tool,
                PlotterTool::PanAndInspect,
                "✋ Pan / Inspect",
            );
            ui.selectable_value(
                &mut self.active_tool,
                PlotterTool::AddWaypoint,
                "📍 Click to Add Waypoint",
            );
            ui.selectable_value(
                &mut self.active_tool,
                PlotterTool::MoveOwnShip,
                "🚢 Click to Teleport Own Ship",
            );

            ui.separator();
            ui.checkbox(&mut self.follow_own_ship, "Follow Ship");
            if ui.button("Center on Ship").clicked() {
                self.center_lat = vessel.latitude;
                self.center_lon = vessel.longitude;
            }
            ui.separator();
            if ui.button("🔍 -").clicked() {
                self.range_nm = (self.range_nm * 1.4).min(50.0);
            }
            ui.label(format!("Scale: {:.2} NM", self.range_nm));
            if ui.button("🔍 +").clicked() {
                self.range_nm = (self.range_nm / 1.4).max(0.15);
            }
            ui.separator();
            ui.checkbox(&mut self.show_range_rings, "Range Rings");
            ui.checkbox(&mut self.show_grid, "Lat/Lon Grid");
            ui.checkbox(&mut self.show_ais_labels, "AIS Labels");
            if ui.button("Clear Track").clicked() {
                vessel.track_history.clear();
            }
        });

        ui.add_space(4.0);

        if self.follow_own_ship {
            self.center_lat = vessel.latitude;
            self.center_lon = vessel.longitude;
        }

        let dark = ui.visuals().dark_mode;
        let avail = ui.available_size();
        let canvas_size = Vec2::new(avail.x.max(160.0), avail.y.max(160.0));
        let (rect, response) = ui.allocate_exact_size(canvas_size, Sense::click_and_drag());

        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.5 {
                let factor = (1.0 - scroll * 0.003).clamp(0.7, 1.4);
                self.range_nm = (self.range_nm * factor as f64).clamp(0.1, 60.0);
            }
        }

        if response.dragged() && self.active_tool == PlotterTool::PanAndInspect {
            let delta = response.drag_delta();
            if delta.length() > 0.5 {
                self.follow_own_ship = false;
                let px_per_nm = (rect.height() * 0.5) / (self.range_nm.max(0.05) as f32);
                let d_east_nm = (delta.x / px_per_nm) as f64;
                let d_north_nm = (-delta.y / px_per_nm) as f64;
                let cos_lat = self.center_lat.to_radians().cos().abs().max(0.05);
                self.center_lat = (self.center_lat - d_north_nm / 60.0).clamp(-89.9, 89.9);
                self.center_lon -= d_east_nm / (60.0 * cos_lat);
            }
        }

        if response.clicked() {
            if let Some(click_pos) = response.interact_pointer_pos() {
                let (clat, clon) = self.screen_to_latlon(rect, click_pos);
                match self.active_tool {
                    PlotterTool::AddWaypoint => {
                        let idx = route.waypoints.len() + 1;
                        route.waypoints.push(Waypoint {
                            id: format!("WP{}", idx),
                            name: format!("WPT-{:02}", idx),
                            latitude: clat,
                            longitude: clon,
                            arrival_radius_nm: 0.10,
                        });
                    }
                    PlotterTool::MoveOwnShip => {
                        vessel.latitude = clat;
                        vessel.longitude = clon;
                        self.active_tool = PlotterTool::PanAndInspect;
                    }
                    PlotterTool::PanAndInspect => {
                        let mut found = None;
                        for tgt in ais_targets.iter() {
                            if !tgt.enabled {
                                continue;
                            }
                            let p = self.latlon_to_screen(rect, tgt.latitude, tgt.longitude);
                            if p.distance(click_pos) < 16.0 {
                                found = Some(tgt.id);
                                break;
                            }
                        }
                        self.selected_ais_id = found;
                    }
                }
            }
        }

        let painter = ui.painter_at(rect);
        let sea_bg = if dark {
            Color32::from_rgb(5, 12, 22)
        } else {
            Color32::from_rgb(232, 244, 255)
        };
        painter.rect_filled(rect, 6.0, sea_bg);
        painter.rect_stroke(
            rect,
            6.0,
            Stroke::new(
                1.5,
                if dark {
                    Color32::from_rgb(80, 115, 160)
                } else {
                    Color32::from_rgb(51, 65, 85)
                },
            ),
            egui::StrokeKind::Inside,
        );

        let px_per_nm = (rect.height() * 0.5) / (self.range_nm.max(0.05) as f32);

        if self.show_grid {
            let step_deg = grid_step_for_range(self.range_nm);
            let grid_line_col = if dark {
                Color32::from_rgb(38, 68, 105)
            } else {
                Color32::from_rgb(148, 178, 210)
            };
            let grid_text_col = if dark {
                Color32::from_rgb(185, 215, 245)
            } else {
                Color32::from_rgb(15, 23, 42)
            };

            let min_lat = (self.center_lat - self.range_nm / 30.0 / step_deg).floor() * step_deg;
            let max_lat = (self.center_lat + self.range_nm / 30.0 / step_deg).ceil() * step_deg;
            let mut lat = (min_lat / step_deg).floor() * step_deg;
            while lat <= max_lat {
                let p = self.latlon_to_screen(rect, lat, self.center_lon);
                if p.y >= rect.top() && p.y <= rect.bottom() {
                    painter.line_segment(
                        [Pos2::new(rect.left(), p.y), Pos2::new(rect.right(), p.y)],
                        Stroke::new(0.8, grid_line_col),
                    );
                    painter.text(
                        Pos2::new(rect.left() + 6.0, p.y - 2.0),
                        Align2::LEFT_BOTTOM,
                        format!("{:.3}°", lat),
                        FontId::monospace(9.5),
                        grid_text_col,
                    );
                }
                lat += step_deg;
            }

            let min_lon = (self.center_lon - self.range_nm / 15.0 / step_deg).floor() * step_deg;
            let max_lon = (self.center_lon + self.range_nm / 15.0 / step_deg).ceil() * step_deg;
            let mut lon = (min_lon / step_deg).floor() * step_deg;
            while lon <= max_lon {
                let p = self.latlon_to_screen(rect, self.center_lat, lon);
                if p.x >= rect.left() && p.x <= rect.right() {
                    painter.line_segment(
                        [Pos2::new(p.x, rect.top()), Pos2::new(p.x, rect.bottom())],
                        Stroke::new(0.8, grid_line_col),
                    );
                    painter.text(
                        Pos2::new(p.x + 3.0, rect.bottom() - 4.0),
                        Align2::LEFT_BOTTOM,
                        format!("{:.3}°", lon),
                        FontId::monospace(9.5),
                        grid_text_col,
                    );
                }
                lon += step_deg;
            }
        }

        let ship_pos = self.latlon_to_screen(rect, vessel.latitude, vessel.longitude);

        if self.show_range_rings {
            let ring_stroke_col = if dark {
                Color32::from_rgba_unmultiplied(85, 175, 235, 155)
            } else {
                Color32::from_rgba_unmultiplied(0, 82, 155, 165)
            };
            let ring_txt_col = if dark {
                Color32::from_rgb(175, 225, 255)
            } else {
                Color32::from_rgb(0, 60, 120)
            };
            let ring_step_nm = (self.range_nm / 3.0).max(0.1);
            for i in 1..=3 {
                let r_nm = ring_step_nm * (i as f64);
                let r_px = (r_nm as f32) * px_per_nm;
                painter.circle_stroke(ship_pos, r_px, Stroke::new(1.2, ring_stroke_col));
                painter.text(
                    ship_pos + Vec2::new(4.0, -r_px + 10.0),
                    Align2::LEFT_TOP,
                    format!("{:.2} NM", r_nm),
                    FontId::monospace(9.5),
                    ring_txt_col,
                );
            }
        }

        if !route.waypoints.is_empty() {
            let mut wp_pts = Vec::with_capacity(route.waypoints.len());
            for wp in &route.waypoints {
                wp_pts.push(self.latlon_to_screen(rect, wp.latitude, wp.longitude));
            }

            for i in 1..wp_pts.len() {
                let is_active_leg = route.active && route.active_leg_to_idx == i;
                let col = if is_active_leg {
                    if dark {
                        Color32::from_rgb(255, 105, 235)
                    } else {
                        Color32::from_rgb(162, 20, 185)
                    }
                } else if dark {
                    Color32::from_rgb(180, 145, 245)
                } else {
                    Color32::from_rgb(90, 25, 185)
                };
                painter.line_segment(
                    [wp_pts[i - 1], wp_pts[i]],
                    Stroke::new(if is_active_leg { 2.8 } else { 1.8 }, col),
                );
            }

            let wp_txt_col = if dark {
                Color32::from_rgb(250, 235, 255)
            } else {
                Color32::from_rgb(59, 7, 100)
            };
            for (idx, wp) in route.waypoints.iter().enumerate() {
                let p = wp_pts[idx];
                let is_dest = route.active && route.active_leg_to_idx == idx;
                let arr_px = (wp.arrival_radius_nm as f32 * px_per_nm).max(6.0);
                painter.circle_stroke(
                    p,
                    arr_px,
                    Stroke::new(
                        1.2,
                        if is_dest {
                            Color32::from_rgba_unmultiplied(217, 70, 239, 195)
                        } else {
                            Color32::from_rgba_unmultiplied(124, 58, 237, 130)
                        },
                    ),
                );
                painter.circle_filled(
                    p,
                    5.5,
                    if is_dest {
                        Color32::from_rgb(217, 70, 239)
                    } else {
                        Color32::from_rgb(109, 40, 217)
                    },
                );
                painter.text(
                    p + Vec2::new(8.0, -8.0),
                    Align2::LEFT_BOTTOM,
                    &wp.name,
                    FontId::monospace(10.5),
                    wp_txt_col,
                );
            }
        }

        if vessel.track_history.len() >= 2 {
            let pts: Vec<Pos2> = vessel
                .track_history
                .iter()
                .map(|&(lat, lon)| self.latlon_to_screen(rect, lat, lon))
                .collect();
            let trail_col = if dark {
                Color32::from_rgba_unmultiplied(90, 220, 255, 185)
            } else {
                Color32::from_rgba_unmultiplied(0, 82, 155, 200)
            };
            painter.add(Shape::line(pts, Stroke::new(1.8, trail_col)));
        }

        for tgt in ais_targets.iter() {
            if !tgt.enabled {
                continue;
            }
            let tp = self.latlon_to_screen(rect, tgt.latitude, tgt.longitude);
            let cpa = tgt.compute_cpa(vessel);
            let is_cpa_alarm = cpa.cpa_nm < 0.5 && cpa.tcpa_min > 0.0 && cpa.tcpa_min < 30.0;
            let is_selected = self.selected_ais_id == Some(tgt.id);

            let base_col = if is_cpa_alarm {
                if dark {
                    Color32::from_rgb(255, 70, 70)
                } else {
                    Color32::from_rgb(185, 0, 15)
                }
            } else if dark {
                Color32::from_rgb(55, 250, 175)
            } else {
                Color32::from_rgb(0, 95, 75)
            };

            let pred_nm = tgt.sog_knots * (self.predictor_minutes / 60.0);
            let pred_px = (pred_nm as f32) * px_per_nm;
            let cog_dir = Self::heading_vec(tgt.cog_deg);
            painter.line_segment([tp, tp + cog_dir * pred_px], Stroke::new(1.8, base_col));

            let hdg_dir = Self::heading_vec(tgt.heading_deg);
            let perp = Vec2::new(-hdg_dir.y, hdg_dir.x);
            let tip = tp + hdg_dir * 10.0;
            let left = tp - hdg_dir * 7.0 - perp * 5.5;
            let right = tp - hdg_dir * 7.0 + perp * 5.5;
            painter.add(Shape::convex_polygon(
                vec![tip, right, left],
                Color32::from_rgba_unmultiplied(base_col.r(), base_col.g(), base_col.b(), 85),
                Stroke::new(if is_selected { 2.6 } else { 1.8 }, base_col),
            ));

            if is_selected {
                painter.circle_stroke(
                    tp,
                    14.0,
                    Stroke::new(
                        2.0,
                        if dark {
                            Color32::YELLOW
                        } else {
                            Color32::from_rgb(165, 65, 0)
                        },
                    ),
                );
            }

            if self.show_ais_labels {
                painter.text(
                    tp + Vec2::new(10.0, -6.0),
                    Align2::LEFT_BOTTOM,
                    format!("{} ({:.1} kn)", tgt.name, tgt.sog_knots),
                    FontId::monospace(10.0),
                    base_col,
                );
                painter.text(
                    tp + Vec2::new(10.0, 6.0),
                    Align2::LEFT_TOP,
                    format!("CPA {:.2}NM / {:.1}m", cpa.cpa_nm, cpa.tcpa_min),
                    FontId::monospace(9.5),
                    if is_cpa_alarm {
                        base_col
                    } else if dark {
                        Color32::from_rgb(220, 238, 255)
                    } else {
                        Color32::from_rgb(0, 0, 0)
                    },
                );
            }
        }

        let hdg_dir = Self::heading_vec(vessel.heading_true_deg);
        let cog_dir = Self::heading_vec(vessel.cog_deg);
        let perp = Vec2::new(-hdg_dir.y, hdg_dir.x);

        painter.line_segment(
            [ship_pos, ship_pos + hdg_dir * 140.0],
            Stroke::new(
                1.5,
                if dark {
                    Color32::from_rgba_unmultiplied(255, 220, 60, 185)
                } else {
                    Color32::from_rgba_unmultiplied(155, 55, 0, 200)
                },
            ),
        );

        let own_pred_nm = vessel.sog_knots * (self.predictor_minutes / 60.0);
        let own_pred_px = (own_pred_nm as f32) * px_per_nm;
        painter.line_segment(
            [ship_pos, ship_pos + cog_dir * own_pred_px],
            Stroke::new(
                2.5,
                if dark {
                    Color32::from_rgb(45, 255, 130)
                } else {
                    Color32::from_rgb(0, 105, 35)
                },
            ),
        );

        let s_tip = ship_pos + hdg_dir * 13.0;
        let s_stbd = ship_pos + perp * 6.0 - hdg_dir * 4.0;
        let s_stern_r = ship_pos + perp * 5.0 - hdg_dir * 10.0;
        let s_stern_l = ship_pos - perp * 5.0 - hdg_dir * 10.0;
        let s_port = ship_pos - perp * 6.0 - hdg_dir * 4.0;
        painter.add(Shape::convex_polygon(
            vec![s_tip, s_stbd, s_stern_r, s_stern_l, s_port],
            Color32::from_rgb(245, 175, 25),
            Stroke::new(
                2.2,
                if dark {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                },
            ),
        ));

        if self.show_wind_vectors {
            let wind_box = Pos2::new(rect.right() - 65.0, rect.top() + 55.0);
            painter.circle_filled(
                wind_box,
                38.0,
                if dark {
                    Color32::from_rgba_unmultiplied(8, 14, 24, 240)
                } else {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 245)
                },
            );
            painter.circle_stroke(
                wind_box,
                38.0,
                Stroke::new(
                    1.5,
                    if dark {
                        Color32::from_rgb(95, 130, 175)
                    } else {
                        Color32::from_rgb(51, 65, 85)
                    },
                ),
            );
            let twd_dir = Self::heading_vec(vessel.wind_true_dir_deg);
            let wind_vec_col = if dark {
                Color32::from_rgb(65, 235, 255)
            } else {
                Color32::from_rgb(0, 82, 155)
            };
            painter.line_segment(
                [wind_box + twd_dir * 28.0, wind_box],
                Stroke::new(2.8, wind_vec_col),
            );
            painter.text(
                wind_box + Vec2::new(0.0, 24.0),
                Align2::CENTER_CENTER,
                format!(
                    "💨 {:.0}° {:.1}kn",
                    vessel.wind_true_dir_deg, vessel.wind_true_speed_knots
                ),
                FontId::monospace(9.5),
                if dark {
                    Color32::from_rgb(255, 255, 255)
                } else {
                    Color32::from_rgb(0, 0, 0)
                },
            );
        }

        let hud_pos = rect.left_top() + Vec2::new(10.0, 10.0);
        let hud_text = if let Some(leg) = route.active_leg_metrics(
            vessel.latitude,
            vessel.longitude,
            vessel.sog_knots,
            vessel.cog_deg,
        ) {
            let dest_name = &route.waypoints[leg.dest_idx].name;
            format!(
                "OWN SHIP: {:.5}°N, {:.5}°E | SOG {:.1} kn | COG {:05.1}°T | HDG {:05.1}°T\nACTIVE LEG -> {}: Dist {:.2} NM | Brg {:05.1}°T | XTE {:.3} NM ({}) | VMG {:.1} kn",
                vessel.latitude,
                vessel.longitude,
                vessel.sog_knots,
                vessel.cog_deg,
                vessel.heading_true_deg,
                dest_name,
                leg.distance_to_dest_nm,
                leg.bearing_present_to_dest_true,
                leg.xte_nm.abs(),
                leg.steer_direction,
                leg.vmg_knots
            )
        } else {
            format!(
                "OWN SHIP: {:.5}°N, {:.5}°E | SOG {:.1} kn | COG {:05.1}°T | HDG {:05.1}°T",
                vessel.latitude,
                vessel.longitude,
                vessel.sog_knots,
                vessel.cog_deg,
                vessel.heading_true_deg
            )
        };

        painter.text(
            hud_pos,
            Align2::LEFT_TOP,
            hud_text,
            FontId::monospace(11.0),
            if dark {
                Color32::from_rgb(255, 255, 255)
            } else {
                Color32::from_rgb(0, 0, 0)
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_step_for_range() {
        assert_eq!(grid_step_for_range(20.0), 0.2);
        assert_eq!(grid_step_for_range(8.0), 0.05);
        assert_eq!(grid_step_for_range(2.0), 0.02);
        assert_eq!(grid_step_for_range(0.5), 0.005);
    }

    #[test]
    fn test_latlon_screen_roundtrip_and_heading_vec() {
        let p = PlotterState::default();
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));

        // Center of lat/lon maps to center of screen
        let center_px = p.latlon_to_screen(rect, p.center_lat, p.center_lon);
        assert!((center_px.x - 400.0).abs() < 1e-4);
        assert!((center_px.y - 300.0).abs() < 1e-4);

        // Arbitrary point roundtrip
        let lat_in = 48.3725;
        let lon_in = -4.5123;
        let screen_pt = p.latlon_to_screen(rect, lat_in, lon_in);
        let (lat_out, lon_out) = p.screen_to_latlon(rect, screen_pt);
        assert!((lat_out - lat_in).abs() < 1e-5);
        assert!((lon_out - lon_in).abs() < 1e-5);

        let v_north = PlotterState::heading_vec(0.0);
        assert!(v_north.x.abs() < 1e-5 && (v_north.y - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_plotter_draw_headless() {
        let ctx = egui::Context::default();
        let mut plotter = PlotterState::default();
        let mut vessel = VesselState::default();
        let mut route = Route::default();
        let mut fleet = TargetVessel::default_fleet();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                plotter.draw(ui, &mut vessel, &mut route, &mut fleet);
            });
        });
    }
}
