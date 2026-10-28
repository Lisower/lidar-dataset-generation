// Необходимо разделить этот файл на несколько так, чтобы был файл общей логики камер и отдельно - на каждый вид камеры
// Нужно добавить к плагину логику выбора типов камер с их настройками, плюс учесть, что могут быть добвалены новые типы камер 
// (т.е. подготовить плагин к тому, что видов камер может быть больше, чем 2)
// Т.е. в приложение встраивается коллекция камер (в коде приложения указыватся, какие), и переключение режима камеры на Tab
// Все коменты, что ниже, сгенерированы DeepSeek на основе кода

//! Камеры: орбитальная и свободная, с плавным переходом между режимами.
//!
//! # Зачем две камеры
//!
//! Генератор показывает объект целиком и в деталях — это два разных движения.
//! Орбитальная камера вращается вокруг точки интереса и подходит для осмотра
//! сцены: облако точек ландшафта видно только с расстояния. Свободная камера
//! позволяет подлететь к дому и рассмотреть фасад. Переключение — `Tab`.
//!
//! # Управление
//!
//! | Режим | Действие | Клавиши |
//! |---|---|---|
//! | Орбитальная | вращение | ЛКМ + движение мыши |
//! | Орбитальная | приближение | колесо мыши |
//! | Свободная | обзор | ПКМ + движение мыши |
//! | Свободная | движение | `W`, `A`, `S`, `D`, `Q` (вниз), `E` (вверх) |
//! | Свободная | ускорение | `Shift` |
//! | Любой | смена режима | `Tab` |
//!
//! Переход между режимами анимируется, поэтому камера не «прыгает»: при
//! переключении в орбитальный режим сохраняются текущее направление взгляда
//! и точка интереса.

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use generation_core::UiModalOpen;

/// Режим управления камерой.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraMode {
    /// Вращение вокруг точки интереса: удобно рассматривать объект целиком.
    #[default]
    Orbit,
    /// Свободный полёт: удобно рассматривать детали.
    Free,
}

impl CameraMode {
    /// Противоположный режим.
    pub fn toggled(self) -> Self {
        match self {
            Self::Orbit => Self::Free,
            Self::Free => Self::Orbit,
        }
    }
}

/// Основная камера приложения.
///
/// Компонент ставит плагин камер; приложения опираются на него только если
/// им нужно двигать камеру самим (например, для перелёта к объекту).
#[derive(Component, Debug)]
pub struct MainCamera;

/// Параметры орбитальной камеры (сферические координаты вокруг точки интереса).
#[derive(Resource, Debug, Clone, Copy)]
pub struct OrbitCamera {
    /// Точка, вокруг которой вращается камера.
    pub focus: Vec3,
    /// Поворот вокруг вертикальной оси.
    pub yaw: f32,
    /// Наклон: 0 — взгляд горизонтально, положительный — сверху вниз.
    pub pitch: f32,
    /// Расстояние до точки интереса, м.
    pub radius: f32,
    /// Минимальное расстояние, м.
    pub min_radius: f32,
    /// Максимальное расстояние, м.
    pub max_radius: f32,
    /// Минимальный наклон (радианы).
    pub min_pitch: f32,
    /// Максимальный наклон (радианы).
    pub max_pitch: f32,
    /// Чувствительность вращения.
    pub rotate_speed: f32,
    /// Чувствительность зума.
    pub zoom_speed: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.55,
            radius: 60.0,
            min_radius: 1.0,
            max_radius: 600.0,
            min_pitch: -1.45,
            max_pitch: 1.45,
            rotate_speed: 0.005,
            zoom_speed: 0.15,
        }
    }
}

impl OrbitCamera {
    /// Направление от точки интереса к камере.
    pub fn direction(&self) -> Vec3 {
        Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        )
    }

    /// Позиция камеры для текущих параметров.
    pub fn position(&self) -> Vec3 {
        self.focus + self.direction() * self.radius
    }

    /// Трансформ камеры для текущих параметров.
    pub fn transform(&self) -> Transform {
        Transform::from_translation(self.position()).looking_at(self.focus, Vec3::Y)
    }
}

/// Параметры свободной камеры.
#[derive(Resource, Debug, Clone, Copy)]
pub struct FreeCamera {
    /// Скорость движения, м/с.
    pub speed: f32,
    /// Множитель ускорения при нажатом `Shift`.
    pub boost: f32,
    /// Чувствительность обзора.
    pub sensitivity: f32,
    /// Поворот вокруг вертикальной оси.
    pub yaw: f32,
    /// Наклон.
    pub pitch: f32,
}

impl Default for FreeCamera {
    fn default() -> Self {
        Self {
            speed: 20.0,
            boost: 3.0,
            sensitivity: 0.003,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

/// Плагин камер: спавнит камеру и включает управление.
///
/// Ожидает, что в приложении есть ввод (`InputPlugin` входит в
/// `DefaultPlugins`): управление читает клавиши, кнопки мыши и её движение.
///
/// ```
/// use bevy::prelude::*;
/// use bevy_utils::cameras::CameraPlugin;
///
/// let mut app = App::new();
/// app.add_plugins((MinimalPlugins, CameraPlugin::new(Vec3::new(0.0, 0.0, 5.0), 40.0)));
/// ```
#[derive(Clone, Copy, Debug)]
pub struct CameraPlugin {
    /// Точка, вокруг которой вращается орбитальная камера.
    pub focus: Vec3,
    /// Начальное расстояние до точки интереса, м.
    pub radius: f32,
    /// Длительность перехода между режимами, с.
    pub transition_seconds: f32,
}

impl CameraPlugin {
    /// Камера вокруг точки `focus` на расстоянии `radius`.
    pub fn new(focus: Vec3, radius: f32) -> Self {
        Self {
            focus,
            radius,
            ..default()
        }
    }
}

impl Default for CameraPlugin {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            radius: 60.0,
            transition_seconds: 0.5,
        }
    }
}

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        let orbit = OrbitCamera {
            focus: self.focus,
            radius: self.radius,
            ..default()
        };
        let radius = self.radius;
        let transform = orbit.transform();

        app.init_resource::<UiModalOpen>()
            .insert_resource(CameraMode::default())
            .insert_resource(orbit)
            .insert_resource(FreeCamera::default())
            .insert_resource(CameraTransition::new(self.transition_seconds))
            .add_systems(Startup, move |mut commands: Commands| {
                commands.spawn((Camera3d::default(), transform, MainCamera));
                info!("Камера: орбитальная, радиус {radius:.1} м");
            })
            .add_systems(
                Update,
                (toggle_mode, camera_transition, orbit_input, free_input).chain(),
            );
    }
}

/// Переход между режимами камеры.
#[derive(Resource)]
struct CameraTransition {
    active: bool,
    timer: Timer,
    start: Transform,
    target: Transform,
}

impl CameraTransition {
    fn new(seconds: f32) -> Self {
        Self {
            active: false,
            timer: Timer::from_seconds(seconds.max(f32::EPSILON), TimerMode::Once),
            start: Transform::default(),
            target: Transform::default(),
        }
    }

    fn start(&mut self, from: Transform, to: Transform) {
        self.start = from;
        self.target = to;
        self.timer.reset();
        self.active = true;
    }
}

/// `Tab` — переключение между орбитальной и свободной камерой.
fn toggle_mode(
    ui: Res<UiModalOpen>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<CameraMode>,
    mut orbit: ResMut<OrbitCamera>,
    mut free: ResMut<FreeCamera>,
    mut transition: ResMut<CameraTransition>,
    camera: Query<&Transform, With<MainCamera>>,
) {
    if ui.0 || !keys.just_pressed(KeyCode::Tab) || transition.active {
        return;
    }

    let Ok(transform) = camera.single() else {
        return;
    };

    match *mode {
        CameraMode::Orbit => {
            // Орбита → свободная: продолжаем смотреть туда же, куда смотрели.
            let forward = transform.forward().as_vec3();
            free.yaw = (-forward.x).atan2(-forward.z);
            free.pitch = forward.y.clamp(-1.0, 1.0).asin().clamp(-1.54, 1.54);
            *mode = CameraMode::Free;
        }
        CameraMode::Free => {
            // Свободная → орбита: точка интереса — центр сцены, радиус —
            // текущее расстояние до неё.
            let offset = transform.translation - orbit.focus;
            let distance = offset.length();

            if distance > 0.001 {
                orbit.radius = distance.clamp(orbit.min_radius, orbit.max_radius);

                let direction = offset / distance;
                orbit.yaw = direction.x.atan2(direction.z);
                orbit.pitch = direction
                    .y
                    .clamp(-1.0, 1.0)
                    .asin()
                    .clamp(orbit.min_pitch, orbit.max_pitch);
            }

            transition.start(*transform, orbit.transform());
            *mode = CameraMode::Orbit;
        }
    }

    info!("Режим камеры: {:?}", *mode);
}

/// Плавный переход камеры к целевому положению.
fn camera_transition(
    time: Res<Time>,
    mut transition: ResMut<CameraTransition>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    if !transition.active {
        return;
    }

    let Ok(mut transform) = camera.single_mut() else {
        return;
    };

    transition.timer.tick(time.delta());
    let linear = transition.timer.fraction();
    // Сглаживание: медленно — быстро — медленно.
    let t = linear * linear * (3.0 - 2.0 * linear);

    transform.translation = transition
        .start
        .translation
        .lerp(transition.target.translation, t);
    transform.rotation = transition
        .start
        .rotation
        .slerp(transition.target.rotation, t);

    if transition.timer.is_finished() {
        *transform = transition.target;
        transition.active = false;
    }
}

/// Орбитальная камера: ЛКМ — вращение, колесо — приближение.
fn orbit_input(
    ui: Res<UiModalOpen>,
    mode: Res<CameraMode>,
    transition: Res<CameraTransition>,
    mut orbit: ResMut<OrbitCamera>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    if ui.0 || *mode != CameraMode::Orbit || transition.active {
        motion.clear();
        wheel.clear();
        return;
    }

    let mut delta = Vec2::ZERO;
    for event in motion.read() {
        delta += event.delta;
    }

    let mut scroll = 0.0;
    for event in wheel.read() {
        scroll += event.y;
    }

    if mouse_buttons.pressed(MouseButton::Left) {
        orbit.yaw -= delta.x * orbit.rotate_speed;
        orbit.pitch =
            (orbit.pitch + delta.y * orbit.rotate_speed).clamp(orbit.min_pitch, orbit.max_pitch);
    }

    if scroll != 0.0 {
        // Экспоненциальный зум: на больших расстояниях он удобнее линейного.
        orbit.radius = (orbit.radius * (1.0 - scroll * orbit.zoom_speed))
            .clamp(orbit.min_radius, orbit.max_radius);
    }

    let Ok(mut transform) = camera.single_mut() else {
        return;
    };

    *transform = orbit.transform();
}

/// Свободная камера: ПКМ — обзор, `WASD`/`QE` — движение, `Shift` — ускорение.
fn free_input(
    ui: Res<UiModalOpen>,
    mode: Res<CameraMode>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut free: ResMut<FreeCamera>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    if ui.0 || *mode != CameraMode::Free {
        motion.clear();
        return;
    }

    let Ok(mut transform) = camera.single_mut() else {
        return;
    };

    let mut delta = Vec2::ZERO;
    for event in motion.read() {
        delta += event.delta;
    }

    if mouse_buttons.pressed(MouseButton::Right) {
        free.yaw -= delta.x * free.sensitivity;
        free.pitch = (free.pitch - delta.y * free.sensitivity).clamp(-1.54, 1.54);

        // Поворот вокруг мировой оси Y, затем наклон вокруг локальной оси X.
        // Крен не возникает: вращение каждый кадр собирается заново.
        transform.rotation = Quat::from_rotation_y(free.yaw) * Quat::from_rotation_x(free.pitch);
    }

    let mut direction = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        direction += *transform.forward();
    }
    if keys.pressed(KeyCode::KeyS) {
        direction -= *transform.forward();
    }
    if keys.pressed(KeyCode::KeyD) {
        direction += *transform.right();
    }
    if keys.pressed(KeyCode::KeyA) {
        direction -= *transform.right();
    }
    if keys.pressed(KeyCode::KeyE) {
        direction += Vec3::Y;
    }
    if keys.pressed(KeyCode::KeyQ) {
        direction -= Vec3::Y;
    }

    if direction == Vec3::ZERO {
        return;
    }

    let mut speed = free.speed;
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        speed *= free.boost;
    }

    transform.translation += direction.normalize() * speed * time.delta_secs();
}
