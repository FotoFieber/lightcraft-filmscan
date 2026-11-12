//! Project links (Help menu, About dialog).
//!
//! This is the FotoFieber Convert Negative build, a modified version of LightCraft. The ArtCraft
//! brand terms (`docs/brand/LICENSE-brand.txt`) ask a published modified version to drop the
//! ArtCraft name, so the ArtCraft website and Discord links are gone and the source and feedback
//! links point at this build's own repository.

/// The app's id in URLs.
pub const APP: &str = "lightcraft";
/// This build's source repository (the fork with Convert Negative).
pub const GITHUB: &str = "https://github.com/FotoFieber/lightcraft";
/// The upstream project this build is based on.
pub const UPSTREAM: &str = "https://github.com/storytold/lightcraft";

/// (UI command id, menu label, URL) for each link, in Help-menu order.
/// The user documentation (docs/ in the repository).
pub const HELP: &str = "https://github.com/storytold/lightcraft/tree/main/docs";

pub const LINKS: &[(&str, &str, &str)] =
    &[("app.help", "LightCraft Help", HELP), ("app.github", "LightCraft on GitHub", GITHUB), ("app.feedback", "Send Feedback…", FEEDBACK)];

/// Where feedback and bug reports go.
pub const FEEDBACK: &str = "https://github.com/FotoFieber/lightcraft/issues/new";

/// The URL behind a link command id.
pub fn url_of(cmd: &str) -> Option<&'static str> {
    LINKS.iter().find(|(id, _, _)| *id == cmd).map(|(_, _, u)| *u)
}

/// Open `url` in the user's browser (through the host's `open_url` service).
pub fn open(app: &mut crate::LightcraftApp, url: &str) -> Result<serde_json::Value, String> {
    let open = app.services.open_url.as_mut().ok_or("can't open links here")?;
    open(url)?;
    Ok(serde_json::json!({ "url": url }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_https_and_name_this_app() {
        for (id, label, url) in LINKS {
            assert!(url.starts_with("https://"), "{id}");
            assert!(!label.is_empty());
            assert_eq!(url_of(id), Some(*url));
        }
        assert!(GITHUB.ends_with(&format!("/FotoFieber/{APP}")));
        assert!(UPSTREAM.ends_with(&format!("/storytold/{APP}")));
        assert!(FEEDBACK.starts_with(GITHUB), "bug reports about this build go to its own repository");
        for (id, label, url) in LINKS {
            assert!(!format!("{id}{label}{url}").to_lowercase().contains("artcraft"), "{id}");
        }
    }
}
