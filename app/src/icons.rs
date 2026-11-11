//! Minimal line icons, drawn as vectors (no icon font needed, crisp at any DPI).

use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Back,
    Settings,
    Contents,
    Bookmark,
    BookmarkFilled,
    Theme,
    SoundOn,
    SoundOff,
    TurnCurl,
    TurnSlide,
    TurnNone,
    Ltr,
    Rtl,
    BlankPage,
    Single,
    Spread,
    AutoSpread,
    Close,
    Text,
}

pub fn draw(p: &Painter, r: Rect, icon: Icon, c: Color32) {
    let s = Stroke::new(1.4, c);
    let u = r.width() / 20.0; // 20-unit grid
    let at = |x: f32, y: f32| pos2(r.left() + x * u, r.top() + y * u);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), s));
    };
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        p.rect_stroke(Rect::from_min_max(at(x0, y0), at(x1, y1)), 1.5 * u, s, egui::StrokeKind::Middle);
    };
    match icon {
        Icon::Back => line(&[(12.0, 4.5), (6.5, 10.0), (12.0, 15.5)]),
        Icon::Close => {
            line(&[(5.5, 5.5), (14.5, 14.5)]);
            line(&[(14.5, 5.5), (5.5, 14.5)]);
        }
        Icon::Settings => {
            for (y, k) in [(6.0, 7.0), (10.0, 13.0), (14.0, 9.0)] {
                line(&[(3.5, y), (16.5, y)]);
                p.circle_filled(at(k, y), 1.9 * u, c);
            }
        }
        Icon::Contents => {
            for y in [6.0, 10.0, 14.0] {
                p.circle_filled(at(4.5, y), 0.9 * u, c);
                line(&[(7.5, y), (16.0, y)]);
            }
        }
        Icon::Bookmark | Icon::BookmarkFilled => {
            let pts: Vec<Pos2> = [(6.0, 3.5), (14.0, 3.5), (14.0, 16.5), (10.0, 13.0), (6.0, 16.5)]
                .iter()
                .map(|&(x, y)| at(x, y))
                .collect();
            if icon == Icon::BookmarkFilled {
                // concave polygon: split into two convex parts
                p.add(Shape::convex_polygon(
                    vec![at(6.0, 3.5), at(14.0, 3.5), at(14.0, 16.5), at(10.0, 13.0)],
                    c,
                    Stroke::NONE,
                ));
                p.add(Shape::convex_polygon(
                    vec![at(6.0, 3.5), at(10.0, 13.0), at(6.0, 16.5)],
                    c,
                    Stroke::NONE,
                ));
            }
            p.add(Shape::closed_line(pts, s));
        }
        Icon::Theme => {
            p.circle_stroke(at(10.0, 10.0), 6.0 * u, s);
            let n = 20;
            let mut pts = vec![at(10.0, 4.0)];
            for i in 0..=n {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / n as f32;
                pts.push(at(10.0 + 6.0 * a.cos(), 10.0 + 6.0 * a.sin()));
            }
            p.add(Shape::convex_polygon(pts, c, Stroke::NONE));
        }
        Icon::SoundOn | Icon::SoundOff => {
            p.add(Shape::closed_line(
                vec![
                    at(3.5, 8.0),
                    at(6.5, 8.0),
                    at(10.0, 5.0),
                    at(10.0, 15.0),
                    at(6.5, 12.0),
                    at(3.5, 12.0),
                ],
                s,
            ));
            if icon == Icon::SoundOn {
                let arc = |rad: f32| {
                    let pts: Vec<Pos2> = (0..=10)
                        .map(|i| {
                            let a = -0.8 + 1.6 * i as f32 / 10.0;
                            at(10.0 + rad * a.cos(), 10.0 + rad * a.sin())
                        })
                        .collect();
                    p.add(Shape::line(pts, s));
                };
                arc(3.2);
                arc(6.0);
            } else {
                line(&[(12.5, 7.5), (17.0, 12.5)]);
                line(&[(17.0, 7.5), (12.5, 12.5)]);
            }
        }
        Icon::TurnCurl => {
            line(&[(4.5, 3.5), (11.5, 3.5), (15.5, 7.5), (15.5, 16.5), (4.5, 16.5), (4.5, 3.5)]);
            line(&[(11.5, 3.5), (11.5, 7.5), (15.5, 7.5)]);
        }
        Icon::TurnSlide => {
            line(&[(3.5, 7.5), (16.5, 7.5)]);
            line(&[(13.5, 4.5), (16.5, 7.5), (13.5, 10.5)]);
            line(&[(16.5, 12.5), (3.5, 12.5)]);
            line(&[(6.5, 9.5), (3.5, 12.5), (6.5, 15.5)]);
        }
        Icon::TurnNone => rect(5.0, 3.5, 15.0, 16.5),
        Icon::Ltr | Icon::Rtl => {
            let (a, b) = if icon == Icon::Ltr { (3.5, 16.5) } else { (16.5, 3.5) };
            let dir = if icon == Icon::Ltr { -1.0 } else { 1.0 };
            line(&[(a, 10.0), (b, 10.0)]);
            line(&[(b + dir * 3.5, 6.5), (b, 10.0), (b + dir * 3.5, 13.5)]);
            line(&[(a, 6.0), (a, 14.0)]);
        }
        Icon::BlankPage => {
            rect(3.0, 4.0, 9.5, 16.0);
            rect(10.5, 4.0, 17.0, 16.0);
            for y in [7.0, 9.5, 12.0] {
                line(&[(12.5, y), (15.0, y)]);
            }
        }
        Icon::Single => rect(6.0, 3.5, 14.0, 16.5),
        Icon::Spread => {
            rect(2.5, 4.0, 9.5, 16.0);
            rect(10.5, 4.0, 17.5, 16.0);
        }
        Icon::AutoSpread => {
            rect(2.5, 4.0, 9.5, 16.0);
            p.add(Shape::dashed_line(
                &[at(10.5, 4.0), at(17.5, 4.0), at(17.5, 16.0), at(10.5, 16.0)],
                s,
                2.0 * u,
                1.6 * u,
            ));
        }
        Icon::Text => {
            p.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "Aa",
                egui::FontId::proportional(13.5 * u / 1.0 * 0.75 + 2.0),
                c,
            );
        }
    }
}

/// A square, frameless icon button. `on` shows a subtle filled state.
pub fn button(ui: &mut Ui, icon: Icon, on: bool, tip: &str) -> Response {
    let size = Vec2::splat(30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let vis = ui.visuals();
    let fg = vis.text_color();
    if on || resp.hovered() {
        let fill = if on { fg.gamma_multiply(0.12) } else { fg.gamma_multiply(0.07) };
        ui.painter().rect_filled(rect, 8.0, fill);
    }
    draw(ui.painter(), Rect::from_center_size(rect.center(), vec2(20.0, 20.0)), icon, fg);
    resp.on_hover_text(tip)
}
