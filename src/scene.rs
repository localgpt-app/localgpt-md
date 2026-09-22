//! Bevy side: spawn the current `WorldManifest`, and respawn it whenever the
//! document changes.
//!
//! The mapping mirrors LocalGPT Gen's (sRGB colours, linear emissive, XYZ
//! rotations in degrees, lux for directional lights and lumens for the rest),
//! so a manifest looks the same in both. Behaviors, audio, NPCs, and mesh
//! assets are not rendered yet.

use std::collections::HashMap;
use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_4};

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use localgpt_world_types as wt;

use crate::doc::Doc;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, rebuild.run_if(resource_changed::<CurrentWorld>));
    }
}

/// The document on screen and the world compiled from it.
#[derive(Resource)]
pub struct CurrentWorld {
    pub doc: Doc,
    pub manifest: wt::WorldManifest,
}

/// The single camera; [`crate::tour`] flies it between waypoints.
#[derive(Component)]
pub struct TourCamera;

/// Everything spawned from the manifest, despawned on rebuild.
#[derive(Component)]
struct FromManifest;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        TourCamera,
        Camera3d::default(),
        Hdr,
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Transform::from_xyz(0.0, 4.0, 14.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
    ));
}

fn rebuild(
    mut commands: Commands,
    world: Res<CurrentWorld>,
    old: Query<Entity, (With<FromManifest>, Without<ChildOf>)>,
    camera: Query<Entity, With<TourCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Despawning a root takes its children with it.
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let manifest = &world.manifest;
    if let Ok(camera) = camera.single() {
        apply_camera(&mut commands, camera, manifest);
    }
    apply_environment(&mut commands, manifest.environment.as_ref());

    let mut spawned = HashMap::new();
    for def in &manifest.entities {
        let entity = spawn_entity(&mut commands, def, &mut meshes, &mut materials);
        spawned.insert(def.id, entity);
    }
    for def in &manifest.entities {
        let parent = def.parent.and_then(|id| spawned.get(&id));
        if let (Some(&parent), Some(&child)) = (parent, spawned.get(&def.id)) {
            commands.entity(child).insert(ChildOf(parent));
        }
    }
}

fn apply_camera(commands: &mut Commands, camera: Entity, manifest: &wt::WorldManifest) {
    let mut camera = commands.entity(camera);
    let fov = manifest.camera.as_ref().map_or(50.0, |c| c.fov_degrees);
    camera.insert(Projection::Perspective(PerspectiveProjection {
        fov: fov.to_radians(),
        ..default()
    }));
    let env = manifest.environment.as_ref();
    match env.and_then(|e| e.fog_density) {
        Some(density) => {
            let color = env.and_then(|e| e.fog_color.or(e.background_color));
            camera.insert(DistanceFog {
                color: color.map_or(Color::WHITE, srgba),
                falloff: FogFalloff::Exponential { density },
                ..default()
            });
        }
        None => {
            camera.remove::<DistanceFog>();
        }
    }
}

fn apply_environment(commands: &mut Commands, env: Option<&wt::EnvironmentDef>) {
    let background = env.and_then(|e| e.background_color);
    commands.insert_resource(background.map_or_else(ClearColor::default, |c| ClearColor(srgba(c))));

    let mut ambient = GlobalAmbientLight::default();
    if let Some(env) = env {
        if let Some(color) = env.ambient_color {
            ambient.color = srgba(color);
        }
        if let Some(brightness) = env.ambient_intensity {
            ambient.brightness = brightness;
        }
    }
    commands.insert_resource(ambient);
}

fn spawn_entity(
    commands: &mut Commands,
    def: &wt::WorldEntity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let t = &def.transform;
    let [rx, ry, rz] = t.rotation_degrees.map(f32::to_radians);
    let mut transform = Transform {
        translation: Vec3::from_array(t.position),
        rotation: Quat::from_euler(EulerRot::XYZ, rx, ry, rz),
        scale: Vec3::from_array(t.scale),
    };
    // Directional and spot lights aim along `direction`, with Gen's defaults.
    if let Some(light) = &def.light {
        let default_direction = match light.light_type {
            wt::LightType::Directional => Some([0.0, -1.0, -0.5]),
            wt::LightType::Spot => Some([0.0, -1.0, 0.0]),
            wt::LightType::Point => None,
        };
        if let Some(direction) = light.direction.or(default_direction) {
            transform.look_to(Vec3::from_array(direction), Vec3::Y);
        }
    }
    let visibility = if t.visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    let mut entity = commands.spawn((
        FromManifest,
        Name::new(def.name.0.clone()),
        transform,
        visibility,
    ));
    if let Some(shape) = &def.shape {
        let material = def
            .material
            .as_ref()
            .map(standard_material)
            .unwrap_or_default();
        entity.insert((
            Mesh3d(meshes.add(shape_mesh(shape))),
            MeshMaterial3d(materials.add(material)),
        ));
    }
    if let Some(light) = &def.light {
        insert_light(&mut entity, light);
    }
    entity.id()
}

fn insert_light(entity: &mut EntityCommands, light: &wt::LightDef) {
    let color = srgba(light.color);
    match light.light_type {
        wt::LightType::Directional => {
            entity.insert(DirectionalLight {
                illuminance: light.intensity,
                shadow_maps_enabled: light.shadows,
                color,
                ..default()
            });
        }
        wt::LightType::Point => {
            let mut point = PointLight {
                intensity: light.intensity,
                shadow_maps_enabled: light.shadows,
                color,
                ..default()
            };
            if let Some(range) = light.range {
                point.range = range;
            }
            entity.insert(point);
        }
        wt::LightType::Spot => {
            let mut spot = SpotLight {
                intensity: light.intensity,
                shadow_maps_enabled: light.shadows,
                color,
                ..default()
            };
            if let Some(range) = light.range {
                spot.range = range;
            }
            if let Some(outer) = light.outer_angle {
                spot.outer_angle = outer;
            }
            if let Some(inner) = light.inner_angle {
                spot.inner_angle = inner;
            }
            entity.insert(spot);
        }
    }
}

/// Same conversion as Gen's `material_def_to_standard`.
fn standard_material(def: &wt::MaterialDef) -> StandardMaterial {
    let [er, eg, eb, ea] = def.emissive;
    let mut material = StandardMaterial {
        base_color: srgba(def.color),
        metallic: def.metallic,
        perceptual_roughness: def.roughness,
        emissive: LinearRgba::new(er, eg, eb, ea),
        ..default()
    };
    if let Some(alpha) = def.alpha_mode {
        material.alpha_mode = match alpha {
            wt::AlphaModeDef::Opaque => AlphaMode::Opaque,
            wt::AlphaModeDef::Mask(cutoff) => AlphaMode::Mask(cutoff),
            wt::AlphaModeDef::Blend => AlphaMode::Blend,
            wt::AlphaModeDef::Add => AlphaMode::Add,
            wt::AlphaModeDef::Multiply => AlphaMode::Multiply,
        };
    }
    if let Some(unlit) = def.unlit {
        material.unlit = unlit;
    }
    if let Some(double_sided) = def.double_sided {
        material.double_sided = double_sided;
    }
    if let Some(reflectance) = def.reflectance {
        material.reflectance = reflectance;
    }
    material
}

/// A mesh for every `Shape`. Bevy has no pyramid or wedge primitive, so those
/// are a four-sided cone and an extruded right triangle.
fn shape_mesh(shape: &wt::Shape) -> Mesh {
    match *shape {
        wt::Shape::Cuboid { x, y, z } => Cuboid::new(x, y, z).into(),
        wt::Shape::Sphere { radius } => Sphere::new(radius).mesh().uv(32, 18),
        wt::Shape::Cylinder { radius, height } => Cylinder::new(radius, height).into(),
        wt::Shape::Cone { radius, height } => Cone::new(radius, height).into(),
        wt::Shape::Capsule {
            radius,
            half_length,
        } => Capsule3d::new(radius, half_length * 2.0).into(),
        wt::Shape::Torus {
            major_radius,
            minor_radius,
        } => Torus {
            minor_radius,
            major_radius,
        }
        .into(),
        wt::Shape::Plane { x, z } => Plane3d::default().mesh().size(x, z).into(),
        wt::Shape::Pyramid {
            base_x,
            base_z,
            height,
        } => Mesh::from(Cone::new(FRAC_1_SQRT_2, height).mesh().resolution(4))
            .rotated_by(Quat::from_rotation_y(FRAC_PI_4))
            .scaled_by(Vec3::new(base_x, 1.0, base_z)),
        // Bevy's default tetrahedron has a circumradius of √0.75.
        wt::Shape::Tetrahedron { radius } => {
            Mesh::from(Tetrahedron::default()).scaled_by(Vec3::splat(radius / 0.75_f32.sqrt()))
        }
        wt::Shape::Icosahedron { radius } => Sphere::new(radius)
            .mesh()
            .ico(0)
            .unwrap_or_else(|_| Sphere::new(radius).mesh().uv(8, 6)),
        wt::Shape::Wedge { x, y, z } => {
            let (hx, hy) = (x / 2.0, y / 2.0);
            let profile =
                Triangle2d::new(Vec2::new(-hx, -hy), Vec2::new(hx, -hy), Vec2::new(-hx, hy));
            Extrusion::new(profile, z).into()
        }
    }
}

fn srgba([r, g, b, a]: [f32; 4]) -> Color {
    Color::srgba(r, g, b, a)
}
