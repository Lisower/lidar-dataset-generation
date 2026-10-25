//! Геометрия крыш: плоская, односкатная, двускатная, вальмовая,
//! полувальмовая и сводчатая.
//!
//! Все крыши собираются в локальной системе координат «пятна крыши»:
//! начало координат — центр здания на уровне верха стен, ось Z вверх.
//! Свесы (overhang) выходят за пределы стен. Под скатами снизу крыша
//! закрыта карнизными досками и софитами — «пустоты» между крышей и стеной
//! нет.
//!
//! Ориентация (поворот скатов вокруг вертикали) выбирается ДО построения
//! геометрии: для 90°/270° крыша строится с «перевёрнутыми» габаритами, и
//! после поворота она встаёт точно на стены здания.

use bevy::math::{Mat3, Quat, Vec3};
use bevy::prelude::default;
use bevy::transform::components::Transform;
use generation_core::geometry::Polygon;
use rand::RngExt;

use super::config::BuildingConfig;
use super::config::RoofStyle;
use super::part::Part;
use super::elements::Facade;
use super::BuildingColors;

/// Толщина кровли (высота «карнизной доски»), берётся из конфигурации.
///
/// `ridge` — сдвиг гребня от центра (для двускатной/полувальмовой), чтобы
/// углы скатов различались.
pub fn roof_faces(
    style: RoofStyle,
    length: f32,
    width: f32,
    overhang: f32,
    pitch: f32,
    config: &BuildingConfig,
    ridge: f32,
) -> Vec<Polygon> {
    let t = config.roof_thickness;
    match style {
        RoofStyle::Flat => flat_polys(length, width, overhang, t),
        RoofStyle::Shed => shed_polys(length, width, overhang, pitch, t),
        RoofStyle::Gable => gable_polys(length, width, overhang, pitch, t, ridge),
        RoofStyle::Hip => hip_polys(length, width, overhang, pitch, t),
        RoofStyle::HalfHip => half_hip_polys(length, width, overhang, pitch, t, ridge),
        RoofStyle::Barrel => barrel_polys(length, width, overhang, pitch, t),
    }
}

/// Плоская крыша — тонкая плита со свесом (плюс, при желании, парапет).
fn flat_polys(length: f32, width: f32, overhang: f32, t: f32) -> Vec<Polygon> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    // Плита от z=0 (верх стен) до z=t.
    let (z0, z1) = (0.0, t);
    let pts = |sx: f32, sy: f32, z: f32| Vec3::new(sx, sy, z);
    vec![
        // Верх плиты.
        Polygon::quad(
            pts(-lx, -wy, z1),
            pts(lx, -wy, z1),
            pts(lx, wy, z1),
            pts(-lx, wy, z1),
        )
        .oriented(Vec3::Z),
        // Торцы плиты (видимые свесы).
        Polygon::quad(pts(lx, -wy, z0), pts(-lx, -wy, z0), pts(-lx, -wy, z1), pts(lx, -wy, z1)).oriented(-Vec3::Y),
        Polygon::quad(pts(-lx, wy, z0), pts(lx, wy, z0), pts(lx, wy, z1), pts(-lx, wy, z1)).oriented(Vec3::Y),
        Polygon::quad(pts(-lx, -wy, z0), pts(-lx, wy, z0), pts(-lx, wy, z1), pts(-lx, -wy, z1)).oriented(-Vec3::X),
        Polygon::quad(pts(lx, -wy, z0), pts(lx, wy, z0), pts(lx, wy, z1), pts(lx, -wy, z1)).oriented(Vec3::X),
    ]
}

/// Односкатная крыша: наклонная плоскость по всей ширине + торцевые
/// фронтоны (треугольники от карниза до конька) + «высокий» фронтон со
/// стороны подъёма + карнизы и софиты (пустот под крышей нет).
fn shed_polys(length: f32, width: f32, overhang: f32, pitch_deg: f32, t: f32) -> Vec<Polygon> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let rise = (pitch_deg.to_radians()).tan() * (2.0 * wy);

    // Верхняя плоскость: низ по Y=-wy, подъём к Y=+wy.
    let p0 = Vec3::new(-lx, -wy, 0.0);
    let p1 = Vec3::new(lx, -wy, 0.0);
    let p2 = Vec3::new(lx, wy, rise);
    let p3 = Vec3::new(-lx, wy, rise);

    let mut polys = vec![Polygon::quad(p0, p1, p2, p3).oriented(Vec3::Z)];

    // Торцевые фронтоны (треугольники от карниза до конька). Основание — на
    // уровне верха стен (z=0); полоса ниже (карниз) закрыта карнизными
    // досками. Фронтоны не выступают за край кровли.
    polys.push(
        Polygon::triangle(
            Vec3::new(lx, -wy, 0.0),
            Vec3::new(lx, wy, 0.0),
            Vec3::new(lx, wy, rise),
        )
        .oriented(Vec3::X),
    );
    polys.push(
        Polygon::triangle(
            Vec3::new(-lx, -wy, 0.0),
            Vec3::new(-lx, wy, rise),
            Vec3::new(-lx, wy, 0.0),
        )
        .oriented(-Vec3::X),
    );

    // Прямоугольный «высокий» фронтон: стена со стороны подъёма ската
    // (от верха стены до конька), чтобы не было пустоты под высоким краем.
    polys.push(
        Polygon::quad(
            Vec3::new(-lx, wy, 0.0),
            Vec3::new(lx, wy, 0.0),
            Vec3::new(lx, wy, rise),
            Vec3::new(-lx, wy, rise),
        )
        .oriented(Vec3::Y),
    );

    // Карнизные доски по периметру и закрытие свесов снизу.
    polys.extend(eave_boards(length, width, overhang, t));
    polys.extend(eave_soffits(length, width, overhang, t));

    polys
}

/// Двускатная крыша: два ската + торцевые фронтоны + карнизы + софиты.
///
/// Гребень может быть сдвинут от центра (`ridge`), из-за чего углы скатов
/// различаются. Рогов-«досок» нет: фронтоны закрывают торцы целиком.
fn gable_polys(
    length: f32,
    width: f32,
    overhang: f32,
    pitch_deg: f32,
    t: f32,
    ridge: f32,
) -> Vec<Polygon> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let rise = (pitch_deg.to_radians()).tan() * wy;
    // Гребень на высоте rise вдоль всей длины, но в плане сдвинут на ridge
    // (по Y): скаты с короткой и длинной стороной имеют разные углы.

    vec![
        // Скат к −Y (от карниза до гребня).
        Polygon::quad(
            Vec3::new(-lx, -wy, 0.0),
            Vec3::new(lx, -wy, 0.0),
            Vec3::new(lx, ridge, rise),
            Vec3::new(-lx, ridge, rise),
        )
        .oriented(Vec3::Z),
        // Скат к +Y (от гребня до карниза).
        Polygon::quad(
            Vec3::new(-lx, ridge, rise),
            Vec3::new(lx, ridge, rise),
            Vec3::new(lx, wy, 0.0),
            Vec3::new(-lx, wy, 0.0),
        )
        .oriented(Vec3::Z),
        // Торцы: треугольники от карниза до гребня (основание z=0, полоса
        // ниже закрыта карнизными досками).
        Polygon::triangle(
            Vec3::new(lx, -wy, 0.0),
            Vec3::new(lx, wy, 0.0),
            Vec3::new(lx, ridge, rise),
        )
        .oriented(Vec3::X),
        Polygon::triangle(
            Vec3::new(-lx, -wy, 0.0),
            Vec3::new(-lx, ridge, rise),
            Vec3::new(-lx, wy, 0.0),
        )
        .oriented(-Vec3::X),
    ]
    .into_iter()
    // Карнизные доски по нижним кромкам скатов и закрытие свесов снизу.
    .chain(eave_boards(length, width, overhang, t))
    .chain(eave_soffits(length, width, overhang, t))
    .collect::<Vec<_>>()
}

/// Dutch half-hip roof: inset upper gables above sloping end hips.
/// The ridge and gable bases share the same inset at each end.
fn half_hip_polys(
    length: f32,
    width: f32,
    overhang: f32,
    pitch_deg: f32,
    t: f32,
    ridge: f32,
) -> Vec<Polygon> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let rise = (pitch_deg.to_radians()).tan() * wy;
    // Высота, до которой «срезана» вальма (ниже — вальма, выше — фронтон).
    let t_f = HALF_HIP_CUT;
    let z_g = rise * t_f;
    let gx = half_hip_fronton_x(lx, wy);
    // Точки среза вальмы на высоте z_g: с каждой стороны от гребня своя
    // «половина» основания фронтона (чтобы всё лежало на плоскости ската).
    let gy_lo = (ridge + wy) * (1.0 - t_f);
    let gy_hi = (wy - ridge) * (1.0 - t_f);

    let (p0, p1, p2, p3) = (
        Vec3::new(-lx, -wy, 0.0),
        Vec3::new(lx, -wy, 0.0),
        Vec3::new(lx, wy, 0.0),
        Vec3::new(-lx, wy, 0.0),
    );
    // The ridge connects the inset gables.
    let apex_p = Vec3::new(gx, ridge, rise);
    let apex_m = Vec3::new(-gx, ridge, rise);
    // Углы оснований фронтонов (на уровне среза вальмы z_g).
    let c_lo = Vec3::new(gx, ridge - gy_lo, z_g); // низ фронтона, передняя (-Y) сторона
    let c_hi = Vec3::new(gx, ridge + gy_hi, z_g); // низ фронтона, задняя (+Y) сторона
    let c_lo_m = Vec3::new(-gx, ridge - gy_lo, z_g);
    let c_hi_m = Vec3::new(-gx, ridge + gy_hi, z_g);

    vec![
        // Передний скат (к −Y), нижняя часть (от карниза до вальмы).
        Polygon::quad(p0, p1, c_lo, c_lo_m).oriented(Vec3::Z),
        // Передний скат (к −Y), верхняя часть (от вальмы до гребня) —
        // именно этот кусок соединяет фронтон с гребнем.
        Polygon::quad(c_lo, apex_p, apex_m, c_lo_m).oriented(Vec3::Z),
        // Задний скат (к +Y), нижняя и верхняя части.
        Polygon::quad(p3, p2, c_hi, c_hi_m).oriented(Vec3::Z),
        Polygon::quad(c_hi, apex_p, apex_m, c_hi_m).oriented(Vec3::Z),
        // Торцевые вальмы: трапеции от карниза до среза.
        Polygon::quad(p1, p2, c_hi, c_lo).oriented(Vec3::X),
        Polygon::quad(p3, p0, c_lo_m, c_hi_m).oriented(-Vec3::X),
        // Фронтоны поверх вальм: треугольники от среза до гребня.
        Polygon::triangle(c_lo, c_hi, apex_p).oriented(Vec3::X),
        Polygon::triangle(c_lo_m, apex_m, c_hi_m).oriented(-Vec3::X),
    ]
    .into_iter()
    // Закрытие свесов снизу и карнизы.
    .chain(eave_boards(length, width, overhang, t))
    .chain(eave_soffits(length, width, overhang, t))
    .collect::<Vec<_>>()
}

/// Вальмовая крыша: четыре ската, сходящихся к короткому коньку.
///
/// Рогов-«досок» (наклонных карнизов вдоль рёбер) нет — только гладкие
/// скаты, встречающиеся по рёбрам.
fn hip_polys(length: f32, width: f32, overhang: f32, pitch_deg: f32, t: f32) -> Vec<Polygon> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let rise = (pitch_deg.to_radians()).tan() * wy;
    // Половина длины конька: чтобы «вальма» была с равным уклоном.
    let rx = (lx - wy).max(0.25);

    let p0 = Vec3::new(-lx, -wy, 0.0);
    let p1 = Vec3::new(lx, -wy, 0.0);
    let p2 = Vec3::new(lx, wy, 0.0);
    let p3 = Vec3::new(-lx, wy, 0.0);
    let r1 = Vec3::new(-rx, 0.0, rise);
    let r2 = Vec3::new(rx, 0.0, rise);

    let mut polys = vec![
        // Передний (к −Y) и задний (к +Y) скаты.
        Polygon::quad(p0, p1, r2, r1).oriented(Vec3::Z),
        Polygon::quad(p2, p3, r1, r2).oriented(Vec3::Z),
        // Боковые вальмы.
        Polygon::triangle(p1, p2, r2).oriented(Vec3::X),
        Polygon::triangle(p3, p0, r1).oriented(-Vec3::X),
    ];

    // Карнизные доски по нижним кромкам и закрытие свесов снизу.
    polys.extend(eave_boards(length, width, overhang, t));
    polys.extend(eave_soffits(length, width, overhang, t));

    polys
}

/// Сводчатая (бочарная) крыша: арка по всей длине здания.
///
/// Торцы закрыты «фронтонами»-арками (набором трапеций по сегментам арки,
/// снизу — от уровня карниза), карниз и софиты — как у остальных скатных.
fn barrel_polys(length: f32, width: f32, overhang: f32, pitch_deg: f32, t: f32) -> Vec<Polygon> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let rise = (pitch_deg.to_radians()).tan() * wy;
    // Окружность арки: хорда 2·wy, стрела подъёма — rise.
    let r = (wy * wy + rise * rise) / (2.0 * rise.max(0.01));
    let theta = (wy / r).min(1.0).asin().max(0.001);
    let zn = rise - r; // низ арки (уровень концов хорды) — 0.

    let segs = 8usize;
    let arc = |i: usize| {
        let a = -theta + 2.0 * theta * i as f32 / segs as f32;
        (r * a.sin(), zn + r * a.cos())
    };

    let mut polys = Vec::new();
    // Боковая поверхность арки (сегменты-«рёбра» вдоль всей длины).
    for i in 0..segs {
        let (y0, z0) = arc(i);
        let (y1, z1) = arc(i + 1);
        let mid = Vec3::new(0.0, (y0 + y1) * 0.5, (z0 + z1) * 0.5);
        let pref = (mid - Vec3::new(0.0, 0.0, zn)).normalize_or_zero();
        polys.push(
            Polygon::quad(
                Vec3::new(-lx, y0, z0),
                Vec3::new(lx, y0, z0),
                Vec3::new(lx, y1, z1),
                Vec3::new(-lx, y1, z1),
            )
            .oriented(pref),
        );
    }

    // Торцевые «фронтоны»: трапеции по сегментам арки (основание на уровне
    // карниза z=0); полоса ниже закрыта карнизными досками. У самых краёв
    // дуга касается карниза, и трапеция вырождается в треугольник.
    for (sx, pref) in [(lx, Vec3::X), (-lx, -Vec3::X)] {
        for i in 0..segs {
            let (y0, z0) = arc(i);
            let (y1, z1) = arc(i + 1);
            let corners = [
                Vec3::new(sx, y0, 0.0),
                Vec3::new(sx, y1, 0.0),
                Vec3::new(sx, y1, z1),
                Vec3::new(sx, y0, z0),
            ];
            // Убираем совпадающие подряд идущие вершины (на краях дуги).
            let mut pts: Vec<Vec3> = Vec::new();
            for c in corners {
                if pts
                    .last()
                    .is_none_or(|q: &Vec3| q.distance(c) > 1e-4)
                {
                    pts.push(c);
                }
            }
            match pts.len() {
                3 => {
                    polys.push(Polygon::triangle(pts[0], pts[1], pts[2]).oriented(pref));
                }
                4 => {
                    polys.push(Polygon::from_points(pts).oriented(pref));
                }
                _ => {}
            }
        }
    }

    // Закрытие свесов снизу (софиты) и карнизы.
    polys.extend(eave_boards(length, width, overhang, t));
    polys.extend(eave_soffits(length, width, overhang, t));

    polys
}

/// Горизонтальные «карнизные доски» по периметру у уровня верха стен
/// (z ∈ [−t, 0]): закрывают свес по вертикали.
fn eave_boards(length: f32, width: f32, overhang: f32, t: f32) -> Vec<Polygon> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    vec![
        // Север (+Y).
        Polygon::quad(
            p(-lx, wy, -t),
            p(lx, wy, -t),
            p(lx, wy, 0.0),
            p(-lx, wy, 0.0),
        )
        .oriented(Vec3::Y),
        // Юг (−Y).
        Polygon::quad(
            p(-lx, -wy, -t),
            p(lx, -wy, -t),
            p(lx, -wy, 0.0),
            p(-lx, -wy, 0.0),
        )
        .oriented(-Vec3::Y),
        // Восток (+X).
        Polygon::quad(
            p(lx, -wy, -t),
            p(lx, wy, -t),
            p(lx, wy, 0.0),
            p(lx, -wy, 0.0),
        )
        .oriented(Vec3::X),
        // Запад (−X).
        Polygon::quad(
            p(-lx, -wy, -t),
            p(-lx, wy, -t),
            p(-lx, wy, 0.0),
            p(-lx, -wy, 0.0),
        )
        .oriented(-Vec3::X),
    ]
}

/// Горизонтальные «софиты» под свесом: закрывают просвет между стеной и
/// кровлей по периметру (на уровне низа карнизной доски).
fn eave_soffits(length: f32, width: f32, overhang: f32, t: f32) -> Vec<Polygon> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let (mx, my) = (length * 0.5, width * 0.5);
    // Чуть ниже карнизных досок, чтобы не драться с ними по z.
    let z = -t + 0.001;
    let p = |x: f32, y: f32| Vec3::new(x, y, z);
    vec![
        // Север: полоса от верха стены до края свеса.
        Polygon::quad(p(-mx, my), p(mx, my), p(mx, wy), p(-mx, wy)).oriented(Vec3::Z),
        // Юг.
        Polygon::quad(p(-mx, -wy), p(mx, -wy), p(mx, -my), p(-mx, -my)).oriented(Vec3::Z),
        // Восток.
        Polygon::quad(p(mx, -my), p(lx, -my), p(lx, my), p(mx, my)).oriented(Vec3::Z),
        // Запад.
        Polygon::quad(p(-lx, -my), p(-mx, -my), p(-mx, my), p(-lx, my)).oriented(Vec3::Z),
    ]
}

/// Парапет — тонкий бортик по периметру плоской крыши.
pub fn parapet_parts(
    length: f32,
    width: f32,
    overhang: f32,
    height: f32,
    color: bevy::color::Color,
    config: &BuildingConfig,
) -> Vec<Part> {
    let (lx, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let t = 0.08;

    // Центр бортика — на внешней кромке плиты. Облако точек — на верхней
    // грани и всех наружных боках (чтобы парапет не «пропадал» из облака).
    let mk = |cx: f32, cy: f32, sx: f32, sy: f32| {
        Part::boxed_outwards(
            "парапет",
            Vec3::new(sx, sy, height),
            &[Vec3::Z, Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y],
            color,
            Transform::from_xyz(cx, cy, height * 0.5),
            config,
        )
    };

    vec![
        mk(0.0, wy, 2.0 * lx, t),   // север
        mk(0.0, -wy, 2.0 * lx, t),  // юг
        mk(lx, 0.0, t, 2.0 * wy),   // восток
        mk(-lx, 0.0, t, 2.0 * wy),  // запад
    ]
}

/// Мансарда на скате двускатной/вальмовой/полувальмовой крыши.
///
/// Ставится на «передний» (к −Y) скат: вертикальный домик, выступающий из
/// плоскости ската, с собственной маленькой крышей и окном на передней
/// грани (смотрит вперёд, а не вверх). Короб опущен в скат, чтобы мансарда
/// не «летела» над крышей.
#[allow(clippy::too_many_arguments)]
pub fn dormer_part(
    length: f32,
    width: f32,
    overhang: f32,
    pitch_deg: f32,
    ridge: f32,
    colors: &BuildingColors,
    config: &BuildingConfig,
) -> Part {
    let lx = length * 0.5 + overhang;
    let wy = width * 0.5 + overhang;
    let rise = pitch_deg.to_radians().tan() * wy;
    let run = (ridge + wy).max(0.01);
    let slope = rise / run;
    let mut rng = config.seed.with_label("integrated-dormer").rng();
    let fraction = rng.random_range(0.25..0.4);
    let front_y = -wy + fraction * run;
    let base_z = slope * (front_y + wy);
    let remaining = rise - base_z;
    // Both valley intersections must lie below the main ridge.
    let wall_h = config.dormer_height.min(remaining * 0.48);
    let w = (config.dormer_width * 0.5).min(lx * 0.3);
    let cap = (w * 0.8).min(remaining * 0.3);
    let eave_z = base_z + wall_h;
    let ridge_z = eave_z + cap;
    let eave_y = front_y + wall_h / slope;
    let ridge_y = front_y + (wall_h + cap) / slope;
    let left_base = Vec3::new(-w, front_y, base_z);
    let right_base = Vec3::new(w, front_y, base_z);
    let left_front = Vec3::new(-w, front_y, eave_z);
    let right_front = Vec3::new(w, front_y, eave_z);
    let peak_front = Vec3::new(0.0, front_y, ridge_z);
    let left_join = Vec3::new(-w, eave_y, eave_z);
    let right_join = Vec3::new(w, eave_y, eave_z);
    let peak_join = Vec3::new(0.0, ridge_y, ridge_z);
    // The back edges are valleys in the main slope, not a rear box wall.
    let roof_polys = vec![
        Polygon::quad(left_front, peak_front, peak_join, left_join).oriented(Vec3::Z),
        Polygon::quad(peak_front, right_front, right_join, peak_join).oriented(Vec3::Z),
    ];
    let walls = vec![
        Polygon::quad(left_base,right_base,right_front,left_front).oriented(-Vec3::Y),
        Polygon::triangle(left_base,left_front,left_join).oriented(-Vec3::X),
        Polygon::triangle(right_base,right_join,right_front).oriented(Vec3::X),
    ];
    let gables = vec![Polygon::triangle(left_front,right_front,peak_front).oriented(-Vec3::Y)];
    let rotation = Quat::from_mat3(&Mat3::from_cols(Vec3::X,Vec3::Z,-Vec3::Y));
    Part::group("integrated-dormer",colors.wall,Transform::IDENTITY)
        .child(Part::from_faces("dormer-walls",walls,colors.wall,Transform::IDENTITY,false,config))
        .child(Part::from_faces("dormer-gables",gables,colors.wall,Transform::IDENTITY,false,config))
        .child(Part::from_faces("dormer-roof",roof_polys,colors.roof,Transform::IDENTITY,true,config))
        .child(Part::boxed("dormer-window",Vec3::new(w*1.3,wall_h*0.55,0.04),Vec3::Z,colors.window,
            Transform { translation:Vec3::new(0.0,front_y-0.025,base_z+wall_h*0.5),rotation,..default() },config))
}

// Roof light aligned with an actual planar slope face.
fn roof_window_part(face: &Polygon, colors: &BuildingColors, config: &BuildingConfig) -> Part {
    let points = face.points();
    let center = points.iter().copied().sum::<Vec3>() / points.len() as f32;
    let normal = face.normal();
    let mut radius = f32::INFINITY;
    for i in 0..points.len() {
        let edge = points[(i + 1) % points.len()] - points[i];
        radius = radius.min((center - points[i]).cross(edge).length() / edge.length());
    }
    let width = config.window_width.min(radius * 1.1);
    let height = 1.2_f32.min(radius * 1.1);
    let along = Vec3::X;
    let up = normal.cross(along).normalize();
    let rotation = Quat::from_mat3(&Mat3::from_cols(along, up, normal));
    let mut window = Part::group("roof-window", colors.window, Transform {
        translation: center + normal * 0.025, rotation, ..default()
    });
    // A shallow frame follows the slope; the glass lies just above it.
    for (size,pos) in [
        (Vec3::new(width,0.06,0.04),Vec3::new(0.0,-height*0.5,0.0)),
        (Vec3::new(width,0.06,0.04),Vec3::new(0.0,height*0.5,0.0)),
        (Vec3::new(0.06,height,0.04),Vec3::new(-width*0.5,0.0,0.0)),
        (Vec3::new(0.06,height,0.04),Vec3::new(width*0.5,0.0,0.0)),
    ] {
        window = window.child(Part::boxed("skylight-frame", size, Vec3::Z, super::palette::FRAME, Transform::from_translation(pos), config));
    }
    window.child(Part::boxed("skylight-glass",Vec3::new(width-0.06,height-0.06,0.015),Vec3::Z,colors.window,Transform::from_xyz(0.0,0.0,0.025),config))
}

/// Дымоход — вертикальный короб на кровле.
///
/// Ставится НЕ обязательно на гребне: случайная точка на скате, низ короба
/// «садится» в кровлю на высоте плоскости ската в этой точке.
fn chimney_part(
    style: RoofStyle,
    lx: f32,
    wy: f32,
    rise: f32,
    ridge: f32,
    config: &BuildingConfig,
) -> Part {
    let mut rng = config
        .seed
        .with_label("дымоход")
        .rng();
    // Keep the entire footprint on the central side slope, away from end hips.
    let half = 0.21;
    let dx = rng.random_range(-0.1..0.1) * (lx - half).max(0.0);
    // Dormers occupy the -Y slope. Reserve the opposite slope for chimneys.
    let back_ridge = if matches!(style, RoofStyle::Gable | RoofStyle::HalfHip) {
        ridge
    } else { 0.0 };
    let dy = back_ridge + rng.random_range(0.35..0.6) * (wy - back_ridge);
    let surface = |y| {
        if style == RoofStyle::Flat {
            config.roof_thickness
        } else {
            pitch_plane_z(style, wy, rise, ridge, y)
        }
    };
    let top = surface(dy - half).max(surface(dy + half)) + 1.0;
    let bottom = |z: f32| surface(dy - z) - 0.02;
    let mut faces = mounted_walls(half, half, top, bottom);
    faces.push(Polygon::quad(
        Vec3::new(-half, top, -half), Vec3::new(-half, top, half),
        Vec3::new(half, top, half), Vec3::new(half, top, -half),
    ).oriented(Vec3::Y));
    let rotation = Quat::from_mat3(&Mat3::from_cols(Vec3::X, Vec3::Z, -Vec3::Y));
    Part::from_faces(
        "chimney", faces, super::palette::CHIMNEY,
        Transform { translation: Vec3::new(dx, dy, 0.0), rotation, ..default() },
        false, config,
    )
}

/// Высота плоскости крыши в точке `(y)` (по Y — поперёк гребня; для всех
/// скатных плоскостей высота не зависит от X). Нужна для установки дымохода
/// (и мансарды) точно на скат.
fn pitch_plane_z(style: RoofStyle, wy: f32, rise: f32, ridge: f32, y: f32) -> f32 {
    match style {
        RoofStyle::Flat => 0.0,
        RoofStyle::Shed => rise * (y + wy) / wy.max(0.01),
        RoofStyle::Gable | RoofStyle::HalfHip => {
            if y <= ridge {
                rise * (y + wy) / (ridge + wy).max(0.01)
            } else {
                rise * (wy - y) / (wy - ridge).max(0.01)
            }
        }
        RoofStyle::Hip => {
            if y <= 0.0 {
                rise * (y + wy) / wy.max(0.01)
            } else {
                rise * (wy - y) / wy.max(0.01)
            }
        }
        RoofStyle::Barrel => {
            let r = (wy * wy + rise * rise) / (2.0 * rise.max(0.01));
            let zn = rise - r;
            let s = (r * r - y * y).clamp(0.0, f32::INFINITY).sqrt();
            zn + s
        }
    }
}

/// Сдвиг гребня двускатной/полувальмовой крыши от центра (в долях
/// полуширины `wy`), чтобы углы скатов различались.
fn ridge_offset(config: &BuildingConfig, wy: f32, style: RoofStyle) -> f32 {
    if !matches!(style, RoofStyle::Gable | RoofStyle::HalfHip) {
        return 0.0;
    }
    config
        .seed
        .with_label("гребень")
        .rng()
        .random_range(-config.max_ridge_shift..=config.max_ridge_shift)
        * wy
}

/// Трансформ крыши: поднята на высоту стен (поворот применяется в
/// [`roof_part`] к преобразованию группы).
pub fn roof_transform(wall_height: f32) -> Transform {
    Transform::from_xyz(0.0, 0.0, wall_height)
}

/// Собрать крышу как часть здания (плита/скаты + парапет + мансарда +
/// дымоход).
#[allow(clippy::too_many_arguments)]
pub fn roof_part(
    length: f32,
    width: f32,
    wall_height: f32,
    style: RoofStyle,
    overhang: f32,
    pitch_deg: f32,
    parapet: bool,
    dormer: bool,
    entrance: Facade,
    colors: &BuildingColors,
    config: &BuildingConfig,
) -> Part {
    let pitched = style.is_pitched();

    // Поворот скатных крыш вокруг вертикали выбираем ДО построения
    // геометрии: для 90°/270° крыша строится с «перевёрнутыми» габаритами,
    // и после поворота встаёт точно на стены (гребень — вдоль нужной оси).
    let mut rng = config.seed.with_label("крыша-поворот").rng();
    let random_spin = rng.random_range(0..4);
    let spin = entrance_roof_spin(style, entrance, random_spin);
    let swap = pitched && (spin == 1 || spin == 3);
    let (roof_length, roof_width) = if swap { (width, length) } else { (length, width) };
    let rotation = if pitched && spin != 0 {
        Quat::from_rotation_z(spin as f32 * std::f32::consts::FRAC_PI_2)
    } else {
        Quat::IDENTITY
    };
    let base = roof_transform(wall_height);
    let transform =
        Transform::from_translation(base.translation) * Transform::from_rotation(rotation);

    let wy = roof_width * 0.5 + overhang;
    let rise = (pitch_deg.to_radians()).tan() * wy;
    // Сдвиг гребня — один на всю крышу (скаты и мансарда используют тот же).
    let ridge = ridge_offset(config, wy, style);

    let mut roof = Part::group("крыша", colors.roof, transform);

    match style {
        RoofStyle::Flat => {
            let t = config.roof_thickness;
            let slab = Part::boxed(
                "кровля-плита",
                Vec3::new(length + 2.0 * overhang, width + 2.0 * overhang, t),
                Vec3::Z,
                colors.roof,
                Transform::from_xyz(0.0, 0.0, t * 0.5),
                config,
            );
            roof = roof.child(slab);
            if parapet {
                let ps = parapet_parts(length, width, overhang, config.parapet_height, colors.roof, config);
                roof = roof.children(ps);
            }
        }
        pitched => {
            let mut faces = roof_faces(pitched, roof_length, roof_width, overhang, pitch_deg, config, ridge);
            let gable_range = match pitched {
                RoofStyle::HalfHip => Some(6..8),
                RoofStyle::Gable => Some(2..4),
                RoofStyle::Shed => Some(1..4),
                _ => None,
            };
            if let Some(range) = gable_range {
                let gables = faces.drain(range).collect();
                roof = roof.child(Part::from_faces(
                    "roof-gables", gables, colors.wall, Transform::IDENTITY, true, config,
                ));
            }
            let window_face = faces[0].clone();
            let canopy =
                Part::from_faces("кровля", faces, colors.roof, Transform::IDENTITY, true, config);
            roof = roof.child(canopy);

            if dormer {
                let protruding_ok = matches!(pitched, RoofStyle::Gable | RoofStyle::HalfHip | RoofStyle::Hip);
                if protruding_ok && rng.random::<f32>() < 0.5 {
                    roof = roof.child(dormer_part(
                        roof_length, roof_width, overhang, pitch_deg, ridge, colors, config,
                    ));
                } else {
                    roof = roof.child(roof_window_part(&window_face, colors, config));
                }
            }

            if rng.random::<f32>() < config.chimney_probability {
                let lx = roof_length * 0.5 + overhang;
                roof = roof.child(chimney_part(pitched, lx, wy, rise, ridge, config));
            }
        }
    }

    roof
}

/// Вращение по умолчанию (без поворота) — используется как вспомогательная
/// константа в тестах.
#[allow(dead_code)]
fn identity() -> Quat {
    Quat::IDENTITY
}

const HALF_HIP_CUT: f32 = 0.65;

// Inset the gables to form sloping end hips; retain room for roof details.
fn half_hip_fronton_x(lx: f32, wy: f32) -> f32 {
    lx - (wy * HALF_HIP_CUT).min(lx * 0.45)
}

// Vertical walls with lower edges following the supporting roof, no exposed floor.
fn mounted_walls(w: f32, d: f32, top: f32, bottom: impl Fn(f32) -> f32) -> Vec<Polygon> {
    let a = Vec3::new(-w, bottom(-d), -d);
    let b = Vec3::new(w, bottom(-d), -d);
    let c = Vec3::new(w, bottom(d), d);
    let e = Vec3::new(-w, bottom(d), d);
    let at = Vec3::new(-w, top, -d);
    let bt = Vec3::new(w, top, -d);
    let ct = Vec3::new(w, top, d);
    let et = Vec3::new(-w, top, d);
    vec![
        Polygon::quad(a, b, bt, at).oriented(-Vec3::Z),
        Polygon::quad(b, c, ct, bt).oriented(Vec3::X),
        Polygon::quad(c, e, et, ct).oriented(Vec3::Z),
        Polygon::quad(e, a, at, et).oriented(-Vec3::X),
    ]
}

#[cfg(test)]
mod mounting_tests {
    use super::*;

    #[test]
    fn chimney_bases_follow_supporting_roof() {
        let config = BuildingConfig::default();
        for style in RoofStyle::ALL {
            for pitch in [5.0_f32, 27.0, 40.0] {
                let wy = 4.4;
                let rise = pitch.to_radians().tan() * wy;
                let ridge = if matches!(style, RoofStyle::Gable | RoofStyle::HalfHip) {
                    0.35 * wy
                } else { 0.0 };
                let part = chimney_part(style, 6.4, wy, rise, ridge, &config);
                for face in &part.faces[..4] {
                    let points = face.points();
                    let top = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
                    for local in points.iter().filter(|p| p.y < top - 0.01) {
                        let p = part.transform.transform_point(*local);
                        let expected = if style == RoofStyle::Flat {
                            config.roof_thickness
                        } else { pitch_plane_z(style, wy, rise, ridge, p.y) };
                        assert!((p.z - expected + 0.02).abs() < 0.001);
                    }
                }
            }
        }
    }

    #[test]
    fn mounted_walls_have_sloped_bases_and_no_floor() {
        for slope in [0.1, 0.7, 1.3] {
            let walls = mounted_walls(0.5, 0.45, 1.0, |z| -slope * z);
            assert_eq!(walls.len(), 4);
            for face in walls {
                let pts = face.points();
                for p in pts.iter().filter(|p| p.y < 0.99) {
                    assert!((p.y + slope * p.z).abs() < 0.001);
                }
                for p in pts {
                    assert!((*p - pts[0]).dot(face.normal()).abs() < 0.001);
                }
            }
        }
    }
}

#[cfg(test)]
mod detail_clearance_tests {
    use super::*;
    #[test]
    fn chimney_keeps_clear_of_front_dormer() {
        let cfg = BuildingConfig::default();
        for style in [RoofStyle::Gable, RoofStyle::HalfHip, RoofStyle::Hip] {
            let wy = 2.9;
            let ridge = if style == RoofStyle::Hip { 0.0 } else { 0.35 * wy };
            let chimney = chimney_part(style, 2.9, wy, 2.0, ridge, &cfg);
            let chimney_front = chimney.transform.translation.y - 0.21;
            // The dormer rear edge stays on the front side of the ridge.
            assert!(chimney_front > ridge + 0.1);
        }
    }
}

#[cfg(test)]
mod shed_surface_tests {
    use super::*;

    #[test]
    fn chimney_height_matches_actual_shed_mesh() {
        let cfg = BuildingConfig::default();
        for (length, width) in [(5.0, 15.0), (15.0, 5.0), (8.0, 8.0)] {
            for pitch in [5.0_f32, 14.0, 20.0, 40.0] {
                let wy = width * 0.5 + 0.4;
                let rise = pitch.to_radians().tan() * wy;
                let mesh = roof_faces(RoofStyle::Shed, length, width, 0.4, pitch, &cfg, 0.0);
                let n = mesh[0].normal();
                let origin = mesh[0].points()[0];
                let actual_height = |x: f32, y: f32| {
                    origin.z - (n.x * (x - origin.x) + n.y * (y - origin.y)) / n.z
                };
                let chimney = chimney_part(RoofStyle::Shed, length * 0.5 + 0.4, wy, rise, 0.0, &cfg);
                for face in &chimney.faces[..4] {
                    let points = face.points();
                    let top = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
                    for local in points {
                        let p = chimney.transform.transform_point(*local);
                        let surface = actual_height(p.x, p.y);
                        if local.y < top - 0.01 {
                            assert!((p.z - surface + 0.02).abs() < 0.001);
                        } else {
                            assert!(p.z - surface >= 0.999);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod dormer_style_tests {
    use super::*;
    use bevy::color::Color;
    use generation_core::Seed;

    fn colors() -> BuildingColors {
        BuildingColors { wall:Color::WHITE,roof:Color::WHITE,window:Color::WHITE,door:Color::WHITE,railing:Color::WHITE }
    }

    #[test]
    fn skylights_follow_real_roof_faces() {
        let cfg = BuildingConfig::default();
        for style in [RoofStyle::Shed,RoofStyle::Gable,RoofStyle::Hip,RoofStyle::HalfHip,RoofStyle::Barrel] {
            for pitch in [5.0,14.0,40.0] {
                let faces = roof_faces(style,8.0,12.0,0.4,pitch,&cfg,0.0);
                let face = &faces[0];
                let skylight = roof_window_part(face,&colors(),&cfg);
                let normal = face.normal();
                assert!((skylight.transform.rotation * Vec3::Z).dot(normal) > 0.999);
                for part in &skylight.children {
                    for polygon in &part.faces {
                        for p in polygon.points() {
                            let world = skylight.transform.transform_point(part.transform.transform_point(*p));
                            let distance = (world-face.points()[0]).dot(normal);
                            assert!(distance > 0.0 && distance < 0.08);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn both_dormer_styles_are_generated() {
        let mut found_window = false;
        let mut found_extension = false;
        for seed in 0..40 {
            let mut cfg = BuildingConfig::default();
            cfg.seed = Seed::new(seed);
            let roof = roof_part(10.0,8.0,3.0,RoofStyle::Gable,0.4,30.0,false,true,Facade::East,&colors(),&cfg);
            found_window |= roof.children.iter().any(|p| p.name == "roof-window");
            found_extension |= roof.children.iter().any(|p| p.children.iter().any(|c| c.name == "dormer-gables"));
        }
        assert!(found_window && found_extension);
    }
}

#[cfg(test)]
mod integrated_dormer_tests {
    use super::*;
    use bevy::color::Color;
    #[test]
    fn dormer_roof_joins_main_slope_before_ridge() {
        let cfg = BuildingConfig::default();
        let colors = BuildingColors { wall:Color::WHITE,roof:Color::WHITE,window:Color::WHITE,door:Color::WHITE,railing:Color::WHITE };
        for pitch in [5.0_f32,14.0,30.0,40.0] {
            for ridge in [-1.0,0.0,1.0] {
                let part = dormer_part(8.0,6.0,0.4,pitch,ridge,&colors,&cfg);
                let slope = pitch.to_radians().tan()*3.4/(ridge+3.4);
                let roof = part.children.iter().find(|p| p.name == "dormer-roof").unwrap();
                for face in &roof.faces {
                    let points = face.points();
                    let front = points.iter().map(|p| p.y).fold(f32::INFINITY,f32::min);
                    let mut joins = 0;
                    for p in points {
                        assert!((*p-points[0]).dot(face.normal()).abs()<0.001);
                        if p.y > front + 0.001 {
                            assert!((p.z-slope*(p.y+3.4)).abs()<0.001);
                            assert!(p.y < ridge);
                            joins += 1;
                        }
                    }
                    assert_eq!(joins,2);
                }
                let walls = part.children.iter().find(|p| p.name == "dormer-walls").unwrap();
                assert_eq!(walls.faces.len(),3);
            }
        }
    }
}

// Local ridge runs along X. The shed rises toward +Y, away from its runoff edge.
fn entrance_roof_spin(style: RoofStyle, entrance: Facade, random_spin: u32) -> u32 {
    match style {
        RoofStyle::Gable | RoofStyle::HalfHip | RoofStyle::Barrel => match entrance {
            Facade::East => 0, Facade::North => 1, Facade::West => 2, Facade::South => 3,
        },
        RoofStyle::Shed => match entrance {
            Facade::North => 0, Facade::West => 1, Facade::South => 2, Facade::East => 3,
        },
        _ => random_spin,
    }
}

#[cfg(test)]
mod entrance_orientation_tests {
    use super::*;
    #[test]
    fn ridge_and_shed_runoff_respect_every_entrance_facade() {
        for facade in Facade::ALL {
            for random in 0..4 {
                for style in [RoofStyle::Gable, RoofStyle::HalfHip, RoofStyle::Barrel] {
                    let spin = entrance_roof_spin(style,facade,random);
                    let rotation = Quat::from_rotation_z(spin as f32 * std::f32::consts::FRAC_PI_2);
                    assert!((rotation * Vec3::X).dot(facade.outward()) > 0.999);
                    assert!((rotation * Vec3::Y).dot(facade.outward()).abs() < 0.001);
                }
                let spin = entrance_roof_spin(RoofStyle::Shed,facade,random);
                let rotation = Quat::from_rotation_z(spin as f32 * std::f32::consts::FRAC_PI_2);
                assert!((rotation * -Vec3::Y).dot(facade.outward()) < -0.999);
                assert_eq!(entrance_roof_spin(RoofStyle::Hip,facade,random),random);
            }
        }
    }
}

#[cfg(test)]
mod balcony_orientation_tests {
    use super::*;
    use crate::generation::building::balcony_facades;
    #[test]
    fn balconies_avoid_roof_runoff_facades() {
        for entrance in Facade::ALL {
            for style in RoofStyle::ALL {
                let spin = entrance_roof_spin(style,entrance,0);
                let rotation = Quat::from_rotation_z(spin as f32*std::f32::consts::FRAC_PI_2);
                let candidates = balcony_facades(style,entrance);
                if matches!(style,RoofStyle::Hip|RoofStyle::HalfHip) { assert_eq!(candidates, Facade::ALL.to_vec()); }
                for facade in candidates {
                    let outward = facade.outward();
                    if matches!(style,RoofStyle::Gable|RoofStyle::Barrel) {
                        assert!(outward.dot(rotation*Vec3::Y).abs()<0.001);
                    } else if style == RoofStyle::Shed {
                        assert!(outward.dot(rotation*(-Vec3::Y))<0.001);
                    }
                }
            }
        }
    }
}
