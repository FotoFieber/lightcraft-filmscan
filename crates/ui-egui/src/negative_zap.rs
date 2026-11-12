//! Zap! from the app: the panel's button and Photo ▸ Zap!… run `develop.negativeZap` with the
//! panel's (or the dialog's) margin, format, sprocket holes and film. One photo runs at once; a selection reads and analyses its scans off
//! the UI thread, listed in the activity stack, and commits them as one undo step.
use crate::LightcraftApp;
use lightcraft_catalog::{Op, PhotoId};
use lightcraft_develop::DevelopSettings;
use lightcraft_engine::negative_zap::{self, Input, Options};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};

const LABEL: &str = "Zap!";
const COMMAND: &str = "develop.negativeZap";

struct Prepared {
    library: u64,
    commits: u64,
    options: Options,
    inputs: Vec<Input>,
}
struct Completed {
    library: u64,
    commits: u64,
    /// Each photo's settings as they were read, and zapped.
    settings: Vec<(PhotoId, Arc<DevelopSettings>, DevelopSettings)>,
    /// Photos left out, with why.
    failed: Vec<(PhotoId, String)>,
    /// What measuring them as one roll did.
    roll: Option<negative_zap::Roll>,
}

/// The call's parameters with the panel's margin and format where it names none.
fn with_panel_choices(app: &LightcraftApp, p: &Value) -> Value {
    let mut p = if p.is_object() { p.clone() } else { json!({}) };
    if let Some(o) = p.as_object_mut() {
        o.entry("margin").or_insert_with(|| json!(app.ui.negative_zap_margin));
        o.entry("format").or_insert_with(|| json!(app.ui.negative_zap_format));
        o.entry("sprockets").or_insert_with(|| json!(app.ui.negative_zap_sprockets));
        o.entry("film").or_insert_with(|| json!(app.ui.negative_zap_film));
    }
    p
}

fn prepare(app: &mut LightcraftApp, p: &Value, ids: Vec<PhotoId>) -> Result<Prepared, String> {
    let options = Options::from_params(p).map_err(|e| format!("{COMMAND}: {e}"))?;
    let mut inputs = Vec::with_capacity(ids.len());
    for id in ids {
        inputs.push(Input::of(&mut app.session, id).ok_or_else(|| format!("{COMMAND}: no such photo: {}", id.0))?);
    }
    Ok(Prepared { library: app.session.library_generation(), commits: app.session.commits(), options, inputs })
}

impl Prepared {
    fn run(self, progress: Option<&lightcraft_engine::activity::TaskHandle>) -> Result<Completed, String> {
        let total = self.inputs.len() as u64;
        let cancelled = || progress.is_some_and(|p| p.is_cancelled());
        let mut zapped = Vec::with_capacity(self.inputs.len());
        let mut read = Vec::with_capacity(self.inputs.len());
        let mut failed = Vec::new();
        for (i, input) in self.inputs.into_iter().enumerate() {
            if cancelled() {
                return Err(crate::i18n::tr("Zap! cancelled").into());
            }
            // a scan with no frame on it (or that can't be read) is left out, not the whole selection
            match input.run(self.options) {
                Ok(z) => {
                    zapped.push((input.id, z));
                    read.push(input.settings);
                }
                Err(e) => failed.push((input.id, e)),
            }
            if let Some(p) = progress {
                p.progress(i as u64 + 1, total);
            }
        }
        if cancelled() {
            return Err(crate::i18n::tr("Zap! cancelled").into());
        }
        if zapped.is_empty() {
            let why = failed.first().map(|(id, e)| format!(" (photo {}: {e})", id.0)).unwrap_or_default();
            return Err(crate::i18n::tr_format!("Zap!: no film frame found on any of the photos{why}", why = why));
        }
        let roll = if self.options.roll { negative_zap::pool(&mut zapped)? } else { None };
        let settings = zapped.into_iter().zip(read).map(|((id, z), before)| (id, before, z.settings)).collect();
        Ok(Completed { library: self.library, commits: self.commits, settings, failed, roll })
    }
}

/// Commits a run as one undo step → the photos changed, those left out and the roll.
fn apply(app: &mut LightcraftApp, completed: Completed) -> Result<(usize, usize, Option<negative_zap::Roll>), String> {
    // An undo or a live edit changes a photo's settings without a commit: compare the settings too.
    let moved = completed.settings.iter().any(|(id, before, _)| app.session.develop_of(*id).is_none_or(|now| now != *before));
    if app.session.library_generation() != completed.library || app.session.commits() != completed.commits || moved {
        return Err(crate::i18n::tr("Zap! was not applied because the library changed; try again").into());
    }
    let ops: Vec<Op> = completed.settings.into_iter().filter_map(|(id, _, d)| app.session.develop_op(id, d, negative_zap::LABEL)).collect();
    let changed = ops.len();
    if changed > 0 {
        // through the command wrapper, so sidecars and the durable save follow as for any edit
        app.session
            .execute_fn(COMMAND, |s| {
                s.commit(negative_zap::LABEL, Op::Batch { ops })?;
                Ok(Value::Null)
            })
            .map_err(|e| e.to_string())?;
    }
    Ok((changed, completed.failed.len(), completed.roll))
}

/// The toast after a run in the background.
fn done_message(changed: usize, left: usize, roll: Option<&negative_zap::Roll>) -> String {
    let mut m = match left {
        0 => crate::i18n::tr_format!("Zap!: {changed} photo(s)", changed = changed),
        _ => crate::i18n::tr_format!("Zap!: {changed} photo(s), no frame found on {left}", changed = changed, left = left),
    };
    if let Some(r) = roll {
        m.push_str(". ");
        m.push_str(&crate::i18n::tr_format!("{frames} measured as one roll, the film base from {bases}", frames = r.frames.len(), bases = r.bases));
        if !r.apart.is_empty() {
            m.push_str(". ");
            m.push_str(&crate::i18n::tr_format!("{apart} with another film base measured on their own", apart = r.apart.len()));
        }
    }
    m
}

/// `develop.negativeZap` from the app: one photo at once (its reply as the engine gives it), a
/// selection in the background (`{pending: true, photos}`).
pub(crate) fn command(app: &mut LightcraftApp, params: &Value) -> Result<Value, String> {
    let p = with_panel_choices(app, params);
    let ids = app.session.targets(&p).map_err(|e| e.to_string())?;
    if ids.len() <= 1 {
        return app.session.execute(COMMAND, &p).map_err(|e| e.to_string());
    }
    if app.tasks.is_running(LABEL) {
        return Err(crate::i18n::tr("Zap! is already running").into());
    }
    let prepared = prepare(app, &p, ids)?;
    let count = prepared.inputs.len();
    let cancel = Arc::new(AtomicBool::new(false));
    crate::tasks::spawn_tracked(
        app,
        LABEL,
        "zap",
        cancel,
        move |progress| prepared.run(Some(&progress)),
        |app, ctx, result| match result.and_then(|done| apply(app, done)) {
            Ok((changed, left, roll)) => app.toast(ctx, done_message(changed, left, roll.as_ref())),
            Err(error) => app.toast(ctx, error),
        },
    )?;
    Ok(json!({"pending": true, "photos": count}))
}
