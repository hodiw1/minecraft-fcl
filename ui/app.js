"use strict";

const tauri = window.__TAURI__;
const invoke = tauri.core.invoke;
const listen = tauri.event.listen;

const NICK_RE = /^[A-Za-z0-9_]{3,16}$/;
const MB = 1024 * 1024;
const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");

/* ═══════════════════════════ ПЕРЕКЛАДИ ═══════════════════════════ */

const DICT = {
  uk: {
    "boot.loading": "Завантаження…",
    "boot.updating": "Оновлення лаунчера…",
    "ob.title": "Ласкаво просимо!",
    "ob.titleBack": "Підключи проєкт",
    "ob.sub": "Щоб почати, введи код проєкту від адміністратора сервера та свій ігровий нік.",
    "ob.subBack": "Введи код проєкту, до якого хочеш підключитися.",
    "ob.code": "Код проєкту",
    "ob.codePh": "Наприклад: FNAF-2026",
    "ob.codeHint": "Код дає адміністратор сервера — зазвичай у Discord чи Telegram.",
    "ob.nick": "Твій ігровий нік",
    "ob.nickPh": "Наприклад: Kozak_2009",
    "ob.nickHint": "Під цим ім'ям тебе бачитимуть у грі. 3–16 символів: латиниця, цифри, _",
    "ob.submit": "Підключитися",
    "ob.checking": "Перевіряємо код…",
    "ob.needCode": "Введи код проєкту",
    "nav.home": "Збірки",
    "nav.profiles": "Профілі",
    "nav.project": "Проєкт",
    "nav.settings": "Налаштування",
    "top.changeProfile": "Змінити профіль",
    "home.repair": "Ремонт",
    "home.repairTitle": "Перевірити й виправити файли збірки",
    "home.folder": "Папка",
    "home.folderTitle": "Відкрити папку збірки (скріншоти, ресурспаки)",
    "home.listTitle": "Збірки проєкту",
    "home.installedMeta": "Встановлено · версія {v}",
    "home.notInstalledMeta": "Ще не встановлено",
    "home.updateMeta": "Є оновлення: {from} → {to}",
    "home.playingAs": "Ти граєш як <b>{nick}</b> · <button type=\"button\" class=\"link\" data-goto=\"profiles\">змінити</button>",
    "home.noNick": "Нікнейм не вибрано — <button type=\"button\" class=\"link\" data-goto=\"profiles\">додай його у «Профілях»</button>",
    "home.empty": "Обери збірку праворуч",
    "list.error": "Не вдалося завантажити список збірок.",
    "list.retry": "Спробувати ще",
    "list.empty": "У цьому проєкті поки немає збірок.",
    "inst.installed": "Встановлено",
    "inst.notInstalled": "Нова",
    "inst.update": "Оновлення",
    "inst.running": "Запущено",
    "inst.more": "Дії зі збіркою",
    "menu.folder": "Відкрити папку",
    "menu.repair": "Перевірити файли",
    "menu.delete": "Видалити збірку…",
    "play.choose": "Обери збірку",
    "play.addNick": "Додай нікнейм",
    "play.play": "Грати",
    "play.install": "Встановити",
    "play.update": "Оновити",
    "play.checking": "Перевірка файлів…",
    "play.downloading": "Завантаження…",
    "play.preparing": "Підготовка гри…",
    "play.launching": "Запуск гри…",
    "play.running": "Гра запущена",
    "play.otherRunning": "Запущена інша збірка",
    "play.wait": "Зачекай…",
    "play.repairing": "Ремонт…",
    "play.subRunning": "закрий гру, щоб запустити знову",
    "play.subUpdate": "нова версія {v}",
    "play.subFirst": "перший запуск довший — качаються Minecraft і Java",
    "stage.launcher-update": "Оновлення лаунчера",
    "stage.verify": "Перевірка файлів збірки",
    "stage.download": "Завантаження збірки",
    "stage.java": "Завантаження Java",
    "stage.client": "Клієнт Minecraft",
    "stage.libraries": "Бібліотеки гри",
    "stage.assets": "Ресурси гри",
    "server.online": "Сервер онлайн · {n}/{max}",
    "server.offline": "Сервер недоступний",
    "server.checking": "Перевірка сервера…",
    "status.loading": "Завантаження списку збірок…",
    "status.ready": "Готово до гри",
    "status.offline": "Немає інтернету — показано збережений список",
    "status.gameRunning": "Гра запущена — приємної гри!",
    "status.gameClosed": "Гру закрито",
    "status.updateFailed": "Не вдалося перевірити оновлення лаунчера",
    "status.restarting": "Перезапуск лаунчера…",
    "status.offlinePlay": "Запущено без інтернету — оновлення не перевірялись",
    "status.installed": "Збірку встановлено — гра запускається",
    "profiles.title": "Профілі",
    "profiles.sub": "Нікнейми, під якими ти заходиш у гру. Кнопка «Грати» використовує активний профіль.",
    "profiles.upload": "Завантажити скін",
    "profiles.reset": "Стандартний",
    "profiles.skinFormat": "Файл .png 64×64 — такий, як на сайтах зі скінами.",
    "profiles.skinNote": "Скін бачиш <b>лише ти</b> (якщо в збірці є мод CustomSkinLoader). Щоб його бачили інші гравці, сервер має підтримувати скіни — уточни в адміністратора.",
    "profiles.newPh": "Новий нікнейм",
    "profiles.add": "Додати",
    "profiles.nickRule": "3–16 символів: латиниця, цифри, _",
    "profiles.nickBad": "Лише латиниця, цифри та _ (3–16 символів)",
    "profiles.active": "Активний",
    "profiles.makeActive": "Обрати",
    "profiles.ownSkin": "Власний скін",
    "profiles.defaultSkin": "Стандартний скін",
    "profiles.skinFor": "Змінити скін",
    "profiles.remove": "Видалити профіль",
    "profiles.removeTitle": "Видалити профіль «{nick}»?",
    "profiles.removeText": "Нік і скін зникнуть зі списку лаунчера. Світи, предмети й прогрес на сервері не зміняться — профіль можна додати знову.",
    "profiles.added": "Профіль «{nick}» додано й обрано",
    "profiles.skinSaved": "Скін збережено",
    "profiles.skinBig": "Файл завеликий — скін має важити до 1 МБ",
    "profiles.skinBad": "Це не схоже на скін Minecraft: потрібен .png розміром 64×64 (або 64×32)",
    "profiles.lastOne": "Має залишитися хоча б один профіль",
    "project.title": "Проєкт",
    "project.sub": "Проєкт — це сервер чи спільнота, до якої прив'язаний лаунчер. Від нього залежить список збірок.",
    "project.code": "Код:",
    "project.refresh": "Оновити список",
    "project.switchTitle": "Підключити інший проєкт",
    "project.connect": "Підключити",
    "project.switchHint": "Встановлені збірки залишаться на диску. Видалити їх можна в Налаштуваннях.",
    "project.leaveTitle": "Вийти з проєкту",
    "project.leaveSub": "Лаунчер забуде код і попросить ввести новий. Профілі й збірки залишаться.",
    "project.leave": "Вийти з проєкту",
    "project.online": "Список збірок актуальний",
    "project.offline": "Немає зв'язку — показано збережений список",
    "project.switched": "Підключено проєкт «{name}»",
    "project.leaveConfirm": "Вийти з проєкту «{name}»?",
    "project.leaveText": "Щоб повернутися, знадобиться код проєкту. Встановлені збірки й профілі залишаться на комп'ютері.",
    "settings.title": "Налаштування",
    "settings.ram": "Оперативна пам'ять для гри",
    "settings.ramHint": "Встановлено на ПК: {total} · Рекомендовано: {rec}",
    "settings.ramHigh": "Забагато: Windows і браузеру може не вистачити пам'яті — комп'ютер почне гальмувати.",
    "settings.ramLow": "Менше рекомендованого — збірка може гальмувати або вилітати.",
    "settings.ramUseRec": "Поставити рекомендоване",
    "settings.graphics": "Пресет графіки",
    "settings.presetHint": "Застосовується автоматично один раз — під час встановлення збірки. Гучність, клавіші й чутливість миші лаунчер не змінює.",
    "settings.applyPreset": "Застосувати до «{name}» зараз",
    "settings.applyPresetNone": "Спершу встанови збірку",
    "settings.lang": "Мова",
    "settings.uiLang": "Мова лаунчера",
    "settings.gameLang": "Мова Minecraft",
    "settings.gameLangHint": "Змінюється у всіх встановлених збірках. У грі мову теж можна змінити будь-коли.",
    "settings.folder": "Папка лаунчера та збірок",
    "settings.change": "Змінити…",
    "settings.open": "Відкрити",
    "settings.resetDir": "Повернути стандартну",
    "settings.folderHint": "Знадобиться, якщо на диску C: мало місця. Java, бібліотеки й ресурси гри спільні для всіх збірок і не дублюються.",
    "settings.installed": "Встановлені збірки",
    "settings.installedTotal": "Разом: {size}",
    "settings.installedEmpty": "Поки що жодної збірки не встановлено.",
    "settings.fromProject": "проєкт {code}",
    "settings.partial": "встановлено не до кінця",
    "settings.deleteAll": "Видалити всі збірки",
    "settings.about": "Лаунчер v{v}",
    "preset.low": "Низька",
    "preset.medium": "Середня",
    "preset.high": "Висока",
    "preset.ultra": "Ультра",
    "preset.title": "Застосувати пресет «{preset}» до «{name}»?",
    "preset.text": "Зміниться лише графіка: дальність прорисовки, тіні, хмари, частинки. Гучність, клавіші й чутливість миші залишаться як є.",
    "preset.apply": "Застосувати",
    "preset.done": "Пресет «{preset}» застосовано",
    "repair.title": "Перевірити файли «{name}»?",
    "repair.text": "Лаунчер звірить усі файли з еталонною збіркою, докачає пошкоджені та прибере зайві моди.<br><b>Не зміняться:</b> світи, скріншоти, налаштування гри, керування й мітки на мапі.",
    "repair.go": "Перевірити",
    "repair.ok": "Усе гаразд — пошкоджених файлів не знайдено",
    "repair.fixed": "Готово: виправлено файлів — {d}, прибрано зайвих — {r}",
    "delete.title": "Видалити збірку «{name}»?",
    "delete.text": "Буде видалено <b>усе</b> з папки цієї збірки: світи одиночної гри, скріншоти, мітки на мапі, налаштування керування. Скасувати це не можна.",
    "delete.go": "Видалити",
    "delete.done": "Збірку «{name}» видалено",
    "deleteAll.title": "Видалити всі збірки?",
    "deleteAll.text": "Буде видалено {n} — це звільнить {size}. Разом зі збірками зникнуть світи одиночної гри, скріншоти й налаштування. Скасувати це не можна.",
    "deleteAll.go": "Видалити всі",
    "deleteAll.done": "Усі збірки видалено",
    "dir.moveTitle": "Перенести збірки в нову папку?",
    "dir.moveText": "Нова папка: <b>{path}</b><br>Якщо перенести — нічого не доведеться качати знову (це може зайняти кілька хвилин). Якщо почати з порожньої — старі файли залишаться в попередній папці.",
    "dir.move": "Перенести",
    "dir.noMove": "Почати з порожньої",
    "dir.done": "Папку змінено",
    "dir.resetTitle": "Повернути стандартну папку?",
    "crash.title": "Гра завершилась із помилкою",
    "crash.lead": "Таке буває. Ось що можна зробити:",
    "crash.memory_low": "Грі <b>не вистачило пам'яті</b>. Збільш ОЗУ в Налаштуваннях і спробуй ще раз.",
    "crash.memory_too_high": "Виділено <b>забагато пам'яті</b> — Java не змогла запуститися. Зменш ОЗУ в Налаштуваннях (кнопка «Поставити рекомендоване»).",
    "crash.java": "Проблема з <b>версією Java</b>. Натисни «Перевірити файли» — лаунчер докачає правильну.",
    "crash.mods": "Схоже на <b>конфлікт модів</b>. Натисни «Перевірити файли» — лаунчер прибере зайві моди й відновить пошкоджені.",
    "crash.graphics": "Проблема з <b>відеодрайвером</b>. Онови драйвер відеокарти з сайту NVIDIA, AMD чи Intel.",
    "crash.generic": "Натисни «Перевірити файли» — це виправляє більшість проблем. Якщо не допоможе — скопіюй звіт і надішли адміністратору.",
    "crash.copy": "Скопіювати звіт для адміністратора",
    "crash.copied": "Скопійовано ✓",
    "crash.repair": "Перевірити файли",
    "crash.folder": "Папка збірки",
    "error.title": "Щось пішло не так",
    "error.copy": "Скопіювати текст помилки",
    "common.cancel": "Скасувати",
    "common.close": "Закрити",
    "common.retry": "Спробувати ще",
    "plural.packs": ["збірка", "збірки", "збірок"],
    "unit.mb": "МБ",
    "unit.gb": "ГБ"
  },
  en: {
    "boot.loading": "Loading…",
    "boot.updating": "Updating the launcher…",
    "ob.title": "Welcome!",
    "ob.titleBack": "Connect a project",
    "ob.sub": "To get started, enter the project code from your server admin and your in-game nickname.",
    "ob.subBack": "Enter the code of the project you want to connect to.",
    "ob.code": "Project code",
    "ob.codePh": "For example: FNAF-2026",
    "ob.codeHint": "The server admin gives you the code — usually on Discord or Telegram.",
    "ob.nick": "Your in-game nickname",
    "ob.nickPh": "For example: Kozak_2009",
    "ob.nickHint": "Other players will see you by this name. 3–16 chars: Latin letters, digits, _",
    "ob.submit": "Connect",
    "ob.checking": "Checking the code…",
    "ob.needCode": "Enter the project code",
    "nav.home": "Modpacks",
    "nav.profiles": "Profiles",
    "nav.project": "Project",
    "nav.settings": "Settings",
    "top.changeProfile": "Change profile",
    "home.repair": "Repair",
    "home.repairTitle": "Check and fix modpack files",
    "home.folder": "Folder",
    "home.folderTitle": "Open the modpack folder (screenshots, resource packs)",
    "home.listTitle": "Project modpacks",
    "home.installedMeta": "Installed · version {v}",
    "home.notInstalledMeta": "Not installed yet",
    "home.updateMeta": "Update available: {from} → {to}",
    "home.playingAs": "You play as <b>{nick}</b> · <button type=\"button\" class=\"link\" data-goto=\"profiles\">change</button>",
    "home.noNick": "No nickname selected — <button type=\"button\" class=\"link\" data-goto=\"profiles\">add one in Profiles</button>",
    "home.empty": "Pick a modpack on the right",
    "list.error": "Couldn't load the modpack list.",
    "list.retry": "Try again",
    "list.empty": "This project has no modpacks yet.",
    "inst.installed": "Installed",
    "inst.notInstalled": "New",
    "inst.update": "Update",
    "inst.running": "Running",
    "inst.more": "Modpack actions",
    "menu.folder": "Open folder",
    "menu.repair": "Check files",
    "menu.delete": "Delete modpack…",
    "play.choose": "Pick a modpack",
    "play.addNick": "Add a nickname",
    "play.play": "Play",
    "play.install": "Install",
    "play.update": "Update",
    "play.checking": "Checking files…",
    "play.downloading": "Downloading…",
    "play.preparing": "Preparing game…",
    "play.launching": "Starting game…",
    "play.running": "Game is running",
    "play.otherRunning": "Another modpack is running",
    "play.wait": "Please wait…",
    "play.repairing": "Repairing…",
    "play.subRunning": "close the game to start again",
    "play.subUpdate": "new version {v}",
    "play.subFirst": "first launch takes longer — Minecraft and Java are downloaded",
    "stage.launcher-update": "Updating launcher",
    "stage.verify": "Checking modpack files",
    "stage.download": "Downloading modpack",
    "stage.java": "Downloading Java",
    "stage.client": "Minecraft client",
    "stage.libraries": "Game libraries",
    "stage.assets": "Game assets",
    "server.online": "Server online · {n}/{max}",
    "server.offline": "Server unavailable",
    "server.checking": "Checking server…",
    "status.loading": "Loading modpacks…",
    "status.ready": "Ready to play",
    "status.offline": "No internet — showing the saved list",
    "status.gameRunning": "Game is running — have fun!",
    "status.gameClosed": "Game closed",
    "status.updateFailed": "Couldn't check for launcher updates",
    "status.restarting": "Restarting launcher…",
    "status.offlinePlay": "Started offline — updates weren't checked",
    "status.installed": "Modpack installed — the game is starting",
    "profiles.title": "Profiles",
    "profiles.sub": "Nicknames you use to join the game. The Play button uses the active profile.",
    "profiles.upload": "Upload skin",
    "profiles.reset": "Default",
    "profiles.skinFormat": "A 64×64 .png file — like the ones on skin websites.",
    "profiles.skinNote": "<b>Only you</b> see this skin (if the modpack has the CustomSkinLoader mod). For other players to see it, the server must support skins — ask the admin.",
    "profiles.newPh": "New nickname",
    "profiles.add": "Add",
    "profiles.nickRule": "3–16 characters: Latin letters, digits, _",
    "profiles.nickBad": "Only Latin letters, digits and _ (3–16 characters)",
    "profiles.active": "Active",
    "profiles.makeActive": "Select",
    "profiles.ownSkin": "Custom skin",
    "profiles.defaultSkin": "Default skin",
    "profiles.skinFor": "Change skin",
    "profiles.remove": "Delete profile",
    "profiles.removeTitle": "Delete profile \"{nick}\"?",
    "profiles.removeText": "The nickname and skin will be removed from the launcher. Worlds, items and server progress won't change — you can add the profile again.",
    "profiles.added": "Profile \"{nick}\" added and selected",
    "profiles.skinSaved": "Skin saved",
    "profiles.skinBig": "The file is too large — a skin must be under 1 MB",
    "profiles.skinBad": "This doesn't look like a Minecraft skin: a 64×64 (or 64×32) .png is required",
    "profiles.lastOne": "At least one profile must remain",
    "project.title": "Project",
    "project.sub": "A project is the server or community the launcher is linked to. It decides which modpacks you see.",
    "project.code": "Code:",
    "project.refresh": "Refresh list",
    "project.switchTitle": "Connect another project",
    "project.connect": "Connect",
    "project.switchHint": "Installed modpacks stay on disk. You can delete them in Settings.",
    "project.leaveTitle": "Leave project",
    "project.leaveSub": "The launcher will forget the code and ask for a new one. Profiles and modpacks stay.",
    "project.leave": "Leave project",
    "project.online": "Modpack list is up to date",
    "project.offline": "No connection — showing the saved list",
    "project.switched": "Connected to \"{name}\"",
    "project.leaveConfirm": "Leave project \"{name}\"?",
    "project.leaveText": "You'll need the project code to come back. Installed modpacks and profiles stay on this computer.",
    "settings.title": "Settings",
    "settings.ram": "Memory (RAM) for the game",
    "settings.ramHint": "Installed in PC: {total} · Recommended: {rec}",
    "settings.ramHigh": "Too much: Windows and your browser may run out of memory — the PC will slow down.",
    "settings.ramLow": "Below the recommendation — the modpack may lag or crash.",
    "settings.ramUseRec": "Use recommended",
    "settings.graphics": "Graphics preset",
    "settings.presetHint": "Applied automatically once — when a modpack is installed. Volume, key bindings and mouse sensitivity are never touched.",
    "settings.applyPreset": "Apply to \"{name}\" now",
    "settings.applyPresetNone": "Install a modpack first",
    "settings.lang": "Language",
    "settings.uiLang": "Launcher language",
    "settings.gameLang": "Minecraft language",
    "settings.gameLangHint": "Changed in all installed modpacks. You can also change it in-game at any time.",
    "settings.folder": "Launcher & modpack folder",
    "settings.change": "Change…",
    "settings.open": "Open",
    "settings.resetDir": "Restore default",
    "settings.folderHint": "Useful if drive C: is low on space. Java, libraries and game assets are shared between modpacks and never duplicated.",
    "settings.installed": "Installed modpacks",
    "settings.installedTotal": "Total: {size}",
    "settings.installedEmpty": "No modpacks installed yet.",
    "settings.fromProject": "project {code}",
    "settings.partial": "not fully installed",
    "settings.deleteAll": "Delete all modpacks",
    "settings.about": "Launcher v{v}",
    "preset.low": "Low",
    "preset.medium": "Medium",
    "preset.high": "High",
    "preset.ultra": "Ultra",
    "preset.title": "Apply the \"{preset}\" preset to \"{name}\"?",
    "preset.text": "Only graphics change: render distance, shadows, clouds, particles. Volume, key bindings and mouse sensitivity stay as they are.",
    "preset.apply": "Apply",
    "preset.done": "\"{preset}\" preset applied",
    "repair.title": "Check files of \"{name}\"?",
    "repair.text": "The launcher compares every file with the reference modpack, re-downloads damaged ones and removes extra mods.<br><b>Untouched:</b> worlds, screenshots, game settings, key bindings and map waypoints.",
    "repair.go": "Check",
    "repair.ok": "All good — no damaged files found",
    "repair.fixed": "Done: fixed files — {d}, removed extras — {r}",
    "delete.title": "Delete modpack \"{name}\"?",
    "delete.text": "<b>Everything</b> in this modpack's folder will be deleted: singleplayer worlds, screenshots, map waypoints, key bindings. This can't be undone.",
    "delete.go": "Delete",
    "delete.done": "Modpack \"{name}\" deleted",
    "deleteAll.title": "Delete all modpacks?",
    "deleteAll.text": "{n} will be deleted, freeing {size}. Singleplayer worlds, screenshots and settings go with them. This can't be undone.",
    "deleteAll.go": "Delete all",
    "deleteAll.done": "All modpacks deleted",
    "dir.moveTitle": "Move modpacks to the new folder?",
    "dir.moveText": "New folder: <b>{path}</b><br>Moving means nothing has to be downloaded again (may take a few minutes). If you start empty, the old files stay in the previous folder.",
    "dir.move": "Move",
    "dir.noMove": "Start empty",
    "dir.done": "Folder changed",
    "dir.resetTitle": "Restore the default folder?",
    "crash.title": "The game closed with an error",
    "crash.lead": "It happens. Here's what you can do:",
    "crash.memory_low": "The game <b>ran out of memory</b>. Increase RAM in Settings and try again.",
    "crash.memory_too_high": "<b>Too much memory</b> was allocated — Java couldn't start. Lower RAM in Settings (\"Use recommended\").",
    "crash.java": "A <b>Java version</b> problem. Press \"Check files\" — the launcher will download the right one.",
    "crash.mods": "Looks like a <b>mod conflict</b>. Press \"Check files\" — the launcher removes extra mods and restores damaged ones.",
    "crash.graphics": "A <b>graphics driver</b> problem. Update your GPU driver from NVIDIA, AMD or Intel.",
    "crash.generic": "Press \"Check files\" — it fixes most problems. If it doesn't help, copy the report and send it to the admin.",
    "crash.copy": "Copy report for the admin",
    "crash.copied": "Copied ✓",
    "crash.repair": "Check files",
    "crash.folder": "Modpack folder",
    "error.title": "Something went wrong",
    "error.copy": "Copy error text",
    "common.cancel": "Cancel",
    "common.close": "Close",
    "common.retry": "Try again",
    "plural.packs": ["modpack", "modpacks", "modpacks"],
    "unit.mb": "MB",
    "unit.gb": "GB"
  }
};

let lang = "uk";

function t(key, vars) {
  let text = DICT[lang][key] ?? DICT.uk[key] ?? key;
  if (vars) {
    for (const name in vars) {
      text = text.split("{" + name + "}").join(String(vars[name]));
    }
  }
  return text;
}

function plural(n, key) {
  const forms = DICT[lang][key];
  if (lang === "en") return n + " " + (n === 1 ? forms[0] : forms[1]);
  const mod10 = n % 10;
  const mod100 = n % 100;
  const form = mod10 === 1 && mod100 !== 11 ? 0 : mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14) ? 1 : 2;
  return n + " " + forms[form];
}

function applyI18n() {
  document.documentElement.lang = lang === "en" ? "en" : "uk";
  for (const node of document.querySelectorAll("[data-i18n]")) node.textContent = t(node.dataset.i18n);
  for (const node of document.querySelectorAll("[data-i18n-html]")) node.innerHTML = t(node.dataset.i18nHtml);
  for (const node of document.querySelectorAll("[data-i18n-ph]")) node.placeholder = t(node.dataset.i18nPh);
  for (const node of document.querySelectorAll("[data-i18n-title]")) node.title = t(node.dataset.i18nTitle);
  for (const group of [el.obLang, el.uiLangSeg]) {
    for (const button of group.querySelectorAll("[data-lang]")) {
      const on = button.dataset.lang === lang;
      button.classList.toggle("on", on);
      button.setAttribute("aria-checked", on);
    }
  }
}

/* ═══════════════════════════ ДОВІДНИКИ ═══════════════════════════ */

const $ = (id) => document.getElementById(id);

const el = {
  boot: $("boot"),
  bootText: $("bootText"),
  onboarding: $("onboarding"),
  obForm: $("obForm"),
  obCode: $("obCode"),
  obNick: $("obNick"),
  obNickField: $("obNickField"),
  obNickHint: $("obNickHint"),
  obError: $("obError"),
  obSubmit: $("obSubmit"),
  obLang: $("obLang"),
  app: $("app"),
  brandLetter: $("brandLetter"),
  nav: $("nav"),
  navInd: $("navInd"),
  island: $("island"),
  status: $("statusText"),
  topProject: $("topProject"),
  topDot: $("topDot"),
  topHead: $("topHead"),
  topNick: $("topNick"),
  hero: $("hero"),
  heroBg: $("heroBg"),
  badgeMc: $("badgeMc"),
  badgeLoader: $("badgeLoader"),
  badgeServer: $("badgeServer"),
  serverText: $("serverText"),
  instName: $("instName"),
  instDesc: $("instDesc"),
  instMeta: $("instMeta"),
  play: $("play"),
  playLabel: $("playLabel"),
  playSub: $("playSub"),
  repairBtn: $("repairBtn"),
  folderBtn: $("folderBtn"),
  progress: $("progress"),
  progressStage: $("progressStage"),
  progressPercent: $("progressPercent"),
  progressFill: $("progressFill"),
  progressFile: $("progressFile"),
  nickNote: $("nickNote"),
  instCount: $("instCount"),
  instList: $("instList"),
  instMenu: $("instMenu"),
  skinBody: $("skinBody"),
  stageNick: $("stageNick"),
  skinUpload: $("skinUpload"),
  skinReset: $("skinReset"),
  skinFile: $("skinFile"),
  addProfileForm: $("addProfileForm"),
  newNick: $("newNick"),
  newNickHint: $("newNickHint"),
  profileList: $("profileList"),
  projBrand: $("projBrand"),
  projName: $("projName"),
  projCode: $("projCode"),
  projCount: $("projCount"),
  projState: $("projState"),
  projRefresh: $("projRefresh"),
  switchForm: $("switchForm"),
  switchCode: $("switchCode"),
  switchSubmit: $("switchSubmit"),
  switchError: $("switchError"),
  leaveBtn: $("leaveBtn"),
  ram: $("ram"),
  ramValue: $("ramValue"),
  ramMin: $("ramMin"),
  ramMax: $("ramMax"),
  ramHint: $("ramHint"),
  ramWarn: $("ramWarn"),
  ramRecommended: $("ramRecommended"),
  presetSeg: $("presetSeg"),
  applyPreset: $("applyPreset"),
  uiLangSeg: $("uiLangSeg"),
  gameLang: $("gameLang"),
  dataDir: $("dataDir"),
  changeDir: $("changeDir"),
  openDir: $("openDir"),
  resetDir: $("resetDir"),
  diskList: $("diskList"),
  installedTotal: $("installedTotal"),
  deleteAll: $("deleteAll"),
  aboutText: $("aboutText"),
  modal: $("modal"),
  modalIcon: $("modalIcon"),
  modalTitle: $("modalTitle"),
  modalText: $("modalText"),
  modalActions: $("modalActions")
};

const ICONS = {
  trash: '<svg viewBox="0 0 24 24"><path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6M10 11v6M14 11v6"/></svg>',
  wrench: '<svg viewBox="0 0 24 24"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.8-3.8a6 6 0 0 1-7.9 7.9l-6.9 6.9a2.1 2.1 0 0 1-3-3l6.9-6.9a6 6 0 0 1 7.9-7.9z"/></svg>',
  alert: '<svg viewBox="0 0 24 24"><path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0zM12 9v4M12 17h.01"/></svg>',
  folder: '<svg viewBox="0 0 24 24"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>',
  logout: '<svg viewBox="0 0 24 24"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/></svg>',
  palette: '<svg viewBox="0 0 24 24"><circle cx="13.5" cy="6.5" r="1"/><circle cx="17.5" cy="10.5" r="1"/><circle cx="8.5" cy="7.5" r="1"/><circle cx="6.5" cy="12.5" r="1"/><path d="M12 2a10 10 0 0 0 0 20c.9 0 1.7-.8 1.7-1.7 0-.4-.2-.8-.4-1.1-.3-.3-.4-.7-.4-1.1 0-.9.8-1.7 1.7-1.7H17a5 5 0 0 0 5-5C22 6 17.5 2 12 2z"/></svg>',
  more: '<svg viewBox="0 0 24 24"><circle cx="12" cy="5" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="12" cy="19" r="1.2"/></svg>',
  shirt: '<svg viewBox="0 0 24 24"><path d="M20.4 6.6 16 4a4 4 0 0 1-8 0L3.6 6.6a1 1 0 0 0-.5 1.1l.9 3.6a1 1 0 0 0 1 .7H7v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-8h2a1 1 0 0 0 1-.7l.9-3.6a1 1 0 0 0-.5-1.1z"/></svg>'
};

/* ═══════════════════════════ СТАН ═══════════════════════════ */

const S = {
  tab: "home",
  launcherVersion: "",
  totalRam: 8192,
  maxRam: 6144,
  settings: null,
  profiles: [],
  activeProfile: "",
  binding: null,
  view: null,
  viewError: null,
  activeId: "",
  busy: null,
  phase: null,
  percent: 0,
  running: null,
  updating: false,
  disk: [],
  skins: new Map(),
  server: new Map(),
  menuFor: null
};

function esc(value) {
  return String(value ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function errText(error) {
  return String(error?.message ?? error ?? "").trim() || "Error";
}

function formatSize(bytes) {
  if (!bytes && bytes !== 0) return "";
  const mb = bytes / MB;
  if (mb >= 1024) return (mb / 1024).toFixed(mb >= 10240 ? 0 : 1).replace(".", lang === "uk" ? "," : ".") + " " + t("unit.gb");
  return Math.max(1, Math.round(mb)) + " " + t("unit.mb");
}

function formatRam(mb) {
  const gb = Math.round((mb / 1024) * 10) / 10;
  const text = String(gb);
  return (lang === "uk" ? text.replace(".", ",") : text) + " " + t("unit.gb");
}

function loaderName(loader) {
  const names = { vanilla: "Vanilla", fabric: "Fabric", forge: "Forge", neoforge: "NeoForge", quilt: "Quilt" };
  return names[String(loader || "").toLowerCase()] || loader || "";
}

function current() {
  return S.view?.instances.find((i) => i.id === S.activeId) || null;
}

function activeProfile() {
  return S.profiles.find((p) => p.id === S.activeProfile) || null;
}

let islandTimer = 0;

/** Повідомлення в "острові" зверху. Разові повідомлення через кілька секунд змінюються на поточний стан. */
function setIsland(kind, text, { sticky = false } = {}) {
  clearTimeout(islandTimer);
  el.island.className = "island is-" + kind;
  el.status.textContent = text;
  el.island.title = text;
  void el.island.offsetWidth;
  el.island.classList.add("pop");
  if (!sticky && kind !== "busy") islandTimer = setTimeout(idleIsland, kind === "error" ? 12000 : 6000);
}

function idleIsland() {
  if (S.busy || S.updating || el.app.hidden) return;
  if (S.running) setIsland("ok", t("status.gameRunning"), { sticky: true });
  else if (S.viewError) setIsland("error", t("list.error"), { sticky: true });
  else if (S.view?.offline) setIsland("warn", t("status.offline"), { sticky: true });
  else if (S.view) setIsland("ok", t("status.ready"), { sticky: true });
}

function updateCalm() {
  document.body.classList.toggle("calm", Boolean(S.running) || document.hidden);
}

/* ═══════════════════════════ МОДАЛЬНЕ ВІКНО ═══════════════════════════ */

let modalResolve = null;
let modalReturnFocus = null;

function closeModal(result) {
  if (el.modal.hidden) return;
  const resolve = modalResolve;
  modalResolve = null;
  const finish = () => {
    el.modal.hidden = true;
    el.modal.classList.remove("closing");
    modalReturnFocus?.focus?.();
  };
  if (reduceMotion.matches) finish();
  else {
    el.modal.classList.add("closing");
    setTimeout(finish, 180);
  }
  resolve?.(result);
}

/**
 * openModal({icon, tone, title, html, buttons:[{id,label,kind,left,onClick}]}) → Promise<id|null>
 * onClick може повернути false, щоб вікно лишилося відкритим.
 */
function openModal({ icon = "", tone = "", title, html, buttons }) {
  if (modalResolve) closeModal(null);
  modalReturnFocus = document.activeElement;
  el.modalIcon.innerHTML = ICONS[icon] || "";
  el.modalIcon.className = "modal-icon" + (tone ? " " + tone : "");
  el.modalTitle.textContent = title;
  el.modalText.innerHTML = !html ? "" : html.trim().startsWith("<p") ? html : `<p>${html}</p>`;
  el.modalActions.innerHTML = "";
  let primary = null;
  for (const spec of buttons) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn " + ({ primary: "btn-primary", danger: "btn-danger-solid", ghost: "btn-ghost" }[spec.kind] || "btn-ghost") + (spec.left ? " left" : "") + (spec.wide ? " wide" : "");
    button.textContent = spec.label;
    button.addEventListener("click", async () => {
      if (spec.onClick) {
        const keep = (await spec.onClick(button)) === false;
        if (keep) return;
      }
      closeModal(spec.id);
    });
    el.modalActions.append(button);
    if (spec.kind === "primary" || spec.kind === "danger") primary = button;
  }
  el.modal.hidden = false;
  (primary || el.modalActions.lastElementChild)?.focus();
  return new Promise((resolve) => (modalResolve = resolve));
}

function confirmDialog({ icon, tone, title, html, ok, danger }) {
  return openModal({
    icon,
    tone,
    title,
    html,
    buttons: [
      { id: "cancel", label: t("common.cancel"), kind: "ghost" },
      { id: "ok", label: ok, kind: danger ? "danger" : "primary" }
    ]
  }).then((id) => id === "ok");
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.append(area);
    area.select();
    const ok = document.execCommand("copy");
    area.remove();
    return ok;
  }
}

function showError(error, extra) {
  const text = errText(error);
  return openModal({
    icon: "alert",
    tone: "danger",
    title: t("error.title"),
    html: `<p>${esc(text)}</p>`,
    buttons: [
      {
        id: "copy",
        label: t("error.copy"),
        kind: "ghost",
        left: true,
        onClick: async (button) => {
          if (await copyText(text)) button.textContent = t("crash.copied");
          return false;
        }
      },
      ...(extra || []),
      { id: "close", label: t("common.close"), kind: "primary" }
    ]
  });
}

el.modal.addEventListener("pointerdown", (event) => {
  if (event.target === el.modal) closeModal(null);
});

/* ═══════════════════════════ СКІНИ ═══════════════════════════ */

async function loadSkin(profile) {
  if (!profile?.has_skin) return null;
  if (S.skins.has(profile.id)) return S.skins.get(profile.id);
  let bitmap = null;
  try {
    const bytes = await invoke("get_skin", { profileId: profile.id });
    if (bytes && bytes.length) bitmap = await createImageBitmap(new Blob([new Uint8Array(bytes)], { type: "image/png" }));
  } catch {
    bitmap = null;
  }
  S.skins.set(profile.id, bitmap);
  return bitmap;
}

function crispCanvas(canvas, cssSize) {
  const ratio = window.devicePixelRatio || 1;
  const size = Math.max(8, Math.round((cssSize * ratio) / 8) * 8);
  if (canvas.width !== size) {
    canvas.width = size;
    canvas.height = size;
  }
  const ctx = canvas.getContext("2d");
  ctx.imageSmoothingEnabled = false;
  return ctx;
}

/** Голова персонажа: обличчя 8×8 + шар шолома, без розмиття. */
function drawHead(canvas, skin, nick, cssSize) {
  const ctx = crispCanvas(canvas, cssSize || canvas.clientWidth || 32);
  const size = canvas.width;
  ctx.clearRect(0, 0, size, size);
  if (skin) {
    const k = skin.width / 64;
    ctx.drawImage(skin, 8 * k, 8 * k, 8 * k, 8 * k, 0, 0, size, size);
    ctx.drawImage(skin, 40 * k, 8 * k, 8 * k, 8 * k, 0, 0, size, size);
    return;
  }
  const gradient = ctx.createLinearGradient(0, 0, size, size);
  gradient.addColorStop(0, "#34d399");
  gradient.addColorStop(1, "#22b8cf");
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, size, size);
  ctx.fillStyle = "#04130d";
  ctx.font = `800 ${Math.round(size * 0.5)}px "Segoe UI", system-ui, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText((nick || "?").slice(0, 1).toUpperCase(), size / 2, size / 2 + size * 0.03);
}

function isSlim(skin) {
  if (skin.height !== skin.width) return false;
  const k = skin.width / 64;
  const probe = new OffscreenCanvas(skin.width, skin.height).getContext("2d");
  probe.drawImage(skin, 0, 0);
  const data = probe.getImageData(54 * k, 20 * k, 2 * k, 12 * k).data;
  for (let i = 3; i < data.length; i += 4) if (data[i] !== 0) return false;
  return true;
}

/** Фронтальний вигляд персонажа з розгортки скіна (64×64 або старий 64×32). */
function drawBody(canvas, skin) {
  const ctx = canvas.getContext("2d");
  const W = canvas.width;
  const H = canvas.height;
  const u = W / 16;
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, W, H);

  if (!skin) {
    const parts = [
      ["#2dd4bf", 4, 0, 8, 8],
      ["#155e75", 4, 8, 8, 12],
      ["#0f766e", 0, 8, 4, 12],
      ["#0f766e", 12, 8, 4, 12],
      ["#1e3a5f", 4, 20, 4, 12],
      ["#1e3a5f", 8, 20, 4, 12]
    ];
    for (const [color, x, y, w, h] of parts) {
      ctx.fillStyle = color;
      ctx.globalAlpha = 0.55;
      ctx.fillRect(x * u, y * u, w * u, h * u);
    }
    ctx.globalAlpha = 1;
    return;
  }

  const k = skin.width / 64;
  const modern = skin.height === skin.width;
  const arm = modern && isSlim(skin) ? 3 : 4;
  const part = (sx, sy, sw, sh, dx, dy, mirror) => {
    if (mirror) {
      ctx.save();
      ctx.translate((dx + sw) * u, dy * u);
      ctx.scale(-1, 1);
      ctx.drawImage(skin, sx * k, sy * k, sw * k, sh * k, 0, 0, sw * u, sh * u);
      ctx.restore();
    } else {
      ctx.drawImage(skin, sx * k, sy * k, sw * k, sh * k, dx * u, dy * u, sw * u, sh * u);
    }
  };
  const rightArmX = 4 - arm;
  // базовий шар
  part(8, 8, 8, 8, 4, 0);
  part(20, 20, 8, 12, 4, 8);
  part(44, 20, arm, 12, rightArmX, 8);
  part(4, 20, 4, 12, 4, 20);
  if (modern) {
    part(36, 52, arm, 12, 12, 8);
    part(20, 52, 4, 12, 8, 20);
  } else {
    part(44, 20, arm, 12, 12, 8, true);
    part(4, 20, 4, 12, 8, 20, true);
  }
  // другий шар (шолом, куртка, рукави, штани)
  part(40, 8, 8, 8, 4, 0);
  if (modern) {
    part(20, 36, 8, 12, 4, 8);
    part(44, 36, arm, 12, rightArmX, 8);
    part(52, 52, arm, 12, 12, 8);
    part(4, 36, 4, 12, 4, 20);
    part(4, 52, 4, 12, 8, 20);
  }
}

async function renderHeads() {
  const profile = activeProfile();
  const skin = await loadSkin(profile);
  drawHead(el.topHead, skin, profile?.nickname, 32);
  el.topNick.textContent = profile ? profile.nickname : "—";
}

/* ═══════════════════════════ ВКЛАДКИ ═══════════════════════════ */

function moveNavIndicator(button, smooth) {
  if (!button) return;
  el.navInd.style.transition = smooth ? "" : "none";
  el.navInd.style.height = button.offsetHeight + "px";
  el.navInd.style.transform = `translate3d(0, ${button.offsetTop}px, 0)`;
  if (!smooth) {
    void el.navInd.offsetWidth;
    el.navInd.style.transition = "";
  }
}

function switchTab(name, smooth = true) {
  closeMenu();
  S.tab = name;
  for (const view of document.querySelectorAll(".view")) {
    const on = view.dataset.view === name;
    view.classList.toggle("on", on);
    view.inert = !on;
  }
  let active = null;
  for (const button of el.nav.querySelectorAll(".nav-item")) {
    const on = button.dataset.tab === name;
    button.classList.toggle("on", on);
    if (on) {
      button.setAttribute("aria-current", "page");
      active = button;
    } else button.removeAttribute("aria-current");
  }
  moveNavIndicator(active, smooth);
  if (name === "settings") refreshSettings();
  if (name === "profiles") renderProfiles();
  if (name === "project") renderProject();
  if (name === "home") scheduleServerPing(true);
}

/* ═══════════════════════════ ЗБІРКИ ═══════════════════════════ */

function stageText(stage) {
  return DICT[lang]["stage." + stage] ? t("stage." + stage) : t("play.checking");
}

function setProgress(payload) {
  const percent = Math.max(0, Math.min(100, Number(payload.percent) || 0));
  S.percent = percent;
  el.progress.hidden = false;
  el.progressFill.style.transform = `scaleX(${percent / 100})`;
  el.progressPercent.textContent = Math.round(percent) + "%";
  el.progressStage.textContent = stageText(payload.stage);
  el.progressFile.textContent = payload.file || " ";
  if (S.phase === "downloading" || S.phase === "preparing") renderPlay();
}

function hideProgress() {
  el.progress.hidden = true;
  el.progressFill.style.transform = "scaleX(0)";
  S.percent = 0;
}

function renderPlay() {
  const inst = current();
  const play = el.play;
  let label = t("play.play");
  let sub = "";
  let disabled = false;
  let mode = "";

  if (S.viewError) {
    label = t("common.retry");
  } else if (!inst) {
    label = S.view ? t("play.choose") : t("play.wait");
    disabled = true;
  } else if (S.running) {
    label = S.running === inst.id ? t("play.running") : t("play.otherRunning");
    sub = t("play.subRunning");
    disabled = true;
    mode = "running";
  } else if (S.busy) {
    disabled = true;
    mode = "busy";
    if (S.busy === "repair") label = t("play.repairing");
    else if (S.phase === "downloading") label = t("play.downloading") + " " + Math.round(S.percent) + "%";
    else if (S.phase === "preparing") label = t("play.preparing");
    else if (S.phase === "launching") label = t("play.launching");
    else if (S.busy === "play") label = t("play.checking");
    else label = t("play.wait");
  } else if (S.updating) {
    label = t("play.wait");
    disabled = true;
  } else if (!activeProfile()) {
    label = t("play.addNick");
  } else if (!inst.installed) {
    label = t("play.install");
    mode = "install";
    const size = formatSize(inst.size_bytes);
    sub = size ? "~" + size : "";
  } else if (inst.update_available) {
    label = t("play.update");
    sub = t("play.subUpdate", { v: inst.remote_version });
  }

  play.disabled = disabled;
  play.classList.toggle("install", mode === "install");
  play.classList.toggle("busy", mode === "busy");
  play.classList.toggle("running", mode === "running");
  el.playLabel.textContent = label;
  el.playSub.textContent = sub;
  play.setAttribute("aria-busy", mode === "busy");

  const locked = !inst || Boolean(S.busy) || Boolean(S.running);
  el.repairBtn.disabled = locked || !inst?.installed;
  el.folderBtn.disabled = !inst?.installed;
}

function renderNickNote() {
  const profile = activeProfile();
  el.nickNote.innerHTML = profile ? t("home.playingAs", { nick: esc(profile.nickname) }) : t("home.noNick");
}

let heroImageToken = 0;

function renderHero() {
  const inst = current();
  if (!inst) {
    el.instName.textContent = S.viewError ? t("list.error") : S.view ? t("home.empty") : "";
    el.instName.classList.toggle("small", Boolean(S.viewError));
    el.instDesc.hidden = !S.viewError;
    el.instDesc.textContent = S.viewError || "";
    el.instMeta.textContent = "";
    el.badgeMc.hidden = true;
    el.badgeLoader.hidden = true;
    el.badgeServer.hidden = true;
    el.heroBg.classList.remove("ready");
    renderPlay();
    return;
  }
  el.instName.textContent = inst.name;
  el.instName.classList.remove("small");
  el.instDesc.textContent = inst.description || "";
  el.instDesc.hidden = !inst.description;

  el.badgeMc.hidden = !inst.minecraft_version;
  el.badgeMc.textContent = "Minecraft " + inst.minecraft_version;
  const loader = loaderName(inst.loader);
  el.badgeLoader.hidden = !loader || loader === "Vanilla";
  el.badgeLoader.textContent = loader + (inst.loader_version ? " " + inst.loader_version : "");

  const meta = [];
  if (inst.update_available) meta.push(t("home.updateMeta", { from: inst.local_version || "?", to: inst.remote_version }));
  else if (inst.installed) meta.push(t("home.installedMeta", { v: inst.local_version || "—" }));
  else meta.push(t("home.notInstalledMeta"));
  if (inst.size_bytes) meta.push(formatSize(inst.size_bytes));
  if (!inst.installed) meta.push(t("play.subFirst"));
  el.instMeta.textContent = meta.join(" · ");

  const token = ++heroImageToken;
  el.heroBg.classList.remove("ready");
  el.hero.classList.remove("has-image");
  if (inst.image) {
    const image = new Image();
    image.onload = () => {
      if (token !== heroImageToken) return;
      el.heroBg.style.backgroundImage = `url("${inst.image.replace(/["\\\n]/g, "")}")`;
      el.heroBg.classList.add("ready");
      el.hero.classList.add("has-image");
    };
    image.src = inst.image;
  }
  renderServer();
  renderPlay();
}

function statusTag(inst) {
  if (S.running === inst.id) return `<span class="tag run">${t("inst.running")}</span>`;
  if (inst.update_available) return `<span class="tag upd">${t("inst.update")}</span>`;
  if (inst.installed) return `<span class="tag ok">${t("inst.installed")}</span>`;
  return `<span class="tag">${t("inst.notInstalled")}</span>`;
}

function renderList() {
  const list = el.instList;
  if (S.viewError) {
    list.innerHTML = `<div class="empty"><span>${esc(t("list.error"))}</span><small>${esc(S.viewError)}</small><button class="btn btn-ghost btn-sm" type="button" data-act="reload">${t("list.retry")}</button></div>`;
    el.instCount.textContent = "0";
    return;
  }
  if (!S.view) {
    list.innerHTML = '<div class="skeleton"></div><div class="skeleton"></div><div class="skeleton"></div>';
    return;
  }
  const items = S.view.instances;
  el.instCount.textContent = String(items.length);
  if (!items.length) {
    list.innerHTML = `<div class="empty">${esc(t("list.empty"))}</div>`;
    return;
  }
  const intro = !list.querySelector(".inst-card");
  list.classList.toggle("intro", intro);
  list.innerHTML = items
    .map((inst, index) => {
      const on = inst.id === S.activeId;
      const details = [loaderName(inst.loader), inst.minecraft_version, formatSize(inst.size_bytes)].filter(Boolean).join(" · ");
      const icon = inst.image
        ? `<span class="inst-icon" style="background-image:url('${esc(inst.image.replace(/['"\\\n]/g, ""))}')"></span>`
        : `<span class="inst-icon">${esc(inst.name.slice(0, 1).toUpperCase())}</span>`;
      const more = inst.installed
        ? `<button class="more" type="button" data-more="${esc(inst.id)}" aria-label="${esc(t("inst.more"))}" title="${esc(t("inst.more"))}">${ICONS.more}</button>`
        : "";
      return `<div class="inst-card${on ? " on" : ""}" role="option" tabindex="0" aria-selected="${on}" data-id="${esc(inst.id)}" style="animation-delay:${Math.min(index, 8) * 30}ms">
        ${icon}
        <span class="inst-info"><b>${esc(inst.name)}</b><small>${esc(details)}</small></span>
        <span class="inst-side">${statusTag(inst)}${more}</span>
      </div>`;
    })
    .join("");
}

function renderServer() {
  const inst = current();
  if (!inst || !inst.server) {
    el.badgeServer.hidden = true;
    return;
  }
  const info = S.server.get(inst.server);
  el.badgeServer.hidden = false;
  el.badgeServer.classList.toggle("online", Boolean(info?.online));
  el.badgeServer.classList.toggle("offline", Boolean(info && !info.online));
  el.serverText.textContent = !info ? t("server.checking") : info.online ? t("server.online", { n: info.players, max: info.max_players }) : t("server.offline");
}

let serverTimer = 0;

async function pingServer() {
  const inst = current();
  if (!inst?.server || document.hidden) return;
  const address = inst.server;
  try {
    const info = await invoke("server_status", { address });
    S.server.set(address, info);
  } catch {
    S.server.set(address, { online: false });
  }
  if (current()?.server === address) renderServer();
}

function scheduleServerPing(now) {
  clearInterval(serverTimer);
  if (now) pingServer();
  serverTimer = setInterval(() => {
    if (S.tab === "home") pingServer();
  }, 60000);
}

let saveActiveTimer = 0;

async function selectInstance(id, { save = true } = {}) {
  if (!S.view?.instances.some((i) => i.id === id)) return;
  const changed = S.activeId !== id;
  S.activeId = id;
  renderList();
  if (changed || save === false) renderHero();
  if (save && changed) {
    clearTimeout(saveActiveTimer);
    saveActiveTimer = setTimeout(() => invoke("save_settings", { patch: { active_instance: id } }).catch(() => {}), 300);
  }
  scheduleServerPing(true);
  renderRamHint();
  try {
    const fresh = await invoke("instance_details", { instanceId: id });
    mergeInstance(fresh);
  } catch {
    /* без мережі показуємо те, що є */
  }
}

function mergeInstance(fresh) {
  if (!S.view) return;
  const index = S.view.instances.findIndex((i) => i.id === fresh.id);
  if (index < 0) return;
  S.view.instances[index] = fresh;
  renderList();
  if (fresh.id === S.activeId) renderHero();
}

async function loadProject({ quiet = false } = {}) {
  if (!quiet) {
    S.view = null;
    S.viewError = null;
    renderList();
    setIsland("busy", t("status.loading"));
  }
  try {
    const view = await invoke("load_project");
    S.view = view;
    S.viewError = null;
    S.activeId = view.active_instance;
    renderTop();
    renderList();
    renderHero();
    if (!S.busy && !S.running) idleIsland();
    selectInstance(S.activeId, { save: false });
  } catch (error) {
    S.view = null;
    S.viewError = errText(error);
    renderList();
    renderHero();
    setIsland("error", errText(error));
  }
}

function renderTop() {
  const name = S.view?.name || S.binding?.name || S.binding?.code || "—";
  el.topProject.textContent = name;
  el.topDot.classList.toggle("off", Boolean(S.view?.offline));
  el.brandLetter.textContent = name.trim().slice(0, 1).toUpperCase() || "M";
}

async function onPlay() {
  if (S.viewError) {
    loadProject();
    return;
  }
  const inst = current();
  if (!inst || S.busy || S.running || S.updating) return;
  if (!activeProfile()) {
    switchTab("profiles");
    el.newNick.focus();
    return;
  }
  S.busy = "play";
  S.phase = "checking";
  renderPlay();
  try {
    const result = await invoke("play", { instanceId: inst.id });
    if (result.offline) setIsland("warn", t("status.offlinePlay"));
    else if (result.first_install) setIsland("ok", t("status.installed"));
    const fresh = current();
    if (fresh) {
      fresh.installed = true;
      fresh.update_available = false;
      if (fresh.remote_version) fresh.local_version = fresh.remote_version;
    }
  } catch (error) {
    setIsland("error", errText(error));
    showError(error, [{ id: "retry", label: t("common.retry"), kind: "ghost", onClick: () => setTimeout(onPlay, 220) }]);
  } finally {
    S.busy = null;
    S.phase = null;
    hideProgress();
    renderList();
    renderHero();
  }
}

async function onRepair(id) {
  const inst = S.view?.instances.find((i) => i.id === (id || S.activeId));
  if (!inst || S.busy || S.running) return;
  const ok = await confirmDialog({ icon: "wrench", title: t("repair.title", { name: inst.name }), html: t("repair.text"), ok: t("repair.go") });
  if (!ok) return;
  if (inst.id !== S.activeId) await selectInstance(inst.id);
  S.busy = "repair";
  S.phase = "checking";
  renderPlay();
  try {
    const report = await invoke("repair_instance", { instanceId: inst.id });
    const nothing = report.downloaded === 0 && report.removed === 0;
    setIsland("ok", nothing ? t("repair.ok") : t("repair.fixed", { d: report.downloaded, r: report.removed }));
    inst.installed = true;
    inst.update_available = false;
    if (inst.remote_version) inst.local_version = inst.remote_version;
  } catch (error) {
    setIsland("error", errText(error));
    showError(error);
  } finally {
    S.busy = null;
    S.phase = null;
    hideProgress();
    renderList();
    renderHero();
  }
}

async function onDelete(id) {
  const inst = S.view?.instances.find((i) => i.id === id) || S.disk.find((d) => d.id === id);
  if (!inst || S.busy) return;
  const name = inst.name || id;
  const ok = await confirmDialog({ icon: "trash", tone: "danger", title: t("delete.title", { name }), html: t("delete.text"), ok: t("delete.go"), danger: true });
  if (!ok) return;
  try {
    await invoke("delete_instance", { instanceId: id });
    setIsland("ok", t("delete.done", { name }));
    const local = S.view?.instances.find((i) => i.id === id);
    if (local) {
      local.installed = false;
      local.update_available = false;
      local.local_version = "";
    }
    renderList();
    renderHero();
    if (S.tab === "settings") refreshSettings();
  } catch (error) {
    showError(error);
  }
}

async function openFolder(id) {
  try {
    await invoke("open_instance_folder", { instanceId: id || S.activeId });
  } catch (error) {
    setIsland("error", errText(error));
  }
}

/* меню картки */

function closeMenu() {
  if (el.instMenu.hidden) return;
  el.instMenu.hidden = true;
  S.menuFor = null;
}

function openMenu(button, id) {
  if (S.menuFor === id && !el.instMenu.hidden) {
    closeMenu();
    return;
  }
  S.menuFor = id;
  const rect = button.getBoundingClientRect();
  el.instMenu.hidden = false;
  const width = el.instMenu.offsetWidth;
  const height = el.instMenu.offsetHeight;
  const top = Math.min(rect.bottom + 6, window.innerHeight - height - 8);
  el.instMenu.style.left = Math.max(8, rect.right - width) + "px";
  el.instMenu.style.top = top + "px";
  const locked = Boolean(S.busy) || Boolean(S.running);
  el.instMenu.querySelector('[data-act="repair"]').disabled = locked;
  el.instMenu.querySelector('[data-act="delete"]').disabled = Boolean(S.busy) || S.running === id;
  el.instMenu.querySelector("button:not(:disabled)")?.focus();
}

/* ═══════════════════════════ ПРОФІЛІ ═══════════════════════════ */

function applyProfiles(view) {
  S.profiles = view.profiles;
  S.activeProfile = view.active_profile;
  renderHeads();
  renderNickNote();
  if (S.tab === "profiles") renderProfiles();
}

async function renderProfiles() {
  const profile = activeProfile();
  el.stageNick.textContent = profile ? profile.nickname : "—";
  el.skinUpload.disabled = !profile;
  el.skinReset.disabled = !profile?.has_skin;
  const skin = await loadSkin(profile);
  drawBody(el.skinBody, skin);

  el.profileList.classList.toggle("intro", !el.profileList.querySelector(".profile-row"));
  el.profileList.innerHTML = S.profiles
    .map((p, index) => {
      const on = p.id === S.activeProfile;
      return `<div class="profile-row${on ? " on" : ""}" data-profile="${esc(p.id)}" tabindex="0" role="button" aria-pressed="${on}" style="animation-delay:${Math.min(index, 8) * 30}ms">
        <canvas class="head" data-head="${esc(p.id)}" width="40" height="40"></canvas>
        <span><b>${esc(p.nickname)}</b><small>${on ? t("profiles.active") + " · " : ""}${p.has_skin ? t("profiles.ownSkin") : t("profiles.defaultSkin")}</small></span>
        <span class="row-actions">
          ${on ? "" : `<button class="btn btn-ghost btn-sm" type="button" data-act="select">${t("profiles.makeActive")}</button>`}
          <button class="icon-sm" type="button" data-act="skin" title="${esc(t("profiles.skinFor"))}" aria-label="${esc(t("profiles.skinFor"))}">${ICONS.shirt}</button>
          <button class="icon-sm danger" type="button" data-act="remove" title="${esc(t("profiles.remove"))}" aria-label="${esc(t("profiles.remove"))}">${ICONS.trash}</button>
        </span>
      </div>`;
    })
    .join("");

  for (const canvas of el.profileList.querySelectorAll("canvas[data-head]")) {
    const p = S.profiles.find((x) => x.id === canvas.dataset.head);
    loadSkin(p).then((img) => drawHead(canvas, img, p?.nickname, 40));
  }
}

let skinTarget = null;

function pickSkin(profileId) {
  skinTarget = profileId;
  el.skinFile.value = "";
  el.skinFile.click();
}

async function onSkinFile() {
  const file = el.skinFile.files?.[0];
  const profileId = skinTarget;
  if (!file || !profileId) return;
  if (file.size > MB) {
    setIsland("error", t("profiles.skinBig"));
    return;
  }
  try {
    const buffer = await file.arrayBuffer();
    const bitmap = await createImageBitmap(new Blob([buffer], { type: "image/png" }));
    const okSize = (bitmap.width === 64 || bitmap.width === 128) && (bitmap.height === bitmap.width || bitmap.height === bitmap.width / 2);
    if (!okSize) throw new Error(t("profiles.skinBad"));
    const view = await invoke("save_skin", { profileId, bytes: Array.from(new Uint8Array(buffer)) });
    S.skins.set(profileId, bitmap);
    applyProfiles(view);
    renderProfiles();
    setIsland("ok", t("profiles.skinSaved"));
  } catch (error) {
    setIsland("error", errText(error).includes("decode") ? t("profiles.skinBad") : errText(error));
  }
}

async function onProfileAction(row, action) {
  const id = row.dataset.profile;
  const profile = S.profiles.find((p) => p.id === id);
  if (!profile) return;
  try {
    if (action === "select" || !action) {
      if (id !== S.activeProfile) applyProfiles(await invoke("set_active_profile", { profileId: id }));
    } else if (action === "skin") {
      pickSkin(id);
    } else if (action === "remove") {
      if (S.profiles.length <= 1) {
        setIsland("warn", t("profiles.lastOne"));
        return;
      }
      const ok = await confirmDialog({ icon: "trash", tone: "danger", title: t("profiles.removeTitle", { nick: profile.nickname }), html: t("profiles.removeText"), ok: t("delete.go"), danger: true });
      if (!ok) return;
      S.skins.delete(id);
      applyProfiles(await invoke("remove_profile", { profileId: id }));
    }
  } catch (error) {
    setIsland("error", errText(error));
  }
}

function nickState(input, hint, defaultKey) {
  const value = input.value.trim();
  const invalid = value.length > 0 && !NICK_RE.test(value);
  input.classList.toggle("invalid", invalid);
  hint.classList.toggle("error", invalid);
  hint.textContent = invalid ? t("profiles.nickBad") : t(defaultKey);
  return !invalid && value.length > 0;
}

async function onAddProfile(event) {
  event.preventDefault();
  const nickname = el.newNick.value.trim();
  if (!NICK_RE.test(nickname)) {
    el.newNick.classList.add("invalid");
    el.newNickHint.classList.add("error");
    el.newNickHint.textContent = t("profiles.nickBad");
    el.newNick.focus();
    return;
  }
  try {
    applyProfiles(await invoke("add_profile", { nickname }));
    el.newNick.value = "";
    nickState(el.newNick, el.newNickHint, "profiles.nickRule");
    setIsland("ok", t("profiles.added", { nick: nickname }));
  } catch (error) {
    el.newNick.classList.add("invalid");
    el.newNickHint.classList.add("error");
    el.newNickHint.textContent = errText(error);
  }
}

/* ═══════════════════════════ ПРОЄКТ ═══════════════════════════ */

function renderProject() {
  const name = S.view?.name || S.binding?.name || "—";
  el.projName.textContent = name;
  el.projBrand.textContent = name.trim().slice(0, 1).toUpperCase() || "M";
  el.projCode.textContent = S.binding?.code || "—";
  el.projCount.textContent = S.view ? plural(S.view.instances.length, "plural.packs") : "";
  el.projState.textContent = S.view ? (S.view.offline ? t("project.offline") : t("project.online")) : S.viewError || "";
}

function setLoading(button, on) {
  button.classList.toggle("loading", on);
  button.disabled = on;
}

async function onSwitchProject(event) {
  event.preventDefault();
  const code = el.switchCode.value.trim();
  el.switchError.hidden = true;
  if (!code) {
    el.switchCode.classList.add("invalid");
    el.switchCode.focus();
    return;
  }
  setLoading(el.switchSubmit, true);
  try {
    S.binding = await invoke("connect_project", { code });
    el.switchCode.value = "";
    await loadProject();
    renderProject();
    setIsland("ok", t("project.switched", { name: S.binding.name || code }));
  } catch (error) {
    el.switchError.textContent = errText(error);
    el.switchError.hidden = false;
  } finally {
    setLoading(el.switchSubmit, false);
  }
}

async function onLeave() {
  const name = S.view?.name || S.binding?.name || S.binding?.code || "";
  const ok = await confirmDialog({ icon: "logout", tone: "warn", title: t("project.leaveConfirm", { name }), html: t("project.leaveText"), ok: t("project.leave"), danger: true });
  if (!ok) return;
  try {
    await invoke("leave_project");
    S.binding = null;
    S.view = null;
    showOnboarding();
  } catch (error) {
    showError(error);
  }
}

/* ═══════════════════════════ НАЛАШТУВАННЯ ═══════════════════════════ */

function recommendedRam() {
  const inst = current();
  const base = inst?.recommended_ram_mb || 4096;
  return Math.min(Math.max(1024, base), S.maxRam);
}

function renderRamHint() {
  const min = Number(el.ram.min);
  const max = Number(el.ram.max);
  const value = Number(el.ram.value);
  const rec = recommendedRam();
  const highFrom = Math.max(rec, Math.round(S.totalRam * 0.75));
  el.ram.style.setProperty("--fill", ((value - min) / Math.max(1, max - min)) * 100 + "%");
  el.ram.style.setProperty("--rec", Math.min(100, ((highFrom - min) / Math.max(1, max - min)) * 100) + "%");
  el.ramValue.textContent = formatRam(value);
  el.ramMin.textContent = formatRam(min);
  el.ramMax.textContent = formatRam(max);
  el.ramHint.textContent = t("settings.ramHint", { total: formatRam(Math.round(S.totalRam / 256) * 256), rec: formatRam(rec) });
  let warn = "";
  if (value > highFrom) warn = t("settings.ramHigh");
  else if (value < rec) warn = t("settings.ramLow");
  el.ramWarn.hidden = !warn;
  el.ramWarn.textContent = warn;
  el.ramRecommended.hidden = value === rec;
}

let ramSaveTimer = 0;

function saveRam() {
  clearTimeout(ramSaveTimer);
  ramSaveTimer = setTimeout(() => {
    S.settings.ram_mb = Number(el.ram.value);
    invoke("save_settings", { patch: { ram_mb: S.settings.ram_mb } }).catch((e) => setIsland("error", errText(e)));
  }, 250);
}

function renderPreset() {
  for (const button of el.presetSeg.querySelectorAll("[data-preset]")) {
    const on = button.dataset.preset === S.settings.graphics_preset;
    button.classList.toggle("on", on);
    button.setAttribute("aria-checked", on);
  }
  const inst = current();
  el.applyPreset.disabled = !inst?.installed || Boolean(S.busy) || S.running === inst?.id;
  el.applyPreset.textContent = inst?.installed ? t("settings.applyPreset", { name: inst.name }) : t("settings.applyPresetNone");
}

async function refreshSettings() {
  renderRamHint();
  renderPreset();
  el.gameLang.value = S.settings.game_lang;
  el.dataDir.textContent = S.settings.data_dir;
  el.resetDir.hidden = S.settings.data_dir === S.settings.default_data_dir;
  el.aboutText.textContent = t("settings.about", { v: S.launcherVersion });
  try {
    S.disk = await invoke("list_installed");
  } catch {
    S.disk = [];
  }
  renderDisk();
}

function renderDisk() {
  const total = S.disk.reduce((sum, d) => sum + d.size_bytes, 0);
  el.installedTotal.textContent = S.disk.length ? t("settings.installedTotal", { size: formatSize(total) }) : "";
  el.deleteAll.hidden = S.disk.length === 0;
  el.deleteAll.disabled = Boolean(S.busy) || Boolean(S.running);
  if (!S.disk.length) {
    el.diskList.innerHTML = `<p class="hint">${esc(t("settings.installedEmpty"))}</p>`;
    return;
  }
  el.diskList.innerHTML = S.disk
    .map((d) => {
      const notes = [formatSize(d.size_bytes)];
      if (d.pack_version) notes.push("v" + d.pack_version);
      if (!d.installed) notes.push(t("settings.partial"));
      if (d.project && S.binding && d.project.toLowerCase() !== S.binding.code.toLowerCase()) notes.push(t("settings.fromProject", { code: d.project }));
      return `<div class="disk-row" data-disk="${esc(d.id)}">
        <span><b>${esc(d.name)}</b><small>${esc(notes.join(" · "))}</small></span>
        <button class="icon-sm" type="button" data-act="folder" title="${esc(t("settings.open"))}" aria-label="${esc(t("settings.open"))}">${ICONS.folder}</button>
        <button class="icon-sm danger" type="button" data-act="delete" title="${esc(t("delete.go"))}" aria-label="${esc(t("delete.go"))}" ${S.running === d.id || S.busy ? "disabled" : ""}>${ICONS.trash}</button>
      </div>`;
    })
    .join("");
}

async function onDeleteAll() {
  if (!S.disk.length || S.busy || S.running) return;
  const total = S.disk.reduce((sum, d) => sum + d.size_bytes, 0);
  const ok = await confirmDialog({
    icon: "trash",
    tone: "danger",
    title: t("deleteAll.title"),
    html: t("deleteAll.text", { n: `<b>${plural(S.disk.length, "plural.packs")}</b>`, size: `<b>${formatSize(total)}</b>` }),
    ok: t("deleteAll.go"),
    danger: true
  });
  if (!ok) return;
  try {
    await invoke("delete_all_instances");
    setIsland("ok", t("deleteAll.done"));
    for (const inst of S.view?.instances || []) {
      inst.installed = false;
      inst.update_available = false;
      inst.local_version = "";
    }
    renderList();
    renderHero();
    refreshSettings();
  } catch (error) {
    showError(error);
  }
}

async function changeDataDir(path) {
  const moveChoice = await openModal({
    icon: "folder",
    title: t("dir.moveTitle"),
    html: t("dir.moveText", { path: esc(path) }),
    buttons: [
      { id: "cancel", label: t("common.cancel"), kind: "ghost", left: true },
      { id: "fresh", label: t("dir.noMove"), kind: "ghost" },
      { id: "move", label: t("dir.move"), kind: "primary" }
    ]
  });
  if (!moveChoice || moveChoice === "cancel") return;
  S.busy = "dir";
  renderPlay();
  setIsland("busy", t("play.wait"));
  try {
    const result = await invoke("set_data_dir", { path, moveFiles: moveChoice === "move" });
    S.settings.data_dir = result;
    setIsland("ok", t("dir.done"));
    await loadProject({ quiet: true });
  } catch (error) {
    showError(error);
  } finally {
    S.busy = null;
    renderPlay();
    refreshSettings();
  }
}

async function onChangeDir() {
  if (S.busy || S.running) return;
  try {
    const picked = await invoke("pick_folder");
    if (picked) await changeDataDir(picked);
  } catch (error) {
    showError(error);
  }
}

/* ═══════════════════════════ ПЕРШИЙ ВХІД ═══════════════════════════ */

function showOnboarding() {
  closeMenu();
  clearTimeout(islandTimer);
  clearInterval(serverTimer);
  el.app.hidden = true;
  el.onboarding.hidden = false;
  const needNick = S.profiles.length === 0;
  el.obNickField.hidden = !needNick;
  const title = el.onboarding.querySelector("h1");
  title.dataset.i18n = needNick ? "ob.title" : "ob.titleBack";
  $("obSub").dataset.i18n = needNick ? "ob.sub" : "ob.subBack";
  applyI18n();
  el.obError.hidden = true;
  el.boot.classList.add("gone");
  setTimeout(() => el.obCode.focus(), 50);
}

async function onOnboardingSubmit(event) {
  event.preventDefault();
  el.obError.hidden = true;
  const code = el.obCode.value.trim();
  const needNick = !el.obNickField.hidden;
  const nickname = el.obNick.value.trim();
  if (!code) {
    el.obCode.classList.add("invalid");
    el.obError.textContent = t("ob.needCode");
    el.obError.hidden = false;
    el.obCode.focus();
    return;
  }
  if (needNick && !NICK_RE.test(nickname)) {
    el.obNick.classList.add("invalid");
    el.obNickHint.classList.add("error");
    el.obNickHint.textContent = t("profiles.nickBad");
    el.obNick.focus();
    return;
  }
  setLoading(el.obSubmit, true);
  el.obSubmit.querySelector(".btn-text").textContent = t("ob.checking");
  try {
    S.binding = await invoke("connect_project", { code });
    if (needNick) applyProfiles(await invoke("add_profile", { nickname }));
    showApp();
  } catch (error) {
    el.obError.textContent = errText(error);
    el.obError.hidden = false;
  } finally {
    setLoading(el.obSubmit, false);
    el.obSubmit.querySelector(".btn-text").textContent = t("ob.submit");
  }
}

function showApp() {
  el.onboarding.hidden = true;
  el.app.hidden = false;
  el.boot.classList.add("gone");
  renderTop();
  renderHeads();
  renderNickNote();
  requestAnimationFrame(() => switchTab("home", false));
  loadProject();
}

/* ═══════════════════════════ МОВА ═══════════════════════════ */

function setLang(next) {
  if (!DICT[next] || next === lang) return;
  lang = next;
  if (S.settings) S.settings.ui_lang = next;
  applyI18n();
  renderAll();
  invoke("save_settings", { patch: { ui_lang: next } }).catch(() => {});
}

function renderAll() {
  if (el.app.hidden) return;
  renderTop();
  renderNickNote();
  renderList();
  renderHero();
  if (S.tab === "settings") refreshSettings();
  if (S.tab === "profiles") renderProfiles();
  if (S.tab === "project") renderProject();
  idleIsland();
  const active = el.nav.querySelector(".nav-item.on");
  requestAnimationFrame(() => moveNavIndicator(active, false));
}

/* ═══════════════════════════ ПОДІЇ ═══════════════════════════ */

async function registerListeners() {
  await listen("launcher-status", (event) => {
    if (S.busy || S.updating) setIsland("busy", String(event.payload));
  });
  await listen("sync-progress", (event) => setProgress(event.payload));
  await listen("play-phase", (event) => {
    S.phase = String(event.payload);
    if (S.phase === "launching") hideProgress();
    renderPlay();
  });
  await listen("game-state", (event) => {
    const payload = event.payload;
    if (payload.state === "running") {
      S.running = payload.instance || S.activeId;
      setIsland("ok", t("status.gameRunning"), { sticky: true });
    } else {
      S.running = null;
      if (payload.crashed) {
        setIsland("error", t("crash.title"));
        showCrash(payload);
      } else setIsland("ok", t("status.gameClosed"));
    }
    updateCalm();
    renderList();
    renderPlay();
    if (S.tab === "settings") renderDisk();
  });
}

function showCrash(payload) {
  const id = payload.instance || S.activeId;
  const hintKey = payload.hint ? "crash." + payload.hint : "crash.generic";
  const hint = DICT[lang][hintKey] ? t(hintKey) : t("crash.generic");
  const showRam = payload.hint === "memory_low" || payload.hint === "memory_too_high";
  openModal({
    icon: "alert",
    tone: "danger",
    title: t("crash.title"),
    html: `<p>${t("crash.lead")}</p><p>${hint}</p>`,
    buttons: [
      {
        id: "copy",
        label: t("crash.copy"),
        kind: "ghost",
        wide: true,
        onClick: async (button) => {
          try {
            const report = await invoke("crash_report", { instanceId: id });
            if (await copyText(report)) button.textContent = t("crash.copied");
          } catch (error) {
            button.textContent = errText(error);
          }
          return false;
        }
      },
      { id: "folder", label: t("crash.folder"), kind: "ghost", onClick: () => (openFolder(id), false) },
      showRam
        ? { id: "ram", label: t("nav.settings"), kind: "primary", onClick: () => setTimeout(() => switchTab("settings"), 200) }
        : { id: "repair", label: t("crash.repair"), kind: "primary", onClick: () => setTimeout(() => onRepair(id), 220) }
    ]
  });
}

/* обвідка за курсором (як у Ranking): один rAF на кадр, passive-слухачі */
let tracked = null;
let center = null;
let angle = 0;
let angleFrame = 0;
let pointerX = 0;
let pointerY = 0;

function nextAngle(x, y) {
  const fresh = (Math.atan2(x - center.x, center.y - y) * 180) / Math.PI;
  let delta = (fresh - angle) % 360;
  if (delta > 180) delta -= 360;
  else if (delta < -180) delta += 360;
  angle += delta;
  return angle;
}

function paintAngle() {
  angleFrame = 0;
  if (tracked) tracked.style.setProperty("--kat", nextAngle(pointerX, pointerY) + "deg");
}

document.addEventListener(
  "pointerover",
  (event) => {
    const target = event.target.closest?.(".nav-item, .icon-btn");
    if (!target || target === tracked || event.pointerType === "touch") return;
    tracked = target;
    const rect = target.getBoundingClientRect();
    center = { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
    angle = 0;
    target.style.setProperty("--kat", nextAngle(event.clientX, event.clientY) + "deg");
  },
  { passive: true }
);

document.addEventListener(
  "pointermove",
  (event) => {
    if (!tracked) return;
    if (!tracked.contains(event.target)) {
      tracked = null;
      return;
    }
    pointerX = event.clientX;
    pointerY = event.clientY;
    if (!angleFrame) angleFrame = requestAnimationFrame(paintAngle);
  },
  { passive: true }
);

function bindEvents() {
  document.addEventListener("click", (event) => {
    const goto = event.target.closest("[data-goto]");
    if (goto && !el.app.hidden) {
      switchTab(goto.dataset.goto);
      return;
    }
    const tab = event.target.closest("[data-tab]");
    if (tab) {
      switchTab(tab.dataset.tab);
      return;
    }
    if (!el.instMenu.hidden && !event.target.closest("#instMenu") && !event.target.closest("[data-more]")) closeMenu();
  });

  el.instList.addEventListener("click", (event) => {
    const more = event.target.closest("[data-more]");
    if (more) {
      event.stopPropagation();
      openMenu(more, more.dataset.more);
      return;
    }
    if (event.target.closest('[data-act="reload"]')) {
      loadProject();
      return;
    }
    const card = event.target.closest(".inst-card");
    if (card) selectInstance(card.dataset.id);
  });

  el.instList.addEventListener("keydown", (event) => {
    const card = event.target.closest(".inst-card");
    if (!card) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectInstance(card.dataset.id);
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const sibling = event.key === "ArrowDown" ? card.nextElementSibling : card.previousElementSibling;
      sibling?.focus();
    }
  });

  el.instMenu.addEventListener("click", (event) => {
    const button = event.target.closest("[data-act]");
    if (!button || button.disabled) return;
    const id = S.menuFor;
    closeMenu();
    if (button.dataset.act === "folder") openFolder(id);
    else if (button.dataset.act === "repair") onRepair(id);
    else if (button.dataset.act === "delete") onDelete(id);
  });

  el.play.addEventListener("click", onPlay);
  el.repairBtn.addEventListener("click", () => onRepair());
  el.folderBtn.addEventListener("click", () => openFolder());

  el.obForm.addEventListener("submit", onOnboardingSubmit);
  el.obCode.addEventListener("input", () => {
    el.obCode.classList.remove("invalid");
    el.obError.hidden = true;
  });
  el.obNick.addEventListener("input", () => nickState(el.obNick, el.obNickHint, "ob.nickHint"));
  el.obLang.addEventListener("click", (event) => {
    const button = event.target.closest("[data-lang]");
    if (button) setLang(button.dataset.lang);
  });

  el.addProfileForm.addEventListener("submit", onAddProfile);
  el.newNick.addEventListener("input", () => nickState(el.newNick, el.newNickHint, "profiles.nickRule"));
  el.profileList.addEventListener("click", (event) => {
    const row = event.target.closest(".profile-row");
    if (!row) return;
    const action = event.target.closest("[data-act]")?.dataset.act;
    onProfileAction(row, action);
  });
  el.profileList.addEventListener("keydown", (event) => {
    const row = event.target.closest(".profile-row");
    if (row && event.target === row && (event.key === "Enter" || event.key === " ")) {
      event.preventDefault();
      onProfileAction(row, "select");
    }
  });
  el.skinUpload.addEventListener("click", () => S.activeProfile && pickSkin(S.activeProfile));
  el.skinReset.addEventListener("click", async () => {
    const id = S.activeProfile;
    if (!id) return;
    try {
      S.skins.delete(id);
      applyProfiles(await invoke("clear_skin", { profileId: id }));
      renderProfiles();
    } catch (error) {
      setIsland("error", errText(error));
    }
  });
  el.skinFile.addEventListener("change", onSkinFile);

  el.switchForm.addEventListener("submit", onSwitchProject);
  el.switchCode.addEventListener("input", () => {
    el.switchCode.classList.remove("invalid");
    el.switchError.hidden = true;
  });
  el.leaveBtn.addEventListener("click", onLeave);
  el.projRefresh.addEventListener("click", async () => {
    setLoading(el.projRefresh, true);
    await loadProject();
    renderProject();
    setLoading(el.projRefresh, false);
  });

  el.ram.addEventListener("input", renderRamHint);
  el.ram.addEventListener("change", saveRam);
  el.ramRecommended.addEventListener("click", () => {
    el.ram.value = String(recommendedRam());
    renderRamHint();
    saveRam();
  });
  el.presetSeg.addEventListener("click", (event) => {
    const button = event.target.closest("[data-preset]");
    if (!button || button.dataset.preset === S.settings.graphics_preset) return;
    S.settings.graphics_preset = button.dataset.preset;
    renderPreset();
    invoke("save_settings", { patch: { graphics_preset: S.settings.graphics_preset } }).catch((e) => setIsland("error", errText(e)));
  });
  el.applyPreset.addEventListener("click", async () => {
    const inst = current();
    if (!inst?.installed) return;
    const preset = t("preset." + S.settings.graphics_preset);
    const ok = await confirmDialog({ icon: "palette", title: t("preset.title", { preset, name: inst.name }), html: t("preset.text"), ok: t("preset.apply") });
    if (!ok) return;
    try {
      await invoke("apply_graphics_preset", { instanceId: inst.id });
      setIsland("ok", t("preset.done", { preset }));
    } catch (error) {
      showError(error);
    }
  });
  el.uiLangSeg.addEventListener("click", (event) => {
    const button = event.target.closest("[data-lang]");
    if (button) setLang(button.dataset.lang);
  });
  el.gameLang.addEventListener("change", () => {
    S.settings.game_lang = el.gameLang.value;
    invoke("save_settings", { patch: { game_lang: S.settings.game_lang } }).catch((e) => setIsland("error", errText(e)));
  });
  el.changeDir.addEventListener("click", onChangeDir);
  el.openDir.addEventListener("click", () => invoke("open_data_folder").catch((e) => setIsland("error", errText(e))));
  el.resetDir.addEventListener("click", () => changeDataDir(S.settings.default_data_dir));
  el.diskList.addEventListener("click", (event) => {
    const row = event.target.closest("[data-disk]");
    const action = event.target.closest("[data-act]")?.dataset.act;
    if (!row || !action) return;
    if (action === "delete") onDelete(row.dataset.disk);
    else if (action === "folder") openFolder(row.dataset.disk);
  });
  el.deleteAll.addEventListener("click", onDeleteAll);

  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (!el.modal.hidden) closeModal(null);
    else closeMenu();
  });
  window.addEventListener("resize", () => {
    closeMenu();
    const active = el.nav.querySelector(".nav-item.on");
    requestAnimationFrame(() => moveNavIndicator(active, false));
  });
  el.instList.addEventListener("scroll", closeMenu, { passive: true });
  document.addEventListener("visibilitychange", () => {
    updateCalm();
    if (!document.hidden && S.tab === "home") pingServer();
  });
}

/* ═══════════════════════════ СТАРТ ═══════════════════════════ */

async function checkUpdate() {
  S.updating = true;
  renderPlay();
  let result = null;
  try {
    result = await invoke("check_launcher_update");
    if (result.restarting) {
      el.bootText.textContent = t("status.restarting");
      el.boot.classList.remove("gone");
      return;
    }
    if (result.error && !el.app.hidden && !S.busy) setIsland("warn", t("status.updateFailed"));
  } catch {
    /* оновлення — не критично */
  }
  S.updating = false;
  renderPlay();
  if (!el.app.hidden && S.view && !S.busy && !S.running && !result?.error) idleIsland();
}

async function init() {
  bindEvents();
  await registerListeners();
  const info = await invoke("get_init_info");
  S.launcherVersion = info.launcher_version;
  S.totalRam = info.total_ram_mb;
  S.maxRam = info.max_ram_mb;
  S.settings = info.settings;
  S.profiles = info.profiles;
  S.activeProfile = info.active_profile;
  S.binding = info.project;
  S.activeId = info.active_instance;
  S.running = info.game_running;
  lang = info.settings.ui_lang === "en" ? "en" : "uk";

  el.ram.min = "1024";
  el.ram.max = String(Math.max(1280, info.max_ram_mb));
  el.ram.step = "256";
  el.ram.value = String(info.settings.ram_mb);
  applyI18n();
  updateCalm();

  if (!info.project || info.profiles.length === 0) showOnboarding();
  else showApp();
  checkUpdate();
}

init().catch((error) => {
  el.boot.classList.add("gone");
  el.app.hidden = false;
  setIsland("error", errText(error));
});
