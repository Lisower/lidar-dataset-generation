//! Запуск внешних команд, которые создают ярлыки.
//!
//! На Linux и macOS ярлык — это обычный текстовый файл, поэтому xtask пишет его
//! сам. На Windows `.lnk` — бинарный формат, и единственный штатный способ его
//! создать без COM-обвязки в Rust — `WScript.Shell` в PowerShell. Отсюда трейт:
//! тесты подменяют запуск и проверяют текст скрипта, не имея Windows под рукой.

use std::process::Command;

/// Запуск сценария, который создаёт ярлык.
pub trait Shell {
    /// Выполнить PowerShell-сценарий; ошибка — текст для пользователя.
    fn run_powershell(&self, script: &str) -> Result<(), String>;
}

/// Настоящий PowerShell текущей машины.
pub struct SystemShell;

impl Shell for SystemShell {
    fn run_powershell(&self, script: &str) -> Result<(), String> {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .output()
            .map_err(|error| format!("не удалось запустить powershell: {error}"))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        if detail.is_empty() {
            Err(format!("powershell завершился с кодом {}", output.status))
        } else {
            Err(format!("powershell: {detail}"))
        }
    }
}
