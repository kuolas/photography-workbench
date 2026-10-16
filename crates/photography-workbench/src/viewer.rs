// SPDX-License-Identifier: GPL-3.0-or-later

//! The viewer in Library's centre: the current photo alone, as large as
//! the centre allows.
//!
//! The photo is fitted in the centre less a margin on every side
//! ([`MARGIN`]), its aspect kept, in the middle, and enlarged when it is
//! smaller than that room, so it always reaches two of the room's sides.
//! It sits straight on the viewer's backdrop with nothing behind it.
//! What is drawn is the photo's thumbnail at the largest size thumbnails
//! are decoded at ([`crate::textures`]), wanted at the highest priority;
//! while that size is on its way, another size of the photo held stands
//! in for it, so a photo stepped to shows at once when the filmstrip
//! holds its thumbnail. A photo that cannot be shown shows an icon in the
//! middle, and why in its tooltip; nothing is drawn until some size of
//! the photo is decoded.
//!
//! The viewer draws the photo it is handed, whatever it was read from,
//! and the thumbnails the app holds, and names the photo to screen
//! readers by its file name.

use catalog::photos::Photo;
use egui::{Margin, Rect, Sense, WidgetInfo, WidgetType};

use crate::textures::{self, CURRENT, Textures};

/// The room kept between the centre's edges and the photo, on every
/// side, so the photo stands clear of the parts of the window around it.
const MARGIN: i8 = 24;

/// The size of the icon shown when the photo cannot be shown.
const FAILED_ICON_SIZE: f32 = 48.0;

/// The viewer of `photo`, filling `ui`, with the thumbnails in
/// `textures`.
pub fn show(ui: &mut egui::Ui, textures: &mut Textures, photo: &Photo) {
    let room = room(ui.max_rect());
    let longest = textures::longest(room.size().max_elem(), ui.pixels_per_point());
    // The current photo, at the priority every view wants it at: the
    // first of the highest tier, which nothing else wanted is above.
    let priority = textures::ranked([(0, CURRENT)])[0].1;
    textures.want(photo.path(), longest, priority);
    let thumbnail = textures.thumbnail(photo.path(), longest);

    let name = photo.name().into_owned();
    let id = ui.make_persistent_id("viewer");
    let response = ui.interact(textures::placed(thumbnail, room), id, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, true, &name));
    // The viewer shows no name, so its tooltip only says why a photo
    // cannot be shown: one naming the photo would cover it whenever the
    // pointer rests on it.
    if let Some(failure) = textures::paint(ui, thumbnail, room, FAILED_ICON_SIZE) {
        response.on_hover_text(failure.to_string());
    }
}

/// Where the photo is fitted in a centre at `centre`: all of it but
/// [`MARGIN`] on every side.
fn room(centre: Rect) -> Rect {
    centre - Margin::same(MARGIN)
}

#[cfg(test)]
mod tests {
    use egui::{Rect, TextureId, pos2, vec2};

    use super::room;
    use crate::textures::{Thumbnail, placed};

    /// Where the viewer draws a photo of `size` pixels in a centre at
    /// `centre`.
    fn drawn(size: [usize; 2], centre: Rect) -> Rect {
        let thumbnail = Thumbnail::Ready {
            texture: TextureId::Managed(1),
            size,
        };
        placed(thumbnail, room(centre))
    }

    /// A photo is fitted in the centre less 24 points on every side, as
    /// large as fits there, enlarged when it is smaller, its aspect kept,
    /// in the middle: a wide one reaches the room's left and right, a tall
    /// one its top and bottom.
    #[test]
    fn a_photo_fills_the_centre_less_the_margin_its_aspect_kept() {
        let centre = Rect::from_min_size(pos2(268.0, 40.0), vec2(648.0, 448.0));
        let room = Rect::from_min_size(pos2(292.0, 64.0), vec2(600.0, 400.0));
        let in_middle = |size| Rect::from_center_size(room.center(), size);
        // Smaller than the room, wide and tall: enlarged.
        assert_eq!(drawn([300, 100], centre), in_middle(vec2(600.0, 200.0)));
        assert_eq!(drawn([100, 200], centre), in_middle(vec2(200.0, 400.0)));
        // Larger than the room.
        assert_eq!(drawn([1200, 200], centre), in_middle(vec2(600.0, 100.0)));
        // Of the room's own aspect: the whole room.
        assert_eq!(drawn([300, 200], centre), room);
    }
}
