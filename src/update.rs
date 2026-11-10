use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;
use std::{
    error::Error,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
    core::*,
};
type Fallible<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
pub const REPOSITORY: &str = "kathoc/thumbpad";
const ASSET: &str = "thumbpad-setup-x64.exe";
static CHECKING: AtomicBool = AtomicBool::new(false);
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PointerMode {
    #[default]
    Mouse,
    Trackball,
}
#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub auto_update: bool,
    pub edge_side_percent: f64,
    pub edge_top_percent: f64,
    pub edge_grace_ms: u64,
    pub edge_pointer_guard: bool,
    pub drag_pixels_per_pad_width: f64,
    pub drag_pixels_per_pad_height: f64,
    pub scroll_units_per_pad_height: f64,
    pub scroll_invert: bool,
    pub scroll_units_per_pad_width: f64,
    pub scroll_horizontal_invert: bool,
    pub inertia_enabled: bool,
    pub inertia_decay: f64,
    pub pointer_mode: PointerMode,
    pub pointer_inertia_decay: f64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            auto_update: true,
            edge_side_percent: 12.0,
            edge_top_percent: 15.0,
            edge_grace_ms: 500,
            edge_pointer_guard: true,
            drag_pixels_per_pad_width: 1600.0,
            drag_pixels_per_pad_height: 1000.0,
            scroll_units_per_pad_height: 2400.0,
            scroll_invert: true,
            scroll_units_per_pad_width: 2400.0,
            scroll_horizontal_invert: true,
            inertia_enabled: true,
            inertia_decay: 5.0,
            pointer_mode: PointerMode::Mouse,
            pointer_inertia_decay: 5.0,
        }
    }
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Thumbpad")
}
impl Config {
    fn validate(&self) -> Fallible<()> {
        if !self.edge_side_percent.is_finite()
            || !(0.0..=45.0).contains(&self.edge_side_percent)
            || !self.edge_top_percent.is_finite()
            || !(0.0..=80.0).contains(&self.edge_top_percent)
            || self.edge_grace_ms > 5000
            || !self.inertia_decay.is_finite()
            || !(1.0..=20.0).contains(&self.inertia_decay)
            || !self.pointer_inertia_decay.is_finite()
            || !(1.0..=20.0).contains(&self.pointer_inertia_decay)
            || ![
                self.drag_pixels_per_pad_width,
                self.drag_pixels_per_pad_height,
                self.scroll_units_per_pad_height,
                self.scroll_units_per_pad_width,
            ]
            .iter()
            .all(|v| v.is_finite() && (1.0..=20000.0).contains(v))
        {
            return Err("Invalid settings: side 0..45%, top 0..80%, grace 0..5000 ms, motion/scroll scales 1..20000".into());
        }
        Ok(())
    }
    pub fn motion_tuning(&self) -> crate::motion::Tuning {
        crate::motion::Tuning {
            drag_x: self.drag_pixels_per_pad_width,
            drag_y: self.drag_pixels_per_pad_height,
            wheel: self.scroll_units_per_pad_height,
            invert: self.scroll_invert,
            wheel_x: self.scroll_units_per_pad_width,
            horizontal_invert: self.scroll_horizontal_invert,
        }
    }
    pub fn load() -> Self {
        let path = data_dir().join("settings.json");
        if !path.exists() {
            let config = Self::default();
            if let Err(e) = config.save() {
                crate::message(&format!("Could not save settings: {e}"));
            }
            return config;
        }
        match fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|data| serde_json::from_slice::<Self>(&data).map_err(|e| e.to_string()))
            .and_then(|c| c.validate().map(|()| c).map_err(|e| e.to_string()))
        {
            Ok(config) => {
                // Preserve existing values, adding defaults introduced by upgrades.
                let needs_migration = fs::read(&path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .is_some_and(|value| {
                        [
                            "edge_pointer_guard",
                            "drag_pixels_per_pad_width",
                            "drag_pixels_per_pad_height",
                            "scroll_units_per_pad_height",
                            "scroll_invert",
                            "scroll_units_per_pad_width",
                            "scroll_horizontal_invert",
                            "inertia_enabled",
                            "inertia_decay",
                            "pointer_mode",
                            "pointer_inertia_decay",
                        ]
                        .iter()
                        .any(|key| value.get(key).is_none())
                    });
                if needs_migration && let Err(error) = config.save() {
                    crate::message(&format!("Could not migrate settings: {error}"));
                }
                config
            }
            Err(e) => {
                crate::message(&format!(
                    "Settings could not be loaded; using defaults for this session.\n{e}\n{}",
                    path.display()
                ));
                Self::default()
            }
        }
    }
    pub fn save(&self) -> Fallible<()> {
        self.validate()?;
        fs::create_dir_all(data_dir())?;
        let temporary = data_dir().join("settings.json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temporary, data_dir().join("settings.json"))?;
        Ok(())
    }
}
pub fn open_settings() -> Fallible<()> {
    Command::new("notepad.exe")
        .arg(data_dir().join("settings.json"))
        .spawn()?;
    Ok(())
}
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
pub fn startup_enabled() -> bool {
    Command::new("reg.exe")
        .args(["query", RUN_KEY, "/v", "Thumbpad"])
        .creation_flags(0x08000000)
        .output()
        .is_ok_and(|o| o.status.success())
}
pub fn toggle_startup() -> Fallible<()> {
    let mut command = Command::new("reg.exe");
    if startup_enabled() {
        command.args(["delete", RUN_KEY, "/v", "Thumbpad", "/f"]);
    } else {
        let value = format!("\"{}\" --background", std::env::current_exe()?.display());
        command.args([
            "add", RUN_KEY, "/v", "Thumbpad", "/t", "REG_SZ", "/d", &value, "/f",
        ]);
    }
    let output = command.creation_flags(0x08000000).output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(())
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}
fn candidate<'a>(release: &'a Release, current: &str) -> Fallible<Option<&'a Asset>> {
    let version = semver::Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )?;
    if release.draft
        || release.prerelease
        || !version.pre.is_empty()
        || version <= semver::Version::parse(current)?
    {
        return Ok(None);
    }
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == ASSET)
        .ok_or("Release has no x64 installer")?;
    let prefix = format!(
        "https://github.com/{REPOSITORY}/releases/download/{}/",
        release.tag_name
    );
    if !asset.browser_download_url.starts_with(&prefix)
        || asset.size == 0
        || asset.size > 100 * 1024 * 1024
    {
        return Err("Invalid release asset URL or size".into());
    }
    let hash = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .ok_or("GitHub SHA-256 digest missing; update refused")?;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid asset digest".into());
    }
    Ok(Some(asset))
}
fn digest(bytes: &[u8]) -> String {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("sha256:{hex}")
}
fn verify(bytes: &[u8], asset: &Asset) -> Fallible<()> {
    let digest = digest(bytes);
    if bytes.len() as u64 != asset.size
        || !asset
            .digest
            .as_ref()
            .is_some_and(|d| d.eq_ignore_ascii_case(&digest))
    {
        return Err("Installer size or SHA-256 mismatch; update refused".into());
    }
    Ok(())
}
fn check(manual: bool) -> Fallible<()> {
    let config = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(concat!("Thumbpad/", env!("CARGO_PKG_VERSION")))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let result = agent
        .get(format!(
            "https://api.github.com/repos/{REPOSITORY}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .call();
    let mut response = match result {
        Err(ureq::Error::StatusCode(404)) => {
            if manual {
                crate::message("No published stable release is available yet.");
            }
            return Ok(());
        }
        other => other?,
    };
    let release: Release = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_json()?;
    let Some(asset) = candidate(&release, env!("CARGO_PKG_VERSION"))? else {
        if manual {
            crate::message("Thumbpad is up to date.");
        }
        return Ok(());
    };
    let prompt = crate::wide(&format!(
        "Thumbpad {} is available.\nDownload and install it now?\nThumbpad will close and restart during installation.",
        release.tag_name
    ));
    unsafe {
        if MessageBoxW(
            None,
            PCWSTR(prompt.as_ptr()),
            w!("Thumbpad update"),
            MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2,
        ) != IDYES
        {
            return Ok(());
        }
    }
    let bytes = agent
        .get(&asset.browser_download_url)
        .call()?
        .body_mut()
        .with_config()
        .limit(100 * 1024 * 1024)
        .read_to_vec()?;
    verify(&bytes, asset)?;
    let updates = data_dir().join("updates");
    fs::create_dir_all(&updates)?;
    let folder = updates.join(format!(
        "{}-{}",
        release.tag_name,
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&folder)?;
    let installer = folder.join(ASSET);
    {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&installer)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    Command::new(&installer)
        .args([
            "/SILENT",
            "/CLOSEAPPLICATIONS",
            "/RESTARTAPPLICATIONS",
            "/UPGRADE",
        ])
        .spawn()?;
    unsafe {
        if let Ok(hwnd) = FindWindowW(w!("ThumbpadWindow"), None) {
            let _ = PostMessageW(Some(hwnd), WM_COMMAND, WPARAM(crate::EXIT), LPARAM(0));
        }
    }
    Ok(())
}
pub fn check_async(manual: bool) {
    if CHECKING.swap(true, Ordering::AcqRel) {
        if manual {
            crate::message("An update check is already running.");
        }
        return;
    }
    std::thread::spawn(move || {
        if let Err(e) = check(manual) {
            let text = format!("Update failed: {e}");
            let _ = fs::write(data_dir().join("update-error.txt"), &text);
            if manual {
                crate::message(&text);
            }
        }
        CHECKING.store(false, Ordering::Release);
    });
}
pub fn start_scheduler() {
    std::thread::spawn(|| {
        // No network work takes place on the input thread.
        std::thread::sleep(Duration::from_secs(15));
        loop {
            // Read silently here: invalid user settings must not trigger background dialogs.
            let enabled = fs::read(data_dir().join("settings.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
                .is_some_and(|c| c.auto_update);
            if enabled {
                check_async(false);
            }
            std::thread::sleep(Duration::from_secs(24 * 60 * 60));
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_mode_defaults_to_mouse_and_trackball_is_explicit() {
        assert_eq!(Config::default().pointer_mode, PointerMode::Mouse);
        let config: Config =
            serde_json::from_str(r#"{"pointer_mode":"trackball","pointer_inertia_decay":7.0}"#)
                .unwrap();
        assert_eq!(config.pointer_mode, PointerMode::Trackball);
        assert!(config.validate().is_ok());
        assert!(serde_json::from_str::<Config>(r#"{"pointer_mode":"unknown"}"#).is_err());
    }
    #[test]
    fn reversed_scroll_defaults_and_saved_direction_preferences() {
        let defaults: Config = serde_json::from_str("{}").unwrap();
        assert!(defaults.scroll_invert && defaults.scroll_horizontal_invert);
        let custom: Config =
            serde_json::from_str(r#"{"scroll_invert":false,"scroll_horizontal_invert":false}"#)
                .unwrap();
        assert!(!custom.motion_tuning().invert && !custom.motion_tuning().horizontal_invert);
    }
    fn asset(bytes: &[u8]) -> Asset {
        Asset {
            name: ASSET.into(),
            browser_download_url: format!(
                "https://github.com/{REPOSITORY}/releases/download/v0.2.0/{ASSET}"
            ),
            digest: Some(digest(bytes)),
            size: bytes.len() as u64,
        }
    }
    #[test]
    fn tampering_and_truncation_rejected() {
        let a = asset(b"installer");
        assert!(verify(b"installer", &a).is_ok());
        assert!(verify(b"Installer", &a).is_err());
        assert!(verify(b"inst", &a).is_err());
    }
    #[test]
    fn refuses_downgrade_prerelease_foreign_url_and_missing_digest() {
        let mut r = Release {
            tag_name: "v0.2.0".into(),
            draft: false,
            prerelease: false,
            assets: vec![asset(b"data")],
        };
        assert!(candidate(&r, "0.1.0").unwrap().is_some());
        assert!(candidate(&r, "0.3.0").unwrap().is_none());
        r.prerelease = true;
        assert!(candidate(&r, "0.1.0").unwrap().is_none());
        r.prerelease = false;
        r.assets[0].browser_download_url = "https://evil.example/installer.exe".into();
        assert!(candidate(&r, "0.1.0").is_err());
        r.assets[0] = asset(b"data");
        r.assets[0].digest = None;
        assert!(candidate(&r, "0.1.0").is_err());
    }
    #[test]
    fn edge_config_validation() {
        assert!(Config::default().validate().is_ok());
        assert!(
            Config {
                edge_side_percent: 50.0,
                ..Config::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                edge_top_percent: f64::NAN,
                ..Config::default()
            }
            .validate()
            .is_err()
        );
    }
}
