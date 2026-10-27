// SPDX-License-Identifier: GPL-3.0-or-later

//! When a view of a folder's photos brings the current photo into view:
//! when the current photo changes, when the list does, or when the view
//! comes back on screen. Nothing else moves
//! it: not time passing, and not the view being scrolled, so a
//! photographer looking along the folder is not pulled back (ADR011).
//!
//! Each view keeps what it followed in egui's memory from one frame to
//! the next, under an id of its own, and asks [`follows`] once a frame,
//! before it scrolls.

/// What a view last brought into view, and when it was last drawn, kept
/// in egui's memory from one frame to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Followed {
    /// Which list of photos it was: a hash of the folder they are of,
    /// and of what else the view takes as a new list to follow, as the
    /// grid does the size chosen for its tiles.
    list: egui::Id,
    /// The index of the current photo.
    current: Option<usize>,
    /// The number of the pass of egui's it was drawn in.
    pass: u64,
}

/// Whether the view `id`, drawn in this pass showing the list `list`
/// with the photo at `current` current, is to bring that photo into
/// view: when the photo or the list changed since the view was last
/// drawn, or the view was not drawn in the pass before, having just come
/// on screen. Remembers what it was asked, for the next pass.
pub fn follows(ctx: &egui::Context, id: egui::Id, list: egui::Id, current: Option<usize>) -> bool {
    let pass = ctx.cumulative_pass_nr();
    let before = ctx.data(|data| data.get_temp::<Followed>(id));
    remember(ctx, id, list, current);
    before.is_none_or(|before| {
        before.list != list || before.current != current || before.pass + 1 < pass
    })
}

/// Tell the view `id`, drawn in this pass showing the list `list`, that
/// the photo at `current` is current and in view already, so the next
/// pass does not bring it into view: a photo made current by a click on
/// it, which is where the pointer is.
pub fn shown(ctx: &egui::Context, id: egui::Id, list: egui::Id, current: Option<usize>) {
    remember(ctx, id, list, current);
}

/// Keep in egui's memory, for the view `id`, that it was drawn in this
/// pass showing the list `list` with the photo at `current` current.
fn remember(ctx: &egui::Context, id: egui::Id, list: egui::Id, current: Option<usize>) {
    let now = Followed {
        list,
        current,
        pass: ctx.cumulative_pass_nr(),
    };
    ctx.data_mut(|data| data.insert_temp(id, now));
}

#[cfg(test)]
mod tests {
    use super::{follows, shown};

    /// What a view does in a pass: drawn showing a list with a current
    /// photo, and that photo then clicked on, or not; or not drawn.
    #[derive(Clone, Copy)]
    enum Pass {
        /// Drawn showing the list and the index of the current photo.
        Drawn(&'static str, usize),
        /// Drawn showing the list and the index of the current photo,
        /// then the photo at the second index clicked on.
        Clicked(&'static str, usize, usize),
        /// Not drawn.
        Hidden,
    }

    /// Whether the view follows the current photo in each of a run of
    /// passes, `None` for a pass it was not drawn in.
    fn follows_in(passes: &[Pass]) -> Vec<Option<bool>> {
        let ctx = egui::Context::default();
        let id = egui::Id::new("view");
        passes
            .iter()
            .map(|pass| {
                let mut follow = None;
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let ctx = ui.ctx();
                    match *pass {
                        Pass::Drawn(list, current) => {
                            let list = egui::Id::new(list);
                            follow = Some(follows(ctx, id, list, Some(current)));
                        }
                        Pass::Clicked(list, current, clicked) => {
                            let list = egui::Id::new(list);
                            follow = Some(follows(ctx, id, list, Some(current)));
                            shown(ctx, id, list, Some(clicked));
                        }
                        Pass::Hidden => {}
                    }
                });
                // No renderer takes the font atlas egui hands it.
                output.textures_delta.clear();
                follow
            })
            .collect()
    }

    /// A view follows when it is first drawn, when the current photo
    /// changes, when the list does, though the current photo is at the
    /// same place in it, and when it comes back after a pass it was not
    /// drawn in; and not otherwise.
    #[test]
    fn a_view_follows_only_when_what_it_shows_changes() {
        use Pass::{Drawn, Hidden};
        let passes = [
            Drawn("a", 0),
            Drawn("a", 0),
            Drawn("a", 3),
            Drawn("a", 3),
            Drawn("b", 3),
            Drawn("b", 3),
            Hidden,
            Drawn("b", 3),
            Drawn("b", 3),
        ];
        let followed = [true, false, true, false, true, false]
            .map(Some)
            .into_iter()
            .chain([None, Some(true), Some(false)]);
        assert_eq!(follows_in(&passes), followed.collect::<Vec<_>>());
    }

    /// A photo made current by a click on it is not followed in the next
    /// pass, as it is where the pointer is; a photo made current after it
    /// is.
    #[test]
    fn a_photo_clicked_on_is_not_followed() {
        use Pass::{Clicked, Drawn};
        let passes = [
            Drawn("a", 0),
            Clicked("a", 0, 5),
            Drawn("a", 5),
            Drawn("a", 6),
        ];
        assert_eq!(
            follows_in(&passes),
            [Some(true), Some(false), Some(false), Some(true)]
        );
    }
}
