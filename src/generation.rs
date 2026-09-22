//! The generation worker (PLAN.md M1, `llm` feature): a plain `std::thread`
//! that owns the model and authors recipes section by section, plus the
//! app-side glue that enqueues uncached sections and applies results as they
//! arrive.
//!
//! The split mirrors Verse's analysis worker: heavy model state lives off the
//! main thread (a multi-GB GGUF never touches the renderer's memory), work
//! and results cross on `std::sync::mpsc` channels drained once per frame.
//! The draft renders immediately — every region with a cached recipe is
//! styled from the sidecar on load — and each remaining region upgrades in
//! place when its recipe lands. The rebuild is cheap and keyed to stable
//! entity ids, so the tour stop you're looking at stays put.
//!
//! Failures are sticky for the run: a section whose generation failed is not
//! retried until the app restarts (Verse's Tier rule — a missing model does
//! not appear mid-run). Editing the section changes its hash, which does
//! retry it.

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::mpsc;

use bevy::prelude::*;

use crate::llm;
use crate::recipe::RegionRecipe;
use crate::scene::CurrentWorld;
use crate::sidecar::RecipeStore;

pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (outcome_tx, outcome_rx) = mpsc::channel::<Outcome>();
        std::thread::spawn(move || worker(job_rx, outcome_tx));
        app.insert_resource(GenState {
            job_tx,
            // `mpsc::Receiver` is !Sync; the Mutex makes the resource shareable
            // (only the main thread ever locks it).
            outcome_rx: Mutex::new(outcome_rx),
            pending: HashSet::new(),
            failed: HashSet::new(),
        })
        .add_systems(Update, (enqueue_uncached, drain_outcomes).chain());
    }
}

/// One section to style. The body crosses as a prompt-ready excerpt so the
/// worker needs no access to app state.
struct Job {
    hash: blake3::Hash,
    heading: String,
    excerpt: String,
    genre: String,
}

struct Outcome {
    hash: blake3::Hash,
    recipe: Option<RegionRecipe>,
}

#[derive(Resource)]
struct GenState {
    job_tx: mpsc::Sender<Job>,
    outcome_rx: Mutex<mpsc::Receiver<Outcome>>,
    /// Section-hash keys with a job in flight.
    pending: HashSet<String>,
    /// Hashes that failed this run; not retried (see module docs).
    failed: HashSet<String>,
}

/// The worker thread: loads the model on the first job — so a document whose
/// sidecar is complete never pays for the load — and keeps it resident.
fn worker(jobs: mpsc::Receiver<Job>, outcomes: mpsc::Sender<Outcome>) {
    let mut model: Option<llm::RecipeModel> = None;
    while let Ok(job) = jobs.recv() {
        if model.is_none() {
            model = llm::RecipeModel::try_load();
        }
        let recipe = model
            .as_mut()
            .and_then(|m| m.generate(&job.heading, &job.excerpt, &job.genre));
        if outcomes
            .send(Outcome {
                hash: job.hash,
                recipe,
            })
            .is_err()
        {
            return; // app gone; stop
        }
    }
}

/// Send a job for every section that has neither a cached recipe, a job in
/// flight, nor a failure on record. Runs every frame — the loop is a few
/// hash-set lookups per section — so it picks up startup, hot reloads, and
/// anything the model just finished without extra bookkeeping.
fn enqueue_uncached(world: Res<CurrentWorld>, store: Res<RecipeStore>, mut jobs: ResMut<GenState>) {
    for section in &world.doc.sections {
        let key = section.hash.to_hex().to_string();
        if store.get(&section.hash).is_some()
            || jobs.pending.contains(&key)
            || jobs.failed.contains(&key)
        {
            continue;
        }
        let job = Job {
            hash: section.hash,
            heading: section.heading.clone(),
            excerpt: llm::prompt_excerpt(&section.body),
            genre: world.doc.genre().to_string(),
        };
        if jobs.job_tx.send(job).is_ok() {
            jobs.pending.insert(key);
        }
    }
}

/// Apply whatever the worker finished: cache it, persist the sidecar, and
/// recompile the manifest (which re-styles just the finished regions — the
/// rest keep the draft, cached recipes stay put). Writing the resource is
/// what triggers the scene rebuild and keeps the tour stop.
fn drain_outcomes(
    mut world: ResMut<CurrentWorld>,
    mut store: ResMut<RecipeStore>,
    mut jobs: ResMut<GenState>,
) {
    let mut applied = 0usize;
    let outcomes: Vec<Outcome> = {
        let rx = jobs.outcome_rx.lock().unwrap();
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    };
    for outcome in outcomes {
        let key = outcome.hash.to_hex().to_string();
        jobs.pending.remove(&key);
        if let Some(recipe) = outcome.recipe {
            store.insert(&outcome.hash, recipe);
            applied += 1;
        } else {
            jobs.failed.insert(key);
        }
    }
    if applied == 0 {
        return;
    }
    if let Err(err) = store.save(&world.doc) {
        warn!("llm: can't write sidecar: {err}");
    }
    info!(
        "llm: {applied} new recipe(s) applied ({} cached)",
        store.len()
    );
    let manifest = crate::draft::compile_with(&world.doc, &store);
    world.manifest = manifest;
}
