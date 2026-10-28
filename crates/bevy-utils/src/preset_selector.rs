//! Выбор демо-пресетов в модальном окне (`Ctrl+S`) и случайный выбор пресета
//! при демонстрации.
//!
//! Плагин знакомит приложение с [`PresetStore`] (индекс пресетов одной
//! категории `presets/<категория>`) и строит окно выбора:
//!
//! - `Ctrl+S` — открыть/закрыть окно, `Esc` — закрыть;
//! - строки-чекбоксы пресетов и групп, кнопка «выбрать все / снять все»;
//! - двойной клик по строке группы — свернуть/развернуть её содержимое
//!   (стрелка `▾`/`▸` у имени);
//! - частично выбранная группа помечается `[-]` янтарным цветом;
//! - кнопка «Применить» сохраняет выбор и закрывает окно; закрытие без неё
//!   (`×`, `Esc`, `Ctrl+S`) **откатывает** не применённые изменения.
//! - при применении изменений запрашивается перегенерация с тем же seed —
//!   генератор подставит новый случайный пресет из выбранных.
//!
//! Окно пересобирается целиком при любой смене выбора/сворачивания (просто и
//! надёжно). Пока окно открыто, общий ресурс [`UiModalOpen`] блокирует горячие
//! клавиши сцены (камера, `R`, `V`).

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::picking::hover::Hovered;
use bevy::scene::EntityScene;
use bevy::text::FontSourceTemplate;
use bevy::ui_widgets::{Activate, Button};
use generation_core::general::RegenerationRequest;
use generation_core::presets::{PresetGroup, PresetMeta, PresetStore};
use generation_core::UiModalOpen;

use crate::ui_window::{
    ACCENT_COLOR, FONT_PATH, GROUP_COLOR, MUTED_COLOR, PANEL_BORDER, PARTIAL_COLOR, ROW_COLOR,
    TEXT_COLOR, WindowSpec, spawn_window_frame,
};

/// Плагин окна выбора пресетов одной категории (`landscapes`, `plants`,
/// `buildings`, `terrains`).
pub struct PresetSelectorPlugin {
    /// Категория: подпапка `presets/`.
    pub category: &'static str,
    /// Корень всех пресетов (`.../presets`).
    pub presets_root: PathBuf,
    /// Папка настроек крейта (`.../settings`): здесь хранится выбор.
    pub settings_dir: PathBuf,
}

/// Состояние окна выбора пресетов.
#[derive(Resource)]
pub struct PresetWindowState {
    /// Корневая сущность текущего окна (оверлей), если оно открыто.
    pub root: Option<Entity>,
    /// Окно нужно пересобрать (выбор или сворачивание изменились).
    pub dirty: bool,
    /// Свернутые группы (id групп), содержимое которых скрыто.
    pub collapsed: BTreeSet<String>,
    /// Снимок применённого выбора. При закрытии без «Применить» выбор
    /// откатывается к нему.
    pub applied: BTreeSet<String>,
    /// «Применить» нажата: при закрытии сохранить выбор и (если он менялся)
    /// перегенерировать сцену.
    pub apply_requested: bool,
    /// Корень всех пресетов (для cross-category подгрузки суб-пресетов
    /// террейнами и другими читателями).
    pub presets_root: PathBuf,
    /// Папка настроек крейта, куда сохраняется выбор.
    pub settings_dir: PathBuf,
}

/// Время последнего клика по строке (ключ — id строки) для распознавания
/// двойного клика — сворачивания/разворачивания группы.
#[derive(Resource, Default)]
pub struct RowClickTracker(pub HashMap<String, f64>);

/// Максимальный промежуток между кликами, считающийся двойным кликом.
const DOUBLE_CLICK_SECS: f64 = 0.35;

impl Plugin for PresetSelectorPlugin {
    fn build(&self, app: &mut App) {
        let mut store = PresetStore::discover(self.category, &self.presets_root);
        store.load_selection(&self.settings_dir);
        if store.presets.is_empty() {
            warn!(
                "preset_selector: в {} нет пресетов — окно выбора будет пустым",
                store.root.display()
            );
        }
        let applied = store.selected.clone();
        app.init_resource::<UiModalOpen>()
            .init_resource::<RowClickTracker>()
            .insert_resource(store)
            .insert_resource(PresetWindowState {
                root: None,
                dirty: false,
                collapsed: BTreeSet::new(),
                applied,
                apply_requested: false,
                presets_root: self.presets_root.clone(),
                settings_dir: self.settings_dir.clone(),
            })
            .add_systems(Update, (preset_window_key, sync_preset_window).chain());
    }
}

/// `Ctrl+S` — открыть/закрыть окно, `Esc` — закрыть.
fn preset_window_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<UiModalOpen>,
    mut state: ResMut<PresetWindowState>,
) {
    let control =
        keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if control && keys.just_pressed(KeyCode::KeyS) {
        ui.0 = !ui.0;
        if ui.0 {
            state.dirty = true;
        }
    } else if keys.just_pressed(KeyCode::Escape) && ui.0 {
        ui.0 = false;
    }
}

/// Собрать окно, когда оно открыто; при закрытии — убрать его.
///
/// Закрытие через «Применить» сохраняет выбор и (если он менялся) запрашивает
/// перегенерацию. Любое другое закрытие (`×`, `Esc`) откатывает не применённые
/// изменения к последнему снимку [`PresetWindowState::applied`].
#[allow(clippy::too_many_arguments)]
fn sync_preset_window(
    mut commands: Commands,
    ui: Res<UiModalOpen>,
    mut state: ResMut<PresetWindowState>,
    mut store: ResMut<PresetStore>,
    mut writer: MessageWriter<RegenerationRequest>,
) {
    if ui.0 {
        if !state.dirty {
            return;
        }
        if let Some(root) = state.root.take() {
            commands.entity(root).despawn();
        }
        let content = window_content(&store, &state.collapsed);
        let frame = spawn_window_frame(
            &mut commands,
            &WindowSpec {
                title: "Демо-пресеты",
                width: Val::Percent(58.0),
                height: Val::Percent(74.0),
            },
            content,
        );
        state.root = Some(frame.root);
        state.dirty = false;
    } else if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
        if state.apply_requested {
            state.apply_requested = false;
            // «Применить»: применяем выбор; перегенерация — только если он реально
            // менялся (иначе окно просто закрылось без изменений сцены).
            let changed = store.selected != state.applied;
            state.applied = store.selected.clone();
            if changed {
                store.save_selection(&state.settings_dir);
                writer.write(RegenerationRequest::same());
            }
        } else {
            // Закрытие без «Применить» — откат не применённых изменений.
            store.selected = state.applied.clone();
        }
    }
}

/// Контент окна: пролистываемый список (свёрнутые группы скрывают детей) +
/// нижние кнопки «выбрать всё» и «Применить».
fn window_content(
    store: &PresetStore,
    collapsed: &BTreeSet<String>,
) -> Vec<Box<dyn SceneList>> {
    let mut rows = Vec::new();
    push_preset_rows(&mut rows, store, &store.root_presets, 0);
    for group in &store.groups {
        push_group_rows(&mut rows, store, group, 0, collapsed);
    }

    let body: Box<dyn SceneList> = Box::new(EntityScene(bsn! {
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            overflow: {Overflow::scroll_y()},
            row_gap: px(4),
        }
        Children [ {rows} ]
    }));
    let footer = footer_scene();
    vec![body, footer]
}

fn push_preset_rows(
    out: &mut Vec<Box<dyn SceneList>>,
    store: &PresetStore,
    presets: &[PresetMeta],
    depth: u32,
) {
    for preset in presets {
        let state = if store.is_selected(&preset.id) {
            RowState::All
        } else {
            RowState::None
        };
        out.push(row_scene(
            preset.id.clone(),
            preset.name.clone(),
            depth,
            false,
            Vec::new(),
            state,
            true,
        ));
    }
}

fn push_group_rows(
    out: &mut Vec<Box<dyn SceneList>>,
    store: &PresetStore,
    group: &PresetGroup,
    depth: u32,
    collapsed: &BTreeSet<String>,
) {
    let state = group_state(store, group);
    let is_collapsed = collapsed.contains(&group.id);
    let member_ids = group
        .all_presets()
        .iter()
        .map(|p| p.id.clone())
        .collect::<Vec<_>>();
    out.push(row_scene(
        group.id.clone(),
        group.name.clone(),
        depth,
        true,
        member_ids,
        state,
        !is_collapsed,
    ));
    if !is_collapsed {
        push_preset_rows(out, store, &group.presets, depth + 1);
        for sub in &group.groups {
            push_group_rows(out, store, sub, depth + 1, collapsed);
        }
    }
}

/// Состояние чекбокса строки: ничего не выбрано / частично / всё.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowState {
    None,
    Partial,
    All,
}

/// Состояние чекбокса группы по всему её поддереву.
fn group_state(store: &PresetStore, group: &PresetGroup) -> RowState {
    let members = group.all_presets();
    if members.is_empty() {
        return RowState::None;
    }
    let selected = members
        .iter()
        .filter(|p| store.is_selected(&p.id))
        .count();
    if selected == 0 {
        RowState::None
    } else if selected == members.len() {
        RowState::All
    } else {
        RowState::Partial
    }
}

/// Одна строка списка: чекбокс + имя пресета/группы.
///
/// Одиночный клик переключает выбор (группу — целиком); двойной клик по группе
/// сворачивает/разворачивает её содержимое. Группа помечается `[-]` янтарным
/// цветом, если выбрана только часть её пресетов.
fn row_scene(
    id: String,
    label: String,
    depth: u32,
    is_group: bool,
    member_ids: Vec<String>,
    state: RowState,
    expanded: bool,
) -> Box<dyn SceneList> {
    let (glyph, glyph_color, text_color) = match state {
        RowState::None => ("[ ]".to_string(), MUTED_COLOR, MUTED_COLOR),
        RowState::Partial => ("[-]".to_string(), PARTIAL_COLOR, TEXT_COLOR),
        RowState::All => ("[x]".to_string(), ACCENT_COLOR, TEXT_COLOR),
    };
    let bg = if is_group { GROUP_COLOR } else { ROW_COLOR };
    // Стрелка сворачивания у групп: «развёрнуто» / «свёрнуто».
    let arrow = if is_group {
        if expanded {
            "▾ "
        } else {
            "▸ "
        }
    } else {
        ""
    };
    let label = format!("{arrow}{label}");
    let indent = Val::Px(6.0 + depth as f32 * 18.0);

    Box::new(EntityScene(bsn! {
        Button
        Hovered::default()
        Node {
            height: px(32),
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Row,
            // Зазор между чекбоксом и текстом.
            column_gap: px(10),
            // Отступ-«лесенка» глубины. Ширина берётся из stretch родителя и
            // уменьшается на отступы/поля, поэтому при любой вложенности строки
            // не выходят за правый край окна (в отличие от width:100% + margin).
            margin: UiRect { left: indent, right: px(8) },
            padding: UiRect::horizontal(px(10)),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({bg})
        on(move |_: On<Activate>, mut store: ResMut<PresetStore>, mut win: ResMut<PresetWindowState>, mut clicks: ResMut<RowClickTracker>, time: Res<Time>| {
            if is_group {
                let now = time.elapsed_secs_f64();
                let double = clicks.0.get(&id).is_some_and(|t| now - *t < DOUBLE_CLICK_SECS);
                clicks.0.insert(id.clone(), now);
                if double {
                    // Двойной клик — свернуть/развернуть содержимое группы.
                    if !win.collapsed.remove(&id) {
                        win.collapsed.insert(id.clone());
                    }
                    win.dirty = true;
                    return;
                }
                let all = !member_ids.is_empty()
                    && member_ids.iter().all(|m| store.is_selected(m));
                if all {
                    for m in &member_ids {
                        store.unselect(m);
                    }
                } else {
                    for m in &member_ids {
                        store.select(m);
                    }
                }
            } else {
                store.toggle(&id);
            }
            win.dirty = true;
        })
        Children [
            Text({glyph})
            TextFont {
                font: FontSourceTemplate::Handle(FONT_PATH),
                font_size: px(13.0),
            }
            TextColor({glyph_color})
            --
            Text({label})
            TextFont {
                font: FontSourceTemplate::Handle(FONT_PATH),
                font_size: px(15.0),
            }
            TextColor({text_color})
        ]
    }))
}

/// Нижние кнопки: «выбрать всё / снять всё» (слева) и «Применить» (справа).
fn footer_scene() -> Box<dyn SceneList> {
    Box::new(EntityScene(bsn! {
        Node {
            width: percent(100),
            height: px(46),
            padding: UiRect::top(px(8)),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: px(10),
        }
        Children [
            Button
            Hovered::default()
            Node {
                height: px(34),
                padding: UiRect::horizontal(px(18)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(px(8)),
            }
            BackgroundColor({PANEL_BORDER})
            on(|_: On<Activate>, mut store: ResMut<PresetStore>, mut win: ResMut<PresetWindowState>| {
                if store.selected_count() == store.presets.len() {
                    store.clear();
                } else {
                    store.select_all();
                }
                win.dirty = true;
            })
            Children [
                Text("Выбрать всё / снять всё")
                TextFont {
                    font: FontSourceTemplate::Handle(FONT_PATH),
                    font_size: px(16.0),
                }
                TextColor({TEXT_COLOR})
            ]
            --
            Button
            Hovered::default()
            Node {
                height: px(34),
                padding: UiRect::horizontal(px(22)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(px(8)),
            }
            BackgroundColor({ACCENT_COLOR})
            on(|_: On<Activate>, mut ui: ResMut<UiModalOpen>, mut win: ResMut<PresetWindowState>| {
                win.apply_requested = true;
                ui.0 = false;
            })
            Children [
                Text("Применить")
                TextFont {
                    font: FontSourceTemplate::Handle(FONT_PATH),
                    font_size: px(16.0),
                }
                TextColor({TEXT_COLOR})
            ]
        ]
    }))
}
