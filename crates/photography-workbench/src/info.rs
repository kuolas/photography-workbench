// SPDX-License-Identifier: GPL-3.0-or-later

//! The Info dock, what the right dock shows in Library: the current
//! photo's information, as a decode of its file told it
//! ([`crate::textures::Details`]).
//!
//! The information is in three boxes, each headed by what it holds. File
//! Information says the file's name, its size on disk and its format.
//! Image Information says when the photo was taken, the size of its
//! image, which way it is laid out and, for a RAW file, the largest
//! preview it holds, which is what 100 % shows, and how many it holds.
//! Shooting Information says what took the photo and how: the camera,
//! the lens and the exposure. A JPEG file's EXIF block is not read, so
//! for one that box says the camera's data is read from RAW files only.
//! Each row reads "Label: value", the value at the box's right edge, cut
//! short with "…" where it does not fit and whole under the pointer then;
//! a value the file does not record is "—".
//!
//! The details come with every texture of the photo. Every frame the
//! dock is shown, once every view of the photos has wanted what it
//! draws, the dock wants a texture of the current photo itself
//! ([`want`]): the smallest held, so it is never let go while the dock
//! shows what it told; or, with none held, the photo's smallest
//! thumbnail, as highly as a view wants the current photo, but only when
//! no view wanted any texture of the photo in the frame, as a view's
//! decode brings the details too. So they come even when no view draws
//! that photo, and cost no decode of their own when a view wants it,
//! but that a texture held from before the photos were last read is
//! decoded again when the dock wants it, as when a view does.
//! Until they come, and once the photo cannot be shown, whatever a decode
//! before told, File Information says what the photo's name and
//! extension tell, and under it a line says the file is being read, or
//! why the photo cannot be shown.
//!
//! Each value is put in words by a function of its own, with no egui in
//! it, which the panel over the viewer's photo ([`crate::overlay`]) writes
//! its values with too. How capture times are written ([`TimeForm`]) and how
//! file sizes are counted ([`SizeUnits`]) are chosen in Settings and saved
//! between runs; each is a parameter of its one function, passed in every
//! frame, so a change shows at once.

use std::fmt::Write as _;

use catalog::photos::Photo;
use desktop_ui::theme::Palette;
use desktop_ui::{dock, fonts};
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align2, CornerRadius, FontFamily, FontId, Frame, Label, Margin, Rect, RichText, Sense, Stroke,
    StrokeKind, WidgetInfo, WidgetType, pos2, vec2,
};
use previews::Size;
use raw_files::{DateTime, Lens, Metadata, Rational, SignedRational};

use crate::textures::{self, CURRENT, Details, SMALLEST_THUMBNAIL, Textures, Thumbnail};

/// The room between the dock's edges and the boxes.
const PADDING: Margin = Margin::same(10);

/// The room between one box and the next.
const BOX_GAP: f32 = 12.0;

/// The room inside a box, between its border and its rows; at its top,
/// under the heading, which sits on the border.
const BOX_MARGIN: Margin = Margin {
    left: 8,
    right: 8,
    top: 4,
    bottom: 8,
};

/// The radius of a box's corners.
const BOX_RADIUS: u8 = 3;

/// The width of a box's border.
const BORDER: f32 = 1.0;

/// The size of a box's heading, in the semibold weight.
const HEADING_TEXT: f32 = 13.0;

/// How far a box's heading stands in from the box's left edge.
const HEADING_INSET: f32 = 8.0;

/// The room kept clear of the border at either side of a heading.
const HEADING_ROOM: f32 = 4.0;

/// The height of a row.
const ROW_HEIGHT: f32 = 19.0;

/// The size of a row's text, and of a note in place of rows.
const ROW_TEXT: f32 = 12.5;

/// The least room between a row's label and its value.
const VALUE_GAP: f32 = 12.0;

/// What stands for a value the file does not record.
pub(crate) const NONE: &str = "—";

/// How a capture time is written, as chosen in Settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeForm {
    /// The date first and year first, as ISO 8601 orders them, then the
    /// time of day and the offset from UTC: "2026-08-15 10:20:30.85
    /// UTC+02:00".
    #[default]
    Numeric,
    /// The date in words, then the time: "15 August 2026 at 10:20:30.85
    /// (UTC+02:00)".
    Long,
}

impl TimeForm {
    /// Every form, in the order Settings lists them.
    pub const ALL: [Self; 2] = [Self::Numeric, Self::Long];

    /// The form's name, as Settings lists it, which is also the saved
    /// value.
    pub fn name(self) -> &'static str {
        match self {
            Self::Numeric => "Numeric",
            Self::Long => "Long",
        }
    }

    /// The form saved as `name`, its [`Self::name`]; `None` when `name` is
    /// no form.
    pub fn from_saved(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|form| form.name() == name)
    }
}

/// How a file's size is counted, as chosen in Settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SizeUnits {
    /// In thousands: a kilobyte is 1,000 bytes, "52.3 MB".
    #[default]
    Decimal,
    /// In powers of 1,024: a kibibyte is 1,024 bytes, "49.9 MiB".
    Binary,
}

impl SizeUnits {
    /// Every kind of unit, in the order Settings lists them.
    pub const ALL: [Self; 2] = [Self::Decimal, Self::Binary];

    /// The units' name, as Settings lists it, which is also the saved
    /// value.
    pub fn name(self) -> &'static str {
        match self {
            Self::Decimal => "Decimal",
            Self::Binary => "Binary",
        }
    }

    /// The units saved as `name`, their [`Self::name`]; `None` when `name`
    /// is none.
    pub fn from_saved(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|units| units.name() == name)
    }

    /// Each unit's symbol, how many bytes it holds, and how many decimals
    /// a size in it is written with, smallest first.
    fn table(self) -> [(&'static str, f64, usize); 4] {
        match self {
            Self::Decimal => [
                ("B", 1.0, 0),
                ("KB", 1e3, 0),
                ("MB", 1e6, 1),
                ("GB", 1e9, 1),
            ],
            Self::Binary => [
                ("B", 1.0, 0),
                ("KiB", 1024.0, 0),
                ("MiB", 1_048_576.0, 1),
                ("GiB", 1_073_741_824.0, 1),
            ],
        }
    }
}

/// The Info dock, filling `ui`: what is known of `photo`, the current
/// photo, as the decodes in `textures` told it, its capture time written
/// in `times` and its size on disk counted in `sizes`, or a message saying
/// no photo is selected. It wants nothing decoded: [`want`] does, once
/// the views have wanted what they draw.
pub fn show(
    ui: &mut egui::Ui,
    palette: &Palette,
    textures: &mut Textures,
    photo: Option<&Photo>,
    times: TimeForm,
    sizes: SizeUnits,
) {
    let Some(photo) = photo else {
        dock::empty_message(
            ui,
            "No photo selected",
            "Select a photo to see its information",
        );
        return;
    };
    let path = photo.path();
    // Whether the photo cannot be shown, at the size `want` wants it at,
    // or, with no texture of it held, at any thumbnail's size a view
    // wanted; reading it wants nothing.
    let longest = textures.smallest_held(path).unwrap_or(SMALLEST_THUMBNAIL);
    let failure = match textures.thumbnail(path, longest) {
        Thumbnail::Failed(failure) => Some(failure),
        Thumbnail::Pending | Thumbnail::Ready { .. } => None,
    };
    // Once the photo cannot be shown, as the views show it, what a decode
    // before told of its file, still held, is not shown either.
    let details = match failure {
        Some(_) => None,
        None => textures.details(path),
    };
    Frame::NONE.inner_margin(PADDING).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        group(ui, palette, "File Information", |ui| {
            file_rows(ui, palette, photo, details, sizes);
        });
        let Some(details) = details else {
            ui.add_space(BOX_GAP);
            let reading =
                failure.map_or_else(|| "Reading…".to_owned(), |failure| failure.to_string());
            note(ui, &reading);
            return;
        };
        ui.add_space(BOX_GAP);
        group(ui, palette, "Image Information", |ui| {
            image_rows(ui, palette, details, times);
        });
        ui.add_space(BOX_GAP);
        group(ui, palette, "Shooting Information", |ui| {
            match &details.metadata {
                Some(metadata) => shooting_rows(ui, palette, metadata),
                None => note(ui, "Camera data is read from RAW files only."),
            }
        });
    });
}

/// What the dock wants of `textures` for `photo`, the current photo, in a
/// frame it is shown in, after every view of the photos has wanted what
/// it draws and before the frame ends ([`Textures::end_frame`]). Any
/// texture of the photo brings its details, so the smallest held is
/// wanted, which keeps it while the dock shows what it told; with none
/// held, the smallest thumbnail, at the current photo's tier, unless a
/// view wanted a texture of the photo in this frame, whose decode brings
/// the details too. Nothing without a current photo.
pub fn want(textures: &mut Textures, photo: Option<&Photo>) {
    let Some(photo) = photo else {
        return;
    };
    let path = photo.path();
    let priority = textures::ranked([(0, CURRENT)])[0].1;
    if let Some(smallest) = textures.smallest_held(path) {
        textures.want(path, smallest, priority);
    } else if !textures.is_wanted(path) {
        textures.want(path, SMALLEST_THUMBNAIL, priority);
    }
}

/// File Information's rows: the file's name, its size on disk in `units`,
/// "—" while `details` are not known, and its format.
fn file_rows(
    ui: &mut egui::Ui,
    palette: &Palette,
    photo: &Photo,
    details: Option<&Details>,
    units: SizeUnits,
) {
    let bytes = details.map_or_else(
        || NONE.to_owned(),
        |details| file_size(details.bytes, units),
    );
    row(ui, palette, "File name", &photo.name());
    row(ui, palette, "Size on disk", &bytes);
    row(ui, palette, "Format", &photo.format());
}

/// Image Information's rows: when the photo was taken, written in `form`,
/// the size of its image, for a RAW file the size of its largest preview
/// and how many it holds, and which way it is laid out.
fn image_rows(ui: &mut egui::Ui, palette: &Palette, details: &Details, form: TimeForm) {
    let metadata = details.metadata.as_ref();
    let captured = metadata.and_then(Metadata::capture_time);
    let captured = captured.map_or_else(|| NONE.to_owned(), |time| capture_time(time, form));
    row(ui, palette, "Capture time", &captured);
    row(ui, palette, "Image size", &pixel_size(image_size(details)));
    if metadata.is_some() {
        let largest = format!("{} (100 %)", pixel_size(details.largest));
        row(ui, palette, "Preview size", &largest);
        row(ui, palette, "Previews", &details.previews.len().to_string());
    }
    row(ui, palette, "Orientation", orientation(details.largest));
}

/// Shooting Information's rows: the camera, the lens and the exposure, as
/// `metadata` records them.
fn shooting_rows(ui: &mut egui::Ui, palette: &Palette, metadata: &Metadata) {
    let shot = metadata.exposure();
    let rows = [
        ("Make", metadata.make().map(str::to_owned)),
        ("Model", metadata.model().map(str::to_owned)),
        ("Lens", metadata.lens().and_then(lens)),
        ("Focal length", shot.focal_length().map(focal_length)),
        ("Exposure", exposure(shot.exposure_time(), shot.f_number())),
        ("ISO", shot.iso().map(iso)),
        ("Exposure bias", shot.exposure_bias().map(compensation)),
    ];
    for (label, value) in rows {
        row(ui, palette, label, value.as_deref().unwrap_or(NONE));
    }
}

/// A box headed `heading`, holding what `add_contents` adds.
///
/// The heading sits on the box's top border, near its left corner, the
/// border broken behind it by a strip of the dock's own fill, as a group
/// box's is: its height is kept above the box, the border drawn from the
/// heading's middle down, then the strip and the heading over it.
fn group(
    ui: &mut egui::Ui,
    palette: &Palette,
    heading: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let font = FontId::new(HEADING_TEXT, FontFamily::Name(fonts::SEMIBOLD.into()));
    let galley = ui
        .painter()
        .layout_no_wrap(heading.to_owned(), font, palette.text);
    let size = vec2(ui.available_width(), galley.size().y);
    let (top, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, heading));
    let inside = Frame::NONE.inner_margin(BOX_MARGIN).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add_contents(ui);
    });
    let border = Rect::from_min_max(
        pos2(top.left(), top.center().y),
        inside.response.rect.right_bottom(),
    );
    let painter = ui.painter();
    let stroke = Stroke::new(BORDER, palette.separator);
    painter.rect_stroke(
        border,
        CornerRadius::same(BOX_RADIUS),
        stroke,
        StrokeKind::Inside,
    );
    let at = pos2(top.left() + HEADING_INSET, top.top());
    let behind = Rect::from_min_size(at, galley.size()).expand2(vec2(HEADING_ROOM, 0.0));
    painter.rect_filled(behind, 0.0, ui.visuals().panel_fill);
    painter.galley(at, galley, palette.text);
}

/// A row: `label` with a colon at the left, and `value` at the right,
/// cut short with "…" where it would come nearer the label than a little
/// room, whole under the pointer then.
///
/// The row is one widget, which screen readers are told of as "label:
/// value", so the file's name in its row is not taken for another thing
/// named as the photo is, such as its tile in the grid.
fn row(ui: &mut egui::Ui, palette: &Palette, label: &str, value: &str) {
    let size = vec2(ui.available_width(), ROW_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response
        .widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, format!("{label}: {value}")));
    let font = FontId::proportional(ROW_TEXT);
    let painter = ui.painter();
    let middle = rect.center().y;
    let at = pos2(rect.left(), middle);
    let label = painter.text(
        at,
        Align2::LEFT_CENTER,
        format!("{label}:"),
        font.clone(),
        palette.text,
    );
    let mut job =
        LayoutJob::single_section(value.to_owned(), TextFormat::simple(font, palette.text));
    job.wrap = TextWrapping::truncate_at_width((rect.right() - label.right() - VALUE_GAP).max(0.0));
    let galley = painter.layout_job(job);
    let elided = galley.elided;
    let at = pos2(
        rect.right() - galley.size().x,
        middle - galley.size().y / 2.0,
    );
    painter.galley(at, galley, palette.text);
    if elided {
        response.on_hover_text(value);
    }
}

/// A line of `text`, dimmed, in place of rows or under a box.
fn note(ui: &mut egui::Ui, text: &str) {
    let text = RichText::new(text).size(ROW_TEXT).weak();
    ui.add(Label::new(text).selectable(false).wrap());
}

/// The size of the photo's image, upright: for a RAW file the visible
/// image the camera recorded, or its largest preview's where it records
/// none; for a JPEG file its own.
pub(crate) fn image_size(details: &Details) -> Size {
    match &details.metadata {
        Some(metadata) => metadata.image_size().map_or(details.largest, |size| {
            upright(size, metadata.orientation().swaps_dimensions())
        }),
        None => details.largest,
    }
}

/// `size` as stored, turned upright: its sides swapped when the
/// orientation `swaps` them.
fn upright(size: raw_files::Size, swaps: bool) -> Size {
    let (width, height) = if swaps {
        (size.height, size.width)
    } else {
        (size.width, size.height)
    };
    Size { width, height }
}

/// A file's size on disk in `units`: in decimal units, a kilobyte being
/// 1,000 bytes, "812 KB" in whole kilobytes, "52.3 MB" and "1.2 GB" to a
/// tenth, "512 B" in bytes; in binary units, a kibibyte being 1,024 bytes,
/// "812 KiB", "49.9 MiB" and "1.2 GiB" alike. A size is written in the
/// next unit up once, rounded in its own, it would read as one of the
/// next: from 1,000 of a decimal unit, from 1,024 of a binary one.
pub fn file_size(bytes: u64, units: SizeUnits) -> String {
    let table = units.table();
    let bytes = bytes as f64;
    let rounded = |(_, scale, decimals): (&str, f64, usize)| {
        let step = 10_f64.powi(decimals as i32);
        (bytes / scale * step).round() / step
    };
    let mut unit = table
        .iter()
        .rposition(|&(_, scale, _)| bytes >= scale)
        .unwrap_or(0);
    if let Some(&(_, next, _)) = table.get(unit + 1)
        && rounded(table[unit]) >= next / table[unit].1
    {
        unit += 1;
    }
    let (name, scale, decimals) = table[unit];
    format!("{:.*} {name}", decimals, bytes / scale)
}

/// A size in pixels, width first: "6000 × 4000".
pub fn pixel_size(size: Size) -> String {
    format!("{} × {}", size.width, size.height)
}

/// Which way an image of `size` is laid out: "Landscape" when it is
/// wider than tall, "Portrait" when taller than wide, "Square" when
/// neither.
pub fn orientation(size: Size) -> &'static str {
    match size.width.cmp(&size.height) {
        std::cmp::Ordering::Greater => "Landscape",
        std::cmp::Ordering::Less => "Portrait",
        std::cmp::Ordering::Equal => "Square",
    }
}

/// An exposure time: "1/320 s" for a fraction of a second whose
/// numerator is 1 in lowest terms; "1/437 s" for any other time shorter
/// than a quarter of a second, one over its reciprocal rounded to a whole
/// number, as 5/2183 s is; and otherwise in seconds to a tenth, with no
/// ".0" for a whole number: "0.3 s", "0.6 s", "1.5 s", "2 s", "30 s".
/// "—" for a fraction with a zero denominator, which is no time.
pub fn shutter_speed(time: Rational) -> String {
    let time = time.reduced();
    if time.denominator == 0 {
        return NONE.to_owned();
    }
    let seconds = time.to_f64();
    if time.numerator == 1 && time.denominator > 1 {
        format!("1/{} s", time.denominator)
    } else if time.numerator != 0 && seconds < 0.25 {
        let reciprocal = f64::from(time.denominator) / f64::from(time.numerator);
        format!("1/{} s", reciprocal.round())
    } else {
        format!("{} s", decimal(seconds, 1))
    }
}

/// An f-number: "f/2.8", "f/9", to a tenth. "—" for a fraction with a
/// zero denominator.
pub fn aperture(f_number: Rational) -> String {
    tenths(f_number).map_or_else(|| NONE.to_owned(), |number| format!("f/{number}"))
}

/// A sensitivity, as its ISO number: "ISO 64"; "ISO 65535 or more" for
/// 65535, which a file records for any sensitivity from 65535 up.
pub fn iso(iso: u32) -> String {
    if iso == 65_535 {
        "ISO 65535 or more".to_owned()
    } else {
        format!("ISO {iso}")
    }
}

/// How a photo was exposed, in one line: the exposure `time` at the
/// aperture `f_number`, "1/2000 s at f/8", or the one of them known,
/// "1/2000 s" or "f/8". `None` when neither is.
pub fn exposure(time: Option<Rational>, f_number: Option<Rational>) -> Option<String> {
    let known = |text: String| (text != NONE).then_some(text);
    let time = time.map(shutter_speed).and_then(known);
    let f_number = f_number.map(aperture).and_then(known);
    match (time, f_number) {
        (Some(time), Some(f_number)) => Some(format!("{time} at {f_number}")),
        (time, f_number) => time.or(f_number),
    }
}

/// A focal length in millimetres: "70 mm", "10.5 mm", to a tenth. "—"
/// for a fraction with a zero denominator.
pub fn focal_length(length: Rational) -> String {
    tenths(length).map_or_else(|| NONE.to_owned(), |number| format!("{number} mm"))
}

/// An exposure compensation, with its sign and a minus sign for one below
/// zero: in lowest terms when its denominator there is 1, 2, 3 or 6, the
/// steps cameras set it in, "−2/3 EV", "+1/2 EV", "−1 EV", "0 EV"; and
/// otherwise as a decimal to a tenth, as a camera that records it in
/// tenths writes it, "+0.3 EV", "−0.7 EV". "—" for a fraction with a zero
/// denominator.
pub fn compensation(bias: SignedRational) -> String {
    let bias = bias.reduced();
    if bias.denominator == 0 {
        return NONE.to_owned();
    }
    let below_zero = (bias.numerator < 0) != (bias.denominator < 0);
    let numerator = bias.numerator.unsigned_abs();
    let denominator = bias.denominator.unsigned_abs();
    let magnitude = match denominator {
        _ if numerator == 0 => "0".to_owned(),
        1 => numerator.to_string(),
        2 | 3 | 6 => format!("{numerator}/{denominator}"),
        _ => decimal(f64::from(numerator) / f64::from(denominator), 1),
    };
    let sign = match (magnitude.as_str(), below_zero) {
        ("0", _) => "",
        (_, true) => "\u{2212}",
        (_, false) => "+",
    };
    format!("{sign}{magnitude} EV")
}

/// When a photo was taken, as its camera's clock said, written in `form`:
/// the date, the time, the fraction of a second only when the camera
/// recorded one, with the digits it recorded, and the clock's offset from
/// UTC only when it recorded one, "2026-08-15 10:20:30.85 UTC+02:00" in
/// the numeric form, "15 August 2026 at 10:20:30.85 (UTC+02:00)" in the
/// long one.
pub fn capture_time(time: DateTime, form: TimeForm) -> String {
    clock(&Clock::from(time), form)
}

/// The months' names in English, January first.
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A capture time's parts, as [`DateTime`] tells them.
#[derive(Clone, Copy, Debug)]
struct Clock {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
    subsecond_digits: u8,
    offset_minutes: Option<i16>,
}

impl From<DateTime> for Clock {
    fn from(time: DateTime) -> Self {
        Self {
            year: time.year(),
            month: time.month(),
            day: time.day(),
            hour: time.hour(),
            minute: time.minute(),
            second: time.second(),
            nanosecond: time.nanosecond(),
            subsecond_digits: time.subsecond_digits(),
            offset_minutes: time.offset_minutes(),
        }
    }
}

/// `time` in words, written in `form`, as [`capture_time`] writes it.
fn clock(time: &Clock, form: TimeForm) -> String {
    let mut of_day = format!("{:02}:{:02}:{:02}", time.hour, time.minute, time.second);
    if time.subsecond_digits > 0 {
        let digits = u32::from(time.subsecond_digits.min(9));
        let fraction = time.nanosecond / 10_u32.pow(9 - digits);
        let _ = write!(of_day, ".{fraction:0width$}", width = digits as usize);
    }
    let offset = time.offset_minutes.map(|offset| {
        let sign = if offset < 0 { '\u{2212}' } else { '+' };
        let minutes = offset.unsigned_abs();
        format!("UTC{sign}{:02}:{:02}", minutes / 60, minutes % 60)
    });
    let mut text = match form {
        TimeForm::Numeric => format!(
            "{:04}-{:02}-{:02} {of_day}",
            time.year, time.month, time.day
        ),
        TimeForm::Long => {
            // A capture time's month is 1 to 12, as raw-files reads it.
            let month = MONTHS[usize::from(time.month) - 1];
            format!("{} {month} {} at {of_day}", time.day, time.year)
        }
    };
    if let Some(offset) = offset {
        let _ = match form {
            TimeForm::Numeric => write!(text, " {offset}"),
            TimeForm::Long => write!(text, " ({offset})"),
        };
    }
    text
}

/// A lens: its model name when the file records one; otherwise its range,
/// the focal lengths at either end and the f-number at each, an end
/// written once where both are the same: "24-70 mm f/2.8-4", "50 mm
/// f/1.8". `None` when the file records neither.
pub fn lens(lens: &Lens) -> Option<String> {
    lens_text(
        lens.model(),
        [
            lens.min_focal_length(),
            lens.max_focal_length(),
            lens.f_number_at_min_focal_length(),
            lens.f_number_at_max_focal_length(),
        ],
    )
}

/// A lens named `model`, or of the `range` [`Lens`] gives, the shortest
/// and longest focal length and the f-number at each, as [`lens`] writes
/// it.
fn lens_text(model: Option<&str>, range: [Option<Rational>; 4]) -> Option<String> {
    if let Some(model) = model {
        return Some(model.to_owned());
    }
    let [shortest, longest, f_shortest, f_longest] = range;
    let focal = span(shortest, longest).map(|span| format!("{span} mm"));
    let aperture = span(f_shortest, f_longest).map(|span| format!("f/{span}"));
    match (focal, aperture) {
        (Some(focal), Some(aperture)) => Some(format!("{focal} {aperture}")),
        (focal, aperture) => focal.or(aperture),
    }
}

/// The values from `low` to `high`, each to a tenth: "24-70", or "50"
/// where both are the same, or the one known.
fn span(low: Option<Rational>, high: Option<Rational>) -> Option<String> {
    let [low, high] = [low, high].map(|end| end.and_then(tenths));
    match (low, high) {
        (Some(low), Some(high)) if low != high => Some(format!("{low}-{high}")),
        (low, high) => low.or(high),
    }
}

/// `value` as a decimal to a tenth, with no ".0" for a whole number:
/// "2.8", "9". `None` for a fraction with a zero denominator.
fn tenths(value: Rational) -> Option<String> {
    (value.denominator != 0).then(|| decimal(value.to_f64(), 1))
}

/// `value` with at most `decimals` decimals, and none of them trailing
/// zeros: "0.6", "2", "1.5".
fn decimal(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use catalog::photos::PhotoList;
    use desktop_ui::theme::Theme;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;
    use previews::Size;
    use raw_files::{Rational, SignedRational};

    use super::{
        Clock, SizeUnits, TimeForm, aperture, clock, compensation, exposure, file_size,
        focal_length, iso, lens_text, orientation, pixel_size, show, shutter_speed, upright, want,
    };
    use crate::textures::{self, Textures};

    /// The dock wants the photo it shows decoded itself, after the views
    /// have wanted what they draw, so what its file holds comes even when
    /// no view of the photos draws it.
    #[test]
    fn the_dock_brings_its_photo_s_details_itself() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DSC_0001.jpg");
        std::fs::write(&path, textures::jpeg(300, 200)).unwrap();
        let list = PhotoList::read(dir.path()).unwrap();
        let photo = list.current_photo().unwrap().clone();
        let palette = Theme::Paper.palette();
        let mut store = Textures::inline();
        let mut fonts_installed = false;
        // The first frame installs the fonts, for the headings' weight,
        // and draws nothing.
        let mut harness = Harness::builder().build_ui(move |ui| {
            if !fonts_installed {
                desktop_ui::fonts::install(ui.ctx());
                fonts_installed = true;
                return;
            }
            store.begin_frame(ui.ctx());
            let (times, sizes) = (TimeForm::default(), SizeUnits::default());
            show(ui, &palette, &mut store, Some(&photo), times, sizes);
            want(&mut store, Some(&photo));
            store.end_frame(ui.ctx());
        });
        harness.run();
        let bytes = std::fs::metadata(&path).unwrap().len();
        let size = file_size(bytes, SizeUnits::Decimal);
        harness.get_by_label(&format!("Size on disk: {size}"));
        harness.get_by_label("Image size: 300 × 200");
        assert!(harness.query_by_label("Reading…").is_none());
    }

    /// A store of textures, and whether the dock is shown, or else the
    /// photo is wanted at 512 pixels, as a view on a screen of two pixels
    /// a point wants it.
    struct Dock {
        store: Textures,
        shown: bool,
    }

    /// With a texture of its photo held, the dock wants that one rather
    /// than have its smallest thumbnail decoded too, as the details come
    /// with either.
    #[test]
    fn the_dock_wants_a_texture_held_rather_than_another() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DSC_0001.jpg");
        std::fs::write(&path, textures::jpeg(600, 400)).unwrap();
        let list = PhotoList::read(dir.path()).unwrap();
        let photo = list.current_photo().unwrap().clone();
        let palette = Theme::Paper.palette();
        let mut fonts_installed = false;
        let dock = Dock {
            store: Textures::inline(),
            shown: false,
        };
        // The first frame installs the fonts, for the headings' weight,
        // and draws nothing.
        let mut harness = Harness::builder().build_ui_state(
            move |ui, dock: &mut Dock| {
                if !fonts_installed {
                    desktop_ui::fonts::install(ui.ctx());
                    fonts_installed = true;
                    return;
                }
                dock.store.begin_frame(ui.ctx());
                if dock.shown {
                    let (times, sizes) = (TimeForm::default(), SizeUnits::default());
                    show(ui, &palette, &mut dock.store, Some(&photo), times, sizes);
                    want(&mut dock.store, Some(&photo));
                } else {
                    dock.store.want(photo.path(), 512, textures::ON_SCREEN);
                }
                dock.store.end_frame(ui.ctx());
            },
            dock,
        );
        harness.run();
        assert!(harness.state().store.texture(&path, 512).is_some());
        harness.state_mut().shown = true;
        harness.step();
        assert_eq!(harness.state().store.pending(), 0);
        harness.run();
        assert_eq!(harness.state().store.texture(&path, 256), None);
        harness.get_by_label("Image size: 600 × 400");
    }

    fn rational(numerator: u32, denominator: u32) -> Rational {
        Rational {
            numerator,
            denominator,
        }
    }

    fn signed(numerator: i32, denominator: i32) -> SignedRational {
        SignedRational {
            numerator,
            denominator,
        }
    }

    /// Sizes on disk are in decimal units: whole kilobytes, megabytes
    /// and gigabytes to a tenth, and the next unit up once a size would
    /// round to a thousand of one.
    #[test]
    fn a_file_size_is_in_decimal_units() {
        let file_size = |bytes| file_size(bytes, SizeUnits::Decimal);
        assert_eq!(file_size(0), "0 B");
        assert_eq!(file_size(999), "999 B");
        assert_eq!(file_size(1_000), "1 KB");
        assert_eq!(file_size(812_400), "812 KB");
        assert_eq!(file_size(812_600), "813 KB");
        assert_eq!(file_size(999_400), "999 KB");
        assert_eq!(file_size(999_600), "1.0 MB");
        assert_eq!(file_size(52_300_000), "52.3 MB");
        assert_eq!(file_size(52_349_999), "52.3 MB");
        assert_eq!(file_size(999_960_000), "1.0 GB");
        assert_eq!(file_size(1_200_000_000), "1.2 GB");
    }

    /// Sizes in binary units count in 1,024s, rounded as decimal ones
    /// are: whole kibibytes, mebibytes and gibibytes to a tenth, and the
    /// next unit up only once a size would round to 1,024 of one, so
    /// "1000 KiB" is written as it is, under a mebibyte.
    #[test]
    fn a_file_size_in_binary_units_counts_in_1024s() {
        let file_size = |bytes| file_size(bytes, SizeUnits::Binary);
        const KIB: u64 = 1024;
        const MIB: u64 = 1024 * KIB;
        const GIB: u64 = 1024 * MIB;
        assert_eq!(file_size(0), "0 B");
        assert_eq!(file_size(999), "999 B");
        assert_eq!(file_size(1_000), "1000 B");
        assert_eq!(file_size(1_023), "1023 B");
        assert_eq!(file_size(KIB), "1 KiB");
        assert_eq!(file_size(812 * KIB), "812 KiB");
        assert_eq!(file_size(812 * KIB + 600), "813 KiB");
        assert_eq!(file_size(999 * KIB + 400), "999 KiB");
        assert_eq!(file_size(1_023_999), "1000 KiB");
        assert_eq!(file_size(MIB - 1), "1.0 MiB");
        assert_eq!(file_size(MIB), "1.0 MiB");
        assert_eq!(file_size(52_300_000), "49.9 MiB");
        assert_eq!(file_size(MIB * 499 / 10), "49.9 MiB");
        assert_eq!(file_size(999 * MIB + 990 * KIB), "1000.0 MiB");
        assert_eq!(file_size(GIB - 1), "1.0 GiB");
        assert_eq!(file_size(GIB * 12 / 10), "1.2 GiB");
    }

    /// Each choice is saved by its name and restored from it; no other
    /// name is one.
    #[test]
    fn a_saved_form_restores_by_its_name() {
        for form in TimeForm::ALL {
            assert_eq!(TimeForm::from_saved(form.name()), Some(form));
        }
        for units in SizeUnits::ALL {
            assert_eq!(SizeUnits::from_saved(units.name()), Some(units));
        }
        assert_eq!(TimeForm::from_saved("ISO 8601"), None);
        assert_eq!(SizeUnits::from_saved("decimal"), None);
        assert_eq!(TimeForm::default(), TimeForm::Numeric);
        assert_eq!(SizeUnits::default(), SizeUnits::Decimal);
    }

    #[test]
    fn a_pixel_size_is_width_by_height() {
        let size = Size {
            width: 6000,
            height: 4000,
        };
        assert_eq!(pixel_size(size), "6000 × 4000");
    }

    #[test]
    fn an_orientation_is_told_by_the_longer_side() {
        let size = |width, height| Size { width, height };
        assert_eq!(orientation(size(6000, 4000)), "Landscape");
        assert_eq!(orientation(size(4000, 6000)), "Portrait");
        assert_eq!(orientation(size(4000, 4000)), "Square");
    }

    /// A RAW file's image size is stored as the sensor lies, and turned
    /// upright where the orientation swaps its sides.
    #[test]
    fn an_image_size_is_turned_upright() {
        let stored = raw_files::Size {
            width: 6000,
            height: 4000,
        };
        let size = |width, height| Size { width, height };
        assert_eq!(upright(stored, false), size(6000, 4000));
        assert_eq!(upright(stored, true), size(4000, 6000));
    }

    /// A fraction of a second with a numerator of 1 in lowest terms reads
    /// as one; any other time shorter than a quarter of a second as one
    /// over its reciprocal, rounded; any other time as a decimal to a
    /// tenth.
    #[test]
    fn a_shutter_speed_is_a_fraction_or_a_decimal() {
        assert_eq!(shutter_speed(rational(1, 320)), "1/320 s");
        assert_eq!(shutter_speed(rational(10, 3200)), "1/320 s");
        assert_eq!(shutter_speed(rational(10, 4366)), "1/437 s");
        assert_eq!(shutter_speed(rational(3, 20)), "1/7 s");
        assert_eq!(shutter_speed(rational(1, 4)), "1/4 s");
        assert_eq!(shutter_speed(rational(3, 10)), "0.3 s");
        assert_eq!(shutter_speed(rational(0, 1)), "0 s");
        assert_eq!(shutter_speed(rational(6, 10)), "0.6 s");
        assert_eq!(shutter_speed(rational(2, 1)), "2 s");
        assert_eq!(shutter_speed(rational(300, 10)), "30 s");
        assert_eq!(shutter_speed(rational(15, 10)), "1.5 s");
        assert_eq!(shutter_speed(rational(1, 1)), "1 s");
        assert_eq!(shutter_speed(rational(2, 3)), "0.7 s");
        assert_eq!(shutter_speed(rational(10, 3)), "3.3 s");
        assert_eq!(shutter_speed(rational(1, 0)), "—");
    }

    #[test]
    fn an_aperture_is_an_f_number_to_a_tenth() {
        assert_eq!(aperture(rational(28, 10)), "f/2.8");
        assert_eq!(aperture(rational(900, 100)), "f/9");
        assert_eq!(aperture(rational(14, 10)), "f/1.4");
        assert_eq!(aperture(rational(71, 10)), "f/7.1");
        assert_eq!(aperture(rational(8, 0)), "—");
    }

    /// A sensitivity is its number after "ISO", and 65535, which stands
    /// for any from 65535 up, says so.
    #[test]
    fn an_iso_is_its_number_after_iso() {
        assert_eq!(iso(64), "ISO 64");
        assert_eq!(iso(10_000), "ISO 10000");
        assert_eq!(iso(65_534), "ISO 65534");
        assert_eq!(iso(65_535), "ISO 65535 or more");
    }

    /// The exposure is the time at the aperture, or the one of them the
    /// file records, or nothing.
    #[test]
    fn an_exposure_is_the_time_at_the_aperture() {
        let (time, f_number) = (Some(rational(1, 2000)), Some(rational(8, 1)));
        assert_eq!(exposure(time, f_number).as_deref(), Some("1/2000 s at f/8"));
        assert_eq!(exposure(time, None).as_deref(), Some("1/2000 s"));
        assert_eq!(exposure(None, f_number).as_deref(), Some("f/8"));
        assert_eq!(exposure(None, None), None);
    }

    #[test]
    fn a_focal_length_is_in_millimetres_to_a_tenth() {
        assert_eq!(focal_length(rational(70, 1)), "70 mm");
        assert_eq!(focal_length(rational(700, 10)), "70 mm");
        assert_eq!(focal_length(rational(105, 10)), "10.5 mm");
        assert_eq!(focal_length(rational(70, 0)), "—");
    }

    /// A compensation has its sign, a minus sign below zero and none at
    /// zero. In lowest terms over 2, 3 or 6 it is a fraction, a whole one
    /// has no denominator, and over any other denominator it is a decimal
    /// to a tenth.
    #[test]
    fn a_compensation_is_a_signed_fraction_in_ev() {
        assert_eq!(compensation(signed(3, 10)), "+0.3 EV");
        assert_eq!(compensation(signed(-3, 10)), "\u{2212}0.3 EV");
        assert_eq!(compensation(signed(7, 10)), "+0.7 EV");
        assert_eq!(compensation(signed(-13, 10)), "\u{2212}1.3 EV");
        assert_eq!(compensation(signed(1, 100)), "0 EV");
        assert_eq!(compensation(signed(1, 2)), "+1/2 EV");
        assert_eq!(compensation(signed(-5, 10)), "\u{2212}1/2 EV");
        assert_eq!(compensation(signed(1, 6)), "+1/6 EV");
        assert_eq!(compensation(signed(-2, 3)), "\u{2212}2/3 EV");
        assert_eq!(compensation(signed(-4, 6)), "\u{2212}2/3 EV");
        assert_eq!(compensation(signed(2, -3)), "\u{2212}2/3 EV");
        assert_eq!(compensation(signed(1, 3)), "+1/3 EV");
        assert_eq!(compensation(signed(-3, -9)), "+1/3 EV");
        assert_eq!(compensation(signed(-6, 6)), "\u{2212}1 EV");
        assert_eq!(compensation(signed(2, 1)), "+2 EV");
        assert_eq!(compensation(signed(0, 6)), "0 EV");
        assert_eq!(compensation(signed(1, 0)), "—");
    }

    /// 10:20:30 on 15 August 2026, with no fraction of a second and no
    /// offset from UTC.
    fn time() -> Clock {
        Clock {
            year: 2026,
            month: 8,
            day: 15,
            hour: 10,
            minute: 20,
            second: 30,
            nanosecond: 0,
            subsecond_digits: 0,
            offset_minutes: None,
        }
    }

    /// A capture time has its fraction of a second only when the camera
    /// recorded one, with the digits it recorded, and its offset from UTC
    /// only when the camera recorded one, a minus sign for one behind.
    #[test]
    fn a_capture_time_says_what_the_camera_recorded() {
        let clock = |time: &Clock| clock(time, TimeForm::Numeric);
        assert_eq!(clock(&time()), "2026-08-15 10:20:30");
        let early = Clock {
            month: 1,
            day: 2,
            hour: 3,
            minute: 4,
            second: 5,
            ..time()
        };
        assert_eq!(clock(&early), "2026-01-02 03:04:05");
        let fraction = |nanosecond, subsecond_digits| Clock {
            nanosecond,
            subsecond_digits,
            ..time()
        };
        assert_eq!(clock(&fraction(850_000_000, 2)), "2026-08-15 10:20:30.85");
        assert_eq!(clock(&fraction(50_000_000, 2)), "2026-08-15 10:20:30.05");
        assert_eq!(clock(&fraction(500_000_000, 1)), "2026-08-15 10:20:30.5");
        assert_eq!(clock(&fraction(0, 1)), "2026-08-15 10:20:30.0");
        let offset = |minutes| Clock {
            nanosecond: 850_000_000,
            subsecond_digits: 2,
            offset_minutes: Some(minutes),
            ..time()
        };
        assert_eq!(clock(&offset(120)), "2026-08-15 10:20:30.85 UTC+02:00");
        assert_eq!(
            clock(&offset(-330)),
            "2026-08-15 10:20:30.85 UTC\u{2212}05:30"
        );
        assert_eq!(clock(&offset(0)), "2026-08-15 10:20:30.85 UTC+00:00");
    }

    /// The long form writes the day, the month's name and the year, then
    /// the time, with its fraction of a second and its offset from UTC,
    /// in parentheses, only when the camera recorded them.
    #[test]
    fn a_long_capture_time_names_the_month() {
        let clock = |time: &Clock| clock(time, TimeForm::Long);
        assert_eq!(clock(&time()), "15 August 2026 at 10:20:30");
        let early = Clock {
            month: 1,
            day: 2,
            hour: 3,
            minute: 4,
            second: 5,
            ..time()
        };
        assert_eq!(clock(&early), "2 January 2026 at 03:04:05");
        let december = Clock {
            month: 12,
            day: 31,
            ..time()
        };
        assert_eq!(clock(&december), "31 December 2026 at 10:20:30");
        let fraction = |nanosecond, subsecond_digits| Clock {
            nanosecond,
            subsecond_digits,
            ..time()
        };
        assert_eq!(
            clock(&fraction(850_000_000, 2)),
            "15 August 2026 at 10:20:30.85"
        );
        assert_eq!(
            clock(&fraction(500_000_000, 1)),
            "15 August 2026 at 10:20:30.5"
        );
        let offset = |minutes, subsecond_digits| Clock {
            nanosecond: 850_000_000,
            subsecond_digits,
            offset_minutes: Some(minutes),
            ..time()
        };
        assert_eq!(
            clock(&offset(120, 2)),
            "15 August 2026 at 10:20:30.85 (UTC+02:00)"
        );
        assert_eq!(
            clock(&offset(-330, 0)),
            "15 August 2026 at 10:20:30 (UTC\u{2212}05:30)"
        );
    }

    /// A lens is named by its model; with none, by its range, an end
    /// written once where both are the same; with neither, not at all.
    #[test]
    fn a_lens_is_its_model_or_its_range() {
        let range =
            |ends: [Option<(u32, u32)>; 4]| ends.map(|end| end.map(|(n, d)| rational(n, d)));
        let zoom = range([Some((24, 1)), Some((70, 1)), Some((28, 10)), Some((4, 1))]);
        assert_eq!(
            lens_text(Some("NIKKOR Z 24-70mm f/2.8 S"), zoom).as_deref(),
            Some("NIKKOR Z 24-70mm f/2.8 S")
        );
        assert_eq!(lens_text(None, zoom).as_deref(), Some("24-70 mm f/2.8-4"));
        let prime = range([
            Some((50, 1)),
            Some((500, 10)),
            Some((18, 10)),
            Some((18, 10)),
        ]);
        assert_eq!(lens_text(None, prime).as_deref(), Some("50 mm f/1.8"));
        let constant = range([Some((70, 1)), Some((200, 1)), Some((4, 1)), Some((4, 1))]);
        assert_eq!(lens_text(None, constant).as_deref(), Some("70-200 mm f/4"));
        let focal = range([Some((14, 1)), Some((30, 1)), None, None]);
        assert_eq!(lens_text(None, focal).as_deref(), Some("14-30 mm"));
        let wide = range([None, None, Some((14, 10)), None]);
        assert_eq!(lens_text(None, wide).as_deref(), Some("f/1.4"));
        assert_eq!(lens_text(None, [None; 4]), None);
    }
}
