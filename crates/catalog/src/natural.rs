// SPDX-License-Identifier: GPL-3.0-or-later

//! Natural order for file and folder names, the order a person expects:
//! `DSC_2` before `DSC_10`, and `a` next to `B` whatever their case.

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::path::Path;

/// The natural order of two file or folder names.
///
/// Each name is read as runs of ASCII digits and runs of anything else.
/// Two runs of digits compare by the number they spell, however long it
/// is; two runs of anything else compare ignoring case; and a run of
/// digits comes before a run of anything else. A name that runs out first
/// comes first, so `a` comes before `a1`.
///
/// Names alike by those rules still have an order, so a folder lists the
/// same way whatever order the system gave its entries in: the name whose
/// numbers have fewer leading zeros comes first (`1` before `01`), then
/// the names compare case by case (`A` before `a`), and last byte by byte,
/// for names that are not valid Unicode. Only a name and itself are equal.
pub(crate) fn cmp(a: &OsStr, b: &OsStr) -> Ordering {
    let (text_a, text_b) = (a.to_string_lossy(), b.to_string_lossy());
    let mut runs_a = Runs(&text_a);
    let mut runs_b = Runs(&text_b);
    // The first difference in leading zeros, the first tie-break.
    let mut zeros = Ordering::Equal;
    loop {
        let (run_a, run_b) = match (runs_a.next(), runs_b.next()) {
            (None, None) => break,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(run_a), Some(run_b)) => (run_a, run_b),
        };
        let order = match (is_digits(run_a), is_digits(run_b)) {
            (true, true) => {
                let (number_a, number_b) =
                    (run_a.trim_start_matches('0'), run_b.trim_start_matches('0'));
                if zeros == Ordering::Equal {
                    zeros = (run_a.len() - number_a.len()).cmp(&(run_b.len() - number_b.len()));
                }
                // Without leading zeros the longer number is the larger,
                // and two of a length compare digit by digit.
                number_a
                    .len()
                    .cmp(&number_b.len())
                    .then_with(|| number_a.cmp(number_b))
            }
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => lowercase(run_a).cmp(lowercase(run_b)),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    zeros
        .then_with(|| text_a.cmp(&text_b))
        .then_with(|| a.cmp(b))
}

/// The natural order of two paths by their file names alone, as [`cmp`]
/// orders them, whatever folders they are in. A path with no file name,
/// such as `/`, has an empty one.
pub(crate) fn cmp_file_names(a: &Path, b: &Path) -> Ordering {
    cmp(
        a.file_name().unwrap_or_default(),
        b.file_name().unwrap_or_default(),
    )
}

/// Whether a run is a run of digits. A run is all digits or none.
fn is_digits(run: &str) -> bool {
    run.starts_with(|c: char| c.is_ascii_digit())
}

/// A run's characters in lower case, without building a new string.
fn lowercase(run: &str) -> impl Iterator<Item = char> + '_ {
    run.chars().flat_map(char::to_lowercase)
}

/// The runs of a name, in order: each a run of ASCII digits or a run of
/// anything else, never empty.
struct Runs<'a>(&'a str);

impl<'a> Iterator for Runs<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let digits = is_digits(self.0);
        let first = self.0.chars().next()?;
        let end = self.0[first.len_utf8()..]
            .find(|c: char| c.is_ascii_digit() != digits)
            .map_or(self.0.len(), |i| i + first.len_utf8());
        let (run, rest) = self.0.split_at(end);
        self.0 = rest;
        Some(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The order of two names given as text.
    fn order(a: &str, b: &str) -> Ordering {
        cmp(OsStr::new(a), OsStr::new(b))
    }

    #[test]
    fn numbers_compare_by_value() {
        assert_eq!(order("DSC_2", "DSC_10"), Ordering::Less);
        assert_eq!(order("DSC_10", "DSC_100"), Ordering::Less);
        assert_eq!(order("DSC_100", "DSC_2"), Ordering::Greater);
        assert_eq!(order("9", "10"), Ordering::Less);
    }

    #[test]
    fn numbers_too_long_for_an_integer_still_compare_by_value() {
        let thirty = "9".repeat(30);
        let thirty_one = format!("1{}", "0".repeat(30));
        assert_eq!(order(&thirty, &thirty_one), Ordering::Less);
        assert_eq!(order(&format!("{thirty}0"), &thirty_one), Ordering::Greater);
    }

    #[test]
    fn case_is_ignored() {
        assert_eq!(order("a", "B"), Ordering::Less);
        assert_eq!(order("B", "a"), Ordering::Greater);
        assert_eq!(order("dsc_2", "DSC_10"), Ordering::Less);
    }

    #[test]
    fn names_alike_but_not_the_same_still_have_an_order() {
        assert_eq!(order("A", "a"), Ordering::Less);
        assert_eq!(order("a", "A"), Ordering::Greater);
        assert_eq!(order("1", "01"), Ordering::Less);
        assert_eq!(order("01", "1"), Ordering::Greater);
        // Fewer leading zeros decides before case does.
        assert_eq!(order("a01", "A1"), Ordering::Greater);
        assert_eq!(order("DSC_0001", "DSC_0001"), Ordering::Equal);
    }

    #[test]
    fn digits_come_before_anything_else() {
        assert_eq!(order("1", "a"), Ordering::Less);
        assert_eq!(order("a_1", "a_b"), Ordering::Less);
        assert_eq!(order("_", "1"), Ordering::Greater);
    }

    #[test]
    fn a_name_that_runs_out_first_comes_first() {
        assert_eq!(order("a", "a1"), Ordering::Less);
        assert_eq!(order("a1", "a"), Ordering::Greater);
        assert_eq!(order("", "a"), Ordering::Less);
        assert_eq!(order("", ""), Ordering::Equal);
    }

    #[test]
    fn paths_compare_by_their_file_names_alone() {
        let order = |a, b| cmp_file_names(Path::new(a), Path::new(b));
        assert_eq!(order("/z/DSC_2.NEF", "/a/DSC_10.NEF"), Ordering::Less);
        assert_eq!(order("/a/DSC_10.NEF", "/z/DSC_2.NEF"), Ordering::Greater);
        assert_eq!(order("/", "/a"), Ordering::Less);
    }

    #[test]
    fn names_run_by_run_across_letters_and_digits() {
        let runs: Vec<_> = Runs("DSC_0010b.NEF").collect();
        assert_eq!(runs, ["DSC_", "0010", "b.NEF"]);
        let runs: Vec<_> = Runs("12ä3").collect();
        assert_eq!(runs, ["12", "ä", "3"]);
    }

    /// Two names that are not valid Unicode read the same once their bad
    /// bytes are replaced, and are still told apart.
    #[cfg(target_os = "linux")]
    #[test]
    fn names_that_are_not_unicode_are_told_apart() {
        use std::os::unix::ffi::OsStrExt;
        let a = OsStr::from_bytes(b"DSC_\xff.NEF");
        let b = OsStr::from_bytes(b"DSC_\xfe.NEF");
        assert_eq!(a.to_string_lossy(), b.to_string_lossy());
        assert_eq!(cmp(a, b), Ordering::Greater);
        assert_eq!(cmp(b, a), Ordering::Less);
        assert_eq!(cmp(a, a), Ordering::Equal);
    }
}
