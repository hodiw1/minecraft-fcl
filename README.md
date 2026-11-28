<div align="center">

# 🏛️ TERRAPOLIS

**Легкий і швидкий самостійний Майнкрафт-лаунчер: запускай гру, збирай власні збірки та підключайся до проєктів спільноти**

<p>
  <img src="https://img.shields.io/badge/Version-0.4.1-f59e0b?style=for-the-badge&logo=github&logoColor=white" alt="Version" />
  <img src="https://img.shields.io/badge/Core-Rust-dea584?style=for-the-badge&logo=rust&logoColor=black" alt="Rust" />
  <img src="https://img.shields.io/badge/Engine-Tauri_v2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/UI-WebView2-0078D7?style=for-the-badge&logo=microsoftedge&logoColor=white" alt="WebView2" />
  <img src="https://img.shields.io/badge/Platform-Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows" />
</p>

> ⚠️ **Увага:** Це тестова версія лаунчера (`v0.4.1`). Функціонал активно тестується та доповнюється перед релізом `1.0.0`.

</div>

---

## ✨ Про проєкт

**Terrapolis** — це насамперед легкий і швидкий самостійний лаунчер Minecraft на сучасному стеку **Rust + WebView2 (Tauri v2)**. У ньому зручно запускати гру, створювати власні збірки з будь-якою версією Minecraft і завантажувачем, а головна фіча — **проєкти**: власні проєкти зі збірками для своєї спільноти та підключення до проєктів інших спільнот за кодом або `owner/repo`.

Проєкт не обов'язковий: лаунчер повністю працює й без нього — створюй власні збірки або встановлюй готові з **Modrinth** і **CurseForge**.

На відміну від важких лаунчерів на Electron або Python, Terrapolis використовує нативний компонент Windows **WebView2** та скомпільований бекенд на **Rust**, тож займає лічені мегабайти й запускається миттєво.

---

## 🚀 Ключові особливості

- ⚡ **Нативна швидкодія (Rust + WebView2):** мінімальне споживання пам'яті у фоні, миттєве відкриття папок і нативні діалоги вибору файлів без фрізів інтерфейсу.
- 🧩 **Власні збірки:** будь-яка версія Minecraft і завантажувач (*Vanilla, Fabric, Forge, NeoForge, Quilt*) у компактному майстрі створення.
- 🛒 **Каталог контенту в стилі Prism Launcher:** моди, ресурс-паки, шейдери (з підказкою та встановленням Iris / Oculus в один клік) і **готові збірки** (`.mrpack` з Modrinth, `.zip` з CurseForge) — з фільтром за версією гри й завантажувачем, вибором версії файлу та керуванням встановленим.
- 🌐 **Проєкти спільноти:** підключення за кодом або `owner/repo`, список публічних проєктів, синхронізація збірок і pre-launch дебагер, що звіряє файли за **SHA-256**.
- 🛡️ **Дзеркала та офлайн-режим:** автоматичне перемикання між резервними джерелами, запуск встановлених збірок без інтернету та імпорт архівів `.zip`.
- 🎨 **Персоналізація:** двокроковий перший запуск (нік + кольорова палітра з живим прев'ю), шість акцентних палітр, двомовний інтерфейс (UKR / ENG).
- 🛠️ **Автоматичне розгортання середовища:** самостійне завантаження **Java (Adoptium Temurin JRE)** і модлоадерів.
- 🧹 **Danger Zone:** очищення даних зі збереженням профілів і проєктів або повне видалення лаунчера разом із кешем WebView2.
- 🚀 **Кероване оновлення клієнта:** перевірка нових версій через **GitHub Releases** із Changelog та опціональним автооновленням.

---

## 🛠️ Технологічний стек

| Компонент | Технологія | Призначення |
| :--- | :--- | :--- |
| **Backend & Core** | ![Rust](https://img.shields.io/badge/-Rust-000000?style=flat-square&logo=rust&logoColor=dea584) | Файлова система, хешування SHA-1/SHA-256/SHA-512, мережеві запити, запуск JVM |
| **Desktop Framework** | ![Tauri](https://img.shields.io/badge/-Tauri_v2-000000?style=flat-square&logo=tauri&logoColor=24C8DB) | Зв'язок між Rust-ядром та інтерфейсом, системні події |
| **Frontend Runtime** | ![WebView2](https://img.shields.io/badge/-WebView2-000000?style=flat-square&logo=microsoftedge&logoColor=0078D7) | Легкий рендеринг сучасного UI засобами Windows |
| **CI/CD & Hosting** | ![GitHub Actions](https://img.shields.io/badge/-GitHub_Actions-000000?style=flat-square&logo=githubactions&logoColor=2088FF) | Автоматична хмарна збірка `.exe` та дистрибуція оновлень |

> CurseForge працює з безкоштовним API-ключем (Налаштування → «Джерела модів» або змінна середовища `CURSEFORGE_API_KEY`). Modrinth працює без ключа.

---

## 🗺️ Roadmap розробки

- ✅ **`v0.1.x`** — Ескіз лаунчера, базовий каркас проєкту.
- ✅ **`v0.2.x`** — Дороблення інтерфейсу та основного функціоналу.
- ✅ **`v0.3.x`** — Повноцінний запуск і створення збірок, менеджер модів, дзеркала та дебагер цілісності.
- 🔄 **`v0.4.x`** *(поточний етап)* — Самостійний лаунчер без обов'язкового проєкту, каталог контенту (ресурс-паки, шейдери, готові збірки), палітри, Danger Zone, швидкі нативні діалоги.
- 🔮 **`v0.5.x – v1.x.x`** — Плани на стадії формування *(ще не усталено)*.

---

<div align="center">
  <sub>Розроблено командою <b>HDT</b> • 2026</sub><br/>
  <sub>NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.</sub>
</div>
