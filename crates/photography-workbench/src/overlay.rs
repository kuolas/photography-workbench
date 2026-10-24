// SPDX-License-Identifier: GPL-3.0-or-later

//! The photo's information over the viewer's photo: a few labelled rows on
//! a dark, see-through panel at the viewer's top-left corner, which a key
//! and, on macOS, the View menu show or hide.
//!
//! Each row is a short label and its value: when the photo was taken, the
//! camera, the lens, the image's size in pixels with its format and its
//! size on disk, the focal length with the ISO, and the exposure with the
//! exposure bias, each left out where the file does not record it
//! ([`rows`]). The values are written by the Info dock's own functions
//! ([`crate::info`]), as chosen in Settings. A JPEG file's EXIF block is
//! not read, so for one only the image's row is shown.
//!
//! The labels stand in one column, and the values start in one column a
//! little past the widest label. The panel is fixed to the viewer's
//! corner, whatever the zoom and the pan, and as wide as its widest row,
//! but never wider than the viewer less its inset on each side, where a
//! longer value is cut short with "…"; a label never is. It is painted
//! only: the photo under it still takes every click, drag and turn of the
//! wheel, and the pointer still shows what they do there. Screen readers
//! are told of it as one label holding its rows, "Label: value" to a line.
//!
//! It shows only what the viewer already has: the app draws it once the
//! viewer has drawn some texture of the photo in the frame, when what a
//! decode told of the photo's file is known
//! ([`crate::textures::Details`]). It wants nothing decoded and opens no
//! file.

use catalog::photos::Photo;
use desktop_ui::theme::Palette;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    CornerRadius, FontId, Galley, Id, Rect, Sense, Stroke, StrokeKind, Vec2, WidgetInfo,
    WidgetType, pos2, vec2,
};

use crate::info::{self, NONE, SizeUnits, TimeForm};
use crate::textures::Details;

/// How far the panel's top-left corner stands in from the viewer's,
/// across and down, in points.
const INSET: Vec2 = vec2(15.0, 7.0);

/// The room between the panel's edges and its text, on every side.
const PADDING: f32 = 10.0;

/// The size of the panel's text, labels and values alike, as of the Info
/// dock's rows.
const TEXT_SIZE: f32 = 12.5;

/// The room between one row and the next.
const LINE_GAP: f32 = 3.0;

/// The room between the widest label and the column the values start in.
const COLUMN_GAP: f32 = 8.0;

/// The radius of the panel's corners.
const RADIUS: u8 = 4;

/// The width of the panel's border, drawn inside its edges.
const BORDER: f32 = 1.0;

/// What stands between the things a value says side by side.
const SEPARATOR: &str = " · ";

/// The label of the row saying when the photo was taken.
const TAKEN: &str = "Taken";

/// The label of the row saying the camera's model.
const CAMERA: &str = "Camera";

/// The label of the row saying the lens.
const LENS: &str = "Lens";

/// The label of the row saying the image's size in pixels, its format and
/// its size on disk.
const IMAGE: &str = "Image";

/// The label of the row saying the focal length and the ISO.
const FOCAL: &str = "Focal";

/// The label of the row saying the exposure and the exposure bias.
const EXPOSURE: &str = "Exposure";

/// What the panel says of `photo`, whose file a decode told `details` of,
/// a row apiece, each its label and its value, in this order, each left
/// out when the file records nothing for it: [`TAKEN`], when the photo
/// was taken, written in `times`; [`CAMERA`], the camera's model, as
/// recorded; [`LENS`] ([`info::lens`]); always [`IMAGE`], its size in
/// pixels, its format and its size on disk counted in `sizes`, "8256 ×
/// 5504 · NEF · 52.3 MB"; [`FOCAL`], the focal length and the ISO, "70 mm
/// · ISO 64", or the one recorded; and [`EXPOSURE`], the exposure and the
/// exposure bias, "1/250 s at f/8 · −2/3 EV", or the part recorded. A JPEG
/// file records none of what the camera did, as far as the app reads it,
/// so it has the one row.
pub fn rows(
    photo: &Photo,
    details: &Details,
    times: TimeForm,
    sizes: SizeUnits,
) -> Vec<(&'static str, String)> {
    // A value written as "—", as of a fraction with a zero denominator,
    // says nothing.
    let known = |text: String| (text != NONE).then_some(text);
    let image = [
        info::pixel_size(info::image_size(details)),
        photo.format(),
        info::file_size(details.bytes, sizes),
    ]
    .join(SEPARATOR);
    let Some(metadata) = &details.metadata else {
        return vec![(IMAGE, image)];
    };
    let shot = metadata.exposure();
    let focal = shot.focal_length().map(info::focal_length).and_then(known);
    let iso = shot.iso().map(info::iso);
    let exposure = info::exposure(shot.exposure_time(), shot.f_number());
    let bias = shot.exposure_bias().map(info::compensation).and_then(known);
    [
        (
            TAKEN,
            metadata
                .capture_time()
                .map(|time| info::capture_time(time, times)),
        ),
        (CAMERA, metadata.model().map(str::to_owned)),
        (LENS, metadata.lens().and_then(info::lens)),
        (IMAGE, Some(image)),
        (FOCAL, side_by_side(focal, iso)),
        (EXPOSURE, side_by_side(exposure, bias)),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|value| (label, value)))
    .collect()
}

/// `first` and `second` side by side, or the one of them there is; `None`
/// when neither is.
fn side_by_side(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some([first, second].join(SEPARATOR)),
        (first, second) => first.or(second),
    }
}

/// The id of the panel's node, by which screen readers and the tests
/// know it.
pub fn id() -> Id {
    Id::new("photo_information")
}

/// The panel at the top-left corner of `ui`, the viewer's whole area, in
/// `palette`'s colours, saying the [`rows`] of `photo`, whose file a
/// decode told `details` of, written in `times` and counted in `sizes`:
/// each label in the panel's left column and its value in the column
/// [`COLUMN_GAP`] past the widest label, both in the panel's colour of
/// text. It senses nothing, so whatever lies under it takes the pointer.
pub fn show(
    ui: &egui::Ui,
    palette: &Palette,
    photo: &Photo,
    details: &Details,
    times: TimeForm,
    sizes: SizeUnits,
) {
    let rows = rows(photo, details, times, sizes);
    let viewer = ui.max_rect();
    let format = TextFormat::simple(FontId::proportional(TEXT_SIZE), palette.overlay_text);
    let lay_out = |text: &str, wrap: TextWrapping| {
        let mut job = LayoutJob::single_section(text.to_owned(), format.clone());
        job.wrap = wrap;
        ui.painter().layout_job(job)
    };
    let labels: Vec<_> = rows
        .iter()
        .map(|(label, _)| lay_out(label, TextWrapping::default()))
        .collect();
    let values_left = widest(&labels) + COLUMN_GAP;
    let room = (viewer.width() - 2.0 * INSET.x - 2.0 * PADDING - values_left).max(0.0);
    let values: Vec<_> = rows
        .iter()
        .map(|(_, value)| lay_out(value, TextWrapping::truncate_at_width(room)))
        .collect();
    let heights: Vec<_> = labels
        .iter()
        .zip(&values)
        .map(|(label, value)| label.size().y.max(value.size().y))
        .collect();
    let gaps = rows.len().saturating_sub(1) as f32 * LINE_GAP;
    let size = vec2(
        values_left + widest(&values),
        heights.iter().sum::<f32>() + gaps,
    );
    let panel = Rect::from_min_size(viewer.min + INSET, size + Vec2::splat(2.0 * PADDING));

    let painter = ui.painter_at(viewer);
    let border = Stroke::new(BORDER, palette.overlay_border);
    painter.rect(
        panel,
        CornerRadius::same(RADIUS),
        palette.overlay_fill,
        border,
        StrokeKind::Inside,
    );
    let left = panel.left() + PADDING;
    let mut top = panel.top() + PADDING;
    for ((label, value), height) in labels.into_iter().zip(values).zip(heights) {
        painter.galley(pos2(left, top), label, palette.overlay_text);
        painter.galley(pos2(left + values_left, top), value, palette.overlay_text);
        top += height + LINE_GAP;
    }

    // A widget that senses no click and no drag takes neither from the
    // photo's under it, which egui still counts as hovered, so the pointer
    // still shows what a click or a drag does there. It gives screen
    // readers a node for the panel: a label holding its rows, as they read
    // any text. The node is the part shown, as the painter clips the panel
    // to the viewer.
    let text = rows
        .iter()
        .map(|(label, value)| format!("{label}: {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    let response = ui.interact(panel.intersect(viewer), id(), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
}

/// The width of the widest of `galleys`; nothing for none.
fn widest(galleys: &[std::sync::Arc<Galley>]) -> f32 {
    galleys
        .iter()
        .map(|galley| galley.size().x)
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use catalog::photos::PhotoList;
    use previews::Size;
    use raw_files::{DateTime, Exposure, Lens, Metadata, Rational, SignedRational};

    use desktop_ui::theme::Theme;

    use super::{COLUMN_GAP, INSET, PADDING, id, rows, show};
    use crate::info::{SizeUnits, TimeForm};
    use crate::textures::Details;

    /// The photo of the file named `name`, in a folder of its own, kept
    /// for as long as the folder returned is.
    fn photo(name: &str) -> (tempfile::TempDir, catalog::photos::Photo) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(name), b"").unwrap();
        let list = PhotoList::read(dir.path()).unwrap();
        let photo = list.current_photo().unwrap().clone();
        (dir, photo)
    }

    fn rational(numerator: u32, denominator: u32) -> Rational {
        Rational::new(numerator, denominator).unwrap()
    }

    /// 10:20:30.85 on 15 August 2026, by a clock two hours ahead of UTC.
    fn taken() -> DateTime {
        DateTime::new(2026, 8, 15, 10, 20, 30)
            .and_then(|time| time.with_subseconds(850_000_000, 2))
            .and_then(|time| time.with_offset_minutes(120))
            .unwrap()
    }

    /// What a decode tells of a RAW file of 52,300,000 bytes whose largest
    /// preview is 8256 by 5504 pixels and whose camera recorded
    /// `metadata`.
    fn raw(metadata: Metadata) -> Details {
        let largest = Size {
            width: 8256,
            height: 5504,
        };
        Details {
            largest,
            bytes: 52_300_000,
            previews: vec![largest],
            metadata: Some(metadata),
        }
    }

    /// A camera that recorded everything the panel says.
    fn everything() -> Metadata {
        let lens = Lens::builder()
            .model("NIKKOR Z 70-200mm f/2.8 VR S")
            .build()
            .unwrap();
        let exposure = Exposure::default()
            .with_focal_length(rational(70, 1))
            .with_iso(64)
            .with_exposure_time(rational(1, 250))
            .with_f_number(rational(8, 1))
            .with_exposure_bias(SignedRational::new(-2, 3).unwrap());
        Metadata::default()
            .with_make("NIKON CORPORATION")
            .with_model("NIKON Z 9")
            .with_capture_time(taken())
            .with_lens(lens)
            .with_exposure(exposure)
    }

    /// The text shapes `show` paints in a viewer of `viewer` for `photo`,
    /// whose file a decode told `details` of, with the panel's node.
    fn painted(
        viewer: egui::Rect,
        photo: &catalog::photos::Photo,
        details: &Details,
    ) -> (Vec<egui::epaint::TextShape>, egui::Rect) {
        let palette = Theme::Dark.palette();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let builder = egui::UiBuilder::new().max_rect(viewer);
            let viewer_ui = ui.new_child(builder);
            show(
                &viewer_ui,
                &palette,
                photo,
                details,
                TimeForm::Numeric,
                SizeUnits::Decimal,
            );
        });
        output.textures_delta.clear();
        let texts = output
            .shapes
            .into_iter()
            .filter_map(|clipped| match clipped.shape {
                egui::Shape::Text(text) => Some(text),
                _ => None,
            })
            .collect();
        (texts, ctx.read_response(id()).unwrap().rect)
    }

    /// In a viewer too short for the panel, its node is the part shown:
    /// it lies within the viewer.
    #[test]
    fn the_node_of_a_panel_taller_than_the_viewer_stays_inside_it() {
        let (_dir, photo) = photo("DSC_0001.NEF");
        let viewer = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(400.0, 60.0));
        let (_, node) = painted(viewer, &photo, &raw(everything()));
        assert!(viewer.contains_rect(node), "{node:?} not in {viewer:?}");
        assert!(node.height() > 0.0, "{node:?}");
    }

    /// Every label starts at the panel's left, inside its padding, and
    /// every value 8 points past the widest label, "Exposure", each in the
    /// panel's colour of text and on its label's line.
    #[test]
    fn the_values_start_in_one_column_past_the_widest_label() {
        let (_dir, photo) = photo("DSC_0001.NEF");
        let viewer = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(900.0, 600.0));
        let details = raw(everything());
        let (texts, _) = painted(viewer, &photo, &details);
        let said = rows(&photo, &details, TimeForm::Numeric, SizeUnits::Decimal);
        assert_eq!(texts.len(), 2 * said.len());
        let at = |text: &str| {
            let shape = texts
                .iter()
                .find(|shape| shape.galley.text() == text)
                .unwrap_or_else(|| panic!("{text:?} not painted"));
            let colour = Theme::Dark.palette().overlay_text;
            let format = &shape.galley.job.sections[0].format;
            assert_eq!((format.color, shape.fallback_color), (colour, colour));
            shape
        };
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        let left = viewer.left() + INSET.x + PADDING;
        let values = left + at("Exposure").galley.size().x + COLUMN_GAP;
        for (label, value) in &said {
            let (label, value) = (at(label), at(value));
            assert!(near(label.pos.x, left), "{label:?}");
            assert!(near(value.pos.x, values), "{value:?} not at {values}");
            assert!(near(value.pos.y, label.pos.y), "{value:?} {label:?}");
        }
    }

    /// A RAW file whose camera recorded everything has six rows: when it
    /// was taken, the camera's model without its make, the lens, the
    /// image's size, format and size on disk, the focal length and ISO,
    /// and the exposure with the exposure bias.
    #[test]
    fn a_raw_file_recording_everything_has_six_rows() {
        let (_dir, photo) = photo("DSC_0001.NEF");
        let said = rows(
            &photo,
            &raw(everything()),
            TimeForm::Numeric,
            SizeUnits::Decimal,
        );
        assert_eq!(
            said,
            [
                ("Taken", "2026-08-15 10:20:30.85 UTC+02:00".to_owned()),
                ("Camera", "NIKON Z 9".to_owned()),
                ("Lens", "NIKKOR Z 70-200mm f/2.8 VR S".to_owned()),
                ("Image", "8256 × 5504 · NEF · 52.3 MB".to_owned()),
                ("Focal", "70 mm · ISO 64".to_owned()),
                ("Exposure", "1/250 s at f/8 · \u{2212}2/3 EV".to_owned()),
            ]
        );
    }

    /// What the file does not record leaves its row out, and a row that
    /// says two things says the one recorded; a fraction with a zero
    /// denominator, which is no value, says nothing either. The image's
    /// row, its size, format and size on disk, is always there, its size
    /// the visible image's the camera recorded, turned upright.
    #[test]
    fn what_the_file_does_not_record_is_left_out() {
        let (_dir, photo) = photo("DSC_0001.NEF");
        let said = |metadata| {
            rows(
                &photo,
                &raw(metadata),
                TimeForm::Numeric,
                SizeUnits::Decimal,
            )
        };
        let row = |label, value: &str| (label, value.to_owned());
        let image = row("Image", "8256 × 5504 · NEF · 52.3 MB");
        assert_eq!(said(Metadata::default()), std::slice::from_ref(&image));

        let iso_and_time = Exposure::default()
            .with_iso(64)
            .with_exposure_time(rational(1, 250));
        let metadata = Metadata::default()
            .with_model("ILCE-7M4")
            .with_exposure(iso_and_time);
        assert_eq!(
            said(metadata),
            [
                row("Camera", "ILCE-7M4"),
                image.clone(),
                row("Focal", "ISO 64"),
                row("Exposure", "1/250 s"),
            ]
        );

        let range = Lens::builder()
            .min_focal_length(rational(24, 1))
            .max_focal_length(rational(70, 1))
            .build()
            .unwrap();
        let focal_and_aperture = Exposure::default()
            .with_focal_length(rational(35, 1))
            .with_f_number(rational(28, 10))
            .with_exposure_bias(SignedRational::new(0, 6).unwrap());
        let metadata = Metadata::default()
            .with_lens(range)
            .with_exposure(focal_and_aperture);
        assert_eq!(
            said(metadata),
            [
                row("Lens", "24-70 mm"),
                image.clone(),
                row("Focal", "35 mm"),
                row("Exposure", "f/2.8 · 0 EV"),
            ]
        );

        let no_number = Exposure::default()
            .with_focal_length(Rational {
                numerator: 35,
                denominator: 0,
            })
            .with_iso(100)
            .with_exposure_bias(SignedRational {
                numerator: 1,
                denominator: 0,
            });
        let metadata = Metadata::default().with_exposure(no_number);
        assert_eq!(said(metadata), [image.clone(), row("Focal", "ISO 100")]);

        let upright = Metadata::default()
            .with_image_size(raw_files::Size {
                width: 6000,
                height: 4000,
            })
            .with_orientation(raw_files::Orientation::Rotate90);
        assert_eq!(said(upright), [row("Image", "4000 × 6000 · NEF · 52.3 MB")]);
    }

    /// The exposure's row of a file that records the exposure bias alone
    /// says the bias.
    #[test]
    fn the_exposure_bias_alone_has_the_exposure_s_row() {
        let (_dir, photo) = photo("DSC_0001.NEF");
        let bias = Exposure::default().with_exposure_bias(SignedRational::new(1, 3).unwrap());
        let metadata = Metadata::default().with_exposure(bias);
        let said = rows(
            &photo,
            &raw(metadata),
            TimeForm::Numeric,
            SizeUnits::Decimal,
        );
        assert_eq!(
            said,
            [
                ("Image", "8256 × 5504 · NEF · 52.3 MB".to_owned()),
                ("Exposure", "+1/3 EV".to_owned()),
            ]
        );
    }

    /// A JPEG file, of which the app reads no camera data, has the one
    /// row of the image's size, format and size on disk.
    #[test]
    fn a_jpeg_file_has_one_row() {
        let (_dir, photo) = photo("DSC_0001.jpg");
        let details = Details {
            largest: Size {
                width: 6000,
                height: 4000,
            },
            bytes: 812_400,
            previews: Vec::new(),
            metadata: None,
        };
        let said = rows(&photo, &details, TimeForm::Numeric, SizeUnits::Decimal);
        assert_eq!(said, [("Image", "6000 × 4000 · JPEG · 812 KB".to_owned())]);
    }

    /// The capture time is written, and the size on disk counted, as
    /// chosen in Settings.
    #[test]
    fn the_time_and_the_size_follow_the_choices() {
        let (_dir, photo) = photo("DSC_0001.NEF");
        let metadata = Metadata::default().with_capture_time(taken());
        let said = rows(&photo, &raw(metadata), TimeForm::Long, SizeUnits::Binary);
        assert_eq!(
            said,
            [
                (
                    "Taken",
                    "15 August 2026 at 10:20:30.85 (UTC+02:00)".to_owned()
                ),
                ("Image", "8256 × 5504 · NEF · 49.9 MiB".to_owned()),
            ]
        );
    }
}
