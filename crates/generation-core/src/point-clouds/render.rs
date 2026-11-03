//! Пользовательский конвейер отрисовки облаков точек.
//!
//! Каждая точка облака — это инстанс: вершинный буфер меша содержит позицию,
//! цвет и базовый размер точки, а на инстанс рисуется экранный квад (4
//! вершины, `TriangleStrip`). Шейдер превращает квад в мягкий круг и
//! масштабирует его по расстоянию до камеры.
//!
//! Почему не материал Bevy: `PointList` на GPU рисует точки размером 1 px,
//! а `StandardMaterial` не умеет круглые точки с прозрачным краем. Квады на
//! CPU в 4–6 раз увеличивают объём вершинных данных, поэтому здесь используется
//! инстансинг прямо по вершинам облака.
//!
//! Мировая матрица облака передаётся через динамический uniform-буфер в
//! bind group 1. Это позволяет обойтись без `NoIndirectDrawing` и не отключать
//! GPU-препроцессинг для остальной сцены.

use bevy::camera::visibility::ViewVisibility;
use bevy::core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d};
use bevy::ecs::query::QueryItem;
use bevy::ecs::system::SystemParamItem;
use bevy::ecs::system::lifetimeless::*;
use bevy::mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{
    MeshPipeline, MeshPipelineKey, MeshPipelineSystems, RenderMeshInstances, SetMeshViewBindGroup,
    ViewKeyCache,
};
use bevy::prelude::*;
use bevy::render::batching::NoAutomaticBatching;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::mesh::{RenderMesh, allocator::MeshAllocator};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
    RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::sync_component::SyncComponent;
use bevy::render::sync_world::MainEntity;
use bevy::render::view::ExtractedView;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

use super::visual::ATTRIBUTE_POINT_SIZE;

/// Маркер сущности облака точек в основном мире.
///
/// Синхронизирует [`PointCloudTransform`] в render world и служит фильтром для
/// пользовательской очереди отрисовки.
///
/// `NoAutomaticBatching` обязателен: у каждой точки облака своя мировая матрица
/// в динамическом uniform-буфере, поэтому объединять облака в один батч нельзя —
/// иначе отрисуется только представитель батча.
#[derive(Component, Clone, Copy, Debug, Default)]
#[require(NoAutomaticBatching)]
pub struct PointCloudTag;

/// Мировая матрица облака, доступная в render world.
#[derive(Component, Clone, Copy, Debug)]
pub struct PointCloudTransform(pub Mat4);

impl SyncComponent<RenderApp> for PointCloudTag {
    type Target = PointCloudTransform;
}

impl ExtractComponent<RenderApp> for PointCloudTag {
    type QueryData = (&'static GlobalTransform, &'static ViewVisibility);
    type QueryFilter = ();
    type Out = PointCloudTransform;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        let (transform, visibility) = item;
        // Скрытые облака не попадают в очередь отрисовки: компонент
        // `PointCloudTransform` снимается, когда `ViewVisibility == false`.
        visibility
            .get()
            .then(|| PointCloudTransform(transform.to_matrix()))
    }
}

/// Плагин, добавляющий конвейер облаков точек.
///
/// Добавляйте его после `DefaultPlugins`:
/// ```ignore
/// app.add_plugins((DefaultPlugins, PointCloudRenderPlugin));
/// ```
pub struct PointCloudRenderPlugin;

impl Plugin for PointCloudRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<PointCloudTag>::default());

        let shader = {
            let Some(mut shaders) = app.world_mut().get_resource_mut::<Assets<Shader>>() else {
                return;
            };
            shaders.add(Shader::from_wesl(
                include_str!("point_cloud.wesl"),
                "embedded://generation_core/point_cloud.wesl",
            ))
        };

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.insert_resource(PointCloudShaderHandle(shader));
        render_app.init_resource::<PointCloudUniforms>();
        render_app.init_resource::<SpecializedMeshPipelines<PointCloudPipeline>>();
        render_app.add_render_command::<Transparent3d, DrawPointCloud>();
        render_app.add_systems(
            RenderStartup,
            init_point_cloud_pipeline.after(MeshPipelineSystems),
        );
        render_app.add_systems(
            Render,
            (
                queue_point_clouds.in_set(RenderSystems::QueueMeshes),
                prepare_point_cloud_uniforms.in_set(RenderSystems::PrepareResources),
            ),
        );
    }
}

/// Хэндл встроенного шейдера, созданный в основном мире.
#[derive(Resource, Clone)]
struct PointCloudShaderHandle(Handle<Shader>);

/// Динамический uniform с мировой матрицей облака.
#[derive(Clone, Copy, ShaderType)]
struct PointCloudUniform {
    world_from_local: Mat4,
}

/// Смещение матрицы конкретного облака в динамическом uniform-буфере.
#[derive(Component, Clone, Copy, Debug)]
struct PointCloudUniformOffset(u32);

/// Ресурсы, общие для подготовки и отрисовки облаков.
#[derive(Resource, Default)]
struct PointCloudUniforms {
    buffer: DynamicUniformBuffer<PointCloudUniform>,
    layout: Option<BindGroupLayout>,
    bind_group: Option<BindGroup>,
}

/// Конвейер облаков точек на основе `MeshPipeline` (общие view-биндинги,
/// MSAA, tonemapping и т.п.).
#[derive(Resource)]
struct PointCloudPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
    uniform_layout: BindGroupLayoutDescriptor,
}

fn point_cloud_uniform_entries() -> [BindGroupLayoutEntry; 1] {
    [BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::VERTEX,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: Some(PointCloudUniform::min_size()),
        },
        count: None,
    }]
}

fn init_point_cloud_pipeline(
    mut commands: Commands,
    mesh_pipeline: Res<MeshPipeline>,
    render_device: Res<RenderDevice>,
    shader: Res<PointCloudShaderHandle>,
    mut uniforms: ResMut<PointCloudUniforms>,
) {
    let entries = point_cloud_uniform_entries();
    uniforms.layout =
        Some(render_device.create_bind_group_layout("point_cloud_uniform_layout", &entries));

    commands.insert_resource(PointCloudPipeline {
        shader: shader.0.clone(),
        mesh_pipeline: mesh_pipeline.clone(),
        uniform_layout: BindGroupLayoutDescriptor::new("point_cloud_uniform_layout", &entries),
    });
}

impl SpecializedMeshPipeline for PointCloudPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        mut key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        // Прозрачность: alpha blending, без записи в depth buffer.
        key |= MeshPipelineKey::BLEND_ALPHA;

        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor.vertex.shader = self.shader.clone();
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = self.shader.clone();
        }

        // Вершины меша становятся инстансами: position/color/size.
        let mut instance_layout = layout.0.get_layout(&[
            bevy::mesh::Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            bevy::mesh::Mesh::ATTRIBUTE_COLOR.at_shader_location(1),
            ATTRIBUTE_POINT_SIZE.at_shader_location(2),
        ])?;
        instance_layout.step_mode = VertexStepMode::Instance;
        descriptor.vertex.buffers = vec![instance_layout];

        // Квад из 4 вершин на инстанс.
        descriptor.primitive.topology = PrimitiveTopology::TriangleStrip;
        descriptor.primitive.strip_index_format = None;
        descriptor.primitive.cull_mode = None;

        // Оставляем только view-биндинги (группа 0) и нашу матрицу облака
        // (группа 1). Группы 1–2 стандартного `MeshPipeline` (bind group
        // массива view и mesh-данные) шейдеру не нужны, а wgpu требует
        // привязывать все группы конвейера — поэтому лишние группы убираем.
        descriptor.layout = vec![descriptor.layout[0].clone(), self.uniform_layout.clone()];

        Ok(descriptor)
    }
}

fn queue_point_clouds(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    pipeline: Res<PointCloudPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<PointCloudPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    point_clouds: Query<(Entity, &MainEntity, &PointCloudTransform)>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<&ExtractedView>,
    view_key_cache: Res<ViewKeyCache>,
) {
    let draw_point_cloud = transparent_3d_draw_functions.read().id::<DrawPointCloud>();

    for view in &views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };
        let Some(&view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };

        for (entity, main_entity, transform) in &point_clouds {
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id()) else {
                continue;
            };

            let key = view_key | MeshPipelineKey::BLEND_ALPHA;
            let Ok(pipeline) = pipelines.specialize(&pipeline_cache, &pipeline, key, &mesh.layout)
            else {
                continue;
            };

            let mesh_center = transform.0.transform_point3(mesh.aabb_center);
            transparent_phase.add_retained(Transparent3d {
                sorting_info: TransparentSortingInfo3d::Sorted {
                    mesh_center,
                    depth_bias: 0.0,
                },
                distance: 0.0,
                pipeline,
                entity: (entity, *main_entity),
                draw_function: draw_point_cloud,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                // Рисуем через `draw`, а не `draw_indexed`.
                indexed: false,
            });
        }
    }
}

fn prepare_point_cloud_uniforms(
    mut uniforms: ResMut<PointCloudUniforms>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    point_clouds: Query<(Entity, &PointCloudTransform)>,
    mut commands: Commands,
) {
    let Some(layout) = uniforms.layout.clone() else {
        return;
    };

    uniforms.buffer.clear();
    for (entity, transform) in &point_clouds {
        let offset = uniforms.buffer.push(&PointCloudUniform {
            world_from_local: transform.0,
        });
        commands
            .entity(entity)
            .insert(PointCloudUniformOffset(offset));
    }
    uniforms.buffer.write_buffer(&render_device, &render_queue);

    let bind_group = match uniforms.buffer.binding() {
        Some(binding) => Some(render_device.create_bind_group(
            "point_cloud_uniform_bind_group",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: binding,
            }],
        )),
        None => None,
    };
    uniforms.bind_group = bind_group;
}

type DrawPointCloud = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetPointCloudUniform,
    DrawPointCloudInstances,
);

/// Привязывает группу 1 с матрицей текущего облака.
struct SetPointCloudUniform;

impl<P: PhaseItem> RenderCommand<P> for SetPointCloudUniform {
    type Param = SRes<PointCloudUniforms>;
    type ViewQuery = ();
    type ItemQuery = Read<PointCloudUniformOffset>;

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        offset: Option<&'w PointCloudUniformOffset>,
        uniforms: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let uniforms = uniforms.into_inner();
        let Some(bind_group) = uniforms.bind_group.as_ref() else {
            return RenderCommandResult::Skip;
        };
        let Some(offset) = offset else {
            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(1, bind_group, &[offset.0]);
        RenderCommandResult::Success
    }
}

/// Рисует 4 вершины (TriangleStrip) на каждый инстанс-пункт.
struct DrawPointCloudInstances;

impl<P: PhaseItem> RenderCommand<P> for DrawPointCloudInstances {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item: Option<()>,
        (meshes, render_mesh_instances, mesh_allocator): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id()) else {
            return RenderCommandResult::Skip;
        };
        let Some(vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id())
        else {
            return RenderCommandResult::Skip;
        };

        let instance_count = vertex_buffer_slice.range.end - vertex_buffer_slice.range.start;
        if instance_count == 0 {
            return RenderCommandResult::Skip;
        }

        // `range` задан в элементах (вершинах), а `set_vertex_buffer`
        // принимает байтовое смещение.
        let stride = gpu_mesh.layout.0.layout().array_stride;
        let byte_offset = u64::from(vertex_buffer_slice.range.start) * stride;
        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(byte_offset..));
        pass.draw(0..4, 0..instance_count);

        RenderCommandResult::Success
    }
}
