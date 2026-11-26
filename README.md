<div align="center">

# 🏛️ TERRAPOLIS

**Кастомний ігровий лаунчер та платформа керування збірками для проєктів команди HDT**

<p>
  <img src="https://img.shields.io/badge/Version-0.3.0-f59e0b?style=for-the-badge&logo=github&logoColor=white" alt="Version" />
  <img src="https://img.shields.io/badge/Core-Rust-dea584?style=for-the-badge&logo=rust&logoColor=black" alt="Rust" />
  <img src="https://img.shields.io/badge/Engine-Tauri_v2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/UI-WebView2-0078D7?style=for-the-badge&logo=microsoftedge&logoColor=white" alt="WebView2" />
  <img src="https://img.shields.io/badge/Platform-Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows" />
</p>

> ⚠️ **Увага:** Це тестова версія лаунчера (`v0.3.0`). Функціонал активно тестується та доповнюється перед релізом `1.0.0`.

</div>

---

## ✨ Про проєкт

**Terrapolis** — це легкий, швидкий та мінімалістичний десктопний лаунчер, розроблений на сучасному системному стеку **Rust + WebView2 (Tauri v2)**. Він створений як єдиний портал для входу в ігрові всесвіти та проєкти.

На відміну від важких лаунчерів на Electron або Python, Terrapolis використовує нативний компонент Windows **WebView2** та скомпільований бекенд на **Rust**, завдяки чому займає лічені мегабайти та запускається миттєво.

---

## 🚀 Ключові особливості

- ⚡ **Нативна швидкодія (Rust + WebView2):** Мінімальне споживання оперативної пам'яті у фоні та швидкий запуск без вбудованого Chromium.
- 🔄 **Розумна синхронізація та дебагер збірок:** Багатопотокова перевірка цілісності файлів за хешами **SHA-256** та pre-launch дебагер, який автоматично докачує відсутні й прибирає зайві моди перед запуском проєкту.
- 🛡️ **Дзеркала та повний офлайн-режим:** Автоматичне перемикання між резервними дзеркалами джерел, запуск встановлених збірок без інтернету та імпорт архівів `.zip`.
- 📦 **Вбудований менеджер модів:** Пошук та встановлення модів із **Modrinth** і **CurseForge** з автоматичним підтягуванням залежностей та керуванням локальними модами.
- 🛠️ **Автоматичне розгортання середовища:** Самостійне завантаження та налаштування необхідної версії **Java (Adoptium Temurin JRE)** і модлоадерів (*Fabric, Forge, NeoForge, Quilt, Vanilla*).
- 🚀 **Кероване оновлення клієнта:** Перевірка нових версій через **GitHub Releases** із переглядом Changelog у модальному вікні та опціональним автооновленням.
- 🎮 **Гнучке налаштування:** Офлайн-профілі зі скінами та зручне керування виділеною оперативною пам'яттю (RAM) перед запуском.

---

## 🛠️ Технологічний стек

| Компонент | Технологія | Призначення |
| :--- | :--- | :--- |
| **Backend & Core** | ![Rust](https://img.shields.io/badge/-Rust-000000?style=flat-square&logo=rust&logoColor=dea584) | Робота з файловою системою, хешування SHA-256, мережеві запити, запуск JVM |
| **Desktop Framework** | ![Tauri](https://img.shields.io/badge/-Tauri_v2-000000?style=flat-square&logo=tauri&logoColor=24C8DB) | Зв'язок між Rust-ядром та інтерфейсом, системні події |
| **Frontend Runtime** | ![WebView2](https://img.shields.io/badge/-WebView2-000000?style=flat-square&logo=microsoftedge&logoColor=0078D7) | Легкий рендеринг сучасного UI засобами Windows |
| **CI/CD & Hosting** | ![GitHub Actions](https://img.shields.io/badge/-GitHub_Actions-000000?style=flat-square&logo=githubactions&logoColor=2088FF) | Автоматична хмарна збірка `.exe` та дистрибуція оновлень |

---

## 🗺️ Roadmap розробки

- ✅ **`v0.1.x`** — Ескіз лаунчера, базовий каркас проєкту.
- ✅ **`v0.2.x`** — Дороблення інтерфейсу та основного функціоналу.
- 🔄 **`v0.3.x`** *(поточний етап)* — Повноцінний запуск і створення збірок, менеджер модів, дзеркала та дебагер цілісності.
- 📋 **`v0.4.x`** — Виправлення багів та подальше покращення UX.
- 🔮 **`v0.5.x – v1.x.x`** — Плани на стадії формування *(ще не усталено)*.

---

<div align="center">
  <sub>Розроблено командою <b>HDT</b> • 2026</sub><br/>
  <sub>NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.</sub>
</div>
