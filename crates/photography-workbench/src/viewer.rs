// SPDX-License-Identifier: GPL-3.0-or-later

//! The viewer in Library's centre: the current photo alone, as large as
//! the centre allows or zoomed, sharp at the screen's resolution.
//!
//! The photo is fitted in the centre less a margin on every side
//! ([`MARGIN`]), its aspect kept, in the middle, and enlarged when it is
//! smaller than that room, so it always reaches two of the room's sides.
//! It sits straight on the viewer's backdrop with nothing behind it.
//!
//! The pointer zooms it ([`crate::zoom`]) once a decode of the photo has
//! told the size of its largest image, which 100 % shows; until then the
//! photo stays fitted. A click on the photo shows it at 100 %, the point
//! clicked at the room's middle, and a click on it zoomed larger than the
//! room, or at 100 % or past it, fits it again, the photo gliding from one
//! to the other; a double-click is one click ([`counts`]). The mouse
//! wheel, the wheel with Ctrl or Cmd held, and a trackpad's pinch zoom it
//! about the pointer, from the fit up to 800 %, as egui smooths them
//! ([`wheel`]), the wheel's events being read in every frame the viewer
//! is drawn, its photo's 100 % known or not, so what egui smooths of them
//! in the frames after is taken for what it is; a drag, and a trackpad's
//! two fingers, move a photo larger than the room. The keys and the
//! toolbar zoom it too ([`zoom`]): between fitted and 100 %, to either,
//! and in and out by steps, about the room's middle, gliding there as
//! after a click. They act before the viewer is drawn in a frame, in the
//! room it was drawn in during the frame before, and do nothing when it
//! was not drawn then, until it is drawn again ([`State::stage`]); and the
//! toolbar reads the same framing the photo is drawn with
//! ([`State::framing`]), so it never says another zoom than the photo's.
//! Only the room shows the photo, so its margin stays clear however it is
//! zoomed. The zoom and the pan are kept from one photo to the next, and
//! bounded for each, and the pointer shows what a click or a drag on the
//! photo does ([`cursor`]).
//!
//! What is drawn is the photo's picture ([`crate::textures`]): the photo
//! decoded at the long side of that room in the screen's pixels, rounded
//! up to a step of [`STEP`] pixels, so resizing the window does not
//! decode it again at every pixel, and no longer than the largest texture
//! the graphics card takes ([`picture_side`]). A picture takes a while to
//! decode, as it is decoded from one of the larger images the file holds,
//! a RAW file's preview or a JPEG's own image or a large copy of it, so the
//! photo's thumbnail is wanted before it, as a quick stand-in: of the
//! smallest size that covers the room, but no larger than the largest
//! every file decodes quickly, from the smaller preview it carries beside
//! the full-size image ([`stand_in_side`]). While the picture is on its
//! way, the best texture of the photo held is shown in its place, so a
//! photo stepped to shows at once when the filmstrip holds its thumbnail.
//! Once the picture is settled, held or known not to decode, or the photo
//! shown as one that cannot be shown, the pictures of the next photo and
//! of the one before are wanted too, at the same size and below everything
//! on screen, so stepping to either shows it sharp at once; not before, so
//! they never hold up the current one, and a damaged current photo does
//! not keep them from being made ready ([`want_order`]). Once the picture
//! is held, or known not to decode, and the photo is drawn longer than it,
//! in the screen's pixels, and its largest image is longer still, the
//! photo is wanted at its largest too, right after the picture: that image
//! whole, no longer than the largest texture the card takes
//! ([`largest_side`]). Only the current photo is; the picture stands in
//! for it meanwhile, and when it cannot be decoded.
//!
//! When the sharpest texture wanted, the picture or the photo at its
//! largest, has taken half a second ([`LOADING_AFTER`]) since the photo
//! or the size wanted last changed, "Loading…" shows in the margin under
//! the photo until it comes; not when it cannot be decoded and something
//! else of the photo is shown, nor while the photo is shown as one that
//! cannot be shown. A photo that cannot be shown at all shows an icon in
//! the middle, and why in its tooltip, once its thumbnails and its picture
//! at some size are known not to decode, while a size wanted that is not
//! tried yet is still decoded; nothing is drawn until some texture of the
//! photo is decoded or that is known.
//!
//! The viewer draws the photos it is handed, whatever they were read
//! from, and the textures the app holds, and names the photo to screen
//! readers by its file name. It tells the app whether it drew some texture
//! of the photo in the frame, for what the app draws over the photo. What
//! it keeps between frames is its [`State`], which the app holds for it:
//! the zoom and the pan, the glide a click, a key or the toolbar began,
//! what the wheel last was, how long it has waited, and the room it last
//! drew the photo in.

use std::path::{Path, PathBuf};
use std::time::Duration;

use catalog::photos::Photo;
use desktop_ui::theme::Palette;
use egui::{
    Color32, CursorIcon, Event, InputOptions, InputState, Margin, Modifiers, MouseWheelUnit,
    PointerButton, Rect, Sense, Vec2, WidgetInfo, WidgetType, epaint::RectShape, pos2, vec2,
};
use previews::Size;

use crate::textures::{self, CURRENT, CURRENT_PICTURE, NEIGHBOURS, Textures, Thumbnail};
use crate::zoom::{self, Framing, Stage, Zooming};

/// The room kept between the centre's edges and the photo, on every
/// side, so the photo stands clear of the parts of the window around it.
const MARGIN: i8 = 24;

/// The size of the icon shown when the photo cannot be shown.
const FAILED_ICON_SIZE: f32 = 48.0;

/// The step a picture's longest side is rounded up to, in pixels: a
/// window resized by a little keeps the picture it has.
const STEP: u32 = 256;

/// How long the viewer waits for its picture before it says the picture
/// is loading, in seconds: one that comes sooner needs no word, and a
/// word shown only for a moment would flicker.
const LOADING_AFTER: f64 = 0.5;

/// What the viewer says while its picture takes a while.
const LOADING: &str = "Loading…";

/// The natural logarithm of the factor a notch of the mouse wheel zooms
/// by: about 1.22, as much as egui zooms by a notch with Ctrl or Cmd held.
const NOTCH_ZOOM: f32 = 0.2;

/// What the viewer keeps from one frame to the next, held by the app and
/// never saved.
#[derive(Debug, Default)]
pub struct State {
    /// How large the photo is shown, and which part of it: kept from one
    /// photo to the next.
    framing: Framing,
    /// The move a click, a key or the toolbar began, once begun, until
    /// another photo is shown.
    glide: Option<Glide>,
    /// What the wheel is, as its last event said, for what egui smooths of
    /// it in the frames after.
    wheel: Wheel,
    /// egui's time of the last click that zoomed the photo, so the click
    /// that makes it a double-click does not undo it.
    clicked: Option<f64>,
    /// How long the viewer has waited for the sharpest texture it wants.
    waiting: Waiting,
    /// The room the photo was shown in when the viewer was last drawn, in
    /// points, which the keys and the toolbar zoom it in, as they act
    /// before the viewer is drawn in a frame, and the number of the pass
    /// of egui's it was drawn in. `None` until it is first drawn.
    room: Option<(Rect, u64)>,
}

impl State {
    /// Where the photo on `stage` is drawn at egui's time `now`: gliding
    /// to where its framing puts it while a move is under way, and
    /// there, its corner on a whole pixel at 100 %, once it is not.
    fn drawn(&self, stage: &Stage, now: f64) -> Rect {
        let to = stage.place(self.framing);
        match &self.glide {
            Some(glide) if zoom::gliding(glide.since, now) => {
                zoom::glided(glide.from, to, glide.since, now)
            }
            _ => stage.snapped(to),
        }
    }

    /// Whether a move a click, a key or the toolbar began is under way at
    /// egui's time `now`.
    fn gliding(&self, now: f64) -> bool {
        self.glide
            .as_ref()
            .is_some_and(|glide| zoom::gliding(glide.since, now))
    }

    /// How large the photo is shown and which part of it: what the viewer
    /// draws it with, and what the toolbar reads, so the two never
    /// disagree. While a move is under way, where it goes.
    pub fn framing(&self) -> Framing {
        self.framing
    }

    /// The stage of a photo whose largest image is `largest`, in the room
    /// the viewer was last drawn in, on the screen of `ctx`: what the keys
    /// and the toolbar zoom it on. `None` until the viewer is first drawn,
    /// and whenever it was drawn neither in this pass of egui's nor in the
    /// one before, as while the grid is shown and until the viewer is
    /// drawn again, since the window or the docks may have changed
    /// meanwhile, and the room with them; and while the photo's 100 % is
    /// not known, as before any of it is decoded. They zoom nothing then.
    ///
    /// The keys act before the viewer is drawn, so in a frame whose room
    /// changes, as the window is resized or a dock is shown or hidden, a
    /// key pressed then zooms about the middle of the room from the frame
    /// before, and the toolbar shows that room's zoom for that frame, the
    /// viewer asking for the next one ([`show`]). Coming back to the
    /// viewer, the toolbar never shows an old room's zoom: the viewer has
    /// egui run the pass again, so it reads the new room in the same
    /// frame.
    pub fn stage(&self, ctx: &egui::Context, largest: Option<Size>) -> Option<Stage> {
        let (room, pass) = self.room?;
        if ctx.cumulative_pass_nr() > pass + 1 {
            return None;
        }
        Some(Stage::new(largest?, room, ctx.pixels_per_point()))
    }
}

/// Zoom the photo at `path`, whose largest image is `largest`, as
/// `zooming` asks for a key or a button of the toolbar, before the viewer
/// is drawn in the frame of `ctx`: the point of the photo at the room's
/// middle stays there ([`Stage::zoomed`]), the photo gliding there as it
/// does after a click. Nothing while there is no [`State::stage`], nor
/// when the zoom would not change.
pub fn zoom(
    state: &mut State,
    ctx: &egui::Context,
    path: &Path,
    largest: Option<Size>,
    zooming: Zooming,
) {
    let Some(stage) = state.stage(ctx, largest) else {
        return;
    };
    let framing = stage.zoomed(state.framing, zooming);
    if framing == state.framing {
        return;
    }
    // A move of another photo, stepped from in this frame, is over.
    if state.glide.as_ref().is_some_and(|glide| glide.path != path) {
        state.glide = None;
    }
    let now = ctx.input(|i| i.time);
    state.glide = Some(Glide {
        path: path.to_path_buf(),
        from: state.drawn(&stage, now),
        since: now,
    });
    state.framing = framing;
}

/// Whether `zooming` can be asked for of the zoom `shown`: the framing the
/// viewer draws its photo with and the stage it zooms it on, `None` while
/// nothing can be zoomed, outside the viewer or before the photo's 100 %
/// is known, when nothing can. Zooming in and out can only where they
/// would change something. The toolbar's zoom controls and the menu bar's
/// zoom items are enabled by it, so the two never disagree.
pub fn can_zoom(shown: Option<(Framing, Stage)>, zooming: Zooming) -> bool {
    match zooming {
        Zooming::In | Zooming::Out => {
            shown.is_some_and(|(framing, stage)| stage.zoomed(framing, zooming) != framing)
        }
        Zooming::Toggle | Zooming::Fit | Zooming::Full => shown.is_some(),
    }
}

/// What the toolbar says of the zoom shown at `percent` of 100 %, as
/// [`Stage::percent`] reads it, fitted or not: "200 %". Nothing while
/// there is no percent to read, before the photo's 100 % is known.
pub fn zoom_label(percent: Option<u32>) -> String {
    percent.map_or_else(String::new, |percent| format!("{percent} %"))
}

/// The move a click, a key or the toolbar began, from where the photo was
/// drawn then to where the new framing puts it.
#[derive(Debug)]
struct Glide {
    /// The file of the photo it moves: another photo shown ends it.
    path: PathBuf,
    /// Where the photo was drawn when it began.
    from: Rect,
    /// egui's time when it began, in seconds.
    since: f64,
}

/// What the scrolling egui smooths over a few frames comes from, as the
/// last event of the wheel or the trackpad said: egui tells the frame's
/// scrolling in points, whatever it came from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Wheel {
    /// Something the viewer leaves alone, as the wheel with Shift or Alt
    /// held, or nothing yet.
    #[default]
    Other,
    /// A mouse wheel turned by notches, each `notch` points of scrolling,
    /// which zooms.
    Notched {
        /// The points of scrolling egui counts a notch as.
        notch: f32,
    },
    /// A trackpad's two fingers, which scroll smoothly, in points, and
    /// pan.
    Smooth,
}

impl Wheel {
    /// What an event of the wheel or the trackpad in `unit`, with
    /// `modifiers` held, says it is, for egui's `options`, on a screen
    /// `page` points high; `None` for one egui takes as a zoom, held with
    /// its zoom modifier, Ctrl or Cmd, which it counts as a zoom and not
    /// as scrolling, so it says nothing of what the scrolling is.
    fn of(
        unit: MouseWheelUnit,
        modifiers: Modifiers,
        options: &InputOptions,
        page: f32,
    ) -> Option<Self> {
        if modifiers.matches_any(options.zoom_modifier) {
            return None;
        }
        if !modifiers.is_none() {
            return Some(Self::Other);
        }
        Some(match unit {
            MouseWheelUnit::Line => Self::Notched {
                notch: options.line_scroll_speed,
            },
            MouseWheelUnit::Page => Self::Notched { notch: page },
            MouseWheelUnit::Point => Self::Smooth,
        })
    }
}

/// How long the viewer has wanted the sharpest texture it wants of its
/// photo.
#[derive(Debug, Default)]
struct Waiting {
    /// What the viewer last wanted, and since when. `None` until the
    /// viewer is first drawn.
    since: Option<Since>,
}

/// The sharpest texture the viewer wanted when it was last drawn, and
/// since when.
#[derive(Debug)]
struct Since {
    /// The photo's file.
    path: PathBuf,
    /// The longest side of the texture, in pixels: the picture's, or the
    /// photo's at its largest.
    longest: u32,
    /// egui's time when the viewer began wanting it, in seconds.
    at: f64,
    /// The number of the pass of egui's the viewer was last drawn in.
    pass: u64,
}

impl Waiting {
    /// How long, in seconds, the viewer drawn at egui's time `now` in its
    /// pass numbered `pass` has wanted the texture of the photo at `path`
    /// at `longest` pixels: since the photo or the size last changed, or
    /// since the viewer came back on screen, not drawn in the pass before.
    /// Remembers what it was asked, for the next pass.
    fn waited(&mut self, path: &Path, longest: u32, now: f64, pass: u64) -> f64 {
        match &mut self.since {
            Some(since)
                if since.path == path && since.longest == longest && pass <= since.pass + 1 =>
            {
                since.pass = pass;
                now - since.at
            }
            _ => {
                self.since = Some(Since {
                    path: path.to_path_buf(),
                    longest,
                    at: now,
                    pass,
                });
                0.0
            }
        }
    }
}

/// Whether the viewer says its picture is loading.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Loading {
    /// Not, as the picture is not on its way: it is shown, or it cannot
    /// be decoded; or the photo is shown as one that cannot be shown.
    No,
    /// Not yet, but in this many seconds if the picture has not come.
    In(f64),
    /// Now.
    Yes,
}

/// Whether the viewer says its picture is loading, when the picture is
/// `coming`, on its way, and has been wanted for `waited` seconds: once
/// it has been wanted for [`LOADING_AFTER`], and never once it is shown
/// or cannot be decoded.
fn loading(coming: bool, waited: f64) -> Loading {
    if !coming {
        Loading::No
    } else if waited >= LOADING_AFTER {
        Loading::Yes
    } else {
        Loading::In(LOADING_AFTER - waited)
    }
}

/// What the viewer wants decoded of a photo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wanted {
    /// Its thumbnail, the quick stand-in for its picture
    /// ([`stand_in_side`]).
    Thumbnail,
    /// Its picture.
    Picture,
    /// The photo at its largest, this many pixels long.
    Largest(u32),
}

/// The viewer of the photo at `current` of `photos`, filling `ui`, with
/// the textures in `textures`, zoomed and timed as `state` keeps; it says
/// in `palette`'s colours when the picture takes a while. Nothing is
/// drawn when there is no photo at `current`.
///
/// Whether it drew a texture of the photo in this frame: its picture, the
/// photo at its largest, or another texture of it standing in, as its
/// thumbnail does. Then what a decode told of the photo's file is known
/// ([`Textures::details`]); not while nothing of the photo is decoded yet,
/// nor while it is shown as one that cannot be shown, nor without a photo
/// at `current`.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
    textures: &mut Textures,
    state: &mut State,
    photos: &[Photo],
    current: usize,
) -> bool {
    let Some(photo) = photos.get(current) else {
        return false;
    };
    let path = photo.path();
    let centre = ui.max_rect();
    let room = room(centre);
    let pass = ui.ctx().cumulative_pass_nr();
    // The toolbar reads the room before the viewer is drawn.
    match state.room {
        // Drawn in the pass before: the toolbar read this room, or, when it
        // changed, as the window is resized or a dock shown or hidden, the
        // one before, and reads this one in the next frame, which is asked
        // for, so an idle window has it too. Running the pass again
        // instead would draw every frame of a resize twice.
        Some((stored, drawn)) if pass <= drawn + 1 => {
            if stored != room {
                ui.ctx().request_repaint();
            }
        }
        // Drawn again after a pass without the viewer, or for the first
        // time: the toolbar read no room, so egui runs the pass again,
        // once, and it reads this one in the same frame, even when no
        // other frame follows. Run again, the room was drawn in the pass
        // before, and asks for nothing more.
        _ => ui.ctx().request_discard("the viewer is drawn again"),
    }
    state.room = Some((room, pass));
    let points = room.size().max_elem();
    let pixels_per_point = ui.pixels_per_point();
    // Asked every frame: what egui holds is the card's own limit once the
    // integration reports it, and its default until then.
    let max_side = ui.input(|i| i.max_texture_side);
    let longest = picture_side(points, pixels_per_point, max_side);
    let now = ui.input(|i| i.time);
    if state.glide.as_ref().is_some_and(|glide| glide.path != path) {
        state.glide = None;
    }
    let largest = textures.largest(path);
    let stage = largest.map(|largest| Stage::new(largest, room, pixels_per_point));
    let picture = textures.picture(path, longest);
    // A photo shown as one that cannot be shown, its picture failed at
    // another size and its thumbnails too, is settled as one whose picture
    // failed at this size is, though this size is still tried: the viewer
    // says nothing of loading meanwhile, nor holds the neighbours up.
    let cannot = matches!(picture.shown, Thumbnail::Failed(_));

    // Where the photo is before this frame's input, for the input to act
    // on: the pointer zooms nothing until the photo's size is known.
    let before = match &stage {
        Some(stage) => state.drawn(stage, now),
        None => textures::placed(picture.shown, room),
    };
    let sense = match stage {
        Some(_) => Sense::CLICK | Sense::DRAG,
        None => Sense::hover(),
    };
    let name = photo.name().into_owned();
    let id = ui.make_persistent_id("viewer");
    let response = ui.interact(before.intersect(room), id, sense);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, true, &name));
    // Every frame, the stage known or not, so the wheel's last event is
    // known however it came, for the scrolling egui smooths over the
    // frames after it.
    let options = ui.ctx().options(|o| o.input_options);
    let wheeled = ui.input(|i| wheel(i, &options, &mut state.wheel));
    let drawn = stage.map(|stage| {
        steer(ui, &response, &stage, state, path, now, wheeled);
        if response.hovered() || response.dragged() {
            let icon = cursor(&stage, state.framing, response.dragged());
            ui.ctx().set_cursor_icon(icon);
        }
        state.drawn(&stage, now)
    });
    if state.gliding(now) {
        ui.ctx().request_repaint();
    }

    // Once the picture is no longer on its way: held, from before the
    // photos were last read or since, or known not to decode.
    let at_largest = match (largest, drawn) {
        (Some(largest), Some(drawn)) if !picture.coming => {
            let pixels = drawn.size().max_elem() * pixels_per_point;
            largest_side(pixels, longest, largest, max_side)
        }
        _ => None,
    };
    let thumbnail = stand_in_side(points, pixels_per_point);
    let order = want_order(
        current,
        photos.len(),
        picture.decoded,
        picture.failed || cannot,
        at_largest,
    );
    for (index, wanted, priority) in order {
        let side = match wanted {
            Wanted::Thumbnail => thumbnail,
            Wanted::Picture => longest,
            Wanted::Largest(side) => side,
        };
        textures.want(photos[index].path(), side, priority);
    }
    // The sharpest texture wanted, which is shown once it is held.
    let sharpest = at_largest.unwrap_or(longest);
    let shown = match at_largest {
        Some(side) => textures.picture(path, side),
        None => picture,
    };

    // The viewer shows no name, so its tooltip only says why a photo
    // cannot be shown: one naming the photo would cover it whenever the
    // pointer rests on it.
    let failure = match (drawn, shown.shown) {
        (Some(drawn), Thumbnail::Ready { texture, size }) => {
            // As every view draws a texture, but where the zoom puts it,
            // and only what is in the room.
            let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            let at = painted(size, drawn);
            let image = RectShape::filled(at, 0, Color32::WHITE).with_texture(texture, whole);
            ui.painter_at(room).add(image);
            None
        }
        _ => textures::paint(ui, shown.shown, room, FAILED_ICON_SIZE),
    };
    // Drawn where the zoom puts it, or, before the photo's 100 % is
    // known, fitted in the room.
    let drew = matches!(shown.shown, Thumbnail::Ready { .. });
    if let Some(failure) = failure {
        response.on_hover_text(failure.to_string());
    }

    let waited = state.waiting.waited(path, sharpest, now, pass);
    match loading(shown.coming && !cannot, waited) {
        Loading::No => {}
        // A frame then, for an idle window to say it on time.
        Loading::In(seconds) => ui
            .ctx()
            .request_repaint_after(Duration::from_secs_f64(seconds)),
        Loading::Yes => {
            // In the margin under the room, which the photo never
            // reaches, so it covers none of the photo.
            let below = Rect::from_x_y_ranges(room.x_range(), room.bottom()..=centre.bottom());
            let text = egui::RichText::new(LOADING).small().color(palette.text);
            ui.put(below, egui::Label::new(text).selectable(false));
        }
    }
    drew
}

/// Frame the photo on `stage`, of the file at `path`, as the pointer asks
/// in this frame, at egui's time `now`, `response` being the photo's own
/// and `wheeled` what the wheel, a pinch and a trackpad ask ([`wheel`]): a
/// click zooms it in or out, gliding there; a drag moves it while it is
/// larger than the room; and while the pointer is over the viewer, the
/// wheel, a pinch or a trackpad's two fingers zoom it about the pointer or
/// move it. While a move is under way, only another click acts.
fn steer(
    ui: &egui::Ui,
    response: &egui::Response,
    stage: &Stage,
    state: &mut State,
    path: &Path,
    now: f64,
    (factor, scroll): (f32, Vec2),
) {
    let drawn = state.drawn(stage, now);
    if response.clicked_by(PointerButton::Primary)
        && let Some(at) = response.interact_pointer_pos()
    {
        let again = response.double_clicked() || response.triple_clicked();
        let delay = ui.ctx().options(|o| o.input_options.max_double_click_delay);
        if counts(again, state.clicked, now, delay) {
            state.clicked = Some(now);
            state.framing = stage.click(state.framing, drawn, at);
            state.glide = Some(Glide {
                path: path.to_path_buf(),
                from: drawn,
                since: now,
            });
        }
        return;
    }
    if state.gliding(now) {
        return;
    }
    if response.dragged() && stage.larger(state.framing) {
        state.framing = stage.pan_by(state.framing, response.drag_delta());
    }
    if !ui.rect_contains_pointer(ui.max_rect()) {
        return;
    }
    if factor != 1.0 {
        let room = self::room(ui.max_rect());
        // A pinch on a touch screen zooms about its fingers' middle.
        let anchor = ui.input(|i| {
            let touch = i.multi_touch().map(|touch| touch.center_pos);
            touch.or_else(|| i.pointer.hover_pos())
        });
        let anchor = anchor
            .filter(|at| room.contains(*at))
            .unwrap_or_else(|| room.center());
        state.framing = stage.zoom_about(state.framing, factor, anchor);
    }
    if scroll != Vec2::ZERO && stage.larger(state.framing) {
        state.framing = stage.pan_by(state.framing, scroll);
    }
}

/// What the wheel, a pinch and a trackpad ask of the viewer in the frame
/// whose input is `input`, under egui's `options`, `wheel` being what the
/// wheel last was, which this keeps up to date: the factor to zoom by,
/// and how far to move the photo, in points.
///
/// egui counts the wheel with its zoom modifier held, Ctrl or Cmd, and a
/// pinch, in its zoom alone, and smooths the scrolling of the wheel over
/// a few frames, telling it in points, whatever it came from; so the
/// scrolling of a mouse's notches zooms, a notch by [`NOTCH_ZOOM`], and a
/// trackpad's moves the photo, as the wheel's last event says
/// ([`Wheel::of`]). Neither is counted twice.
fn wheel(input: &InputState, options: &InputOptions, wheel: &mut Wheel) -> (f32, Vec2) {
    let page = input.viewport_rect().height();
    for event in &input.events {
        if let Event::MouseWheel {
            unit, modifiers, ..
        } = *event
            && let Some(of) = Wheel::of(unit, modifiers, options, page)
        {
            *wheel = of;
        }
    }
    let zoom = input.zoom_delta();
    let scroll = input.smooth_scroll_delta;
    match *wheel {
        // Up, away from the photographer, zooms in.
        Wheel::Notched { notch } => (zoom * (NOTCH_ZOOM * scroll.y / notch).exp(), Vec2::ZERO),
        Wheel::Smooth => (zoom, scroll),
        Wheel::Other => (zoom, Vec2::ZERO),
    }
}

/// Whether a click at egui's time `now` zooms the photo, `again` when it
/// makes a double- or triple-click of the clicks before it, the last
/// click that zoomed being at `clicked`, egui taking clicks `delay`
/// seconds apart at most as a double-click: a click no part of one does,
/// and one that is only when the click it follows zoomed nothing, as when
/// both came in one frame. So a double-click zooms once.
fn counts(again: bool, clicked: Option<f64>, now: f64, delay: f64) -> bool {
    // egui takes three clicks within twice the delay as a triple-click.
    !again || clicked.is_none_or(|at| now - at > 2.0 * delay)
}

/// The pointer over the photo on `stage` framed by `framing`, `dragged`
/// or not: a closed hand while a drag moves it, an open one while it is
/// larger than the room, and otherwise what a click does, to fit or to
/// 100 %: zooming in when that shows the photo larger, and out when it
/// shows it smaller ([`Stage::click_enlarges`]).
fn cursor(stage: &Stage, framing: Framing, dragged: bool) -> CursorIcon {
    if stage.larger(framing) {
        if dragged {
            CursorIcon::Grabbing
        } else {
            CursorIcon::Grab
        }
    } else if stage.click_enlarges(framing) {
        CursorIcon::ZoomIn
    } else {
        CursorIcon::ZoomOut
    }
}

/// The longest side, in pixels, to decode a photo at its largest at, when
/// it is drawn `drawn` pixels long on the screen, its picture is decoded
/// at `longest`, its largest image is `largest`, and the graphics card
/// takes textures of at most `max_side` pixels a side: that image's long
/// side, no longer than `max_side`, once the photo is drawn longer than
/// its picture, by more than half a pixel, and only if that is longer than
/// the picture. `None` while the picture holds every pixel drawn, or every
/// pixel there is.
fn largest_side(drawn: f32, longest: u32, largest: Size, max_side: usize) -> Option<u32> {
    let max_side = u32::try_from(max_side).unwrap_or(u32::MAX);
    let side = largest.width.max(largest.height).min(max_side);
    (side > longest && drawn > longest as f32 + 0.5).then_some(side)
}

/// Where to draw a texture of `size` pixels of the photo the zoom places
/// at `place`, its largest image's aspect: fitted in it, its own aspect
/// kept, in the middle, as a stand-in decoded from a RAW file's preview of
/// another aspect than the largest is; or the whole place when its own
/// aspect is the largest's but for the rounding of its sides to whole
/// pixels, so a photo at 100 % keeps its corner on a whole pixel. Its
/// short side is rounded by half a pixel at most, which leaves it short of
/// the place's long side by half a pixel times the aspect, long side over
/// short, at most: twice that is taken for rounding. Where the photo is
/// placed is what the pointer and the zoom go by, whatever is drawn.
fn painted(size: [usize; 2], place: Rect) -> Rect {
    let size = vec2(size[0] as f32, size[1] as f32);
    let fitted = textures::fitted(size, place);
    let texel = fitted.width() / size.x;
    let aspect = place.size().max_elem() / place.size().min_elem();
    let short = (place.size() - fitted.size()).max_elem();
    if short <= texel * aspect {
        place
    } else {
        fitted
    }
}

/// Where the photo is fitted in a centre at `centre`: all of it but
/// [`MARGIN`] on every side.
fn room(centre: Rect) -> Rect {
    centre - Margin::same(MARGIN)
}

/// The longest side, in pixels, to decode the thumbnail at that stands in
/// for the picture of a photo fitted in a room whose long side is `points`
/// long, on a screen of `pixels_per_point`: the smallest thumbnail size
/// that covers the room ([`textures::longest`]), but no larger than the
/// largest every camera's file decodes quickly
/// ([`textures::QUICK_THUMBNAIL`]), so the stand-in of a file that
/// carries a smaller preview never waits on the full-size image as the
/// picture may, and a file with no larger preview than that does not
/// decode the full-size image twice over.
fn stand_in_side(points: f32, pixels_per_point: f32) -> u32 {
    textures::longest(points, pixels_per_point).min(textures::QUICK_THUMBNAIL)
}

/// The longest side, in pixels, to decode the picture of a photo at that
/// is fitted in a room whose long side is `points` long, on a screen of
/// `pixels_per_point`, where the graphics card takes textures of at most
/// `max_side` pixels a side: the room's long side in pixels, rounded up to
/// a step of [`STEP`], no longer than `max_side`, and never as short as a
/// thumbnail ([`textures::LARGEST_THUMBNAIL`]), so it is always a picture,
/// at least a step longer than the largest thumbnail. Every card takes
/// textures of 2048 pixels a side, more than that.
pub fn picture_side(points: f32, pixels_per_point: f32, max_side: usize) -> u32 {
    // A negative or not-a-number length is cast to none.
    let pixels = (points * pixels_per_point).ceil() as u32;
    let stepped = pixels.div_ceil(STEP).saturating_mul(STEP);
    let max_side = u32::try_from(max_side).unwrap_or(u32::MAX);
    stepped
        .min(max_side)
        .max(textures::LARGEST_THUMBNAIL + STEP)
}

/// What the viewer of the photo at `current`, of `len` photos, wants
/// decoded, the best first, each with a priority of its own from the
/// highest down ([`textures::ranked`]), when the photo's picture is
/// `decoded` or not, and is known to have `failed` or not, as it is too
/// while the photo is shown as one that cannot be shown, and the photo is
/// wanted at its largest, `at_largest` pixels long, or not.
///
/// Until the picture is decoded, the photo's thumbnail ([`CURRENT`]), the
/// quick stand-in, then its picture ([`CURRENT_PICTURE`]); once it is, the
/// picture alone, kept by being wanted, as the thumbnail is not shown.
/// The photo at its largest comes right after its picture, in the same
/// tier, which stands in for it and is what the fit shows again. Once
/// the picture is settled, decoded or known to have failed, the pictures
/// of the next photo and of the one before ([`NEIGHBOURS`]) come after,
/// as far as there are photos, so stepping to either shows it sharp at
/// once; they are wanted only then, so they never hold up the current
/// one, and a current photo that cannot be decoded does not stop them.
fn want_order(
    current: usize,
    len: usize,
    decoded: bool,
    failed: bool,
    at_largest: Option<u32>,
) -> Vec<(usize, Wanted, u32)> {
    let mut order = Vec::new();
    if !decoded {
        order.push((current, Wanted::Thumbnail, CURRENT));
    }
    order.push((current, Wanted::Picture, CURRENT_PICTURE));
    if let Some(side) = at_largest {
        order.push((current, Wanted::Largest(side), CURRENT_PICTURE));
    }
    if decoded || failed {
        let next = (current + 1 < len).then_some(current + 1);
        for neighbour in [next, current.checked_sub(1)].into_iter().flatten() {
            order.push((neighbour, Wanted::Picture, NEIGHBOURS));
        }
    }
    let tiers = order
        .iter()
        .enumerate()
        .map(|(at, &(_, _, tier))| (at, tier));
    textures::ranked(tiers)
        .into_iter()
        .map(|(at, priority)| (order[at].0, order[at].1, priority))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use egui::{CursorIcon, InputOptions, Modifiers, MouseWheelUnit, Rect, TextureId, pos2, vec2};
    use previews::Size;

    use super::{
        LOADING_AFTER, Loading, Waiting, Wanted, Wheel, counts, cursor, largest_side, loading,
        painted, picture_side, room, stand_in_side, want_order, zoom_label,
    };
    use crate::textures::{self, CURRENT, CURRENT_PICTURE, NEIGHBOURS, Thumbnail, placed};
    use crate::zoom::{Framing, Stage, Zoom};

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

    /// The stand-in is the thumbnail that covers the room, up to 512
    /// pixels, the largest every file decodes quickly.
    #[test]
    fn the_stand_in_is_the_thumbnail_that_covers_the_room_up_to_the_quick_size() {
        assert_eq!(stand_in_side(200.0, 1.0), 256);
        assert_eq!(stand_in_side(300.0, 1.0), 384);
        assert_eq!(stand_in_side(500.0, 1.0), 512);
        assert_eq!(stand_in_side(700.0, 1.0), 512);
        assert_eq!(stand_in_side(1000.0, 2.0), 512);
        assert_eq!(stand_in_side(200.0, 2.0), 512);
    }

    /// A picture is as long as the room's long side in pixels, rounded up
    /// to a step of 256, on a screen of one pixel a point or two; no
    /// longer than the card's largest texture; and always a step longer
    /// than the largest thumbnail, however small the room.
    #[test]
    fn a_picture_is_the_room_in_pixels_rounded_up_to_a_step() {
        assert_eq!(picture_side(1300.0, 1.0, 16384), 1536);
        assert_eq!(picture_side(1536.0, 1.0, 16384), 1536);
        assert_eq!(picture_side(1536.5, 1.0, 16384), 1792);
        assert_eq!(picture_side(1000.0, 2.0, 16384), 2048);
        assert_eq!(picture_side(1100.0, 2.0, 16384), 2304);
        assert_eq!(picture_side(1100.0, 2.0, 2048), 2048);
        assert_eq!(picture_side(9000.0, 2.0, 16384), 16384);
        assert_eq!(picture_side(1000.0, 1.0, 16384), 1280);
        assert_eq!(picture_side(1024.0, 1.0, 16384), 1280);
        assert_eq!(picture_side(1024.5, 1.0, 16384), 1280);
        assert_eq!(picture_side(300.0, 1.0, 16384), 1280);
        assert_eq!(picture_side(0.0, 2.0, 16384), 1280);
        assert!(picture_side(512.0, 1.0, 16384) > textures::LARGEST_THUMBNAIL);
    }

    /// Until the current photo's picture is decoded, the viewer wants its
    /// thumbnail, then the picture; once it is, the picture alone, and
    /// the photo at its largest right after it when that is wanted. Once
    /// the picture is settled, decoded or known to have failed, the next
    /// photo's picture and the one before's come after, as far as there
    /// are photos, and not before. Each is at a priority of its own,
    /// falling, the largest in the picture's tier.
    #[test]
    fn the_neighbours_are_wanted_once_the_picture_is_settled() {
        use Wanted::{Largest, Picture, Thumbnail};
        let wanted = |current, len, decoded, failed| -> Vec<(usize, Wanted)> {
            let order = want_order(current, len, decoded, failed, None);
            assert!(
                order.windows(2).all(|pair| pair[0].2 > pair[1].2),
                "{order:?}"
            );
            order
                .into_iter()
                .map(|(index, of, _)| (index, of))
                .collect()
        };
        assert_eq!(wanted(4, 10, false, false), [(4, Thumbnail), (4, Picture)]);
        assert_eq!(
            wanted(4, 10, true, false),
            [(4, Picture), (5, Picture), (3, Picture)]
        );
        assert_eq!(
            wanted(4, 10, false, true),
            [(4, Thumbnail), (4, Picture), (5, Picture), (3, Picture)]
        );
        assert_eq!(wanted(0, 10, true, false), [(0, Picture), (1, Picture)]);
        assert_eq!(wanted(9, 10, true, false), [(9, Picture), (8, Picture)]);
        assert_eq!(wanted(0, 1, true, false), [(0, Picture)]);
        let priorities = |tiers: &[(usize, u32)]| -> Vec<u32> {
            let ranked = textures::ranked(tiers.iter().copied());
            ranked.into_iter().map(|(_, p)| p).collect()
        };
        let of = |decoded, failed| -> Vec<u32> {
            let order = want_order(4, 10, decoded, failed, None);
            order.into_iter().map(|(_, _, p)| p).collect()
        };
        assert_eq!(
            of(false, false),
            priorities(&[(0, CURRENT), (1, CURRENT_PICTURE)])
        );
        let settled = [(0, CURRENT_PICTURE), (1, NEIGHBOURS), (2, NEIGHBOURS)];
        assert_eq!(of(true, false), priorities(&settled));

        let order = want_order(4, 10, true, false, Some(6000));
        let wanted: Vec<(usize, Wanted)> = order.iter().map(|&(i, of, _)| (i, of)).collect();
        assert_eq!(
            wanted,
            [(4, Picture), (4, Largest(6000)), (5, Picture), (3, Picture)]
        );
        let largest = [
            (0, CURRENT_PICTURE),
            (1, CURRENT_PICTURE),
            (2, NEIGHBOURS),
            (3, NEIGHBOURS),
        ];
        let given: Vec<u32> = order.into_iter().map(|(_, _, p)| p).collect();
        assert_eq!(given, priorities(&largest));
    }

    /// The photo is wanted at its largest once it is drawn longer than its
    /// picture by more than half a pixel, and only when its largest image
    /// is longer than the picture: the image's long side, no longer than
    /// the card's largest texture.
    #[test]
    fn the_largest_is_wanted_once_the_photo_is_drawn_past_its_picture() {
        let wide = Size {
            width: 6000,
            height: 4000,
        };
        assert_eq!(largest_side(1536.0, 1536, wide, 16384), None);
        assert_eq!(largest_side(1536.5, 1536, wide, 16384), None);
        assert_eq!(largest_side(1537.0, 1536, wide, 16384), Some(6000));
        assert_eq!(largest_side(6000.0, 1536, wide, 4096), Some(4096));
        let tall = Size {
            width: 4000,
            height: 6000,
        };
        assert_eq!(largest_side(3000.0, 1536, tall, 16384), Some(6000));
        // The picture holds the whole of a photo no longer than it.
        let small = Size {
            width: 1200,
            height: 800,
        };
        assert_eq!(largest_side(2400.0, 1536, small, 16384), None);
        assert_eq!(largest_side(9000.0, 2048, wide, 2048), None);
    }

    /// A click zooms unless it makes a double- or a triple-click of the
    /// click before it that zoomed; one that makes a double-click of a
    /// click that zoomed nothing, as when both came in one frame, zooms.
    #[test]
    fn a_double_click_zooms_once() {
        let delay = 0.3;
        assert!(counts(false, None, 10.0, delay));
        assert!(counts(false, Some(9.9), 10.0, delay));
        assert!(!counts(true, Some(9.9), 10.0, delay), "a double-click");
        assert!(!counts(true, Some(9.5), 10.0, delay), "a triple-click");
        assert!(counts(true, None, 10.0, delay), "both in one frame");
        assert!(counts(true, Some(5.0), 10.0, delay), "both in one frame");
    }

    /// The pointer over the photo is a zoom-in while a click shows it
    /// larger, at 100 % or fitted, a zoom-out while a click shows it
    /// smaller and it is no larger than the room, an open hand while it is
    /// larger, and a closed one while a drag moves it.
    #[test]
    fn the_pointer_shows_what_a_click_or_a_drag_does() {
        let room = Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 400.0));
        let stage = |width, height| Stage::new(Size { width, height }, room, 1.0);
        let at = |scale| Framing {
            zoom: Zoom::Scale(scale),
            ..Framing::default()
        };
        let large = stage(3000, 2000);
        assert_eq!(
            cursor(&large, Framing::default(), false),
            CursorIcon::ZoomIn
        );
        assert_eq!(cursor(&large, at(1.0), false), CursorIcon::Grab);
        assert_eq!(cursor(&large, at(1.0), true), CursorIcon::Grabbing);
        // Fitted at 200 %: a click shows it at 100 %, smaller, and one at
        // 100 % or past it, no larger than the room, fits it, larger.
        let small = stage(300, 200);
        assert_eq!(
            cursor(&small, Framing::default(), false),
            CursorIcon::ZoomOut
        );
        assert_eq!(cursor(&small, at(1.0), false), CursorIcon::ZoomIn);
        assert_eq!(cursor(&small, at(1.0), true), CursorIcon::ZoomIn);
        assert_eq!(cursor(&small, at(1.5), false), CursorIcon::ZoomIn);
        assert_eq!(cursor(&small, at(0.5), false), CursorIcon::ZoomIn);
    }

    /// A texture is drawn fitted in the place the zoom puts the photo, its
    /// own aspect kept, in the middle, as a RAW file's preview of another
    /// aspect standing in for its largest is; one of the largest's aspect
    /// but for the rounding of its sides fills the place.
    #[test]
    fn a_texture_of_another_aspect_keeps_its_own_in_the_photo_s_place() {
        // A 668 by 432 preview of a photo whose largest is 5472 by 3648,
        // at 100 %.
        let place = Rect::from_min_size(pos2(-100.0, -50.0), vec2(5472.0, 3648.0));
        let drawn = painted([668, 432], place);
        let aspect = drawn.width() / drawn.height();
        assert!((aspect - 668.0 / 432.0).abs() < 1e-4, "{drawn:?}");
        assert_eq!(drawn.width(), place.width(), "{drawn:?}");
        assert!(
            (drawn.center() - place.center()).length() < 0.01,
            "{drawn:?}"
        );
        // The largest decoded at 2048 pixels, 2048 by 1365 of 2400 by 1600.
        let place = Rect::from_min_size(pos2(10.0, 20.0), vec2(2400.0, 1600.0));
        assert_eq!(painted([2048, 1365], place), place);
        assert_eq!(painted([2400, 1600], place), place);
        // A panorama of 8000 by 1999 decoded at 2048 pixels: 2048 by 512,
        // short of its place across by a texel and more.
        let place = Rect::from_min_size(pos2(0.0, 0.0), vec2(8000.0, 1999.0));
        assert_eq!(painted([2048, 512], place), place);
    }

    /// A mouse wheel's notches, in lines or in pages, with nothing held,
    /// zoom, a notch being the points egui scrolls a line by, or a page;
    /// a trackpad's scrolling, in points, moves the photo; with Shift or
    /// Alt held neither does; and the wheel with Ctrl or Cmd held, which
    /// egui counts as a zoom alone, says nothing of the scrolling.
    #[test]
    fn the_wheel_s_last_event_says_what_its_scrolling_does() {
        let options = InputOptions::default();
        let of = |unit, modifiers| Wheel::of(unit, modifiers, &options, 800.0);
        let line = options.line_scroll_speed;
        assert_eq!(
            of(MouseWheelUnit::Line, Modifiers::NONE),
            Some(Wheel::Notched { notch: line })
        );
        assert_eq!(
            of(MouseWheelUnit::Page, Modifiers::NONE),
            Some(Wheel::Notched { notch: 800.0 })
        );
        assert_eq!(
            of(MouseWheelUnit::Point, Modifiers::NONE),
            Some(Wheel::Smooth)
        );
        for unit in [MouseWheelUnit::Line, MouseWheelUnit::Point] {
            assert_eq!(of(unit, Modifiers::SHIFT), Some(Wheel::Other), "{unit:?}");
            assert_eq!(of(unit, Modifiers::ALT), Some(Wheel::Other), "{unit:?}");
            assert_eq!(of(unit, Modifiers::COMMAND), None, "{unit:?}");
        }
    }

    /// The toolbar reads the zoom as a whole percent, with a space before
    /// the sign, and says nothing with no percent to read.
    #[test]
    fn the_zoom_reads_as_a_whole_percent() {
        assert_eq!(zoom_label(Some(100)), "100 %");
        assert_eq!(zoom_label(Some(25)), "25 %");
        assert_eq!(zoom_label(Some(800)), "800 %");
        assert_eq!(zoom_label(None), "");
    }

    /// The wait for a picture runs from when its photo or its size last
    /// changed, or the viewer last came back on screen, not drawn in the
    /// pass before.
    #[test]
    fn the_wait_runs_from_a_change_of_photo_or_size_or_the_viewer_coming_back() {
        let (a, b) = (Path::new("a.jpg"), Path::new("b.jpg"));
        let mut waiting = Waiting::default();
        assert_eq!(waiting.waited(a, 1024, 10.0, 1), 0.0);
        assert_eq!(waiting.waited(a, 1024, 10.25, 2), 0.25);
        assert_eq!(waiting.waited(a, 1024, 10.5, 3), 0.5);
        assert_eq!(waiting.waited(b, 1024, 11.0, 4), 0.0, "another photo");
        assert_eq!(waiting.waited(b, 1024, 11.25, 5), 0.25);
        assert_eq!(waiting.waited(b, 1280, 11.5, 6), 0.0, "another size");
        assert_eq!(waiting.waited(b, 1280, 12.0, 7), 0.5);
        assert_eq!(waiting.waited(b, 1280, 13.0, 9), 0.0, "back on screen");
    }

    /// The viewer says its picture is loading once the picture has been on
    /// its way for half a second, asking meanwhile for a frame when that
    /// will be; never when it is not on its way, shown or failed.
    #[test]
    fn loading_shows_after_half_a_second_while_the_picture_is_on_its_way() {
        assert_eq!(loading(true, 0.0), Loading::In(LOADING_AFTER));
        assert_eq!(loading(true, 0.375), Loading::In(0.125));
        assert_eq!(loading(true, 0.5), Loading::Yes);
        assert_eq!(loading(true, 3.0), Loading::Yes);
        assert_eq!(loading(false, 0.0), Loading::No);
        assert_eq!(loading(false, 3.0), Loading::No);
    }
}
