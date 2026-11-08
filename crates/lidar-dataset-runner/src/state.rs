//! Состояние приложения: найденные пути и текущая задача генерации.
//!
//! Каталог пресетов ищется по приоритету: переменная окружения (разработка и
//! тесты), каталог ресурсов бандла, сам каталог ресурсов и, наконец, каталог
//! воркспейса относительно `CARGO_MANIFEST_DIR`. Найденный путь отдаётся в UI,
//! чтобы пустой каталог в бандле было видно сразу, а не по пустому списку.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Найденные пути приложения.
#[derive(Debug, Clone, Default)]
pub struct AppPaths {
    /// Корень каталога пресетов (каталог с папками категорий).
    pub presets_root: Option<PathBuf>,
}

impl AppPaths {
    /// Найти каталог пресетов.
    ///
    /// `resource_dir` — каталог ресурсов бандла; `env_override` — значение
    /// `LIDAR_PRESETS_ROOT`. Считается валидным только существующий путь:
    /// «нашли несуществующий каталог» выглядит в UI как пустой каталог без
    /// причины.
    pub fn resolve(resource_dir: Option<PathBuf>, env_override: Option<PathBuf>) -> Self {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(dir) = env_override {
            candidates.push(dir);
        }
        if let Some(resources) = &resource_dir {
            candidates.push(resources.join("presets"));
            candidates.push(resources.clone());
        }
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets"),
        );

        let presets_root = candidates.into_iter().find(|path| path.is_dir());
        Self { presets_root }
    }

    /// Корень каталога пресетов или каталог воркспейса как последний вариант.
    pub fn presets_or_workspace(&self) -> PathBuf {
        self.presets_root
            .clone()
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets"))
    }
}

/// Текущая задача генерации.
#[derive(Debug, Clone)]
pub struct JobHandle {
    /// Идентификатор запуска.
    pub id: String,
    /// Флаг отмены.
    pub cancel: Arc<AtomicBool>,
}

/// Состояние приложения, доступное командам Tauri.
#[derive(Debug, Default)]
pub struct AppState {
    /// Найденные пути.
    pub paths: AppPaths,
    /// Текущая задача.
    pub job: std::sync::Mutex<Option<JobHandle>>,
}

impl AppState {
    /// Состояние с найденными путями.
    pub fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            job: std::sync::Mutex::new(None),
        }
    }

    /// Зарегистрировать текущую задачу.
    pub fn set_job(&self, job: Option<JobHandle>) {
        if let Ok(mut guard) = self.job.lock() {
            *guard = job;
        }
    }

    /// Идентификатор текущей задачи, если она есть.
    pub fn current_job(&self) -> Option<String> {
        self.job
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|job| job.id.clone()))
    }

    /// Флаг отмены задачи `job_id`; `None` — такой задачи нет.
    pub fn cancel_flag(&self, job_id: &str) -> Option<Arc<AtomicBool>> {
        let guard = self.job.lock().ok()?;
        let job = guard.as_ref()?;
        (job.id == job_id).then(|| job.cancel.clone())
    }

    /// Каталог вывода по умолчанию: `<документы>/lidar-datasets`.
    pub fn default_output_dir(documents: Option<&Path>) -> PathBuf {
        let base = documents
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::temp_dir());
        base.join("lidar-datasets")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_prefer_env_over_bundle() {
        let env_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets");
        let res = PathBuf::from("/tmp/bundle");
        let paths = AppPaths::resolve(Some(res.clone()), Some(env_dir.clone()));
        assert_eq!(paths.presets_root, Some(env_dir));

        let paths = AppPaths::resolve(None, None);
        assert!(
            paths.presets_root.is_some(),
            "фолбэк на workspace-каталог"
        );
        assert!(paths.presets_root.unwrap().is_dir());
    }

    #[test]
    fn nonexistent_paths_are_skipped() {
        let bogus_env = PathBuf::from("/tmp/definitely-not-a-presets-dir-12345");
        let paths = AppPaths::resolve(Some(PathBuf::from("/tmp/nonexistent-bundle")), Some(bogus_env));
        let root = paths.presets_root.expect("должен найтись каталог воркспейса");
        assert!(root.is_dir(), "{}", root.display());
    }

    #[test]
    fn job_cancel_flag_requires_matching_id() {
        let state = AppState::new(AppPaths::resolve(None, None));
        let flag = Arc::new(AtomicBool::new(false));
        state.set_job(Some(JobHandle {
            id: "job-1".into(),
            cancel: flag.clone(),
        }));
        assert_eq!(state.current_job().as_deref(), Some("job-1"));
        assert!(state.cancel_flag("job-1").is_some());
        assert!(state.cancel_flag("job-2").is_none());
        state.set_job(None);
        assert!(state.current_job().is_none());
    }

    #[test]
    fn default_output_dir_sits_under_documents() {
        let dir = AppState::default_output_dir(Some(Path::new("/tmp/docs")));
        assert_eq!(dir, PathBuf::from("/tmp/docs/lidar-datasets"));
    }

    #[test]
    fn resource_dir_presets_win_over_workspace() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/state-resource-dir");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("presets/landscapes")).unwrap();

        let paths = AppPaths::resolve(Some(root.clone()), None);
        assert_eq!(paths.presets_root, Some(root.join("presets")));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn resource_dir_itself_is_a_fallback() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/state-resource-plain");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("landscapes")).unwrap();

        let paths = AppPaths::resolve(Some(root.clone()), None);
        assert_eq!(paths.presets_root, Some(root.clone()));

        std::fs::remove_dir_all(&root).unwrap();
    }
}
