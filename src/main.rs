//! LocalGPT MD — open a Markdown file and walk through it as a 3D world.
//!
//! Scaffold milestone (PLAN.md M0): every `##` section becomes a place laid
//! out by a rule-based draft, ←/→ walks between them, and saving the file
//! rebuilds the world. The local-LLM tiers ported from LocalGPT Verse come
//! next (PLAN.md M1).
//!
//! Pipeline: [`doc`] (Markdown → sections) → [`draft`] (sections →
//! `WorldManifest`, LocalGPT's shared world format) → [`scene`] (manifest →
//! Bevy), with [`tour`] for navigation and [`watch`] for hot reload.

mod doc;
mod draft;
mod scene;
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
Usage: localgpt-md [FILE.md] [--print-ron]

Opens FILE.md (default: samples/hello.md) as a walkable 3D world. Each ##
section becomes a place, and the world rebuilds whenever the file is saved.

  --print-ron   Print the compiled world as RON (LocalGPT Gen's world.ron
                format) and exit without opening a window.

Environment:
  LOCALGPT_MD_SCREENSHOT=out.png   Render the first stop offscreen (no window),
                                   save it as a PNG, and exit.";

const DEFAULT_DOC: &str = "samples/hello.md";

fn main() -> AppExit {
    let mut path = None;
    let mut print_ron = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--print-ron" => print_ron = true,
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

    let world = match load(&path) {
        Ok(world) => world,
        Err(err) => {
            eprintln!("localgpt-md: {}: {err}", path.display());
            return AppExit::error();
        }
    };
    for issue in draft::validate(&world.manifest) {
        eprintln!("localgpt-md: {:?}: {}", issue.severity, issue.message);
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
        .add_plugins((scene::ScenePlugin, tour::TourPlugin, watch::WatchPlugin));

    if let Some(path) = smoke {
        app.insert_resource(Smoke::new(path))
            .add_systems(PostStartup, smoke_target)
            .add_systems(Update, smoke_screenshot);
    }
    app.run()
}

/// Read, parse, and compile a document.
fn load(path: &Path) -> std::io::Result<CurrentWorld> {
    let src = std::fs::read_to_string(path)?;
    let fallback_title = path.file_stem().map_or_else(
        || "Untitled".into(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let doc = doc::Doc::parse(&src, &fallback_title);
    let manifest = draft::compile(&doc);
    Ok(CurrentWorld { doc, manifest })
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
