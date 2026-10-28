// SPDX-License-Identifier: GPL-3.0-or-later

//! When a view of a folder's photos brings the current photo into view:
//! when the current photo changes, when the list does, or when the view
//! comes back on screen. Nothing else moves
//! it: not time passing, and not the view being scrolled, so a
//! photographer looking along the folder is not pulled back (ADR011).
//!
//! Each view keeps what it followed in egui's memory from one frame to
//! the next, under an id of its own, and asks [`follows`] once a frame,
//! before it scrolls. It is told why, too: a view that has just come on
//! screen may move differently from one the photo changed in.
//!
//! A view that glides to the photo, as egui scrolls when asked to show
//! something, keeps where it asked the glide to end ([`gliding`]) and
//! judges what is in view from there ([`glide_end`]) while the glide is
//! under way: egui keeps the end to itself, and a photo in view now may
//! be out of view when the glide ends.

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

/// Why a view brings the current photo into view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The view was not drawn in the pass before: it has just come on
    /// screen, whether or not the photo changed meanwhile.
    Return,
    /// The current photo, or the list, changed since the view was last
    /// drawn, which was the pass before.
    Change,
}

/// Whether the view `id`, drawn in this pass showing the list `list`
/// with the photo at `current` current, is to bring that photo into
/// view, and why: [`Reason::Return`] when the view was not drawn in the
/// pass before, having just come on screen, or [`Reason::Change`] when
/// the photo or the list changed since the pass before; `None` otherwise,
/// and when asked again in the same pass. Remembers what it was asked,
/// for the next pass.
pub fn follows(
    ctx: &egui::Context,
    id: egui::Id,
    list: egui::Id,
    current: Option<usize>,
) -> Option<Reason> {
    let pass = ctx.cumulative_pass_nr();
    let before = ctx.data(|data| data.get_temp::<Followed>(id));
    remember(ctx, id, list, current);
    match before {
        None => Some(Reason::Return),
        Some(before) if before.pass + 1 < pass => Some(Reason::Return),
        Some(before) => {
            (before.list != list || before.current != current).then_some(Reason::Change)
        }
    }
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

/// The longest a glide takes, in seconds, and a little over: egui glides
/// for 0.3 seconds at most. A glide asked for longer ago than this is
/// over, or was overtaken by the user's scrolling.
const LONGEST_GLIDE: f64 = 0.5;

/// Where a view asked egui to glide to, and when, kept in egui's memory
/// from one frame to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Gliding {
    /// The scroll offset the glide ends at.
    to: f32,
    /// egui's time when the glide was asked for.
    since: f64,
}

/// Keep in egui's memory that the view `id` has asked egui for a glide
/// ending with its scroll offset at `to`.
pub fn gliding(ctx: &egui::Context, id: egui::Id, to: f32) {
    let since = ctx.input(|input| input.time);
    ctx.data_mut(|data| data.insert_temp(id.with("glide"), Gliding { to, since }));
}

/// Where the glide the view `id` asked for ends, while it is under way:
/// `at` is the view's scroll offset now, and the glide is under way
/// until the view is within a point of its end, as egui counts arriving,
/// or [`LONGEST_GLIDE`] has passed since it was asked for. Over, it is
/// forgotten, and `None`.
pub fn glide_end(ctx: &egui::Context, id: egui::Id, at: f32) -> Option<f32> {
    let key = id.with("glide");
    let gliding: Option<Gliding> = ctx.data(|data| data.get_temp(key));
    let now = ctx.input(|input| input.time);
    match gliding {
        Some(glide) if (glide.to - at).abs() >= 1.0 && now - glide.since < LONGEST_GLIDE => {
            Some(glide.to)
        }
        Some(_) => {
            ctx.data_mut(|data| data.remove_temp::<Gliding>(key));
            None
        }
        None => None,
    }
}

/// Forget the glide the view `id` asked for, if any: the view has come
/// back on screen where it was put, not where the glide was going.
pub fn glide_over(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|data| data.remove_temp::<Gliding>(id.with("glide")));
}

#[cfg(test)]
mod tests {
    use super::{Reason, follows, glide_end, glide_over, gliding, shown};

    /// What a view does in a pass: drawn showing a list with a current
    /// photo, and that photo then clicked on, or not; or not drawn.
    #[derive(Clone, Copy)]
    enum Pass {
        /// Drawn showing the list and the index of the current photo.
        Drawn(&'static str, usize),
        /// Drawn showing the list and the index of the current photo,
        /// then the photo at the second index clicked on.
        Clicked(&'static str, usize, usize),
        /// Drawn showing the list and the index of the current photo, and
        /// asked again in the same pass, as a view drawn twice would.
        Twice(&'static str, usize),
        /// Not drawn.
        Hidden,
    }

    /// Whether the view follows the current photo in each of a run of
    /// passes, and why, `None` for a pass it was not drawn in.
    fn follows_in(passes: &[Pass]) -> Vec<Option<Option<Reason>>> {
        let ctx = egui::Context::default();
        let id = egui::Id::unique("view");
        passes
            .iter()
            .map(|pass| {
                let mut follow = None;
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let ctx = ui.ctx();
                    match *pass {
                        Pass::Drawn(list, current) => {
                            let list = egui::Id::unique(list);
                            follow = Some(follows(ctx, id, list, Some(current)));
                        }
                        Pass::Clicked(list, current, clicked) => {
                            let list = egui::Id::unique(list);
                            follow = Some(follows(ctx, id, list, Some(current)));
                            shown(ctx, id, list, Some(clicked));
                        }
                        Pass::Twice(list, current) => {
                            let list = egui::Id::unique(list);
                            follows(ctx, id, list, Some(current));
                            follow = Some(follows(ctx, id, list, Some(current)));
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

    /// A view follows when it is first drawn, as it has just come on
    /// screen, when the current photo changes, when the list does, though
    /// the current photo is at the same place in it, and when it comes
    /// back after a pass it was not drawn in, whether or not the photo
    /// changed meanwhile; and not otherwise, nor when asked again in the
    /// same pass.
    #[test]
    fn a_view_follows_only_when_what_it_shows_changes() {
        use Pass::{Drawn, Hidden, Twice};
        use Reason::{Change, Return};
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
            Hidden,
            Drawn("b", 4),
            Twice("b", 5),
        ];
        let followed = [Some(Return), None, Some(Change), None, Some(Change), None]
            .map(Some)
            .into_iter()
            .chain([
                None,
                Some(Some(Return)),
                Some(None),
                None,
                Some(Some(Return)),
                Some(None),
            ]);
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
            [
                Some(Some(Reason::Return)),
                Some(None),
                Some(None),
                Some(Some(Reason::Change))
            ]
        );
    }

    /// Run a pass at egui's time `time`, and what `act` returns in it.
    fn at(
        ctx: &egui::Context,
        time: f64,
        act: impl FnOnce(&egui::Context) -> Option<f32>,
    ) -> Option<f32> {
        let mut end = None;
        let mut act = Some(act);
        let input = egui::RawInput {
            time: Some(time),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| end = act.take().and_then(|act| act(ui.ctx())));
        // No renderer takes the font atlas egui hands it.
        output.textures_delta.clear();
        end
    }

    /// A glide asked for ends where it was asked to, while the view is
    /// not within a point of there and half a second has not passed
    /// since; then it is over and forgotten, as it is when the view comes
    /// back on screen.
    #[test]
    fn a_glide_ends_where_it_was_asked_to_until_the_view_arrives() {
        let ctx = egui::Context::default();
        let id = egui::Id::unique("view");
        let asked = |ctx: &egui::Context, to| {
            gliding(ctx, id, to);
            None
        };
        assert_eq!(at(&ctx, 0.0, |ctx| asked(ctx, 500.0)), None);
        assert_eq!(at(&ctx, 0.1, |ctx| glide_end(ctx, id, 0.0)), Some(500.0));
        assert_eq!(at(&ctx, 0.2, |ctx| glide_end(ctx, id, 120.0)), Some(500.0));
        assert_eq!(
            at(&ctx, 0.3, |ctx| glide_end(ctx, id, 499.5)),
            None,
            "arrived"
        );
        assert_eq!(
            at(&ctx, 0.3, |ctx| glide_end(ctx, id, 0.0)),
            None,
            "forgotten"
        );
        at(&ctx, 1.0, |ctx| asked(ctx, 900.0));
        assert_eq!(at(&ctx, 1.4, |ctx| glide_end(ctx, id, 100.0)), Some(900.0));
        assert_eq!(
            at(&ctx, 1.6, |ctx| glide_end(ctx, id, 100.0)),
            None,
            "too old"
        );
        at(&ctx, 2.0, |ctx| asked(ctx, 300.0));
        at(&ctx, 2.0, |ctx| {
            glide_over(ctx, id);
            None
        });
        assert_eq!(at(&ctx, 2.1, |ctx| glide_end(ctx, id, 0.0)), None, "over");
    }
}
