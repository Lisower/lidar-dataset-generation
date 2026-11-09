# Отчёт: развитие terrains-generator

Ветка разработки: `codex/terrain-preset-groups`. Отчёт фиксирует реализацию и проверки до публикации ветки.

## Реализовано

- Одна обновляемая информационная панель в общем стиле генераторов: название, seed, размер, ландшафт, вода, количества объектов, время генерации, режим и управление. Исправлен поиск шрифта при запуске exe.
- Несколько источников каждой категории, веса, вложенные группы, стабильная сортировка, устранение повторных ссылок и явные ошибки отсутствующих/пустых/неоднозначных источников. Сохранён старый RON с одиночными id.
- Самостоятельные конфигурации с произвольным именем; неизвестное имя не превращается в Field. Альтернативные ландшафты выбираются по одному, с возможностью сохранять вручную настроенный файловый LandscapeConfig.
- Общие правила растительности: кластеры, просветы, пределы уклона, расстояние до воды, отдельная область ствола для травы, пространственный индекс.
- Общие стратегии улиц и кварталов: резервирование коридоров до расстановки, учёт размера здания, поворот фасада к улице. Дорожные меши не создаются.
- 29 RON-пресетов: 17 новых и 12 обновлённых, старые id и пути сохранены. Полный перечень и формат настроек — в [[Генерация территорий]].
- Пул вариантов травы в существующем plants-generator, лёгкие экземпляры с точной высотой, поворотом и масштабом. Общие handles мешей/материалов используют автоматический instancing Bevy. Представление данных и classified point cloud доступны без GPU.
- Полный headless-экспорт территории в XYZ через существующий экспортёр generation-core.
- Численная защита существующего можжевельника: его внутренние побеги могли получать отрицательное подкоренное выражение, NaN и неповторяемые облака. Добавлена одна операция max(0), без новой модели или алгоритма геометрии.
- Исправлена обработка быстрого Shift+R в общих контролах: отпускание Shift в том же кадре больше не превращает повтор в новый seed.

Геометрия зданий, растений и рельефа по-прежнему создаётся исключительно существующими generators. Изменения других крейтов ограничены каталогом групп, общими контролами, переиспользованием травы, материалом берега и численной защитой можжевельника.

## Проверено

- `cargo check --workspace --offline`: успешно.
- `cargo test -p terrains-generator --offline`: 3 unit + 19 интеграционных тестов, успешно.
- `cargo test --workspace --no-fail-fast -j 1 --offline`: 133 успешно, 2 ошибки, 2 пропущено. Пройдены все тесты plants-generator, buildings-generator, landscapes-generator, lidar-simulator, terrains-generator и doc-tests (кроме ранее помеченного ignored).
- Два оставшихся сбоя: `generation-core::point_clouds::classes::tests::different_classes_have_different_sizes` и `low_noise_is_two_to_three_times_larger`. Они относятся к уже существующим размерам точек. `classes.rs` полностью совпадает с HEAD: SHA-1 Git blob `ea6069ef9aa9c7e059b4102f8f60f16c4b3c1a72`. Его значения в этой работе не менялись.
- Обычный параллельный `cargo test --workspace` на Windows сначала столкнулся с конфликтующими именами examples `dump_presets` у четырёх крейтов. Последовательный `-j 1` позволяет выполнить весь набор, оставляя только указанные две ошибки тестов.
- `cargo fmt --all -- --check` выполнен: не проходит из-за ранее неотформатированных модулей, включая bevy-utils/cameras, preset_selector, buildings-generator и исходную часть plants-generator/skeleton. Массовое форматирование посторонних модулей не выполнялось. Новые и основные изменённые файлы отформатированы rustfmt.
- `git diff --check`: успешно.
- CLI `--list`: отображает 29 пресетов.
- Headless `river 42 --output local/river-smoke.xyz`: успешно; 12 деревьев, 9 кустов, 3469 экземпляров травы. Прочитаны строки XYZ с кодом класса и координатами.
- В окне Bevy реально проверены пляж, вода, пальмы, информационная панель и переключение V в облако точек; открывался также город с небоскрёбами. Полная визуальная проверка всех 29 пресетов и всех seed не проводилась. После просмотра города увеличен обзор камеры для высоких зданий; окончательная настройка этого ракурса отдельным скриншотом не подтверждена.
- Регрессионные тесты проверяют загрузку всего каталога, старый RON, пользовательские имена, группы, веса, пропущенные источники, все типы территорий, границы/пересечения/воду, ориентацию зданий, ограничения деревьев в поле, три хвойных вида, обе пальмы, можжевельник, правила берега, повторяемость и преобразования облаков травы.

## Измерение травы

Воспроизводимый сценарий: `cargo run -p terrains-generator --example profile_grass -- 10000`.

Один локальный запуск dev-сборки:

| Измерение | Результат |
| --- | ---: |
| Прежний подход: 10000 отдельных вызовов generate_tuft | 1120,07 мс |
| Пул 4 вариантов + 10000 размещений | 0,65 мс |
| Подготовка ECS рендерера, включая общее облако | 49,66 мс |
| Меши | 5 |
| Материалы | 4 |
| Сущности | 10003 |
| Плотный лес: 5009 размещённых пучков / 8 вариантов | трава 325,78 мс |
| Та же полная лесная сцена, включая существующую генерацию деревьев | 11512,58 мс |

Это изолированное сравнение стоимости геометрии и подготовки ECS, не замер FPS или общего ускорения приложения. Число сущностей линейно; число мешей и материалов зависит от пула вариантов. Итоговая густота ограничена размерами пучков и пригодной площадью.

## Ограничения

- В исходном каталоге один файловый пресет многоквартирного дома. Разнообразие этого типа обеспечивается существующими параметрами генератора и seed; отдельные отсутствующие модели не подменяются выдуманными.
- Дистанция до воды — сеточное приближение chamfer по 8 соседям. Проверка сухого основания независимо анализирует водные узлы с запасом.
- Консервативные габариты и отклонение неподходящих мест могут уменьшать количество объектов, особенно больших зданий на карте 100×100 м.
- `TerrainScene.grass` теперь GrassField с `.variants` и `.instances`. Публичные функции generate/generate_from_data сохранены; для обработки ошибок добавлена try_generate_from_data. Старые PlacedTuft/PlantScene других генераторов сохранены.
- Размеры точек и прежнее форматирование других модулей оставлены для отдельного исправления. Полный workspace пока не зелёный из-за двух описанных тестов.

## Запуск и дальнейшая проверка

```sh
cargo run -p terrains-generator -- conifer-forest 42
cargo run -p terrains-generator -- beach 42
cargo run -p terrains-generator -- residential-city 42
cargo run -p terrains-generator -- skyscraper-city 42
cargo run -p terrains-generator -- river 42 --headless --output assets/export/river.xyz
cargo test -p terrains-generator
cargo check --workspace
cargo test --workspace --no-fail-fast -j 1
```

## Изменённые файлы

- `crates/generation-core/src/general/controls.rs`
- `crates/generation-core/src/presets/mod.rs`
- `crates/landscapes-generator/src/visual/mod.rs`
- `crates/plants-generator/src/generation/instances.rs`
- `crates/plants-generator/src/generation/mod.rs`
- `crates/plants-generator/src/generation/skeleton.rs`
- `crates/plants-generator/src/visual/mod.rs`
- `crates/terrains-generator/examples/dump_presets.rs`
- `crates/terrains-generator/examples/profile_grass.rs`
- `crates/terrains-generator/src/lib.rs`
- `crates/terrains-generator/src/main.rs`
- `crates/terrains-generator/src/placement.rs`
- `crates/terrains-generator/src/preset.rs`
- `crates/terrains-generator/src/scene.rs`
- `crates/terrains-generator/src/sources.rs`
- `crates/terrains-generator/src/viewer.rs`
- `crates/terrains-generator/tests/territories.rs`
- `presets/terrains/anthropogenic/apartment-city/config.ron`
- `presets/terrains/anthropogenic/apartment-city/manifest.ron`
- `presets/terrains/anthropogenic/residential-city/config.ron`
- `presets/terrains/anthropogenic/residential-city/manifest.ron`
- `presets/terrains/anthropogenic/skyscraper-city/config.ron`
- `presets/terrains/anthropogenic/skyscraper-city/manifest.ron`
- `presets/terrains/anthropogenic/town/config.ron`
- `presets/terrains/anthropogenic/town/manifest.ron`
- `presets/terrains/anthropogenic/village/config.ron`
- `presets/terrains/anthropogenic/village/manifest.ron`
- `presets/terrains/nature/dry/conifer-forest/config.ron`
- `presets/terrains/nature/dry/conifer-forest/manifest.ron`
- `presets/terrains/nature/dry/conifer-slope/config.ron`
- `presets/terrains/nature/dry/conifer-slope/manifest.ron`
- `presets/terrains/nature/dry/conifer-terraces/config.ron`
- `presets/terrains/nature/dry/conifer-terraces/manifest.ron`
- `presets/terrains/nature/dry/field/config.ron`
- `presets/terrains/nature/dry/field/manifest.ron`
- `presets/terrains/nature/dry/forest-cabin/config.ron`
- `presets/terrains/nature/dry/forest-cabin/manifest.ron`
- `presets/terrains/nature/dry/forest-slope/config.ron`
- `presets/terrains/nature/dry/forest-slope/manifest.ron`
- `presets/terrains/nature/dry/forest-terraces/config.ron`
- `presets/terrains/nature/dry/forest-terraces/manifest.ron`
- `presets/terrains/nature/dry/forest/config.ron`
- `presets/terrains/nature/dry/forest/manifest.ron`
- `presets/terrains/nature/dry/hill-slope/config.ron`
- `presets/terrains/nature/dry/hill-slope/manifest.ron`
- `presets/terrains/nature/dry/open-field/config.ron`
- `presets/terrains/nature/dry/open-field/manifest.ron`
- `presets/terrains/nature/dry/terraced-slope/config.ron`
- `presets/terrains/nature/dry/terraced-slope/manifest.ron`
- `presets/terrains/nature/dry/woodland-slope/config.ron`
- `presets/terrains/nature/dry/woodland-slope/manifest.ron`
- `presets/terrains/nature/dry/woodland-terraces/config.ron`
- `presets/terrains/nature/dry/woodland-terraces/manifest.ron`
- `presets/terrains/nature/dry/woodland/config.ron`
- `presets/terrains/nature/dry/woodland/manifest.ron`
- `presets/terrains/nature/water/beach/config.ron`
- `presets/terrains/nature/water/beach/manifest.ron`
- `presets/terrains/nature/water/coast-open/config.ron`
- `presets/terrains/nature/water/coast-open/manifest.ron`
- `presets/terrains/nature/water/coast/config.ron`
- `presets/terrains/nature/water/coast/manifest.ron`
- `presets/terrains/nature/water/pond-open/config.ron`
- `presets/terrains/nature/water/pond-open/manifest.ron`
- `presets/terrains/nature/water/pond/config.ron`
- `presets/terrains/nature/water/pond/manifest.ron`
- `presets/terrains/nature/water/river-open/config.ron`
- `presets/terrains/nature/water/river-open/manifest.ron`
- `presets/terrains/nature/water/river/config.ron`
- `presets/terrains/nature/water/river/manifest.ron`
- `presets/terrains/nature/water/sloping-river/config.ron`
- `presets/terrains/nature/water/sloping-river/manifest.ron`
- `presets/terrains/nature/water/waterfall-open/config.ron`
- `presets/terrains/nature/water/waterfall-open/manifest.ron`
- `presets/terrains/nature/water/waterfall/config.ron`
- `presets/terrains/nature/water/waterfall/manifest.ron`
- `README.md`
- `wiki/Генерация территорий.md`
- `wiki/Отчёт terrains-generator.md`
