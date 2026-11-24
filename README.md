<div align="center">

# 🏛️ TERRAPOLIS

**Кастомний ігровий лаунчер та платформа керування збірками для проєктів команди HDT**

<p>
  <img src="https://img.shields.io/badge/Status-Beta_0.1.0-f59e0b?style=for-the-badge&logo=github&logoColor=white" alt="Status Beta" />
  <img src="https://img.shields.io/badge/Core-Rust-dea584?style=for-the-badge&logo=rust&logoColor=black" alt="Rust" />
  <img src="https://img.shields.io/badge/Engine-Tauri_v2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/UI-WebView2-0078D7?style=for-the-badge&logo=microsoftedge&logoColor=white" alt="WebView2" />
  <img src="https://img.shields.io/badge/Platform-Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows" />
</p>

> ⚠️ **Увага:** Це тестова бета-версія лаунчера (`v0.1.0-beta`). Функціонал активно тестується та доповнюється перед релізом `1.0.0`.

</div>

---

## ✨ Про проєкт

**Terrapolis** — це легкий, швидкий та мінімалістичний десктопний лаунчер, розроблений на сучасному системному стеку **Rust + WebView2 (Tauri v2)**[span_4](start_span)[span_4](end_span)[span_5](start_span)[span_5](end_span). Він створений як єдиний портал для входу в ігрові всесвіти та політичні сервери (**PolitAgora**: *Legacy, New Horizon, Genesis*) без зайвого навантаження на систему[span_6](start_span)[span_6](end_span)[span_7](start_span)[span_7](end_span).

На відміну від важких лаунчерів на Electron або Python, Terrapolis використовує нативний компонент Windows **WebView2** та скомпільований бекенд на **Rust**, завдяки чому займає лічені мегабайти та запускається миттєво[span_8](start_span)[span_8](end_span)[span_9](start_span)[span_9](end_span).

---

## 🚀 Ключові особливості

* ⚡ **Нативна швидкодія (Rust + WebView2):** Мінімальне споживання оперативної пам'яті у фоні та швидкий запуск без вбудованого Chromium[span_10](start_span)[span_10](end_span)[span_11](start_span)[span_11](end_span).
* 🔄 **Розумна синхронізація збірок:** Багатопотокова перевірка цілісності файлів за хешами **SHA-256** — лаунчер завантажує лише нові або змінені моди, конфіги, ресурспаки та шейдери[span_12](start_span)[span_12](end_span).
* 🛠️ **Автоматичне розгортання середовища:** Самостійне завантаження та налаштування необхідної версії **Java (Adoptium Temurin JRE)** і модлоадерів (*Fabric, Forge, NeoForge, Vanilla*)[span_13](start_span)[span_13](end_span).
* 🚀 **Автооновлення клієнта:** Перевірка актуальної версії лаунчера через **GitHub Releases** при кожному старті з безшовною заміною виконуваного файлу[span_14](start_span)[span_14](end_span).
* 🎮 **Гнучке налаштування:** Вхід за нікнеймом без прив'язки до ліцензії та зручне керування виділеною оперативною пам'яттю (RAM) перед запуском[span_15](start_span)[span_15](end_span).

---

## 🛠️ Технологічний стек

| Компонент | Технологія | Призначення |
| :--- | :--- | :--- |
| **Backend & Core** | ![Rust](https://img.shields.io/badge/-Rust-000000?style=flat-square&logo=rust&logoColor=dea584) | Робота з файловою системою, хешування SHA-256, мережеві запити, запуск JVM[span_16](start_span)[span_16](end_span)[span_17](start_span)[span_17](end_span) |
| **Desktop Framework** | ![Tauri](https://img.shields.io/badge/-Tauri_v2-000000?style=flat-square&logo=tauri&logoColor=24C8DB) | Зв'язок між Rust-ядром та інтерфейсом, системні події[span_18](start_span)[span_18](end_span)[span_19](start_span)[span_19](end_span) |
| **Frontend Runtime** | ![WebView2](https://img.shields.io/badge/-WebView2-000000?style=flat-square&logo=microsoftedge&logoColor=0078D7) | Легкий рендеринг сучасного UI засобами Windows[span_20](start_span)[span_20](end_span)[span_21](start_span)[span_21](end_span) |
| **CI/CD & Hosting** | ![GitHub Actions](https://img.shields.io/badge/-GitHub_Actions-000000?style=flat-square&logo=githubactions&logoColor=2088FF) | Автоматична хмарна збірка `.exe` та дистрибуція оновлень[span_22](start_span)[span_22](end_span)[span_23](start_span)[span_23](end_span) |

---

<div align="center">
  <sub>Розроблено командою <b>HDT</b> • 2026</sub>[span_24](start_span)[span_24](end_span)[span_25](start_span)[span_25](end_span)
</div>
