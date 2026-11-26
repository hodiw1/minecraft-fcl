# Проєкти та збірки (для адміністратора)

Лаунчер 0.2 більше не прив'язаний до однієї збірки. Гравець при першому запуску вводить
**код проєкту** і свій нік, а далі бачить усі збірки цього проєкту.

## 1. Як код перетворюється на список збірок

```
Код "FNAF-2026"
   └─► projects.json у репозиторії лаунчера (hodiw1/minecraft-fcl, гілка main)
          └─► instances.json у репозиторії проєкту
                 └─► manifest.json кожної збірки (той самий формат, що й раніше)
```

### projects.json — у корені репозиторію лаунчера

```json
{
  "projects": {
    "FNAF-2026": {
      "name": "FNAF RP",
      "repo": "hodiw1/fnaf-packs",
      "branch": "main"
    },
    "OLD-SURVIVAL": {
      "name": "Old Survival",
      "repo": "hodiw1/minecraft-fcl"
    },
    "PARTNER": {
      "name": "Partner Server",
      "index_url": "https://example.com/launcher/instances.json"
    }
  }
}
```

- Код порівнюється без урахування регістру (`fnaf-2026` = `FNAF-2026`).
- `branch` за замовчуванням `main`, `index` (шлях до індексу) — `instances.json`.
- Для тестів адміністратор може ввести замість коду `owner/repo` або `owner/repo@branch` —
  гравцям ця підказка не показується.

### instances.json — у репозиторії проєкту

```json
{
  "name": "FNAF RP",
  "description": "Рольовий сервер",
  "instances": [
    {
      "id": "legacy",
      "name": "Legacy",
      "description": "Основний політичний сервер, сезон 2",
      "manifest": "legacy/manifest.json",
      "server": "play.example.com:25565",
      "image": "https://raw.githubusercontent.com/hodiw1/fnaf-packs/main/legacy/cover.jpg",
      "recommended_ram_mb": 6144
    },
    {
      "id": "new-horizon",
      "name": "New Horizon",
      "description": "Технічна збірка з Create",
      "manifest": "new-horizon/manifest.json",
      "server": "nh.example.com:25570"
    }
  ]
}
```

| Поле | Обов'язкове | Що робить |
|---|---|---|
| `id` | так | Назва папки `instances/<id>/`. Лише `a-z`, `0-9`, `-`, `_`. **Не змінюйте** після релізу — інакше гравці качатимуть збірку заново. |
| `name` | ні | Велика назва на головному екрані. |
| `description` | ні | Людський опис замість технічного шляху. |
| `manifest` | ні | Відносний шлях від `instances.json` або повне `https://`. За замовчуванням `<id>/manifest.json`. |
| `server` | ні | `host:port` для індикатора «Сервер онлайн · 12/100». SRV-записи не підтримуються — вказуйте порт явно. |
| `image` | ні | `https://` картинка-фон (≈1280×720, JPG до 300 КБ). |
| `recommended_ram_mb` | ні | Підказка в налаштуваннях ОЗУ. Можна вказати й у `manifest.json`. |
| `size_mb` | ні | Розмір для кнопки «Встановити ~650 МБ». Якщо не вказано — рахується з `manifest.json`. |
| `minecraft_version`, `loader` | ні | Бейджі до завантаження маніфесту. Зазвичай не потрібні. |

### Сумісність зі старим форматом

Якщо `instances.json` немає, лаунчер шукає `manifest.json` поруч і показує одну збірку з id `main`.
Тобто поточний репозиторій `hodiw1/minecraft-fcl` працює як проєкт без жодних змін —
достатньо додати його код у `projects.json`.

`manifest-builder` і далі генерує `manifest.json` однієї збірки. Для кількох збірок у одному
репозиторії запускайте його в окремих папках/гілках або тримайте кожну збірку в окремому репозиторії
й посилайтесь на `manifest.json` повним `https://`.

## 2. Розкладка файлів у гравця

```
%AppData%\com.custom.mclauncher\     config.json, skins\, projects\ (кеш)
<папка лаунчера>\                     змінюється в Налаштуваннях
  minecraft\   versions, libraries, assets — СПІЛЬНІ, без дублювання
  runtime\     Java — спільна
  instances\
    legacy\        mods, config, resourcepacks, shaderpacks, servers.dat, saves, options.txt…
    new-horizon\
```

## 3. Що лаунчер робить із файлами збірки

- **Грати** — швидка перевірка (розмір + записаний SHA-256), докачування змінених файлів,
  видалення зайвих `.jar` у `mods/`.
- **Ремонт** — повний SHA-256 кожного файлу + перевірка бібліотек і ресурсів Minecraft.
- `config/*` і `servers.dat` докачуються, лише якщо їх немає або **адмін змінив** їх у маніфесті.
  Правки гравця не перезаписуються.
- Ніколи не чіпаються: `options.txt`, `optionsof.txt`, `optionsshaders.txt`, `saves/`, `screenshots/`,
  `xaero*/`, `journeymap/`, `logs/`, `crash-reports/`.
- Пресет графіки й мова гри пишуться в `options.txt` **один раз** — коли файлу ще немає
  (перше встановлення), або вручну кнопкою в Налаштуваннях (лише графічні ключі).

## 4. Скіни

Скін профілю копіюється в `instances/<id>/CustomSkinLoader/LocalSkin/skins/<нік>.png`.
Щоб гравець бачив свій скін, додайте в збірку мод **CustomSkinLoader**.
Щоб скін бачили інші — потрібен плагін на сервері (наприклад, SkinsRestorer).
