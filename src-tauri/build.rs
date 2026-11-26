//! Збірка ресурсів Windows: PE-метадані (VERSIONINFO) і маніфест застосунку.
//!
//! `tauri-build` записує у VERSIONINFO поля з `tauri.conf.json`:
//! - `ProductName` / `FileDescription` ← `productName`;
//! - `CompanyName` ← `bundle.publisher`;
//! - `LegalCopyright` ← `bundle.copyright`;
//! - `FileVersion` / `ProductVersion` ← `version`.
//!
//! Повні метадані та явний маніфест з `asInvoker` — одна з ознак «звичайного» підписаного
//! застосунку для евристик Windows Defender / SmartScreen.

/// Стандартний маніфест: Common Controls v6 (як у Tauri за замовчуванням), запуск із правами
/// поточного користувача без запиту UAC і перелік підтримуваних версій Windows.
const WINDOWS_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 / 11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
      <!-- Windows 8.1 -->
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}" />
      <!-- Windows 8 -->
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}" />
      <!-- Windows 7 -->
      <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}" />
    </application>
  </compatibility>
</assembly>
"#;

/// FileVersion береться з `tauri.conf.json`, тож він мусить збігатися з версією в Cargo.toml
/// (яку перевіряє реліз-воркфлоу та показує лаунчер).
fn check_conf_version() {
    println!("cargo:rerun-if-changed=tauri.conf.json");
    let version = env!("CARGO_PKG_VERSION");
    let conf = std::fs::read_to_string("tauri.conf.json").expect("не вдалося прочитати tauri.conf.json");
    let compact: String = conf.chars().filter(|c| !c.is_whitespace()).collect();
    if !compact.contains(&format!("\"version\":\"{version}\"")) {
        panic!(
            "tauri.conf.json: поле \"version\" має дорівнювати версії з Cargo.toml ({version}) — \
             інакше у PE-метаданих .exe буде неправильний FileVersion"
        );
    }
}

fn main() {
    check_conf_version();
    let windows = tauri_build::WindowsAttributes::new().app_manifest(WINDOWS_MANIFEST);
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("помилка tauri-build");
}
