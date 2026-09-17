use crate::crop::crops_for_square;
use egui::{
    Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2,
};

pub const HORIZONTAL: Color32 = Color32::from_rgb(88, 196, 220);
pub const VERTICAL: Color32 = Color32::from_rgb(232, 176, 90);

pub fn draw_square_preview(
    ui: &mut Ui,
    texture: Option<&TextureHandle>,
    side: Option<u32>,
    placeholder: &str,
) -> Rect {
    let size = ui.available_width().min(560.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8.0, Color32::from_rgb(12, 12, 14));

    if let Some(texture) = texture {
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            placeholder,
            FontId::proportional(15.0),
            Color32::from_gray(170),
        );
    }

    let crops = crops_for_square(side.unwrap_or(1080));
    let scale_x = rect.width() / crops.side as f32;
    let scale_y = rect.height() / crops.side as f32;
    let map = |x: u32, y: u32, w: u32, h: u32| {
        Rect::from_min_size(
            Pos2::new(
                rect.min.x + x as f32 * scale_x,
                rect.min.y + y as f32 * scale_y,
            ),
            Vec2::new(w as f32 * scale_x, h as f32 * scale_y),
        )
    };
    let horizontal = map(
        crops.horizontal.x,
        crops.horizontal.y,
        crops.horizontal.w,
        crops.horizontal.h,
    );
    let vertical = map(
        crops.vertical.x,
        crops.vertical.y,
        crops.vertical.w,
        crops.vertical.h,
    );

    let dim = Color32::from_black_alpha(110);
    for corner in [
        Rect::from_min_max(rect.min, Pos2::new(vertical.min.x, horizontal.min.y)),
        Rect::from_min_max(
            Pos2::new(vertical.max.x, rect.min.y),
            Pos2::new(rect.max.x, horizontal.min.y),
        ),
        Rect::from_min_max(
            Pos2::new(rect.min.x, horizontal.max.y),
            Pos2::new(vertical.min.x, rect.max.y),
        ),
        Rect::from_min_max(Pos2::new(vertical.max.x, horizontal.max.y), rect.max),
    ] {
        if corner.width() > 0.5 && corner.height() > 0.5 {
            painter.rect_filled(corner, 0.0, dim);
        }
    }

    painter.rect_stroke(
        horizontal,
        0.0,
        Stroke::new(2.0, HORIZONTAL),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        vertical,
        0.0,
        Stroke::new(2.0, VERTICAL),
        StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(horizontal.center().x, horizontal.min.y + 12.0),
        Align2::CENTER_CENTER,
        "16:9 horizontal",
        FontId::proportional(12.0),
        HORIZONTAL,
    );
    painter.text(
        Pos2::new(vertical.center().x, vertical.min.y + 12.0),
        Align2::CENTER_CENTER,
        "16:9 vertical",
        FontId::proportional(12.0),
        VERTICAL,
    );

    rect
}
