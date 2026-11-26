//! Ошибки раннера: валидация рецепта, отсутствующие пресеты, ввод-вывод.
//!
//! Все ошибки возвращаются значениями, ни одна не паникует: приложение живёт
//! в десктопном окне, и падение на отсутствующем каталоге пресетов или
//! недоступном каталоге вывода — это отказ, а не поведение.

use std::path::PathBuf;

use thiserror::Error;

use crate::dataset::recipe::ValidationIssue;

/// Ошибка раннера.
#[derive(Debug, Error)]
pub enum RunnerError {
    /// Рецепт не прошёл валидацию.
    #[error("рецепт не прошёл проверку: {} замечаний", .0.len())]
    Validation(Vec<ValidationIssue>),

    /// В каталоге нет пресета с таким id.
    #[error("в каталоге пресетов нет {category}/«{id}»")]
    MissingPreset {
        /// Категория каталога (`landscapes`, `buildings`, `plants`).
        category: String,
        /// Запрошенный id.
        id: String,
    },

    /// Пресет есть, но его конфигурация не читается (или не подходит генератору).
    #[error("пресет «{id}» не поддерживается: не читается config.ron")]
    UnsupportedPreset {
        /// id пресета.
        id: String,
    },

    /// Каталог пресетов пуст или не найден.
    #[error("каталог пресетов пуст или не найден")]
    EmptyCatalog,

    /// Ошибка ввода-вывода с путём.
    #[error("ошибка ввода-вывода по пути {path}: {source}")]
    Io {
        /// Путь, на котором произошла ошибка.
        path: PathBuf,
        /// Исходная ошибка.
        #[source]
        source: std::io::Error,
    },
}

impl RunnerError {
    /// Ошибка ввода-вывода с путём.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_keeps_the_path() {
        let err = RunnerError::io(
            "/tmp/x.xyz",
            std::io::Error::new(std::io::ErrorKind::NotFound, "нет файла"),
        );
        assert!(err.to_string().contains("/tmp/x.xyz"), "{err}");
    }

    #[test]
    fn validation_error_counts_issues() {
        let err = RunnerError::Validation(vec![ValidationIssue {
            field: "scenes".into(),
            message: "должно быть не меньше одной сцены".into(),
        }]);
        assert!(err.to_string().contains('1'), "{err}");
    }
}
