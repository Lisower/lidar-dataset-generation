//! Элементы фасадов: окна, двери, балконы.
//!
//! Горизонталь и вертикаль: стена — блок высотой в этаж, его локальная ось Z
//! (вертикаль) идёт от низа этажа к верху. Вложенные элементы (окна, двери,
//! балконы) позиционируются относительно ЦЕНТРА стены, а сама стена — так,
//! чтобы её низ совпадал с низом этажа (см. `wall_part`).

use bevy::color::Color;
use bevy::math::Vec3;
use bevy::transform::components::Transform;
use generation_core::point_clouds::primitives::{GridNoise, apply_noise, polygon_points};

use super::config::BuildingConfig;
use super::palette::{BALCONY_RAILING, BALCONY_SLAB, FRAME};
use super::part::Part;

/// Стена здания (фасад) по стороне света.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facade {
    /// +Y.
    North,
    /// +X.
    East,
    /// −Y.
    South,
    /// −X.
    West,
}

impl Facade {
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    /// Вектор «наружу» от стены (мировые оси).
    pub fn outward(self) -> Vec3 {
        match self {
            Self::North => Vec3::Y,
            Self::East => Vec3::X,
            Self::South => -Vec3::Y,
            Self::West => -Vec3::X,
        }
    }

    /// Горизонтальная ось вдоль стены.
    pub fn along(self) -> Vec3 {
        match self {
            Self::North | Self::South => Vec3::X,
            Self::East | Self::West => Vec3::Y,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::North => "север",
            Self::East => "восток",
            Self::South => "юг",
            Self::West => "запад",
        }
    }
}

/// Данные стены для построения её элементов.
pub struct WallCtx {
    pub facade: Facade,
    /// Центр стены по длине и толщине (координаты этажа, z = 0 у низа).
    pub center: Vec3,
    /// Толщина стены.
    pub thickness: f32,
}

impl WallCtx {
    /// Позиция на внешней грани стены (относительно центра стены).
    pub fn outer_point(&self) -> Vec3 {
        self.facade.outward() * (self.thickness * 0.5)
    }

    /// Позиция на внешней грани со сдвигом вдоль стены и по вертикали
    /// (относительно ЦЕНТРА стены — так позиционируются дочерние сущности
    /// вложенных элементов).
    pub fn local_point(&self, along: f32, z_from_center: f32) -> Vec3 {
        self.outer_point() + self.facade.along() * along + Vec3::Z * z_from_center
    }
}

/// Прямоугольный проём на внешней грани стены (в локальных координатах
/// стены: `along` — вдоль неё, `z` — от центра стены по вертикали).
#[derive(Debug, Clone, Copy)]
pub struct FaceCutout {
    pub along_min: f32,
    pub along_max: f32,
    pub z_min: f32,
    pub z_max: f32,
}

/// Размер бокса с осями из `outward`/`along`: (ширина, глубина, высота).
fn sized(along_x: bool, w: f32, d: f32, h: f32) -> Vec3 {
    if along_x {
        Vec3::new(w, d, h)
    } else {
        Vec3::new(d, w, h)
    }
}

/// Стена: блок высотой `floor_height`, низ — на уровне низа этажа.
///
/// Внешняя грань для облака точек формируется без проёмов `cutouts` (окна,
/// двери, балконы), чтобы облако стены не дублировало точки своих элементов.
pub fn wall_part(
    ctx: &WallCtx,
    length_along: f32,
    floor_height: f32,
    color: Color,
    config: &BuildingConfig,
    cutouts: &[FaceCutout],
) -> Part {
    let outward = ctx.facade.outward();
    let size = if ctx.facade.along() == Vec3::X {
        Vec3::new(length_along, ctx.thickness, floor_height)
    } else {
        Vec3::new(ctx.thickness, length_along, floor_height)
    };

    // Облако внешней грани: вся грань минус проёмы. У стен сетка крупнее,
    // чем у мелких элементов/крыши, чтобы стены не выглядели плотнее.
    let face = outer_face_polygon(size, outward);
    let step = if config.wall_point_step > 0.0 {
        config.wall_point_step
    } else {
        config.point_step
    };
    let mut positions = polygon_points(&face, step);
    if !cutouts.is_empty() {
        positions.retain(|p| !inside_cutout(ctx, *p, cutouts));
    }
    apply_noise(
        &mut positions,
        GridNoise {
            amplitude: config.point_jitter,
        },
        config.seed.with_label("стена"),
    );

    // Стена стоит на полу этажа: центр блока — на высоте floor_height/2.
    let transform = Transform::from_xyz(ctx.center.x, ctx.center.y, floor_height * 0.5);
    Part::boxed_cloud_positions("стена", size, color, transform, positions, config)
}

/// Попадает ли точка внешней грани (локальные координаты стены) внутрь
/// какого-то проёма.
fn inside_cutout(ctx: &WallCtx, p: Vec3, cutouts: &[FaceCutout]) -> bool {
    let along = if ctx.facade.along() == Vec3::X {
        p.x
    } else {
        p.y
    };
    cutouts.iter().any(|c| {
        along >= c.along_min && along <= c.along_max && p.z >= c.z_min && p.z <= c.z_max
    })
}

/// Полигон внешней грани параллелепипеда (для облака точек).
fn outer_face_polygon(size: Vec3, outward: Vec3) -> generation_core::geometry::Polygon {
    let outward = outward.normalize();
    // Ось «вдоль» перпендикулярна outward (выбираем оси X или Y).
    let along = if outward.x.abs() < 0.9 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let up = outward.cross(along).normalize();
    let h_along = along.x * size.x + along.y * size.y + along.z * size.z;
    let h_up = up.x * size.x + up.y * size.y + up.z * size.z;
    let h_out = outward.x * size.x + outward.y * size.y + outward.z * size.z;

    let center = outward * (h_out * 0.5);
    let c1 = center + along * (h_along * 0.5) + up * (h_up * 0.5);
    let c2 = center - along * (h_along * 0.5) + up * (h_up * 0.5);
    let c3 = center - along * (h_along * 0.5) - up * (h_up * 0.5);
    let c4 = center + along * (h_along * 0.5) - up * (h_up * 0.5);
    generation_core::geometry::Polygon::quad(c1, c2, c3, c4).oriented(outward)
}

/// Окно как группа: рама + стекло.
///
/// Группа позиционируется относительно центра стены: `offset` — вдоль,
/// `z_from_center` — по вертикали (0 = центр стены = середина этажа).
/// Ширина окна `width` может меняться от окна к окну (высота — общая из
/// `config`).
pub fn window_part(
    ctx: &WallCtx,
    offset: f32,
    z_from_center: f32,
    width: f32,
    window_color: Color,
    config: &BuildingConfig,
) -> Part {
    let along_x = ctx.facade.along() == Vec3::X;
    let wh = config.window_height;
    // Шаг сетки облака точек у окон должен совпадать с шагом стены,
    // чтобы фасад выглядел единой равномерной решёткой.
    let fstep = facade_step(config);

    let base = ctx.local_point(offset, z_from_center);
    // База группы — внешняя грань стены на высоте центра окна.
    Part::group(
        "окно",
        window_color,
        Transform::from_translation(base),
    )
    .with_child(Part::boxed_step(
        "рама",
        sized(along_x, width + 0.12, 0.08, wh + 0.16),
        ctx.facade.outward(),
        FRAME,
        Transform::IDENTITY,
        config,
        fstep,
    ))
    .with_child(Part::boxed_step(
        "стекло",
        sized(along_x, width - 0.1, 0.04, wh - 0.12),
        ctx.facade.outward(),
        window_color,
        Transform::from_translation(ctx.facade.outward() * 0.07),
        config,
        fstep,
    ))
}

/// Дверь (группа: коробка + полотно) на внешней грани стены.
///
/// `z_from_center` — вертикаль центра двери относительно центра стены;
/// `height` — высота проёма. Для двери на полу этажа:
/// `height*0.5 − floor_height*0.5`, `height = config.door_height`.
pub fn door_part(
    ctx: &WallCtx,
    z_from_center: f32,
    door_color: Color,
    height: f32,
    config: &BuildingConfig,
) -> Part {
    let along_x = ctx.facade.along() == Vec3::X;
    let dw = config.door_width;
    let dh = height;
    let fstep = facade_step(config);

    Part::group(
        "дверь",
        door_color,
        Transform::from_translation(ctx.local_point(0.0, z_from_center)),
    )
    .with_child(Part::boxed_step(
        "дверная-коробка",
        sized(along_x, dw + 0.16, 0.1, dh + 0.14),
        ctx.facade.outward(),
        FRAME,
        Transform::IDENTITY,
        config,
        fstep,
    ))
    .with_child(Part::boxed_step(
        "дверь-полотно",
        sized(along_x, dw - 0.06, 0.04, dh - 0.04),
        ctx.facade.outward(),
        door_color,
        // Полотно должно выступать НЕМНОГО за переднюю грань коробки
        // (коробка: глубина 0.1, её передняя грань на +0.05; полотно
        // глубиной 0.04 по центру +0.08 → занимает [+0.06, +0.10]).
        // Тогда его тыльная грань не копланарна грани коробки, и у двери
        // не получается «меш в меше» (дёрганье цвета).
        Transform::from_translation(ctx.facade.outward() * 0.08),
        config,
        fstep,
    ))
}

/// Балкон: плита + ограждение (и маленькая дверь на стену).
///
/// Балкон — дочерний элемент стены; все его части позиционируются
/// относительно центра стены.
#[allow(clippy::too_many_arguments)]
pub fn balcony_part(
    ctx: &WallCtx,
    floor_height: f32,
    door_color: Color,
    balcony_color: Color,
    config: &BuildingConfig,
) -> Part {
    let along_x = ctx.facade.along() == Vec3::X;
    let outward = ctx.facade.outward();
    let along = ctx.facade.along();
    let t = ctx.thickness;

    let bw = config.balcony_width;
    let bd = config.balcony_depth;
    // Высота плиты (от низа этажа) и центр ограждения (от центра стены).
    let slab_z = config.balcony_floor_level - floor_height * 0.5 + 0.06;
    let railing_z = config.balcony_floor_level - floor_height * 0.5 + 0.12
        + config.balcony_railing_height * 0.5;

    let slab_size = sized(along_x, bw, bd, 0.12);
    let front_size = sized(along_x, bw, 0.08, config.balcony_railing_height);
    // Для торцов: тонкая сторона должна быть вдоль стены.
    let end_size = if along_x {
        Vec3::new(0.08, bd, config.balcony_railing_height)
    } else {
        Vec3::new(bd, 0.08, config.balcony_railing_height)
    };

    // Дверь из комнаты на балкон — на стене, вход на уровне плиты.
    // Высота двери не выше этажа; верх — у потолка этажа.
    let door_height = (floor_height - config.balcony_floor_level).clamp(1.4, config.door_height);
    let door = door_part(
        ctx,
        config.balcony_floor_level - floor_height * 0.5 + door_height * 0.5,
        door_color,
        door_height,
        config,
    );
    // Шаг сетки у балкона — как у стены (единая решётка фасада).
    let fstep = facade_step(config);

    Part::group(
        "балкон",
        balcony_color,
        // Сам балкон — в центре стены; дети позиционируются относительно него.
        Transform::IDENTITY,
    )
    .with_child(door)
    .with_child(Part::boxed_outwards_step(
        "балкон-плита",
        slab_size,
        // Пол (верх), передняя кромка и торцы плиты — чтобы точки были
        // и на полу балкона, и по его бокам.
        &[outward, Vec3::Z, along, -along],
        BALCONY_SLAB,
        Transform::from_translation(
            outward * (t * 0.5 + bd * 0.5) + Vec3::Z * slab_z,
        ),
        config,
        fstep,
    ))
    .with_child(Part::boxed_outwards_step(
        "балкон-ограждение",
        front_size,
        // Передняя грань и верхняя кромка ограждения.
        &[outward, Vec3::Z],
        BALCONY_RAILING,
        Transform::from_translation(
            outward * (t * 0.5 + bd - 0.04) + Vec3::Z * railing_z,
        ),
        config,
        fstep,
    ))
    .with_child(Part::boxed_outwards_step(
        "балкон-ограждение-торцы",
        end_size,
        // Наружная грань левого торца (вдоль стены) и его верх.
        &[-along, Vec3::Z],
        BALCONY_RAILING,
        Transform::from_translation(
            outward * (t * 0.5 + bd * 0.5) - along * (bw * 0.5 - 0.04)
                + Vec3::Z * railing_z,
        ),
        config,
        fstep,
    ))
    .with_child(Part::boxed_outwards_step(
        "балкон-ограждение-торцы",
        end_size,
        // Наружная грань правого торца (вдоль стены) и его верх.
        &[along, Vec3::Z],
        BALCONY_RAILING,
        Transform::from_translation(
            outward * (t * 0.5 + bd * 0.5) + along * (bw * 0.5 - 0.04)
                + Vec3::Z * railing_z,
        ),
        config,
        fstep,
    ))
}

/// Шаг сетки облака точек фасадных элементов (окна, двери, балконы) —
/// такой же, как у стен, чтобы фасад был единой решёткой.
fn facade_step(config: &BuildingConfig) -> f32 {
    if config.wall_point_step > 0.0 {
        config.wall_point_step
    } else {
        config.point_step
    }
}
