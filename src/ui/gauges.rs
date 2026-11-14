use egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, Ui, Vec2,
};

pub fn angle_to_vec(deg_clockwise_from_top: f32) -> Vec2 {
    let rad = (deg_clockwise_from_top - 90.0).to_radians();
    Vec2::new(rad.cos(), rad.sin())
}

pub fn pointer_to_compass_deg(center: Pos2, pointer: Pos2) -> Option<f64> {
    let delta = pointer - center;
    if delta.length() <= 8.0 {
        return None;
    }
    let mut deg = delta.y.atan2(delta.x).to_degrees() + 90.0;
    if deg < 0.0 {
        deg += 360.0;
    }
    Some((deg as f64).round().rem_euclid(360.0))
}

pub fn pointer_to_wind_dir_deg(
    center: Pos2,
    pointer: Pos2,
    vessel_heading_deg: f64,
) -> Option<f64> {
    let rel_deg = pointer_to_compass_deg(center, pointer)?;
    Some((vessel_heading_deg + rel_deg).rem_euclid(360.0).round())
}

pub fn seabed_y_fraction(depth_m: f64, frac_x: f32, sim_time: f64) -> f32 {
    let max_scale = (depth_m * 1.5).max(10.0) as f32;
    let phase = (sim_time as f32) * 1.2 + frac_x * 5.0;
    let d_local = (depth_m as f32) + (phase.sin() * 0.6 + (phase * 2.3).cos() * 0.25);
    (d_local / max_scale).clamp(0.1, 0.92)
}

/// Interactive 360-degree Compass Rose showing True Heading, COG, and Commanded Heading.
/// Dragging inside the dial adjusts `heading_true_deg` (and `cog_deg`).
/// All digital readouts and legends are placed below the dial so they never overlap it.
pub fn draw_compass_gauge(
    ui: &mut Ui,
    size: f32,
    heading_true_deg: &mut f64,
    cog_deg: &mut f64,
    sog_knots: f64,
    rot_deg_min: f64,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let center = rect.center();
    let radius = (size * 0.44).max(40.0);

    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(deg) = pointer_to_compass_deg(center, pos) {
                *heading_true_deg = deg;
                *cog_deg = deg;
            }
        }
    }

    let painter = ui.painter_at(rect);

    let outer_bg = if dark {
        Color32::from_rgb(10, 15, 24)
    } else {
        Color32::from_rgb(215, 224, 236)
    };
    let ring_stroke_col = if dark {
        Color32::from_rgb(95, 130, 175)
    } else {
        Color32::from_rgb(51, 65, 85)
    };
    let dial_bg = if dark {
        Color32::from_rgb(5, 8, 15)
    } else {
        Color32::from_rgb(255, 255, 255)
    };

    painter.circle_filled(center, radius + 5.0, outer_bg);
    painter.circle_stroke(center, radius + 5.0, Stroke::new(2.4, ring_stroke_col));
    painter.circle_filled(center, radius, dial_bg);

    let tick_cardinal = if dark {
        Color32::from_rgb(255, 255, 255)
    } else {
        Color32::from_rgb(0, 0, 0)
    };
    let tick_major = if dark {
        Color32::from_rgb(215, 230, 250)
    } else {
        Color32::from_rgb(15, 23, 42)
    };
    let tick_minor = if dark {
        Color32::from_rgb(130, 155, 185)
    } else {
        Color32::from_rgb(71, 85, 105)
    };
    let deg_text_col = if dark {
        Color32::from_rgb(225, 238, 255)
    } else {
        Color32::from_rgb(0, 0, 0)
    };

    for deg in (0..360).step_by(5) {
        let dir = angle_to_vec(deg as f32);
        let is_cardinal = deg % 90 == 0;
        let is_major = deg % 30 == 0;
        let inner = if is_cardinal {
            radius - 13.0
        } else if is_major {
            radius - 9.0
        } else {
            radius - 4.5
        };
        let stroke = if is_cardinal {
            Stroke::new(2.4, tick_cardinal)
        } else if is_major {
            Stroke::new(1.6, tick_major)
        } else {
            Stroke::new(1.0, tick_minor)
        };
        painter.line_segment([center + dir * inner, center + dir * radius], stroke);

        if is_major && !is_cardinal && radius >= 65.0 {
            let label_pos = center + dir * (radius - 20.0);
            painter.text(
                label_pos,
                Align2::CENTER_CENTER,
                format!("{:03}", deg),
                FontId::monospace(9.5),
                deg_text_col,
            );
        }
    }

    let north_col = if dark {
        Color32::from_rgb(255, 70, 70)
    } else {
        Color32::from_rgb(185, 0, 15)
    };
    let cardinals = [
        (0.0, "N", north_col),
        (90.0, "E", tick_cardinal),
        (180.0, "S", tick_cardinal),
        (270.0, "W", tick_cardinal),
    ];
    for (deg, text, col) in cardinals {
        let pos = center + angle_to_vec(deg) * (radius - 21.0);
        painter.text(
            pos,
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(13.5),
            col,
        );
    }

    let cog_col = if dark {
        Color32::from_rgb(45, 255, 130)
    } else {
        Color32::from_rgb(0, 105, 35)
    };
    let cog_dir = angle_to_vec(*cog_deg as f32);
    painter.line_segment(
        [center, center + cog_dir * (radius - 8.0)],
        Stroke::new(2.6, cog_col),
    );
    painter.circle_filled(center + cog_dir * (radius - 10.0), 4.5, cog_col);

    let hdg_fill = if dark {
        Color32::from_rgba_unmultiplied(255, 195, 30, 235)
    } else {
        Color32::from_rgba_unmultiplied(205, 95, 0, 240)
    };
    let hdg_stroke = if dark {
        Color32::from_rgb(255, 240, 130)
    } else {
        Color32::from_rgb(110, 40, 0)
    };
    let hdg_f = *heading_true_deg as f32;
    let hdg_dir = angle_to_vec(hdg_f);
    let perp = Vec2::new(-hdg_dir.y, hdg_dir.x);
    let tip = center + hdg_dir * (radius - 12.0);
    let tail = center - hdg_dir * (radius * 0.28);
    painter.add(Shape::convex_polygon(
        vec![tip, center + perp * 6.0, tail, center - perp * 6.0],
        hdg_fill,
        Stroke::new(1.5, hdg_stroke),
    ));

    // Clean center pivot pin (no overlapping text inside the dial)
    let pivot_fill = if dark {
        Color32::from_rgb(30, 42, 60)
    } else {
        Color32::from_rgb(226, 232, 240)
    };
    painter.circle_filled(center, 6.0, pivot_fill);
    painter.circle_stroke(center, 6.0, Stroke::new(1.8, ring_stroke_col));

    // Digital readouts and legend placed BELOW the compass gauge (high contrast & single line)
    let hdg_txt_col = if dark {
        Color32::from_rgb(255, 220, 50)
    } else {
        Color32::from_rgb(145, 50, 0)
    };
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "HDG: {:05.1}°T  |  COG: {:05.1}°T",
                *heading_true_deg, *cog_deg
            ))
            .monospace()
            .strong()
            .size(11.5)
            .color(hdg_txt_col),
        )
        .truncate(),
    );
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "SOG: {:4.1} kn  |  ROT: {:+5.1}°/min",
                sog_knots, rot_deg_min
            ))
            .monospace()
            .strong()
            .size(11.0)
            .color(cog_col),
        )
        .truncate(),
    );

    response
}

/// Interactive Wind Rose (Bow-Up) showing Apparent Wind (Cyan) and True Wind (Orange).
/// Dragging inside the dial adjusts `wind_true_dir_deg`.
/// All digital readouts and legends are placed below the dial so they never overlap it.
pub fn draw_wind_gauge(
    ui: &mut Ui,
    size: f32,
    wind_true_dir_deg: &mut f64,
    wind_true_speed_knots: f64,
    awa_deg: f64,
    aws_knots: f64,
    twa_deg: f64,
    vessel_heading_deg: f64,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let center = rect.center();
    let radius = (size * 0.44).max(40.0);

    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(twd) = pointer_to_wind_dir_deg(center, pos, vessel_heading_deg) {
                *wind_true_dir_deg = twd;
            }
        }
    }

    let painter = ui.painter_at(rect);

    let outer_bg = if dark {
        Color32::from_rgb(10, 15, 24)
    } else {
        Color32::from_rgb(215, 224, 236)
    };
    let ring_stroke_col = if dark {
        Color32::from_rgb(95, 130, 175)
    } else {
        Color32::from_rgb(51, 65, 85)
    };
    let dial_bg = if dark {
        Color32::from_rgb(5, 8, 15)
    } else {
        Color32::from_rgb(255, 255, 255)
    };

    painter.circle_filled(center, radius + 5.0, outer_bg);
    painter.circle_stroke(center, radius + 5.0, Stroke::new(2.4, ring_stroke_col));
    painter.circle_filled(center, radius, dial_bg);

    let stbd_sector = if dark {
        Color32::from_rgb(45, 240, 120)
    } else {
        Color32::from_rgb(0, 125, 45)
    };
    let port_sector = if dark {
        Color32::from_rgb(255, 70, 70)
    } else {
        Color32::from_rgb(195, 15, 25)
    };
    for step in 20..60 {
        let d1 = step as f32;
        let d2 = (step + 1) as f32;
        painter.line_segment(
            [
                center + angle_to_vec(d1) * (radius - 4.0),
                center + angle_to_vec(d2) * (radius - 4.0),
            ],
            Stroke::new(4.2, stbd_sector),
        );
        painter.line_segment(
            [
                center + angle_to_vec(360.0 - d1) * (radius - 4.0),
                center + angle_to_vec(360.0 - d2) * (radius - 4.0),
            ],
            Stroke::new(4.2, port_sector),
        );
    }

    let tick_col = if dark {
        Color32::from_rgb(210, 225, 245)
    } else {
        Color32::from_rgb(15, 23, 42)
    };
    let lbl_col = if dark {
        Color32::from_rgb(235, 245, 255)
    } else {
        Color32::from_rgb(0, 0, 0)
    };

    for deg in (0..360).step_by(10) {
        let dir = angle_to_vec(deg as f32);
        let is_major = deg % 30 == 0;
        let inner = if is_major { radius - 10.5 } else { radius - 5.5 };
        painter.line_segment(
            [center + dir * inner, center + dir * radius],
            Stroke::new(if is_major { 1.7 } else { 1.0 }, tick_col),
        );
        if is_major && deg != 0 && deg != 180 && radius >= 65.0 {
            let lbl = if deg <= 180 { deg } else { 360 - deg };
            let pos = center + dir * (radius - 20.0);
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                format!("{}", lbl),
                FontId::monospace(9.5),
                lbl_col,
            );
        }
    }

    let boat_stroke = if dark {
        Color32::from_rgb(155, 185, 220)
    } else {
        Color32::from_rgb(30, 41, 59)
    };
    let bow = center + Vec2::new(0.0, -radius * 0.36);
    let stbd_mid = center + Vec2::new(radius * 0.14, 0.0);
    let stbd_stern = center + Vec2::new(radius * 0.10, radius * 0.32);
    let port_stern = center + Vec2::new(-radius * 0.10, radius * 0.32);
    let port_mid = center + Vec2::new(-radius * 0.14, 0.0);
    painter.add(Shape::closed_line(
        vec![bow, stbd_mid, stbd_stern, port_stern, port_mid],
        Stroke::new(1.8, boat_stroke),
    ));

    let twa_col = if dark {
        Color32::from_rgb(255, 175, 45)
    } else {
        Color32::from_rgb(160, 50, 0)
    };
    let twa_dir = angle_to_vec(twa_deg as f32);
    painter.line_segment(
        [center, center + twa_dir * (radius - 12.0)],
        Stroke::new(2.5, twa_col),
    );
    painter.circle_filled(center + twa_dir * (radius - 12.0), 4.8, twa_col);

    let awa_col = if dark {
        Color32::from_rgb(65, 235, 255)
    } else {
        Color32::from_rgb(0, 82, 155)
    };
    let awa_dir = angle_to_vec(awa_deg as f32);
    painter.line_segment(
        [center, center + awa_dir * (radius - 10.0)],
        Stroke::new(3.2, awa_col),
    );
    painter.circle_filled(center + awa_dir * (radius - 10.0), 5.2, awa_col);

    // Clean center pivot pin
    let pivot_fill = if dark {
        Color32::from_rgb(30, 42, 60)
    } else {
        Color32::from_rgb(226, 232, 240)
    };
    painter.circle_filled(center, 5.5, pivot_fill);
    painter.circle_stroke(center, 5.5, Stroke::new(1.8, ring_stroke_col));

    // Digital readouts and legend placed BELOW the wind gauge (high contrast & single line)
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "AWS: {:4.1} kn @ AWA: {:03.0}° (Apparent)",
                aws_knots, awa_deg
            ))
            .monospace()
            .strong()
            .size(11.5)
            .color(awa_col),
        )
        .truncate(),
    );
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "TWS: {:4.1} kn @ TWD: {:03.0}° (True)",
                wind_true_speed_knots, *wind_true_dir_deg
            ))
            .monospace()
            .strong()
            .size(11.0)
            .color(twa_col),
        )
        .truncate(),
    );

    response
}

/// Draws a miniature Echo Sounder / Depth & Rudder Graphic Strip with readouts placed below the graphic.
pub fn draw_depth_and_rudder_panel(
    ui: &mut Ui,
    depth_m: f64,
    offset_m: f64,
    water_temp_c: f64,
    rudder_deg: f64,
    pitch_deg: f64,
    roll_deg: f64,
    sim_time: f64,
) {
    let dark = ui.visuals().dark_mode;
    let desired = Vec2::new(ui.available_width().max(150.0), 125.0);
    let (rect, _resp) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(rect);

    let outer_bg = if dark {
        Color32::from_rgb(6, 10, 18)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    let border_col = if dark {
        Color32::from_rgb(95, 130, 175)
    } else {
        Color32::from_rgb(51, 65, 85)
    };
    painter.rect_filled(rect, 6.0, outer_bg);
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.6, border_col),
        egui::StrokeKind::Inside,
    );

    let split_x = rect.left() + rect.width() * 0.58;

    let echo_rect = Rect::from_min_max(
        rect.min + Vec2::new(8.0, 8.0),
        Pos2::new(split_x - 6.0, rect.bottom() - 8.0),
    );
    let water_bg = if dark {
        Color32::from_rgb(4, 18, 34)
    } else {
        Color32::from_rgb(224, 238, 255)
    };
    painter.rect_filled(echo_rect, 4.0, water_bg);

    let num_pts = 40;
    let mut seabed_pts = Vec::with_capacity(num_pts + 2);
    seabed_pts.push(Pos2::new(echo_rect.left(), echo_rect.bottom()));
    for i in 0..=num_pts {
        let frac = i as f32 / num_pts as f32;
        let x = echo_rect.left() + frac * echo_rect.width();
        let y_frac = seabed_y_fraction(depth_m, frac, sim_time);
        let y = echo_rect.top() + y_frac * echo_rect.height();
        seabed_pts.push(Pos2::new(x, y));
    }
    seabed_pts.push(Pos2::new(echo_rect.right(), echo_rect.bottom()));

    let seabed_col = if dark {
        Color32::from_rgb(255, 195, 50)
    } else {
        Color32::from_rgb(155, 55, 0)
    };
    painter.add(Shape::line(
        seabed_pts[1..=num_pts + 1].to_vec(),
        Stroke::new(2.5, seabed_col),
    ));

    let right_rect = Rect::from_min_max(
        Pos2::new(split_x + 4.0, rect.top() + 8.0),
        rect.max - Vec2::new(8.0, 8.0),
    );
    let r_center = right_rect.center();

    let horizon_col = if dark {
        Color32::from_rgb(75, 230, 255)
    } else {
        Color32::from_rgb(0, 82, 155)
    };
    let roll_rad = (roll_deg as f32).to_radians();
    let pitch_offset = (pitch_deg as f32).clamp(-20.0, 20.0) * 1.2;
    let h_half = right_rect.width() * 0.36;
    let dx = h_half * roll_rad.cos();
    let dy = h_half * roll_rad.sin();
    let horizon_c = r_center + Vec2::new(0.0, -10.0 + pitch_offset);
    painter.line_segment(
        [horizon_c - Vec2::new(dx, dy), horizon_c + Vec2::new(dx, dy)],
        Stroke::new(2.8, horizon_col),
    );

    let bar_y = right_rect.bottom() - 14.0;
    let bar_half_w = right_rect.width() * 0.42;
    let bar_rect =
        Rect::from_center_size(Pos2::new(r_center.x, bar_y), Vec2::new(bar_half_w * 2.0, 12.0));
    let bar_bg = if dark {
        Color32::from_rgb(22, 32, 48)
    } else {
        Color32::from_rgb(203, 213, 225)
    };
    painter.rect_filled(bar_rect, 3.0, bar_bg);
    let norm_rud = (rudder_deg as f32 / 35.0).clamp(-1.0, 1.0);
    let rud_x = r_center.x + norm_rud * bar_half_w;
    let rud_col = if rudder_deg >= 0.0 {
        if dark {
            Color32::from_rgb(45, 255, 130)
        } else {
            Color32::from_rgb(0, 105, 35)
        }
    } else if dark {
        Color32::from_rgb(255, 75, 75)
    } else {
        Color32::from_rgb(185, 0, 15)
    };
    let fill_min_x = r_center.x.min(rud_x);
    let fill_max_x = r_center.x.max(rud_x);
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(fill_min_x, bar_rect.top()),
            Pos2::new(fill_max_x, bar_rect.bottom()),
        ),
        2.0,
        rud_col,
    );

    // Digital readouts placed BELOW the Echo Sounder & Attitude/Rudder graphic strip
    let dbt_txt_col = if dark {
        Color32::from_rgb(80, 235, 255)
    } else {
        Color32::from_rgb(0, 72, 145)
    };
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "DBT: {:4.1} m (Surf: {:4.1} m)  |  Temp: {:4.1} °C",
                depth_m,
                depth_m + offset_m,
                water_temp_c
            ))
            .monospace()
            .strong()
            .size(11.5)
            .color(dbt_txt_col),
        )
        .truncate(),
    );
    let rud_dir = if rudder_deg > 0.2 {
        "STBD"
    } else if rudder_deg < -0.2 {
        "PORT"
    } else {
        "MID"
    };
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "Pitch {:+4.1}° | Roll {:+4.1}° | Rudder: {:4.1}° {}",
                pitch_deg,
                roll_deg,
                rudder_deg.abs(),
                rud_dir
            ))
            .monospace()
            .strong()
            .size(11.0)
            .color(rud_col),
        )
        .truncate(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_angle_to_vec_cardinals() {
        let v_top = angle_to_vec(0.0);
        assert!(v_top.x.abs() < 1e-5 && (v_top.y - (-1.0)).abs() < 1e-5);

        let v_right = angle_to_vec(90.0);
        assert!((v_right.x - 1.0).abs() < 1e-5 && v_right.y.abs() < 1e-5);

        let v_bottom = angle_to_vec(180.0);
        assert!(v_bottom.x.abs() < 1e-5 && (v_bottom.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_pointer_to_compass_and_wind_dir() {
        let c = Pos2::new(100.0, 100.0);
        // Too close to center (< 8 px) returns None
        assert!(pointer_to_compass_deg(c, Pos2::new(103.0, 103.0)).is_none());

        // Straight up -> 0 deg
        assert_eq!(pointer_to_compass_deg(c, Pos2::new(100.0, 50.0)), Some(0.0));
        // Straight right -> 90 deg
        assert_eq!(pointer_to_compass_deg(c, Pos2::new(150.0, 100.0)), Some(90.0));
        // Straight down -> 180 deg
        assert_eq!(pointer_to_compass_deg(c, Pos2::new(100.0, 150.0)), Some(180.0));
        // Straight left -> 270 deg
        assert_eq!(pointer_to_compass_deg(c, Pos2::new(50.0, 100.0)), Some(270.0));

        // Wind relative 90 deg on a ship heading 200 deg -> 290 deg TWD
        assert_eq!(
            pointer_to_wind_dir_deg(c, Pos2::new(150.0, 100.0), 200.0),
            Some(290.0)
        );
    }

    #[test]
    fn test_seabed_y_fraction_bounds() {
        for i in 0..=10 {
            let f = seabed_y_fraction(25.0, i as f32 * 0.1, 3.5);
            assert!(f >= 0.1 && f <= 0.92);
        }
    }

    #[test]
    fn test_draw_all_gauges_headless() {
        let ctx = egui::Context::default();
        let mut hdg = 240.0;
        let mut cog = 242.0;
        let mut twd = 290.0;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw_compass_gauge(ui, 200.0, &mut hdg, &mut cog, 7.5, 1.2);
                draw_wind_gauge(ui, 200.0, &mut twd, 15.0, 35.0, 19.0, 46.0, hdg);
                draw_depth_and_rudder_panel(ui, 24.5, 0.8, 15.6, 5.0, 1.0, -2.0, 10.0);
            });
        });
    }
}
