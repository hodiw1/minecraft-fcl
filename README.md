<div align="center">

# 🏛️ TERRAPOLIS

**Кастомний ігровий лаунчер та платформа керування збірками для проєктів команди HDT**

<p>
  <img src="https://img.shields.io/badge/Status-Beta_0.2.3-f59e0b?style=for-the-badge&logo=github&logoColor=white" alt="Status Beta" />
  <img src="https://img.shields.io/badge/Core-Rust-dea584?style=for-the-badge&logo=rust&logoColor=black" alt="Rust" />
  <img src="https://img.shields.io/badge/Engine-Tauri_v2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/UI-WebView2-0078D7?style=for-the-badge&logo=microsoftedge&logoColor=white" alt="WebView2" />
  <img src="https://img.shields.io/badge/Platform-Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows" />
</p>

> ⚠️ **Увага:** Це тестова бета-версія лаунчера (`v0.2.3-beta`). Функціонал активно тестується та доповнюється перед релізом `1.0.0`.

</div>

---

## ✨ Про проєкт

**Terrapolis** — це легкий, швидкий та мінімалістичний десктопний лаунчер, розроблений на сучасному системному стеку **Rust + WebView2 (Tauri v2)**. Він створений як єдиний портал для входу в ігрові всесвіти та проекти.

На відміну від важких лаунчерів на Electron або Python, Terrapolis використовує нативний компонент Windows **WebView2** та скомпільований бекенд на **Rust**, завдяки чому займає лічені мегабайти та запускається миттєво.

---

## 🚀 Ключові особливості

* ⚡ **Нативна швидкодія (Rust + WebView2):** Мінімальне споживання оперативної пам'яті у фоні та швидкий запуск без вбудованого Chromium.
* 🔄 **Розумна синхронізація збірок:** Багатопотокова перевірка цілісності файлів за хешами **SHA-256** — лаунчер завантажує лише нові або змінені моди, конфіги, ресурспаки та шейдери.
* 🛠️ **Автоматичне розгортання середовища:** Самостійне завантаження та налаштування необхідної версії **Java (Adoptium Temurin JRE)** і модлоадерів (*Fabric, Forge, NeoForge, Vanilla*).
* 🚀 **Автооновлення клієнта:** Перевірка актуальної версії лаунчера через **GitHub Releases** при кожному старті з безшовною заміною виконуваного файлу.
* 🎮 **Гнучке налаштування:** Вхід за нікнеймом без прив'язки до ліцензії та зручне керування виділеною оперативною пам'яттю (RAM) перед запуском.

---

## 🛠️ Технологічний стек

| Компонент | Технологія | Призначення |
| :--- | :--- | :--- |
| **Backend & Core** | ![Rust](https://img.shields.io/badge/-Rust-000000?style=flat-square&logo=rust&logoColor=dea584) | Робота з файловою системою, хешування SHA-256, мережеві запити, запуск JVM |
| **Desktop Framework** | ![Tauri](https://img.shields.io/badge/-Tauri_v2-000000?style=flat-square&logo=tauri&logoColor=24C8DB) | Зв'язок між Rust-ядром та інтерфейсом, системні події |
| **Frontend Runtime** | ![WebView2](https://img.shields.io/badge/-WebView2-000000?style=flat-square&logo=microsoftedge&logoColor=0078D7) | Легкий рендеринг сучасного UI засобами Windows |
| **CI/CD & Hosting** | ![GitHub Actions](https://img.shields.io/badge/-GitHub_Actions-000000?style=flat-square&logo=githubactions&logoColor=2088FF) | Автоматична хмарна збірка `.exe` та дистрибуція оновлень |

---

<div align="center">
  <sub>Розроблено командою <b>HDT</b> • 2026</sub>
</div>
