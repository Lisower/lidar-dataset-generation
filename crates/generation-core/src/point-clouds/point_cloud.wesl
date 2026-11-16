//! Шейдер облака точек: экранный билборд на каждую точку.
//!
//! Вершинный буфер меша используется как инстанс-буфер: `@location(0)` —
//! позиция точки, `@location(1)` — цвет, `@location(2)` — базовый размер в
//! пикселях. На инстанс рисуются 4 вершины (`TriangleStrip`), которые шейдер
//! раздвигает в экранные координаты. Фрагментный шейдер делает круг с мягким
//! краем: в центре точка чуть ярче, к краю прозрачность растёт до нуля.

import bevy_pbr::render::mesh_view_bindings::view;

@if(TONEMAP_IN_SHADER) import bevy_core_pipeline::tonemapping::tone_mapping;

struct PointCloudUniform {
    world_from_local: mat4x4<f32>,
};

@group(1) @binding(0) var<uniform> point_cloud: PointCloudUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) size: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_uv: vec2<f32>,
};

@vertex
fn vertex(input: VertexInput, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let world_position = point_cloud.world_from_local * vec4<f32>(input.position, 1.0);
    var clip = view.clip_from_world * world_position;

    // Размер зависит от расстояния: ближе — крупнее, дальше — мельче.
    // Опорная дистанция 40 м; множитель ограничен, чтобы точки не исчезали
    // и не разрастались в огромные пятна.
    let distance = max(length(world_position.xyz - view.world_position), 0.05);
    let size_scale = clamp(40.0 / distance, 0.5, 2.0);
    let size_px = clamp(input.size * size_scale, 1.5, 9.0);

    // Углы квада: (-1,-1), (1,-1), (-1,1), (1,1).
    var corner = vec2<f32>(-1.0, -1.0);
    if vertex_index == 1u {
        corner = vec2<f32>(1.0, -1.0);
    } else if vertex_index == 2u {
        corner = vec2<f32>(-1.0, 1.0);
    } else if vertex_index == 3u {
        corner = vec2<f32>(1.0, 1.0);
    }

    // Пиксельный размер переводим в NDC через размер viewport.
    let viewport = max(view.viewport.zw, vec2<f32>(1.0, 1.0));
    let ndc_offset = corner * size_px / viewport;
    clip = vec4<f32>(clip.xy + ndc_offset * clip.w, clip.z, clip.w);

    var out: VertexOutput;
    out.clip_position = clip;
    out.color = input.color;
    out.local_uv = corner;
    return out;
}

/// Мягкий край: 1 в центре, 0 за пределами круга.
fn point_alpha(uv: vec2<f32>) -> f32 {
    let distance = length(uv);
    let edge = max(fwidth(distance), 0.001);
    return 1.0 - smoothstep(0.5 - edge, 0.5 + edge, distance);
}

/// Яркость центра: небольшая подсветка, чтобы точка не выглядела плоской.
fn point_core(uv: vec2<f32>) -> f32 {
    let distance = length(uv);
    let edge = max(fwidth(distance), 0.001);
    return 1.0 - smoothstep(0.0, 0.5 + edge, distance);
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = point_alpha(input.local_uv);
    if alpha <= 0.001 {
        discard;
    }

    let core = point_core(input.local_uv);
    let color = input.color.rgb * (0.78 + 0.30 * core);
    var out = vec4<f32>(color, input.color.a * alpha);
    @if(TONEMAP_IN_SHADER) {
        out = tone_mapping(out, view.color_grading);
    }
    return out;
}
