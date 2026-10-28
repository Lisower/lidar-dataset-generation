//! Модальное окно-«оболочка» на BSN: тёмный полупрозрачный оверлей,
//! центрированная панель с рамкой и шапка с заголовком и кнопкой закрытия.
//!
//! Окно не знает, что за содержимое в нём: вызывающий строит контент
//! ([`SceneList`]) и передаёт его в [`spawn_window_frame`]. Кнопка «×» и
//! внешняя логика закрытия выставляют [`UiModalOpen(false)`](UiModalOpen),
//! а хозяин окна (например [`crate::preset_selector`]) следит за этим
//! флагом и снимает сцену с экрана.
//!
//! Стиль: белый текст на тёмном полупрозрачном фоне, как в панелях
//! информации генераторов.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::FontSourceTemplate;
use bevy::ui_widgets::{Activate, Button};
use generation_core::UiModalOpen;

/// Шрифт окон (совпадает с панелями информации генераторов).
pub const FONT_PATH: &str = "fonts/Roboto-Regular.ttf";

/// Цвет оверлея (затемнение сцены за окном).
pub const OVERLAY_COLOR: Color = Color::srgba(0.02, 0.02, 0.03, 0.68);
/// Цвет панели окна.
pub const PANEL_COLOR: Color = Color::srgb(0.07, 0.09, 0.11);
/// Рамка панели.
pub const PANEL_BORDER: Color = Color::srgb(0.24, 0.28, 0.36);
/// Основной цвет текста.
pub const TEXT_COLOR: Color = Color::srgb(0.94, 0.96, 0.98);
/// Приглушённый текст (счётчики, невыбранные пункты).
pub const MUTED_COLOR: Color = Color::srgb(0.6, 0.64, 0.7);
/// Акцент (выбранные пункты, основная кнопка).
pub const ACCENT_COLOR: Color = Color::srgb(0.36, 0.64, 0.96);
/// Цвет строки списка.
pub const ROW_COLOR: Color = Color::srgb(0.1, 0.12, 0.15);
/// Цвет строки-группы списка.
pub const GROUP_COLOR: Color = Color::srgb(0.14, 0.16, 0.2);
/// Цвет частично выбранной группы (выбрана только часть пресетов).
pub const PARTIAL_COLOR: Color = Color::srgb(0.96, 0.74, 0.25);

/// Параметры рамки окна.
pub struct WindowSpec {
    /// Заголовок окна.
    pub title: &'static str,
    /// Ширина панели.
    pub width: Val,
    /// Высота панели.
    pub height: Val,
}

/// Дескриптор спавннутого окна.
pub struct UiWindowFrame {
    /// Корневая сущность окна (оверлей). При закрытии её нужно удалить.
    pub root: Entity,
}

/// Спавн рамки модального окна; `content` размещается под шапкой и
/// растягивается на всю оставшуюся высоту панели (вызывающий сам задаёт
/// скролл-область и нижнюю кнопку).
pub fn spawn_window_frame(
    commands: &mut Commands,
    spec: &WindowSpec,
    content: impl SceneList,
) -> UiWindowFrame {
    let title = spec.title;
    let width = spec.width;
    let height = spec.height;
    let root = commands
        .spawn_scene(bsn! {
            #UiWindowOverlay
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
            }
            BackgroundColor({OVERLAY_COLOR})
            Children [
                #UiWindowPanel
                Node {
                    width: {width},
                    height: {height},
                    flex_direction: FlexDirection::Column,
                    // Расстояние между шапкой и содержимым.
                    row_gap: px(12),
                    border: px(1),
                    border_radius: BorderRadius::all(px(10)),
                    padding: UiRect::all(px(14)),
                }
                BackgroundColor({PANEL_COLOR})
                BorderColor::from(PANEL_BORDER)
                Children [
                    #UiWindowHeader
                    Node {
                        width: percent(100),
                        height: px(34),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                    }
                    Children [
                        Text({title})
                        TextFont {
                            font: FontSourceTemplate::Handle(FONT_PATH),
                            font_size: px(20.0),
                        }
                        TextColor({TEXT_COLOR})
                        TextShadow::default()
                        --
                        #UiWindowClose
                        Button
                        Hovered::default()
                        Node {
                            width: px(30),
                            height: px(30),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::MAX,
                        }
                        BackgroundColor({PANEL_BORDER})
                        on(|_: On<Activate>, mut ui: ResMut<UiModalOpen>| {
                            ui.0 = false;
                        })
                        Children [
                            Text("×")
                            TextFont {
                                font: FontSourceTemplate::Handle(FONT_PATH),
                                font_size: px(18.0),
                            }
                            TextColor({TEXT_COLOR})
                        ]
                    ]
                    --
                    {content}
                ]
            ]
        })
        .id();
    UiWindowFrame { root }
}
