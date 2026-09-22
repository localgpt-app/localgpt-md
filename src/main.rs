//! LocalGPT MD — open a Markdown file and walk through it as a 3D world.
//!
//! Every `##` section becomes a place: a rule-based draft lays it out
//! instantly (PLAN.md M0), the local LLM restyles it from the prose when the
//! `llm` feature and a model are present (M1), and results are cached per
//! section hash in a sidecar next to the document (M2) so nothing is ever
//! generated twice. ←/→ walks between sections; saving the file rebuilds.
//!
//! Pipeline: [`doc`] (Markdown → sections) → [`draft`] (sections →
//! `WorldManifest`, LocalGPT's shared world format, styled by [`recipe`] +
//! [`sidecar`] entries) → [`scene`] (manifest → Bevy), with [`tour`] for
//! navigation, [`watch`] for hot reload, and [`llm`]/[`gen`] for authoring.

mod doc;
mod draft;
#[cfg(feature = "llm")]
mod generation;
#[cfg(feature = "llm")]
mod llm;
mod recipe;
mod scene;
mod sidecar;
mod tour;
mod watch;

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::camera::{RenderTarget, ShadowLodOrigin};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::ui::IsDefaultUiCamera;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

use crate::scene::{CurrentWorld, TourCamera};

const USAGE: &str = "\
Usage: localgpt-md [FILE.md] [--print-ron] [--generate]

Opens FILE.md (default: samples/hello.md) as a walkable 3D world. Each ##
section becomes a place, and the world rebuilds whenever the file is saved.

  --print-ron   Print the compiled world as RON (LocalGPT Gen's world.ron
                format) and exit without opening a window. Cached LLM recipes
                (the .world.json sidecar) are applied.
  --generate    Have the local LLM style every section without a cached
                recipe, write the sidecar, and exit — no window. Needs the
                `llm`/`llm-metal` feature and a model (scripts/fetch-bonsai.sh).

Environment:
  LOCALGPT_MD_SCREENSHOT=out.png   Render the first stop offscreen (no window),
                                   save it as a PNG, and exit.
  LOCALGPT_MD_LLM=dir              Model directory (default: assets/llm, then
                                   ../localgpt-verse/assets/llm).";

const DEFAULT_DOC: &str = "samples/hello.md";

fn main() -> AppExit {
    let mut path = None;
    let mut print_ron = false;
    let mut generate = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--print-ron" => print_ron = true,
            "--generate" => generate = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return AppExit::Success;
            }
            flag if flag.starts_with('-') => {
                eprintln!("localgpt-md: unknown option {flag}\n\n{USAGE}");
                return AppExit::error();
            }
            _ => path = Some(PathBuf::from(&arg)),
        }
    }
    let path = path.unwrap_or_else(|| PathBuf::from(DEFAULT_DOC));
    let sidecar_path = path.with_extension("world.json");
    let store = sidecar::RecipeStore::load(&sidecar_path);

    let world = match load(&path, &store) {
        Ok(world) => world,
        Err(err) => {
            eprintln!("localgpt-md: {}: {err}", path.display());
            return AppExit::error();
        }
    };
    for issue in draft::validate(&world.manifest) {
        eprintln!("localgpt-md: {:?}: {}", issue.severity, issue.message);
    }

    if generate {
        #[cfg(feature = "llm")]
        {
            return generate_headless(&world, store, &sidecar_path);
        }
        #[cfg(not(feature = "llm"))]
        {
            eprintln!(
                "localgpt-md: --generate needs a build with the llm feature:\n  \
                 cargo run --features llm-metal -- {} --generate",
                path.display()
            );
            return AppExit::error();
        }
    }

    if print_ron {
        let config = ron::ser::PrettyConfig::default();
        return match ron::ser::to_string_pretty(&world.manifest, config) {
            Ok(text) => {
                println!("{text}");
                AppExit::Success
            }
            Err(err) => {
                eprintln!("localgpt-md: {err}");
                AppExit::error()
            }
        };
    }

    let smoke = std::env::var("LOCALGPT_MD_SCREENSHOT").ok();
    let mut app = App::new();
    if smoke.is_some() {
        // Windowless: reading back a window's frame returns black on a locked
        // or headless Mac, so the smoke shot renders to an offscreen image.
        app.add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>(),
            ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)),
        ));
    } else {
        let window = Window {
            title: format!("{} — LocalGPT MD", world.doc.title),
            resolution: (1280, 800).into(),
            ..default()
        };
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(window),
            ..default()
        }));
    }
    app.insert_resource(watch::DocSource::new(path))
        .insert_resource(world)
        .insert_resource(store)
        .add_plugins((scene::ScenePlugin, tour::TourPlugin, watch::WatchPlugin));

    // The worker owns a multi-GB model; skip it in the screenshot smoke run
    // (which exits in seconds). Cached recipes still apply via the store.
    #[cfg(feature = "llm")]
    if smoke.is_none() {
        app.add_plugins(generation::GenerationPlugin);
    }

    if let Some(path) = smoke {
        app.insert_resource(Smoke::new(path))
            .add_systems(PostStartup, smoke_target)
            .add_systems(Update, smoke_screenshot);
    }
    app.run()
}

/// Read, parse, and compile a document, applying whatever recipes the
/// sidecar already holds for its sections.
fn load(path: &Path, recipes: &sidecar::RecipeStore) -> std::io::Result<CurrentWorld> {
    let src = std::fs::read_to_string(path)?;
    let fallback_title = path.file_stem().map_or_else(
        || "Untitled".into(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let doc = doc::Doc::parse(&src, &fallback_title);
    let manifest = draft::compile_with(&doc, recipes);
    Ok(CurrentWorld { doc, manifest })
}

/// `--generate` (PLAN.md M1): style every uncached section with the local
/// LLM, headless — no window, no Bevy — then write the sidecar. One line per
/// section keeps progress visible in a terminal or CI log.
#[cfg(feature = "llm")]
fn generate_headless(
    world: &CurrentWorld,
    mut store: sidecar::RecipeStore,
    sidecar_path: &Path,
) -> AppExit {
    let mut model = match llm::RecipeModel::try_load() {
        Some(model) => model,
        None => {
            eprintln!("localgpt-md: no model found — run scripts/fetch-bonsai.sh");
            return AppExit::error();
        }
    };
    let mut fresh = 0;
    let mut failed = 0;
    for section in &world.doc.sections {
        if store.get(&section.hash).is_some() {
            println!("have    {}", section.heading);
            continue;
        }
        let started = std::time::Instant::now();
        match model.generate(&section.heading, &section.body, world.doc.genre()) {
            Some(recipe) => {
                store.insert(&section.hash, recipe);
                fresh += 1;
                println!(
                    "styled  {} ({:.1}s)",
                    section.heading,
                    started.elapsed().as_secs_f32()
                );
            }
            None => failed += 1,
        }
    }
    if let Err(err) = store.save(&world.doc) {
        eprintln!("localgpt-md: {}: {err}", sidecar_path.display());
        return AppExit::error();
    }
    println!(
        "{} styled, {} failed, {} cached -> {}",
        fresh,
        failed,
        store.len(),
        sidecar_path.display()
    );
    if fresh == 0 && failed > 0 {
        AppExit::error()
    } else {
        AppExit::Success
    }
}

/// `LOCALGPT_MD_SCREENSHOT=out.png`: render the first stop offscreen once
/// shaders have warmed up, save it, and exit — a visual check that needs no
/// human and no display.
#[derive(Resource)]
struct Smoke {
    path: String,
    target: Option<Handle<Image>>,
    frame: u32,
    saved: bool,
}

impl Smoke {
    /// Frames to wait before capturing, so pipelines have compiled.
    const WARM_UP: u32 = 240;
    /// Give up after this many frames if no capture arrives.
    const TIMEOUT: u32 = 1200;

    fn new(path: String) -> Self {
        Self {
            path,
            target: None,
            frame: 0,
            saved: false,
        }
    }
}

/// Point the camera at an offscreen image the size of the normal window.
fn smoke_target(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    camera: Query<Entity, With<TourCamera>>,
    mut smoke: ResMut<Smoke>,
) {
    let image = Image::new_target_texture(1280, 800, TextureFormat::Rgba8UnormSrgb, None);
    let target = images.add(image);
    // With no window, Bevy won't pick this camera for UI or shadow LOD on its own.
    if let Ok(camera) = camera.single() {
        commands.entity(camera).insert((
            RenderTarget::Image(target.clone().into()),
            IsDefaultUiCamera,
            ShadowLodOrigin,
        ));
    }
    smoke.target = Some(target);
}

fn smoke_screenshot(
    mut commands: Commands,
    mut smoke: ResMut<Smoke>,
    mut exit: MessageWriter<AppExit>,
) {
    smoke.frame += 1;
    if smoke.frame == Smoke::WARM_UP
        && let Some(target) = smoke.target.clone()
    {
        commands
            .spawn(Screenshot::image(target))
            .observe(save_to_disk(smoke.path.clone()))
            .observe(|_: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| smoke.saved = true);
    }
    // `save_to_disk` writes synchronously, so the file is complete by now.
    if smoke.saved {
        exit.write(AppExit::Success);
    } else if smoke.frame > Smoke::TIMEOUT {
        error!("no screenshot captured after {} frames", Smoke::TIMEOUT);
        exit.write(AppExit::error());
    }
}
