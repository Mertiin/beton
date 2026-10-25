//! Compile-time / CPU-side check that the public API stays constructible without a GPU.

use beton::{
    integrate_velocities, step_rigid_bodies, Animation, Camera, Config, Font, GameObject, Net,
    NetRole, PeerId, Projection, RigidBody, SoundId, Sprite, SpriteSheet, TextureId, Vec3,
    Velocity, World, MAX_PAYLOAD, MAX_PEERS,
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Body {
    vel: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Collider {
    radius: f32,
}

#[test]
fn public_types_construct() {
    let _config = Config::default();
    let net = Net::new();
    assert_eq!(net.role(), NetRole::Offline);
    assert_eq!(PeerId::HOST, PeerId(0));
    assert!(MAX_PAYLOAD > 0);
    assert!(MAX_PEERS > 0);

    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 20.0,
            near: 0.1,
            far: 100.0,
        },
    };
    let _vp = camera.view_proj(16.0 / 9.0);

    let texture = TextureId::from_raw(0);
    let _sound = SoundId::from_raw(0);
    let sprite = Sprite::new(texture, Vec3::ZERO, [1.0, 1.0]);
    let _font = Font {
        texture,
        columns: 10,
        rows: 1,
        first_char: '0',
    };

    let sheet = SpriteSheet {
        texture,
        columns: 3,
        rows: 1,
    };
    let mut anim = Animation::new(sheet.clone(), [0, 1, 2, 1], 10.0);
    anim.tick(0.1);
    let mut drawn = sprite.clone();
    anim.apply(&mut drawn);
    assert_ne!(drawn.uv, [0.0, 0.0, 1.0, 1.0]);
    let mut static_sprite = sprite.clone();
    sheet.apply(&mut static_sprite, 2);
    let _other = anim.instance();

    let mut world = World::new();
    let a = GameObject {
        pos: Vec3::ZERO,
        rotation: 0.0,
        scale: [1.0, 1.0],
        parent: None,
        size: [2.0, 2.0],
        sprite: Some(sprite.clone()),
        tag: 1,
    };
    let b = GameObject {
        pos: Vec3::new(0.5, 0.0, 0.0),
        rotation: 0.0,
        scale: [1.0, 1.0],
        parent: None,
        size: [2.0, 2.0],
        sprite: None,
        tag: 2,
    };
    let id_a = world.spawn(a);
    let id_b = world.spawn(b);
    assert!(world.overlaps(id_a, id_b));

    world.insert(id_a, Body { vel: Vec3::X });
    world.insert(id_a, Collider { radius: 0.5 });
    assert!(world.contains_component::<Body>(id_a));
    let mut stepped = 0;
    world.each::<(Body, Collider)>(|_id, object, (body, _col)| {
        object.pos += body.vel;
        stepped += 1;
    });
    assert_eq!(stepped, 1);
    assert!((world.get(id_a).unwrap().pos.x - 1.0).abs() < 1e-5);
    assert_eq!(
        world.get_component::<Collider>(id_a).unwrap().radius,
        0.5
    );

    world.despawn(id_a);
    assert!(world.get(id_a).is_none());
    assert!(!world.contains_component::<Body>(id_a));

    let mover = world.spawn(GameObject {
        pos: Vec3::ZERO,
        rotation: 0.0,
        scale: [1.0, 1.0],
        parent: None,
        size: [1.0, 1.0],
        sprite: None,
        tag: 0,
    });
    world.insert(mover, Velocity::linear(Vec3::new(2.0, 0.0, 0.0)));
    integrate_velocities(&mut world, 0.5);
    assert!((world.get(mover).unwrap().pos.x - 1.0).abs() < 1e-5);

    let crate_id = world.spawn(GameObject {
        pos: Vec3::new(0.0, 2.0, 0.0),
        rotation: 0.0,
        scale: [1.0, 1.0],
        parent: None,
        size: [1.0, 1.0],
        sprite: None,
        tag: 0,
    });
    world.insert(crate_id, Velocity::ZERO);
    world.insert(crate_id, RigidBody::dynamic(1.0));
    let _fixed = RigidBody::fixed();
    step_rigid_bodies(&mut world, 0.1, Vec3::new(0.0, -32.0, 0.0));
    assert!(world.get_component::<Velocity>(crate_id).unwrap().linear.y < 0.0);
}
