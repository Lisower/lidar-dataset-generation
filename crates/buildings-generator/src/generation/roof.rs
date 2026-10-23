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
    let t_f = 0.55;
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
    let (_, wy) = (length * 0.5 + overhang, width * 0.5 + overhang);
    let rise = (pitch_deg.to_radians()).tan() * wy;

    // Точка установки на переднем скате (к −Y) между свесом и гребнем
    // (с учётом сдвига гребня: длина ската = ridge + wy).
    let run = (ridge + wy).max(0.5);
    let mut rng = config.seed.with_label("мансарда").rng();
    let t = rng.random_range(0.3..0.55);
    let y = -wy + t * run;
    let z = rise * (y + wy) / run;
    let base = Vec3::new(0.0, y, z);

    // Локальная система мансарды: X — вдоль конька, Y — вверх (вертикаль),
    // Z — вперёд (к свесу, горизонтально наружу). Домик «стоит», не лежит.
    let m = Mat3::from_cols(Vec3::X, Vec3::Z, -Vec3::Y);
    let rotation = Quat::from_mat3(&m);

    let dw = config.dormer_width;
    let dd = config.dormer_depth;
    let dh = config.dormer_height;

    // Короб выступает из крыши, а его дно опущено НИЖЕ точки базы (в скат),
    // чтобы между ним и покрытием не было зазора (мансарда не «летает»).
    let center = base + (-Vec3::Y) * (dd * 0.6) + Vec3::Z * (dh * 0.15);
    let transform = Transform {
        translation: center,
        rotation,
        ..default()
    };

    // Локальная система «домика»: X — вдоль конька, Y — вверх, Z — вперёд.
    let (w, zf, zb, yt) = (dw * 0.5, dd * 0.5, -dd * 0.5, dh * 0.5);
    let cap = 0.18; // высота маленькой крыши над коробом
    let yr = yt + cap;

    // Маленькая двухскатная крыша мансарды: конёк над коробом, скаты к
    // передней и задней его кромкам, треугольные торцы.
    let roof_polys = vec![
        Polygon::quad(
            Vec3::new(-w, yr, 0.0),
            Vec3::new(w, yr, 0.0),
            Vec3::new(w, yt, zf),
            Vec3::new(-w, yt, zf),
        )
        .oriented(Vec3::Y),
        Polygon::quad(
            Vec3::new(-w, yr, 0.0),
            Vec3::new(-w, yt, zb),
            Vec3::new(w, yt, zb),
            Vec3::new(w, yr, 0.0),
        )
        .oriented(Vec3::Y),
        Polygon::triangle(Vec3::new(w, yr, 0.0), Vec3::new(w, yt, zb), Vec3::new(w, yt, zf))
            .oriented(Vec3::X),
        Polygon::triangle(Vec3::new(-w, yr, 0.0), Vec3::new(-w, yt, zf), Vec3::new(-w, yt, zb))
            .oriented(-Vec3::X),
    ];

    Part::group("мансарда", colors.roof, transform)
        .with_child(Part::boxed(
            "мансарда-короб",
            Vec3::new(dw, dh, dd),
            Vec3::Z, // наружу — локальный +Z (вперёд)
            colors.wall,
            Transform::IDENTITY,
            config,
        ))
        .with_child(Part::from_faces(
            "мансарда-кровля",
            roof_polys,
            colors.roof,
            Transform::IDENTITY,
            true,
            config,
        ))
        .with_child(Part::boxed(
            "мансарда-окно",
            Vec3::new(dw * 0.7, 0.6, 0.06),
            Vec3::Z, // нормаль наружу — локальный +Z (вперёд)
            colors.window,
            Transform::from_xyz(0.0, 0.0, zf + 0.03),
            config,
        ))
}

/// Второй вариант мансарды: просто окно во фронтоне крыши.
///
/// Ставится на торцевую стенку (фронтон) крыши на небольших скатных типах
/// (у которых есть фронтоны: одно/двускатная, полувальмовая). После поворота
/// крыши вместе с ней повернётся и окно.
#[allow(clippy::too_many_arguments)]
fn fronton_window_part(
    length: f32,
    width: f32,
    overhang: f32,
    pitch_deg: f32,
    style: RoofStyle,
    ridge: f32,
    colors: &BuildingColors,
    config: &BuildingConfig,
) -> Part {
    let wy = width * 0.5 + overhang;
    let rise = (pitch_deg.to_radians()).tan() * wy;
    let z_g = rise * 0.55;

    // Положение и размер окна в пределах торцевого фронтона.
    let (z_mid, w_max, h) = match style {
        // У односкатной фронтон низкий (треугольник от карниза до конька).
        RoofStyle::Shed => (rise * 0.22, 0.7, (rise * 0.4).clamp(0.3, 0.6)),
        // У полувальмовой фронтон — узкий треугольник над срезом вальмы.
        RoofStyle::HalfHip => ((z_g + rise) * 0.5, 0.75, 0.7),
        // Двускатная — самый просторный треугольник.
        _ => (rise * 0.45, config.window_width, 0.8),
    };
    let w = w_max.min(config.window_width).max(0.5);

    let lx = length * 0.5 + overhang;
    let lx = if style == RoofStyle::HalfHip {
        half_hip_fronton_x(lx, wy)
    } else {
        lx
    };
    // Окно — небольшой бортик-окошко во фронтоне (+X), центр — на высоте
    // середины фронтона; выступ наружу 0.03 м.
    Part::boxed_step(
        "фронтон-окно",
        Vec3::new(0.06, w, h),
        Vec3::X,
        colors.window,
        Transform::from_xyz(lx, ridge, z_mid),
        config,
        config.point_step,
    )
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
    let (dx, dy) = match style {
        RoofStyle::Flat => (
            rng.random_range(-0.6..0.6),
            rng.random_range(-0.6..0.6),
        ),
        _ => (
            rng.random_range(-(lx * 0.55)..(lx * 0.55)),
            rng.random_range(-(wy * 0.5)..(wy * 0.5)),
        ),
    };
    // Высота кровли в точке установки, чтобы низ короба «сидел» в крыше.
    let plane = pitch_plane_z(style, wy, rise, ridge, dy);
    let h = 1.0;
    let size = Vec3::new(0.42, 0.42, h);
    let pos = Vec3::new(dx, dy, plane + h * 0.5 - 0.12);
    // Точки — на всех боках и сверху.
    Part::boxed_outwards(
        "дымоход",
        size,
        &[Vec3::Z, Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y],
        super::palette::CHIMNEY,
        Transform::from_translation(pos),
        config,
    )
}

/// Высота плоскости крыши в точке `(y)` (по Y — поперёк гребня; для всех
/// скатных плоскостей высота не зависит от X). Нужна для установки дымохода
/// (и мансарды) точно на скат.
fn pitch_plane_z(style: RoofStyle, wy: f32, rise: f32, ridge: f32, y: f32) -> f32 {
    match style {
        RoofStyle::Flat => 0.0,
        RoofStyle::Shed => rise * (y + wy) / (2.0 * wy.max(0.01)),
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
    colors: &BuildingColors,
    config: &BuildingConfig,
) -> Part {
    let pitched = style.is_pitched();

    // Поворот скатных крыш вокруг вертикали выбираем ДО построения
    // геометрии: для 90°/270° крыша строится с «перевёрнутыми» габаритами,
    // и после поворота встаёт точно на стены (гребень — вдоль нужной оси).
    let mut rng = config.seed.with_label("крыша-поворот").rng();
    let spin = rng.random_range(0..4);
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
            let faces = roof_faces(pitched, roof_length, roof_width, overhang, pitch_deg, config, ridge);
            let canopy =
                Part::from_faces("кровля", faces, colors.roof, Transform::IDENTITY, true, config);
            roof = roof.child(canopy);

            if dormer {
                // Два варианта мансарды: выступающий из ската домик (скатные
                // типы с передним скатом) и окно во фронтоне (типы с
                // фронтонами).
                let has_fronton =
                    matches!(pitched, RoofStyle::Gable | RoofStyle::HalfHip | RoofStyle::Shed);
                let protruding_ok =
                    matches!(pitched, RoofStyle::Gable | RoofStyle::HalfHip | RoofStyle::Hip);
                let use_fronton_window =
                    has_fronton && (!protruding_ok || rng.random::<f32>() < 0.5);
                if use_fronton_window {
                    roof = roof.child(fronton_window_part(
                        roof_length,
                        roof_width,
                        overhang,
                        pitch_deg,
                        pitched,
                        ridge,
                        colors,
                        config,
                    ));
                } else if protruding_ok {
                    roof = roof.child(dormer_part(
                        roof_length,
                        roof_width,
                        overhang,
                        pitch_deg,
                        ridge,
                        colors,
                        config,
                    ));
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

// Inset the gables to form sloping end hips; retain room for roof details.
fn half_hip_fronton_x(lx: f32, wy: f32) -> f32 {
    lx - (wy * 0.55).min(lx * 0.4)
}
