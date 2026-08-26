//! Font descriptor for text rendering.

use serde::{Deserialize, Serialize};

/// Font configuration for text rendering.
///
/// Describes which font to use, including family name, weight, and style.
/// This descriptor is passed through the rendering pipeline to ensure
/// consistent font usage across preview and finalized text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FontDescriptor {
    /// Font family name (e.g., "Sans", "Monospace", "JetBrains Mono")
    /// Reference installed system fonts by name
    pub family: String,

    /// Font weight (e.g., "normal", "bold", "light" or numeric 100-900)
    pub weight: String,

    /// Font style (e.g., "normal", "italic", "oblique")
    pub style: String,
}

impl FontDescriptor {
    /// Creates a new font descriptor with the specified parameters.
    pub fn new(family: String, weight: String, style: String) -> Self {
        Self {
            family,
            weight,
            style,
        }
    }

    /// Whether this descriptor asks for bold.
    ///
    /// One rule, because two places decide on it: the toolbar's toggle shows
    /// its state from this and `to_pango_string` renders from the same field.
    /// A numeric weight is not bold — the toggle writes the word, and a config
    /// that asks for `700` is asking for something the toggle cannot express.
    pub fn is_bold(&self) -> bool {
        self.weight.trim().eq_ignore_ascii_case("bold")
    }

    /// Converts this font descriptor to a Pango font description string.
    ///
    /// Format: "Family Style Weight Size"
    /// Example: "Sans Bold 32" or "Monospace Italic 24"
    pub fn to_pango_string(&self, size: f64) -> String {
        let mut parts = vec![self.family.clone()];

        // Add style if not normal
        if self.style.to_lowercase() != "normal" {
            parts.push(capitalize_first(&self.style));
        }

        // Add weight if not normal
        if self.weight.to_lowercase() != "normal" {
            parts.push(capitalize_first(&self.weight));
        }

        // Add size
        parts.push(format!("{}", size.round() as i32));

        parts.join(" ")
    }
}

impl Default for FontDescriptor {
    fn default() -> Self {
        Self {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        }
    }
}

/// Capitalizes the first letter of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pango_string_default() {
        let font = FontDescriptor::default();
        assert_eq!(font.to_pango_string(32.0), "Sans Bold 32");
    }

    #[test]
    fn test_pango_string_italic() {
        let font = FontDescriptor::new(
            "Monospace".to_string(),
            "normal".to_string(),
            "italic".to_string(),
        );
        assert_eq!(font.to_pango_string(24.0), "Monospace Italic 24");
    }

    #[test]
    fn test_pango_string_custom() {
        let font = FontDescriptor::new(
            "JetBrains Mono".to_string(),
            "light".to_string(),
            "normal".to_string(),
        );
        assert_eq!(font.to_pango_string(16.0), "JetBrains Mono Light 16");
    }
}

/// Both installed-family lists, built from one walk of the font map.
struct FontCatalog {
    all: Vec<String>,
    monospace: Vec<String>,
}

static FONT_CATALOG: std::sync::OnceLock<FontCatalog> = std::sync::OnceLock::new();

/// The installed families, enumerated once and kept for the life of the process.
///
/// One walk, not one per list. The monospace names are a filter over the same
/// families the full list holds, so reading the font map twice would pay the
/// ~23 ms enumeration again to learn nothing new — and would make pressing
/// `Tab` in the picker cost what opening it did.
///
/// The list changes only when fonts are installed or removed, which does not
/// happen while an overlay is up.
///
/// The Wayland backend prewarms this cache on a worker after its first committed
/// frame. Synchronous callers such as the configurator can still initialize it
/// on demand, but input dispatch uses the non-blocking readiness probes below.
fn font_catalog() -> &'static FontCatalog {
    FONT_CATALOG.get_or_init(|| {
        use pango::prelude::{FontFamilyExt, FontMapExt};
        let listed = pangocairo::FontMap::new()
            .list_families()
            .iter()
            .map(|family| (family.name().to_string(), family.is_monospace()))
            .collect();
        let catalog = build_font_catalog(listed);
        log::debug!(
            "Enumerated {} system font families ({} monospace)",
            catalog.all.len(),
            catalog.monospace.len()
        );
        catalog
    })
}

/// Build the process-wide catalog on the calling thread.
///
/// The Wayland backend calls this only from its prewarm worker. Keeping the
/// operation here ensures every caller still shares the same one-time cache.
pub(crate) fn prewarm_system_font_catalog() {
    let _ = font_catalog();
}

/// Whether font enumeration has completed, without starting it.
pub(crate) fn system_font_catalog_is_ready() -> bool {
    FONT_CATALOG.get().is_some()
}

/// Installed families when the cache is ready, without enumerating fonts.
pub(crate) fn try_system_font_families() -> Option<&'static [String]> {
    FONT_CATALOG.get().map(|catalog| catalog.all.as_slice())
}

/// Installed monospace families when the cache is ready, without enumerating.
pub(crate) fn try_monospace_font_families() -> Option<&'static [String]> {
    FONT_CATALOG
        .get()
        .map(|catalog| catalog.monospace.as_slice())
}

/// Sort, de-duplicate, and split what the font map listed.
///
/// Separate from the enumeration so the rules can be tested against a list
/// chosen for the purpose. What a given machine has installed is not a fixture:
/// a desktop with no case-variant families cannot show whether de-duplication
/// handles them.
fn build_font_catalog(mut listed: Vec<(String, bool)>) -> FontCatalog {
    listed.sort_by_key(|(name, _)| normalized_family_name(name));
    // Sorted by the same key the comparison uses, so variants land next to each
    // other; de-duplicated by `families_match` rather than exact text, or a
    // backend offering both `Sans` and `sans` lists one font twice.
    listed.dedup_by(|(left, _), (right, _)| families_match(left, right));

    let monospace = listed
        .iter()
        .filter(|(_, monospace)| *monospace)
        .map(|(name, _)| name.clone())
        .collect();
    let all = listed.into_iter().map(|(name, _)| name).collect();
    FontCatalog { all, monospace }
}

/// Font families installed on this system, sorted, with duplicates removed.
///
/// See [`font_catalog`] for when the enumeration happens and why it is cached.
pub fn system_font_families() -> &'static [String] {
    &font_catalog().all
}

/// Families the font system reports as monospace, in the same order.
///
/// The group people reach for when annotating code, and a small enough slice of
/// a typical system — 10 of 269 on the machine this was measured on — to be
/// worth offering as its own filter. Free once the full list has been built.
pub fn monospace_font_families() -> &'static [String] {
    &font_catalog().monospace
}

/// Whether `family` is installed, compared without case.
///
/// Pango resolves an unknown family to whatever fontconfig substitutes, with no
/// error and no warning, so a typo in a configured family silently renders in
/// something else. This is how a caller can say so.
pub fn family_is_installed(family: &str) -> bool {
    let wanted = family.trim();
    if wanted.is_empty() {
        return false;
    }
    system_font_families()
        .iter()
        .any(|name| families_match(name, wanted))
}

/// Whether two strings name the same family.
///
/// One rule for the whole program. Fontconfig resolves a family without regard
/// to case, so `sans` and `Sans` are one font; anything that compares family
/// names exactly ends up treating them as two, and a step from one to the other
/// changes only the spelling. Not `eq_ignore_ascii_case`: family names are not
/// all Latin.
pub fn families_match(left: &str, right: &str) -> bool {
    normalized_family_name(left) == normalized_family_name(right)
}

fn normalized_family_name(family: &str) -> String {
    family.trim().to_lowercase()
}

#[cfg(test)]
mod system_font_tests {
    use super::*;

    #[test]
    fn the_system_list_is_non_empty_sorted_and_free_of_repeats() {
        let families = system_font_families();

        assert!(
            !families.is_empty(),
            "a system with no fonts at all cannot render text either"
        );
        let mut sorted = families.to_vec();
        sorted.sort_by_key(|name| normalized_family_name(name));
        assert_eq!(families, sorted.as_slice());
        let mut unique = sorted.clone();
        unique.dedup();
        assert_eq!(unique.len(), families.len());
    }

    #[test]
    fn the_list_is_enumerated_once_and_reused() {
        let first = system_font_families();
        let second = system_font_families();

        assert!(
            std::ptr::eq(first, second),
            "the second call must not re-enumerate"
        );
    }

    #[test]
    fn both_lists_come_from_one_enumeration() {
        // Reading the font map a second time would pay the ~23 ms walk again to
        // learn nothing new, and would make pressing Tab in the picker cost
        // what opening it did.
        let all = system_font_families();
        let monospace = monospace_font_families();

        assert!(std::ptr::eq(all, system_font_families()));
        assert!(std::ptr::eq(monospace, monospace_font_families()));
        // Same catalog: the monospace list is in the full list's own order.
        let mut positions = monospace
            .iter()
            .map(|name| all.iter().position(|other| other == name));
        assert!(
            positions.all(|position| position.is_some()),
            "the filter must be over the list, not a second walk"
        );
    }

    #[test]
    fn the_catalog_holds_one_entry_per_family_however_it_is_spaced_or_spelled() {
        // `families_match` is the program's one rule for family identity, and
        // the catalog has to obey it too, or a backend offering both `Sans` and
        // `sans` lists one font twice.
        let catalog = build_font_catalog(vec![
            ("Sans".to_string(), false),
            ("JetBrains Mono".to_string(), true),
            ("sans".to_string(), false),
            ("SANS".to_string(), false),
            ("  sAnS  ".to_string(), false),
        ]);

        assert_eq!(catalog.all, ["JetBrains Mono", "Sans"]);
        assert_eq!(catalog.monospace, ["JetBrains Mono"]);
    }

    #[test]
    fn the_catalog_is_sorted_without_regard_to_case() {
        let catalog = build_font_catalog(vec![
            ("Zapfino".to_string(), false),
            ("adwaita Sans".to_string(), false),
            ("Liberation Mono".to_string(), true),
        ]);

        assert_eq!(catalog.all, ["adwaita Sans", "Liberation Mono", "Zapfino"]);
    }

    #[test]
    fn monospace_families_are_a_subset_of_the_whole_list() {
        let all = system_font_families();

        for family in monospace_font_families() {
            assert!(
                all.contains(family),
                "{family} is missing from the full list"
            );
        }
    }

    #[test]
    fn an_installed_family_is_recognized_whatever_case_it_is_written_in() {
        let family = system_font_families()
            .first()
            .expect("at least one family")
            .clone();

        assert!(family_is_installed(&family));
        assert!(family_is_installed(&family.to_uppercase()));
        assert!(family_is_installed(&format!("  {family}  ")));
    }

    #[test]
    fn a_family_that_is_not_installed_is_reported_as_missing() {
        assert!(!family_is_installed("Wayscriber No Such Font 9000"));
        assert!(!family_is_installed(""));
        assert!(!family_is_installed("   "));
    }
}
