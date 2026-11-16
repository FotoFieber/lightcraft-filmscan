//! The names a build uses for its settings folder, its default library and its app id.
//!
//! LightCraft Film Scan runs beside an official LightCraft. It must not share their settings or
//! library: the official LightCraft can't show photos edited with Convert Negative and could save
//! them back without the film settings. On Linux the Film Scan launcher sets
//! `LIGHTCRAFT_CONFIG_DIR`, `LIGHTCRAFT_LIBRARY` and `LIGHTCRAFT_APP_ID`. Windows and macOS builds
//! have no launcher script, so they are built with `LIGHTCRAFT_FILMSCAN=1` and take
//! [`FILM_SCAN`]'s names as their defaults. The environment variables still win over them.

/// One build's default names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Names {
    /// The settings folder in `Application Support` (macOS) and `%APPDATA%` (Windows).
    pub settings: &'static str,
    /// The settings folder in `$XDG_CONFIG_HOME` (Linux and the other Unixes).
    pub settings_xdg: &'static str,
    /// The default library's folder in the user's Pictures folder.
    pub library: &'static str,
    /// The reverse-DNS app id: Wayland app id, `.desktop` file and icon name.
    pub app_id: &'static str,
}

/// The official LightCraft.
pub const STANDARD: Names =
    Names { settings: "LightCraft", settings_xdg: "lightcraft", library: "LightCraft Library", app_id: "ai.storyteller.lightcraft" };

/// LightCraft Film Scan; the same names its Linux launcher sets.
pub const FILM_SCAN: Names = Names {
    settings: "LightCraft Film Scan",
    settings_xdg: "lightcraft-filmscan",
    library: "LightCraft Film Scan Library",
    app_id: "ch.fotofieber.LightCraftFilmScan",
};

/// This build's names: [`FILM_SCAN`] when it was built with `LIGHTCRAFT_FILMSCAN` set, else
/// [`STANDARD`].
pub const NAMES: Names = if option_env!("LIGHTCRAFT_FILMSCAN").is_some() { FILM_SCAN } else { STANDARD };

#[cfg(test)]
mod tests {
    use super::*;

    /// The Film Scan names are the ones its Linux launcher sets, and none is the official one.
    #[test]
    fn film_scan_names_differ_from_the_official_ones() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/fotofieber/lightcraft-filmscan.sh");
        let launcher = std::fs::read_to_string(path).unwrap();
        for name in [FILM_SCAN.settings_xdg, FILM_SCAN.library, FILM_SCAN.app_id] {
            assert!(launcher.contains(name), "{name} is not in the launcher");
        }
        assert_ne!(FILM_SCAN.settings, STANDARD.settings);
        assert_ne!(FILM_SCAN.settings_xdg, STANDARD.settings_xdg);
        assert_ne!(FILM_SCAN.library, STANDARD.library);
        assert_ne!(FILM_SCAN.app_id, STANDARD.app_id);
    }

    /// `LIGHTCRAFT_FILMSCAN` at build time picks the names (run once with it set, once without).
    #[test]
    fn the_build_variable_picks_the_names() {
        let film_scan = option_env!("LIGHTCRAFT_FILMSCAN").is_some();
        assert_eq!(NAMES, if film_scan { FILM_SCAN } else { STANDARD });
        assert_eq!(crate::library::DEFAULT_NAME, NAMES.library);
    }
}
