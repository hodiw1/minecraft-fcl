"use strict";

const tauri = window.__TAURI__;
const invoke = tauri.core.invoke;
const listen = tauri.event.listen;

const NICK_RE = /^[A-Za-z0-9_]{3,16}$/;
/** @username акаунта Terrapolis (збігається з `config::validate_username`). */
const USER_RE = /^[A-Za-z0-9_]{3,20}$/;
const MB = 1024 * 1024;
const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");
/** CurseForge у каталозі контенту (потрібен API-ключ гравця). Має збігатися з
 *  `ENABLE_CURSEFORGE` у src-tauri/src/constants.rs. */
const ENABLE_CURSEFORGE = true;

/* ═══════════════════════════ ПОВЕДІНКА ВІКНА ═══════════════════════════ */

/** Типи <input>, у яких немає тексту для вставки — там меню ПКМ не потрібне. */
const NON_TEXT_INPUTS = new Set(["range", "checkbox", "radio", "file", "button", "submit", "reset", "image", "color", "hidden"]);

/** Текстове поле, де гравцеві зручно вставити код проєкту чи нікнейм через ПКМ. */
function isTextField(node) {
  if (!(node instanceof Element)) return false;
  if (node.closest("textarea")) return true;
  const input = node.closest("input");
  return Boolean(input) && !NON_TEXT_INPUTS.has((input.getAttribute("type") || "text").toLowerCase());
}

// Лаунчер — десктопний застосунок, а не сторінка: стандартне меню WebView2 («Назад»,
// «Перезавантажити», «Перевірити елемент») не показуємо ніде, крім текстових полів.
document.addEventListener(
  "contextmenu",
  (event) => {
    if (!isTextField(event.target)) event.preventDefault();
  },
  { capture: true }
);

/* ═══════════════════════════ ПЕРЕКЛАДИ ═══════════════════════════ */

const DICT = {
  uk: {
    "boot.loading": "Завантаження…",
    "boot.updating": "Оновлення лаунчера…",
    "ob.title": "Ласкаво просимо!",
    "ob.sub1": "Terrapolis — легкий і швидкий лаунчер Minecraft. Створи акаунт і обери, яким буде лаунчер.",
    "ob.next": "Далі",
    "ob.title2": "Проєкти спільноти",
    "ob.sub2": "Підключись до проєкту — і його збірки з'являться в лаунчері. Або пропусти цей крок: власні збірки можна створювати й запускати одразу.",
    "ob.back": "Назад",
    "ob.skip": "Пропустити",
    "ob.pick": "Доступні проєкти",
    "ob.loadingProjects": "Шукаємо проєкти…",
    "ob.noProjects": "Публічних проєктів поки не знайдено. Якщо маєш код — введи його нижче.",
    "ob.code": "Або введи код проєкту чи owner/repo",
    "ob.codePh": "Код проєкту або owner/repo",
    "ob.nick": "Перший ігровий нік",
    "ob.nickPh": "Введіть ігровий нікнейм",
    "ob.nickHint": "Під цим ім'ям тебе бачитимуть у грі (за замовчуванням — як @username). 3–16 символів",
    "ob.submit": "Підключитися",
    "ob.checking": "Підключаємося…",
    "ob.needCode": "Обери проєкт або введи код — чи натисни «Пропустити»",
    "theme.title": "Кольорова палітра",
    "theme.hint": "Акцентний колір кнопок, перемикачів і виділення. Змінюється миттєво.",
    "theme.previewChip": "Fabric 1.21.1",
    "theme.emerald": "Смарагдовий",
    "theme.cobalt": "Кобальтовий",
    "theme.amber": "Бурштиновий",
    "theme.ruby": "Рубіновий",
    "theme.amethyst": "Аметистовий",
    "theme.graphite": "Графіт",
    "top.noProject": "Без проєкту — лише власні збірки",
    "home.connectTitle": "Підключити проєкт",
    "home.connectText": "Збірки спільноти — за кодом проєкту або owner/repo",
    "home.browsePacks": "Готові збірки з Modrinth і CurseForge",
    "project.none": "Проєкт не підключено",
    "project.noneSub": "Лаунчер повністю працює й без проєкту: створюй власні збірки або встановлюй готові з Modrinth і CurseForge.",
    "project.connectTitle": "Підключити проєкт",
    "project.connectHint": "Обери проєкт зі списку або введи код від адміністратора чи репозиторій GitHub у форматі owner/repo.",
    "project.left": "Ти вийшов з проєкту — власні збірки залишились на місці",
    "create.loader": "Завантажувач",
    "create.browse": "Готова збірка…",
    "content.title": "Контент — {name}",
    "content.mods": "Моди",
    "content.resourcepacks": "Ресурс-паки",
    "content.shaders": "Шейдери",
    "content.modpacks": "Готові збірки",
    "content.browse": "Знайти",
    "content.leadResourcepacks": "Ресурс-паки для Minecraft {mc}. Увімкнути встановлений пак можна в грі: Налаштування → Пакети ресурсів.",
    "content.leadShaders": "Шейдерпаки для Minecraft {mc}. Обрати шейдер можна в грі: Налаштування → Відео → Шейдери.",
    "content.leadModpacks": "Готові збірки спільноти: лаунчер створить нову окрему збірку з потрібною версією Minecraft і завантажувачем та завантажить усі її файли.",
    "content.searchPhRp": "Пошук ресурс-паків: Faithful, Fresh Animations…",
    "content.searchPhSh": "Пошук шейдерів: Complementary, BSL, Sildur's…",
    "content.searchPhMp": "Пошук збірок: Fabulously Optimized, Cobblemon…",
    "content.popular": "Популярне для цієї версії",
    "content.popularPacks": "Популярні готові збірки",
    "content.none": "Тут поки порожньо. Перейди у «Знайти», щоб додати перше.",
    "content.folderRp": "Папка resourcepacks",
    "content.folderSh": "Папка shaderpacks",
    "content.enable": "Увімкнути або вимкнути",
    "content.remove": "Видалити",
    "content.removed": "«{name}» видалено",
    "content.shaderNeed": "Шейдерам потрібен мод Iris або Oculus — у цій збірці його ще немає.",
    "content.shaderInstall": "Встановити {mod}",
    "content.shaderVanilla": "Шейдерам потрібен мод Iris або Oculus, а у ванільній збірці моди не працюють. Створи збірку на Fabric, NeoForge, Forge або Quilt.",
    "content.shaderModDone": "{mod} встановлено — шейдер обирається в грі: Налаштування → Відео → Шейдери",
    "content.modsVanilla": "Моди потребують завантажувача. Ресурс-паки працюють і у ванільній збірці — вкладка «Ресурс-паки».",
    "mp.installing": "Встановлення збірки…",
    "mp.progress": "Встановлення «{name}» — {p}%",
    "mp.doneTitle": "Збірку «{name}» встановлено",
    "mp.doneText": "Створено окрему збірку: Minecraft {mc} · {loader}. Натисни «Грати» — лаунчер докачає гру та завантажувач.",
    "mp.files": "Завантажено файлів",
    "mp.overrides": "З overrides",
    "mp.skipped": "Пропущено",
    "mp.missing": "Автори цих файлів заборонили завантаження стороннім лаунчерам — завантаж їх вручну з CurseForge у відповідну папку збірки:",
    "mp.mc": "Minecraft {v}",
    "mp.latest": "Найновіша версія",
    "danger.title": "Danger Zone",
    "danger.resetTitle": "Видалити все, окрім акаунту та проєктів",
    "danger.resetSub": "Збірки, Java, бібліотеки й ресурси гри, кеш завантажень і технічні налаштування. Акаунт @username, ігрові ніки, скіни та проєкти залишаться.",
    "danger.resetGo": "Очистити…",
    "danger.resetConfirmTitle": "Очистити дані лаунчера?",
    "danger.resetConfirmText": "<p>Буде видалено: <b>усі збірки</b> разом зі світами та скріншотами, Java, бібліотеки й ресурси гри, кеш завантажень. Технічні налаштування (ОЗУ, графіка, мова гри, папка даних, ключ CurseForge) повернуться до стандартних.</p><p><b>Залишаться:</b> нікнейми, скіни, підключений проєкт, мова інтерфейсу та палітра.</p>",
    "danger.resetting": "Очищення даних лаунчера…",
    "danger.resetDone": "Дані лаунчера очищено",
    "danger.uninstallTitle": "Видалити лаунчер повністю",
    "danger.uninstallSub": "Усі збірки, ігрові ніки, налаштування, кеш WebView2 і сам лаунчер. Після цього програма закриється. Збірки, якими ти поділився з друзями, залишаться в хмарі — спершу зроби їх приватними.",
    "danger.uninstallGo": "Видалити лаунчер…",
    "danger.uninstallConfirmTitle": "Видалити Terrapolis повністю?",
    "danger.uninstallConfirmText": "<p>Буде безповоротно видалено <b>усе</b>: збірки, світи, скріншоти, профілі, скіни, налаштування, кеш WebView2 і сам лаунчер. Скасувати це не можна.</p><p>Після видалення програма закриється.</p>",
    "danger.uninstalling": "Видалення лаунчера…",
    "danger.bye": "Лаунчер видалено. До зустрічі!",
    "danger.typeHint": "Щоб підтвердити, введи <b>{word}</b>",
    "danger.word": "ВИДАЛИТИ",
    "nav.home": "Збірки",
    "nav.profiles": "Акаунт",
    "nav.project": "Проєкт",
    "nav.changelog": "Changelog",
    "cl.read": "Читати повністю",
    "cl.github": "Читати на GitHub",
    "cl.noBody": "Опису немає.",
    "nav.settings": "Налаштування",
    "top.changeProfile": "Акаунт та ігрові ніки",
    "top.sync": "Синхронізувати: перевірити оновлення лаунчера та збірок",
    "sync.title": "Синхронізація",
    "sync.running": "Синхронізація: перевірка оновлень лаунчера та збірок…",
    "sync.found": "Знайдено {list}",
    "sync.launcher": "оновлення лаунчера v{v}",
    "sync.and": " та ",
    "sync.upToDate": "Усе актуально: оновлень не знайдено",
    "sync.newList": "Нові збірки: {names}",
    "sync.updList": "Є оновлення: {names}",
    "sync.launcherFailed": "Не вдалося перевірити оновлення лаунчера: джерела недоступні",
    "sync.projectFailed": "Не вдалося оновити список збірок: {error}",
    "sync.manifestFailed": "Не вдалося перевірити збірок: {n} (дзеркала недоступні)",
    "sync.offline": "Джерела проєкту недоступні — показано збережений список збірок",
    "sync.failed": "Синхронізація не вдалася: джерела недоступні",
    "plural.newPacks": ["нову збірку", "нові збірки", "нових збірок"],
    "plural.packUpdates": ["оновлення збірки", "оновлення збірок", "оновлень збірок"],
    "home.repair": "Ремонт",
    "home.repairTitle": "Перевірити та відновити пошкоджені файли збірки",
    "home.folder": "Папка",
    "home.folderTitle": "Відкрити папку збірки зі скріншотами та світами",
    "home.listTitle": "Збірки",
    "home.projectTitle": "Збірки проєкту",
    "home.customTitle": "Власні збірки",
    "home.customEmpty": "Власних збірок поки немає. Натисни «+», щоб створити.",
    "home.customMeta": "Власна збірка",
    "home.customDescNew": "Натисни «Грати» — лаунчер сам завантажить Minecraft {mc} і {loader}. Моди, ресурс-паки й шейдери можна додати вже зараз кнопкою «Контент».",
    "home.customDescMods": "Кнопка «Контент» — моди, ресурс-паки й шейдери: показується лише сумісне з {loader} і Minecraft {mc}, залежності ставляться самі.",
    "home.customDescVanilla": "Чиста гра Minecraft {mc} без модів. Ресурс-паки можна додати кнопкою «Контент».",
    "create.button": "Створити власну збірку",
    "create.title": "Нова власна збірка",
    "create.lead": "Будь-яка версія Minecraft і завантажувач на вибір — усе завантажиться під час першого запуску.",
    "create.name": "Назва",
    "create.namePh": "Наприклад, «Fabric 1.21.1»",
    "create.mc": "Версія Minecraft",
    "create.snapshots": "Снапшоти",
    "create.loader": "Завантажувач",
    "create.loaderVersion": "Версія завантажувача",
    "create.loading": "Завантаження…",
    "create.noLoader": "Немає версій для цієї версії Minecraft",
    "create.go": "Створити",
    "create.done": "Збірку «{name}» створено — натисни «Грати», щоб встановити",
    "delete.leadCustom": "Власну збірку буде видалено з комп'ютера разом з модами.",
    "plural.mods": ["мод", "моди", "модів"],
    "create.ctaTitle": "Створити власну збірку",
    "create.ctaText": "Будь-яка версія Minecraft, Fabric, Forge, NeoForge або Quilt і моди з Modrinth та CurseForge",
    "create.ctaTextModrinth": "Будь-яка версія Minecraft, Fabric, Forge, NeoForge або Quilt і моди з Modrinth",
    "create.step1": "1. Версія гри",
    "create.step2": "2. Завантажувач модів",
    "create.step3": "3. Назва",
    "create.mcHint": "Офіційний список версій з серверів Mojang",
    "create.loaderHint": "Обрано найновішу версію — зазвичай саме вона потрібна",
    "create.recommended": "Рекомендовано",
    "loader.vanilla": "Чиста гра без модів",
    "loader.fabric": "Легкий і швидкий, більшість сучасних модів",
    "loader.forge": "Класика: великі моди та старі версії",
    "loader.neoforge": "Сучасне продовження Forge (1.20.1+)",
    "home.mods": "Контент",
    "home.modsTitle": "Моди, ресурс-паки й шейдери цієї збірки",
    "home.modsVanilla": "Моди потребують завантажувача — створи збірку на Fabric, Forge, NeoForge або Quilt",
    "mods.title": "Моди — {name}",
    "mods.lead": "{loader} · Minecraft {mc}. Показано лише сумісні моди, потрібні залежності встановлюються автоматично.",
    "mods.browse": "Знайти моди",
    "mods.installed": "Встановлені",
    "mods.folder": "Папка mods",
    "mods.searchPh": "Пошук на Modrinth: Sodium, JEI, Create…",
    "mods.popular": "Популярні моди для цієї версії",
    "mods.nothing": "Нічого не знайдено. Спробуй іншу назву.",
    "mods.none": "Модів ще немає. Перейди у «Знайти моди», щоб додати перший.",
    "mods.install": "Встановити",
    "mods.installing": "Встановлення…",
    "mods.isInstalled": "Встановлено",
    "mods.more": "Показати ще",
    "mods.by": "від {author}",
    "mods.manual": "Додано вручну",
    "mods.dependency": "Залежність",
    "mods.enable": "Увімкнути або вимкнути мод",
    "mods.remove": "Видалити мод",
    "mods.done": "Встановлено: {list}",
    "mods.missing": "Не знайдено сумісних версій для: {list}",
    "mods.removed": "Мод «{name}» видалено",
    "mods.running": "Закрий гру, щоб змінювати моди",
    "common.done": "Готово",
    "home.installedMeta": "Встановлено · версія {v}",
    "home.notInstalledMeta": "Ще не встановлено",
    "home.updateMeta": "Є оновлення: {from} → {to}",
    "home.playingAs": "Ти граєш як <b>{nick}</b> · <button type=\"button\" class=\"link\" data-goto=\"profiles\">змінити</button>",
    "home.noNick": "Ігровий нік не вибрано — <button type=\"button\" class=\"link\" data-goto=\"profiles\">додай його у вкладці «Акаунт»</button>",
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
    "play.subRunning": "зараз граємо",
    "play.stop": "Закрити гру",
    "play.stopAsk": "Закрити гру?",
    "play.stopYes": "Так, закрити гру",
    "play.stopNo": "Скасувати",
    "play.subUpdate": "нова версія {v}",
    "play.subFirst": "перший запуск довший — качаються Minecraft і Java",
    "stage.launcher-update": "Оновлення лаунчера",
    "stage.verify": "Перевірка файлів збірки",
    "stage.download": "Завантаження збірки",
    "stage.java": "Завантаження Java",
    "stage.client": "Клієнт Minecraft",
    "stage.libraries": "Бібліотеки гри",
    "stage.assets": "Ресурси гри",
    "step.manifest": "Маніфест збірки",
    "step.files": "Файли збірки",
    "step.minecraft": "Minecraft",
    "step.java": "Java",
    "step.loader": "Завантажувач модів",
    "step.libraries": "Бібліотеки гри",
    "step.assets": "Ресурси гри",
    "step.launch": "Запуск гри",
    "progress.step": "Етап {n} з {total}",
    "progress.starting": "Підготовка…",
    "progress.failedAt": "Помилка на етапі «{step}»",
    "progress.details": "Що робить лаунчер",
    "progress.launched": "Гру запущено",
    "progress.checked": "Перевірку завершено",
    "progress.left": "ще ~{time}",
    "unit.files": "файлів",
    "unit.sec": "с",
    "unit.min": "хв",
    "unit.mbps": "МБ/с",
    "home.delete": "Видалити",
    "home.deleteTitle": "Видалити збірку з комп'ютера, щоб звільнити місце",
    "home.ramLow": "Виділено {cur} ОЗУ — цій збірці радять {rec}",
    "home.ramFix": "Змінити",
    "play.verifyN": "Перевірка файлів {n}/{total}",
    "play.downloadMods": "Завантаження модів — {p}%",
    "play.prepMinecraft": "Підготовка Minecraft",
    "play.prepJava": "Підготовка Java",
    "play.prepLoader": "Встановлення завантажувача",
    "play.gameFiles": "Файли гри — {p}%",
    "play.subWait": "не закривай лаунчер",
    "play.subInstall": "завантаження {size}",
    "delete.lead": "Файли збірки буде видалено з комп'ютера. Встановити її знову можна будь-коли.",
    "delete.free": "Звільниться",
    "delete.worlds": "Світи",
    "delete.shots": "Скріншоти",
    "delete.keep": "Зберегти мої світи та скріншоти",
    "delete.keepHint": "Залишиться {size}. Після повторного встановлення все буде на місці.",
    "delete.noPersonal": "Світів і скріншотів у цій збірці немає — нічого особистого не загубиться.",
    "delete.doneKept": "Збірку «{name}» видалено, світи й скріншоти збережено",
    "delete.running": "Спершу закрий гру — запущену збірку видалити не можна",
    "settings.kept": "збережено лише світи й скріншоти",
    "err.offline": "Немає з'єднання з інтернетом. Перевір підключення і спробуй ще раз.",
    "err.timeout": "Сервер відповідає надто довго. Спробуй ще раз за хвилину.",
    "err.notFound": "Потрібний файл не знайдено на сервері. Напиши адміністратору проєкту — збірку, мабуть, саме оновлюють.",
    "err.rateLimit": "GitHub тимчасово обмежив завантаження. Зачекай кілька хвилин і спробуй ще раз.",
    "err.server": "Сервер із файлами тимчасово недоступний. Спробуй трохи пізніше.",
    "err.disk": "На диску закінчилося місце. Звільни місце або обери іншу папку в Налаштуваннях.",
    "err.access": "Немає доступу до папки лаунчера. Закрий гру та інші програми, що можуть її використовувати, або обери іншу папку.",
    "err.corrupt": "Файл завантажився пошкодженим. Натисни «Спробувати ще» — лаунчер завантажить його заново.",
    "err.blocked": "Джерело заблоковано у твоїй мережі, а всі дзеркала теж недоступні. Спробуй пізніше, через інше підключення або імпортуй збірку з .zip.",
    "err.details": "Технічні подробиці",
    "server.online": "Сервер онлайн · {n}/{max}",
    "server.offline": "Сервер недоступний",
    "server.checking": "Перевірка сервера…",
    "status.loading": "Завантаження списку збірок…",
    "status.ready": "Готово до гри",
    "status.offline": "Джерела недоступні — показано збережений список",
    "status.gameRunning": "Гра запущена — приємної гри!",
    "status.gameClosed": "Гру закрито",
    "status.updateFailed": "Не вдалося перевірити оновлення лаунчера",
    "status.restarting": "Перезапуск лаунчера…",
    "status.offlinePlay": "Офлайн-режим: гру запущено з локального кешу без перевірки оновлень",
    "status.installed": "Збірку встановлено — гра запускається",
    "profiles.title": "Акаунт",
    "profiles.sub": "Один акаунт Terrapolis — кілька ігрових ніків. Кнопка «Грати» використовує активний нік.",
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
    "profiles.removeTitle": "Видалити нік «{nick}»?",
    "profiles.removeText": "Нік і скін зникнуть з акаунта. Світи, предмети й прогрес на сервері не зміняться — нік можна додати знову.",
    "profiles.added": "Профіль «{nick}» додано й обрано",
    "profiles.skinSaved": "Скін збережено",
    "profiles.skinBig": "Файл завеликий — скін має важити до 1 МБ",
    "profiles.skinBad": "Це не схоже на скін Minecraft: потрібен .png розміром 64×64 (або 64×32)",
    "profiles.lastOne": "В акаунті має залишитися хоча б один ігровий нік",
    "project.title": "Проєкт",
    "project.sub": "Проєкт — це сервер чи спільнота зі своїми збірками. Підключення необов'язкове: без проєкту лаунчер працює з власними збірками.",
    "project.code": "Код:",
    "project.refresh": "Оновити список",
    "cl.title": "Що нового",
    "cl.launcher": "Лаунчер",
    "cl.project": "Проєкт",
    "cl.loading": "Завантаження…",
    "cl.empty": "Записів поки немає",
    "cl.noRepo": "Для цього проєкту історія змін недоступна",
    "cl.release": "Реліз",
    "cl.commit": "Коміт",
    "project.switchTitle": "Підключити інший проєкт",
    "project.connect": "Підключити",
    "project.switchHint": "Встановлені збірки залишаться на диску. Видалити їх можна в Налаштуваннях.",
    "project.leaveTitle": "Вийти з проєкту",
    "project.leaveSub": "Лаунчер забуде код проєкту. Профілі та власні збірки залишаться — грати можна й без проєкту.",
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
    "unit.gb": "ГБ",
    "legal.notice": "НЕ Є ОФІЦІЙНИМ ПРОДУКТОМ MINECRAFT. НЕ СХВАЛЕНО MOJANG АБО MICROSOFT І НЕ ПОВ'ЯЗАНО З НИМИ.",
    "settings.aboutTitle": "Про лаунчер",
    "settings.updates": "Оновлення лаунчера",
    "settings.autoUpdate": "Автоматично завантажувати та встановлювати оновлення лаунчера",
    "settings.autoUpdateHint": "Якщо вимкнено — лаунчер лише покаже сповіщення з описом змін, а встановиш, коли буде зручно.",
    "settings.checkUpdate": "Перевірити оновлення",
    "settings.version": "Поточна версія: v{v}",
    "update.title": "Доступна нова версія лаунчера",
    "update.later": "Пізніше",
    "update.changelog": "Що нового",
    "update.download": "Завантажити",
    "update.downloadFull": "Завантажити оновлення",
    "update.downloading": "Завантаження оновлення…",
    "update.restarting": "Перезапуск лаунчера…",
    "update.released": "реліз від {date}",
    "update.modalTitle": "Що нового у v{v}",
    "update.noNotes": "Автор релізу не додав опис змін.",
    "update.upToDate": "У тебе найновіша версія лаунчера — v{v}",
    "update.checkFailed": "Не вдалося перевірити оновлення: усі джерела недоступні",
    "update.failed": "Не вдалося встановити оновлення",
    "update.gameRunning": "Закрий гру, щоб встановити оновлення",
    "update.autoOn": "Автооновлення увімкнено",
    "update.autoOff": "Автооновлення вимкнено — про нові версії лаунчер повідомлятиме",
    "settings.network": "Мережа та дзеркала",
    "settings.networkHint": "Якщо основне джерело недоступне (блокування, збій, ліміт GitHub), лаунчер сам переходить на наступне дзеркало — і для збірок, і для оновлень.",
    "mirrors.source.builtin": "Вбудований список дзеркал",
    "mirrors.source.cache": "Збережений конфіг дзеркал",
    "mirrors.source.remote": "Актуальний конфіг дзеркал",
    "mirrors.source.custom": "Власний файл дзеркал",
    "mirrors.content": "Файли збірок",
    "mirrors.update": "Оновлення лаунчера",
    "mirrors.down": "не відповідає",
    "mirrors.custom": "Власні дзеркала: створи файл {file} у форматі {format}. Шаблони: {owner}, {repo}, {branch} або {url}.",
    "settings.modSources": "Джерела модів",
    "settings.modrinthOk": "Modrinth — працює без ключа",
    "settings.cfKey": "Ключ CurseForge API",
    "settings.cfKeyPh": "Встав ключ із console.curseforge.com",
    "settings.cfSave": "Зберегти",
    "settings.cfClear": "Видалити ключ",
    "settings.cfSet": "Ключ збережено: {hint}",
    "settings.cfEnv": "Ключ узято зі змінної середовища CURSEFORGE_API_KEY",
    "settings.cfNone": "Ключ не вказано — пошук на CurseForge вимкнено",
    "settings.cfHint": "Безкоштовний ключ видає CurseForge for Studios (console.curseforge.com → API keys). Ключ зберігається лише на цьому комп'ютері й надсилається тільки на api.curseforge.com.",
    "settings.cfSaved": "Ключ CurseForge збережено",
    "settings.cfCleared": "Ключ CurseForge видалено",
    "dd.search": "Пошук…",
    "dd.nothing": "Нічого не знайдено",
    "dd.loading": "Завантаження…",
    "badge.release": "Release",
    "badge.snapshot": "Snapshot",
    "badge.beta": "Beta",
    "badge.alpha": "Alpha",
    "badge.latest": "Latest",
    "badge.recommended": "Recommended",
    "badge.auto": "Авто",
    "status.localOnly": "Офлайн-режим: усі дзеркала недоступні — показано встановлені збірки",
    "home.offlineBadge": "Офлайн-режим",
    "play.subOffline": "офлайн · з локального кешу",
    "play.subNeedNet": "потрібен інтернет або імпорт .zip",
    "step.debug": "Дебагер збірки",
    "stage.debug": "Дебагер: перевірка модів",
    "stage.debug-download": "Дебагер: докачування модів",
    "play.debugging": "Дебагер збірки…",
    "play.debugDownload": "Дебагер: докачування — {p}%",
    "menu.debug": "Перевірити збірку (Дебагер)",
    "menu.import": "Імпортувати з .zip…",
    "menu.mods": "Контент збірки",
    "debug.title": "Дебагер збірки «{name}»",
    "debug.clean": "Усе гаразд: кожен мод на місці, SHA-256 збігаються, сторонніх .jar у mods/ немає.",
    "debug.checked": "Перевірено",
    "debug.ok": "У порядку",
    "debug.fixed": "Виправлено",
    "debug.failed": "Не вдалося",
    "debug.removed": "Прибрано зайві моди (перенесено в {dir} — їх можна повернути)",
    "debug.downloaded": "Докачано відсутні моди",
    "debug.replaced": "Замінено пошкоджені моди",
    "debug.failedList": "Не вдалося виправити",
    "debug.offline": "Без мережі відсутні й пошкоджені моди лише позначено — лаунчер виправить їх, щойно з'явиться зв'язок.",
    "debug.report": "Звіт",
    "import.button": "Імпорт",
    "import.buttonTitle": "Встановити цю збірку з .zip архіву — без інтернету",
    "import.newTitle": "Імпортувати .zip як нову власну збірку",
    "import.confirmTitle": "Імпортувати архів у «{name}»?",
    "import.confirmText": "З архіву <b>{file}</b> буде розпаковано лише manifest.json, mods/, config/, resourcepacks/, shaderpacks/ і servers.dat. Світи, скріншоти й options.txt не зміняться.",
    "import.go": "Імпортувати",
    "import.running": "Імпорт збірки з архіву…",
    "import.doneTitle": "Збірку «{name}» імпортовано",
    "import.files": "Розпаковано",
    "import.verified": "Збіглося з manifest.json",
    "import.skipped": "Пропущено",
    "import.mismatched": "Ці файли відрізняються від manifest.json — лаунчер оновить їх, щойно буде інтернет:",
    "import.ready": "Збірку можна запускати навіть без інтернету. Особисті файли (світи, скріншоти, options.txt) не змінювались.",
    "mods.searchPhCurse": "Пошук на CurseForge: JEI, Create, Biomes O' Plenty…",
    "mods.allCategories": "Усі категорії",
    "mods.category": "Категорія",
    "mods.autoFilter": "Автофільтр",
    "mods.latestCompatible": "Найновіша сумісна",
    "mods.version": "Версія файлу",
    "mods.loadingVersions": "Завантаження версій…",
    "mods.cfNeedKey": "Для CurseForge потрібен API-ключ. Додай його в Налаштуваннях → «Джерела модів». Modrinth працює без ключа.",
    "mods.cfOpenSettings": "Відкрити налаштування",
    "mods.managedTitle": "Моди збірки «{name}»",
    "mods.managedLead": "Склад збірки визначає manifest.json проєкту. Дебагер перед кожним запуском докачує відсутні моди й прибирає сторонні. Щоб додавати свої моди — створи власну збірку.",
    "mods.managed": "Частина збірки",
    "mods.managedMissing": "Відсутній — дебагер докачає перед запуском",
    "mods.extra": "Сторонній — дебагер прибере перед запуском",
    "loader.quilt": "Форк Fabric: запускає більшість модів Fabric",
    "ob.user": "Унікальний @username",
    "ob.userPh": "username",
    "ob.userHint": "Логін акаунта — за ним тебе знайдуть друзі. 3–20 символів: латиниця, цифри, _",
    "ob.userBad": "@username: 3–20 символів — латиниця, цифри та _",
    "ob.offlineCreated": "Акаунт @{user} створено офлайн — його буде зареєстровано, щойно з'явиться інтернет",
    "ob.restored": "З поверненням, @{user}! Ніки та скіни акаунта відновлено",
    "mig.badge": "Оновлення Terrapolis v0.4.1",
    "mig.title": "Обери свій унікальний @username",
    "mig.sub": "Тепер у Terrapolis один акаунт із кількома ігровими ніками, друзі та спільні збірки. За @username тебе знайдуть друзі.",
    "mig.hint": "3–20 символів: латиниця, цифри, _. Підставили твій основний нік — можна змінити.",
    "mig.keepNicks": "Ігрові ніки та скіни",
    "mig.keepTheme": "Палітра й налаштування",
    "mig.keepPacks": "Збірки та проєкти",
    "mig.go": "Продовжити",
    "mig.conflictBadge": "Потрібне інше ім'я",
    "mig.conflictTitle": "@{user} уже зайнятий",
    "mig.conflictSub": "Поки лаунчер працював офлайн, це ім'я зайняв інший гравець. Обери інше — ніки, збірки та налаштування лишаються на місці.",
    "mig.done": "Готово! Тепер ти @{user}",
    "account.logout": "Вийти з акаунта",
    "account.logoutTitle": "Вийти з акаунта @{user}?",
    "account.logoutText": "Лаунчер повернеться до екрана першого запуску. Збірки, проєкт і налаштування залишаться на цьому комп'ютері, а якщо знову увійти з тим самим @username — повернуться й ігрові ніки та скіни.",
    "account.loggedOut": "Вихід з акаунта виконано",
    "account.synced": "Синхронізовано з хмарою Terrapolis",
    "account.pending": "Очікує синхронізації — немає зв'язку з хмарою",
    "account.conflict": "Ім'я зайняте — обери інше",
    "ap.nicks": "Ігрові ніки",
    "ap.manage": "Керувати акаунтом",
    "ap.addNick": "Додати нік…",
    "cloud.online": "Хмара: онлайн",
    "cloud.offline": "Хмара недоступна",
    "cloud.pending": "Акаунт не синхронізовано",
    "nav.friends": "Друзі",
    "friends.title": "Друзі",
    "friends.sub": "Бачиш, хто з друзів у грі й у яку збірку, — і можеш приєднатися: увійти в той самий проєкт чи завантажити їхню збірку.",
    "friends.addPh": "username друга",
    "friends.add": "Додати",
    "friends.addHint": "Введи @username друга — йому прийде заявка в друзі.",
    "friends.incoming": "Заявки в друзі",
    "friends.access": "Запити на доступ до збірок",
    "friends.outgoing": "Надіслані заявки",
    "friends.list": "Список друзів",
    "friends.empty": "Поки що друзів немає. Додай друга за його @username — і бачитимеш, у що грають друзі.",
    "friends.offlineState": "Немає зв'язку з хмарою Terrapolis. Гра та власні збірки працюють як завжди — друзі з'являться, щойно повернеться інтернет.",
    "friends.pendingState": "Акаунт @{user} ще не синхронізовано з хмарою. Щойно з'явиться інтернет, тут з'являться друзі.",
    "friends.retry": "Спробувати ще",
    "friends.loading": "Завантаження друзів…",
    "friends.accept": "Прийняти",
    "friends.decline": "Відхилити",
    "friends.allow": "Дозволити",
    "friends.deny": "Відхилити",
    "friends.cancel": "Скасувати",
    "friends.wantsAccess": "просить доступ до «{pack}»",
    "friends.wantsFriend": "хоче додати тебе в друзі",
    "friends.waiting": "очікує відповіді",
    "st.playing": "У грі",
    "st.online": "В мережі",
    "st.offline": "Офлайн",
    "friends.playingPack": "Грає у «{pack}»",
    "friends.playingProject": "«{pack}» · проєкт {project}",
    "friends.playingPrivate": "Грає у власну збірку",
    "friends.playingAs": "як {nick}",
    "friends.timer": "Грає {time}",
    "friends.lastSeen": "востаннє в мережі {ago}",
    "time.justNow": "щойно",
    "time.minAgo": "{n} хв тому",
    "time.hourAgo": "{n} год тому",
    "time.dayAgo": "{n} дн тому",
    "time.hm": "{h} год {m} хв",
    "time.m": "{m} хв",
    "time.s": "{s} с",
    "friends.openPack": "Відкрити збірку",
    "friends.joinProject": "Увійти в проєкт",
    "friends.download": "Завантажити збірку",
    "friends.requestAccess": "Запросити доступ",
    "friends.requestSent": "Запит надіслано…",
    "friends.denied": "Доступ відхилено — запитати ще раз",
    "friends.packs": "Збірки ({n})",
    "friends.packsTitle": "Збірки @{user}",
    "friends.packsLead": "Спільні збірки друга. Публічні можна завантажити одразу, «За проханням» — після схвалення.",
    "friends.remove": "Видалити з друзів",
    "friends.removeTitle": "Видалити @{user} з друзів?",
    "friends.removeText": "Ви більше не бачитимете статусів одне одного, а доступи до збірок буде скасовано. Додати друга знову можна будь-коли.",
    "friends.sent": "Заявку надіслано @{user}",
    "friends.accepted": "Тепер ви друзі з @{user}",
    "friends.already": "@{user} уже у твоїх друзях",
    "friends.alreadySent": "Заявку @{user} уже надіслано",
    "friends.removed": "@{user} видалено з друзів",
    "friends.declined": "Заявку відхилено",
    "friends.accessAsked": "Запит на доступ до «{pack}» надіслано",
    "friends.accessGranted": "@{user} тепер може завантажити «{pack}»",
    "friends.accessDenied": "Запит @{user} відхилено",
    "friends.noResults": "Нікого не знайдено",
    "rel.friend": "Друг",
    "rel.outgoing": "Заявку надіслано",
    "rel.incoming": "Хоче дружити",
    "friends.joinTitle": "Увійти в проєкт «{name}»?",
    "friends.joinText": "@{user} грає у збірку «{pack}» проєкту «{name}» (код <b>{code}</b>). Лаунчер підключить проєкт і відкриє цю збірку — і ви зможете грати разом.",
    "friends.joinReplace": "Поточний проєкт «{cur}» буде замінено. Встановлені збірки залишаться на диску.",
    "friends.joinGo": "Увійти в проєкт",
    "friends.joined": "Проєкт «{name}» підключено",
    "friends.downloading": "Завантаження збірки «{name}»…",
    "friends.downloaded": "Збірку «{name}» від @{user} встановлено",
    "friends.packMissing": "Цієї збірки немає у твоєму списку — оновлюю проєкт",
    "vis.label": "4. Хто з друзів бачить",
    "vis.private": "Приватна",
    "vis.public": "Публічна для друзів",
    "vis.on_request": "За проханням",
    "vis.privateHint": "Бачиш лише ти. Друзі бачать «Грає у власну збірку» без кнопки завантаження.",
    "vis.publicHint": "Кожен друг може завантажити її в один клік.",
    "vis.on_requestHint": "Друзі бачать назву й кнопку «Запросити доступ» — завантажать після твого схвалення.",
    "vis.title": "Видимість «{name}»",
    "vis.lead": "Хто з друзів бачить цю збірку й може її завантажити. Для публічної та «за проханням» лаунчер публікує в хмарі легку копію: список модів із CDN + config/, servers.dat (до 45 МБ).",
    "vis.save": "Зберегти",
    "vis.update": "Зберегти й оновити копію",
    "vis.publishing": "Публікуємо «{name}» для друзів…",
    "vis.published": "«{name}» доступна друзям · з CDN: {cdn}, в архіві: {bundled}",
    "vis.privateDone": "«{name}» тепер приватна",
    "vis.pending": "Видимість збережено — збірку опублікуємо, щойно з'явиться зв'язок з хмарою",
    "vis.skipped": "Не вмістилося в 45 МБ, тож друзі цього не отримають: {files}",
    "vis.publishedTag": "опубліковано",
    "vis.failed": "Збірку не вдалося опублікувати",
    "menu.visibility": "Видимість для друзів…",
    "menu.icon": "Іконка збірки…",
    "create.loaderVanilla": "Не потрібна для Vanilla",
    "icon.step": "5. Іконка збірки (опціонально)",
    "icon.label": "Іконка збірки",
    "icon.hint": "PNG · JPG · WebP",
    "icon.hintFull": "PNG, JPG або WebP — картинку буде обрізано до квадрата й зменшено до 128×128",
    "icon.pick": "Обрати файл…",
    "icon.clear": "Прибрати іконку",
    "icon.bad": "Не вдалося прочитати картинку — спробуй інший файл",
    "icon.title": "Іконка «{name}»",
    "icon.lead": "Іконка показується у списку власних збірок, а також у друзів, яким ти передаєш цю збірку.",
    "icon.save": "Зберегти",
    "icon.saved": "Іконку «{name}» оновлено",
    "account.closeGame": "Спершу закрий гру — під час гри вийти з акаунта не можна",
    "friends.badProject": "Код проєкту друга некоректний — підключення скасовано",
    "friends.trustTitle": "Завантажити «{name}» від @{user}?",
    "friends.trustText": "Лаунчер створить нову окрему збірку: моди з Modrinth і CurseForge завантажаться з їхніх серверів, а налаштування й файли, додані вручну, — з архіву @{user}. Такі файли ніхто не перевіряє, тож завантажуй збірки лише від друзів, яким довіряєш. Твої світи та інші збірки не зміняться."
  },
  en: {
    "boot.loading": "Loading…",
    "boot.updating": "Updating the launcher…",
    "ob.title": "Welcome!",
    "ob.sub1": "Terrapolis is a light and fast Minecraft launcher. Create your account and choose how the launcher looks.",
    "ob.next": "Next",
    "ob.title2": "Community projects",
    "ob.sub2": "Connect to a project and its modpacks will appear in the launcher. Or skip this step: you can create and play your own modpacks right away.",
    "ob.back": "Back",
    "ob.skip": "Skip",
    "ob.pick": "Available projects",
    "ob.loadingProjects": "Looking for projects…",
    "ob.noProjects": "No public projects found yet. If you have a code, enter it below.",
    "ob.code": "Or enter a project code or owner/repo",
    "ob.codePh": "Project code or owner/repo",
    "ob.nick": "First in-game nickname",
    "ob.nickPh": "Enter your in-game nickname",
    "ob.nickHint": "Other players see you by this name in-game (defaults to your @username). 3–16 chars",
    "ob.submit": "Connect",
    "ob.checking": "Connecting…",
    "ob.needCode": "Pick a project or enter a code — or press Skip",
    "theme.title": "Color palette",
    "theme.hint": "Accent color of buttons, toggles and highlights. Applies instantly.",
    "theme.previewChip": "Fabric 1.21.1",
    "theme.emerald": "Emerald",
    "theme.cobalt": "Cobalt blue",
    "theme.amber": "Amber",
    "theme.ruby": "Ruby",
    "theme.amethyst": "Amethyst",
    "theme.graphite": "Graphite",
    "top.noProject": "No project — your own modpacks only",
    "home.connectTitle": "Connect a project",
    "home.connectText": "Community modpacks — by project code or owner/repo",
    "home.browsePacks": "Ready-made modpacks from Modrinth and CurseForge",
    "project.none": "No project connected",
    "project.noneSub": "The launcher fully works without a project: create your own modpacks or install ready-made ones from Modrinth and CurseForge.",
    "project.connectTitle": "Connect a project",
    "project.connectHint": "Pick a project from the list or enter a code from the admin or a GitHub repository as owner/repo.",
    "project.left": "You left the project — your own modpacks are still here",
    "create.loader": "Loader",
    "create.browse": "Ready-made modpack…",
    "content.title": "Content — {name}",
    "content.mods": "Mods",
    "content.resourcepacks": "Resource packs",
    "content.shaders": "Shaders",
    "content.modpacks": "Modpacks",
    "content.browse": "Browse",
    "content.leadResourcepacks": "Resource packs for Minecraft {mc}. Enable an installed pack in game: Options → Resource Packs.",
    "content.leadShaders": "Shader packs for Minecraft {mc}. Choose a shader in game: Options → Video Settings → Shader Packs.",
    "content.leadModpacks": "Community modpacks: the launcher creates a separate new modpack with the right Minecraft version and loader and downloads all its files.",
    "content.searchPhRp": "Search resource packs: Faithful, Fresh Animations…",
    "content.searchPhSh": "Search shaders: Complementary, BSL, Sildur's…",
    "content.searchPhMp": "Search modpacks: Fabulously Optimized, Cobblemon…",
    "content.popular": "Popular for this version",
    "content.popularPacks": "Popular modpacks",
    "content.none": "Nothing here yet. Open \"Browse\" to add something.",
    "content.folderRp": "resourcepacks folder",
    "content.folderSh": "shaderpacks folder",
    "content.enable": "Enable or disable",
    "content.remove": "Remove",
    "content.removed": "\"{name}\" removed",
    "content.shaderNeed": "Shaders need the Iris or Oculus mod — this modpack doesn't have it yet.",
    "content.shaderInstall": "Install {mod}",
    "content.shaderVanilla": "Shaders need the Iris or Oculus mod, and vanilla modpacks can't run mods. Create a Fabric, NeoForge, Forge or Quilt modpack.",
    "content.shaderModDone": "{mod} installed — choose a shader in game: Options → Video Settings → Shader Packs",
    "content.modsVanilla": "Mods need a loader. Resource packs work in vanilla too — see the \"Resource packs\" tab.",
    "mp.installing": "Installing the modpack…",
    "mp.progress": "Installing \"{name}\" — {p}%",
    "mp.doneTitle": "Modpack \"{name}\" installed",
    "mp.doneText": "A separate modpack was created: Minecraft {mc} · {loader}. Press Play — the launcher will download the game and the loader.",
    "mp.files": "Files downloaded",
    "mp.overrides": "From overrides",
    "mp.skipped": "Skipped",
    "mp.missing": "The authors of these files disabled downloads by third-party launchers — download them manually from CurseForge into the matching modpack folder:",
    "mp.mc": "Minecraft {v}",
    "mp.latest": "Latest version",
    "danger.title": "Danger Zone",
    "danger.resetTitle": "Delete everything except account and projects",
    "danger.resetSub": "Modpacks, Java, game libraries and assets, download cache and technical settings. Your @username account, game nicknames, skins and projects stay.",
    "danger.resetGo": "Clear…",
    "danger.resetConfirmTitle": "Clear launcher data?",
    "danger.resetConfirmText": "<p>This deletes <b>all modpacks</b> together with worlds and screenshots, Java, game libraries and assets, and the download cache. Technical settings (RAM, graphics, game language, data folder, CurseForge key) are reset to defaults.</p><p><b>Kept:</b> nicknames, skins, the connected project, interface language and palette.</p>",
    "danger.resetting": "Clearing launcher data…",
    "danger.resetDone": "Launcher data cleared",
    "danger.uninstallTitle": "Uninstall the launcher completely",
    "danger.uninstallSub": "All modpacks, game nicknames, settings, the WebView2 cache and the launcher itself. The app closes afterwards. Modpacks you shared with friends stay in the cloud — make them private first.",
    "danger.uninstallGo": "Uninstall…",
    "danger.uninstallConfirmTitle": "Uninstall Terrapolis completely?",
    "danger.uninstallConfirmText": "<p><b>Everything</b> will be permanently deleted: modpacks, worlds, screenshots, profiles, skins, settings, the WebView2 cache and the launcher itself. This can't be undone.</p><p>The app closes when it's done.</p>",
    "danger.uninstalling": "Uninstalling the launcher…",
    "danger.bye": "The launcher has been removed. Goodbye!",
    "danger.typeHint": "To confirm, type <b>{word}</b>",
    "danger.word": "DELETE",
    "nav.home": "Modpacks",
    "nav.profiles": "Account",
    "nav.project": "Project",
    "nav.changelog": "Changelog",
    "cl.read": "Read in full",
    "cl.github": "Read on GitHub",
    "cl.noBody": "No description.",
    "nav.settings": "Settings",
    "top.changeProfile": "Account and game nicknames",
    "top.sync": "Sync: check for launcher and modpack updates",
    "sync.title": "Sync",
    "sync.running": "Sync: checking for launcher and modpack updates…",
    "sync.found": "Found {list}",
    "sync.launcher": "launcher update v{v}",
    "sync.and": " and ",
    "sync.upToDate": "Everything is up to date: no updates found",
    "sync.newList": "New modpacks: {names}",
    "sync.updList": "Updates available: {names}",
    "sync.launcherFailed": "Couldn't check for launcher updates: sources are unreachable",
    "sync.projectFailed": "Couldn't refresh the modpack list: {error}",
    "sync.manifestFailed": "Couldn't check {n} modpack(s): mirrors are unreachable",
    "sync.offline": "Project sources are unreachable — showing the saved modpack list",
    "sync.failed": "Sync failed: sources are unreachable",
    "plural.newPacks": ["new modpack", "new modpacks", "new modpacks"],
    "plural.packUpdates": ["modpack update", "modpack updates", "modpack updates"],
    "home.repair": "Repair",
    "home.repairTitle": "Check and restore damaged modpack files",
    "home.folder": "Folder",
    "home.folderTitle": "Open the modpack folder with your screenshots and worlds",
    "home.listTitle": "Modpacks",
    "home.projectTitle": "Project modpacks",
    "home.customTitle": "Custom modpacks",
    "home.customEmpty": "No custom modpacks yet. Press \"+\" to create one.",
    "home.customMeta": "Custom modpack",
    "home.customDescNew": "Press Play — the launcher will download Minecraft {mc} and {loader} by itself. You can already add mods, resource packs and shaders with the Content button.",
    "home.customDescMods": "The Content button: mods, resource packs and shaders — only what's compatible with {loader} and Minecraft {mc} is shown, dependencies are installed automatically.",
    "home.customDescVanilla": "Pure Minecraft {mc}, no mods. Add resource packs with the Content button.",
    "create.button": "Create a custom modpack",
    "create.title": "New custom modpack",
    "create.lead": "Any Minecraft version and a loader of your choice — everything is downloaded on first launch.",
    "create.name": "Name",
    "create.namePh": "For example, \"Fabric 1.21.1\"",
    "create.mc": "Minecraft version",
    "create.snapshots": "Snapshots",
    "create.loader": "Loader",
    "create.loaderVersion": "Loader version",
    "create.loading": "Loading…",
    "create.noLoader": "No versions for this Minecraft version",
    "create.go": "Create",
    "create.done": "Modpack \"{name}\" created — press Play to install it",
    "delete.leadCustom": "The custom modpack will be removed from your computer along with its mods.",
    "plural.mods": ["mod", "mods", "mods"],
    "create.ctaTitle": "Create a custom modpack",
    "create.ctaText": "Any Minecraft version, Fabric, Forge, NeoForge or Quilt and mods from Modrinth and CurseForge",
    "create.ctaTextModrinth": "Any Minecraft version, Fabric, Forge, NeoForge or Quilt and mods from Modrinth",
    "create.step1": "1. Game version",
    "create.step2": "2. Mod loader",
    "create.step3": "3. Name",
    "create.mcHint": "The official version list from Mojang's servers",
    "create.loaderHint": "The newest version is selected — usually that's the one you need",
    "create.recommended": "Recommended",
    "loader.vanilla": "Pure game, no mods",
    "loader.fabric": "Light and fast, most modern mods",
    "loader.forge": "The classic: big mods and old versions",
    "loader.neoforge": "The modern successor of Forge (1.20.1+)",
    "home.mods": "Content",
    "home.modsTitle": "Mods, resource packs and shaders of this modpack",
    "home.modsVanilla": "Mods need a loader — create a modpack with Fabric, Forge, NeoForge or Quilt",
    "mods.title": "Mods — {name}",
    "mods.lead": "{loader} · Minecraft {mc}. Only compatible mods are shown; required dependencies are installed automatically.",
    "mods.browse": "Find mods",
    "mods.installed": "Installed",
    "mods.folder": "Mods folder",
    "mods.searchPh": "Search Modrinth: Sodium, JEI, Create…",
    "mods.popular": "Popular mods for this version",
    "mods.nothing": "Nothing found. Try another name.",
    "mods.none": "No mods yet. Open \"Find mods\" to add the first one.",
    "mods.install": "Install",
    "mods.installing": "Installing…",
    "mods.isInstalled": "Installed",
    "mods.more": "Show more",
    "mods.by": "by {author}",
    "mods.manual": "Added manually",
    "mods.dependency": "Dependency",
    "mods.enable": "Enable or disable the mod",
    "mods.remove": "Remove mod",
    "mods.done": "Installed: {list}",
    "mods.missing": "No compatible versions found for: {list}",
    "mods.removed": "Mod \"{name}\" removed",
    "mods.running": "Close the game to change mods",
    "common.done": "Done",
    "home.installedMeta": "Installed · version {v}",
    "home.notInstalledMeta": "Not installed yet",
    "home.updateMeta": "Update available: {from} → {to}",
    "home.playingAs": "You play as <b>{nick}</b> · <button type=\"button\" class=\"link\" data-goto=\"profiles\">change</button>",
    "home.noNick": "No game nickname selected — <button type=\"button\" class=\"link\" data-goto=\"profiles\">add one in the Account tab</button>",
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
    "play.subRunning": "now playing",
    "play.stop": "Close game",
    "play.stopAsk": "Close the game?",
    "play.stopYes": "Yes, close the game",
    "play.stopNo": "Cancel",
    "play.subUpdate": "new version {v}",
    "play.subFirst": "first launch takes longer — Minecraft and Java are downloaded",
    "stage.launcher-update": "Updating launcher",
    "stage.verify": "Checking modpack files",
    "stage.download": "Downloading modpack",
    "stage.java": "Downloading Java",
    "stage.client": "Minecraft client",
    "stage.libraries": "Game libraries",
    "stage.assets": "Game assets",
    "step.manifest": "Modpack manifest",
    "step.files": "Modpack files",
    "step.minecraft": "Minecraft",
    "step.java": "Java",
    "step.loader": "Mod loader",
    "step.libraries": "Game libraries",
    "step.assets": "Game assets",
    "step.launch": "Launching",
    "progress.step": "Step {n} of {total}",
    "progress.starting": "Getting ready…",
    "progress.failedAt": "Failed at “{step}”",
    "progress.details": "What the launcher is doing",
    "progress.launched": "Game started",
    "progress.checked": "Check complete",
    "progress.left": "~{time} left",
    "unit.files": "files",
    "unit.sec": "s",
    "unit.min": "min",
    "unit.mbps": "MB/s",
    "home.delete": "Delete",
    "home.deleteTitle": "Remove this modpack from your computer to free up space",
    "home.ramLow": "{cur} of RAM allocated — this modpack recommends {rec}",
    "home.ramFix": "Change",
    "play.verifyN": "Checking files {n}/{total}",
    "play.downloadMods": "Downloading mods — {p}%",
    "play.prepMinecraft": "Preparing Minecraft",
    "play.prepJava": "Preparing Java",
    "play.prepLoader": "Installing mod loader",
    "play.gameFiles": "Game files — {p}%",
    "play.subWait": "keep the launcher open",
    "play.subInstall": "{size} download",
    "delete.lead": "The modpack files will be removed from your computer. You can install it again any time.",
    "delete.free": "Frees up",
    "delete.worlds": "Worlds",
    "delete.shots": "Screenshots",
    "delete.keep": "Keep my worlds and screenshots",
    "delete.keepHint": "{size} will stay. After reinstalling, everything will be back in place.",
    "delete.noPersonal": "There are no worlds or screenshots in this modpack — nothing personal will be lost.",
    "delete.doneKept": "\"{name}\" deleted, worlds and screenshots kept",
    "delete.running": "Close the game first — a running modpack can't be deleted",
    "settings.kept": "only worlds and screenshots kept",
    "err.offline": "No internet connection. Check your connection and try again.",
    "err.timeout": "The server is taking too long to respond. Try again in a minute.",
    "err.notFound": "A required file is missing on the server. Contact the project administrator — the modpack is probably being updated.",
    "err.rateLimit": "GitHub is temporarily limiting downloads. Wait a few minutes and try again.",
    "err.server": "The file server is temporarily unavailable. Try again a bit later.",
    "err.disk": "The disk is full. Free up some space or choose another folder in Settings.",
    "err.access": "The launcher can't access its folder. Close the game and other programs that may be using it, or choose another folder.",
    "err.corrupt": "A file was downloaded damaged. Press \"Try again\" — the launcher will download it again.",
    "err.blocked": "The source is blocked on your network and all mirrors are unreachable too. Try later, use another connection or import the modpack from a .zip.",
    "err.details": "Technical details",
    "server.online": "Server online · {n}/{max}",
    "server.offline": "Server unavailable",
    "server.checking": "Checking server…",
    "status.loading": "Loading modpacks…",
    "status.ready": "Ready to play",
    "status.offline": "Sources unreachable — showing the saved list",
    "status.gameRunning": "Game is running — have fun!",
    "status.gameClosed": "Game closed",
    "status.updateFailed": "Couldn't check for launcher updates",
    "status.restarting": "Restarting launcher…",
    "status.offlinePlay": "Offline mode: the game started from the local cache without checking for updates",
    "status.installed": "Modpack installed — the game is starting",
    "profiles.title": "Account",
    "profiles.sub": "One Terrapolis account — several game nicknames. The Play button uses the active nickname.",
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
    "profiles.removeTitle": "Delete nickname \"{nick}\"?",
    "profiles.removeText": "The nickname and skin will be removed from your account. Worlds, items and server progress won't change — you can add it again.",
    "profiles.added": "Profile \"{nick}\" added and selected",
    "profiles.skinSaved": "Skin saved",
    "profiles.skinBig": "The file is too large — a skin must be under 1 MB",
    "profiles.skinBad": "This doesn't look like a Minecraft skin: a 64×64 (or 64×32) .png is required",
    "profiles.lastOne": "Your account needs at least one game nickname",
    "project.title": "Project",
    "project.sub": "A project is a server or community with its own modpacks. Connecting one is optional: without a project the launcher works with your own modpacks.",
    "project.code": "Code:",
    "project.refresh": "Refresh list",
    "cl.title": "What's new",
    "cl.launcher": "Launcher",
    "cl.project": "Project",
    "cl.loading": "Loading…",
    "cl.empty": "No entries yet",
    "cl.noRepo": "Change history is not available for this project",
    "cl.release": "Release",
    "cl.commit": "Commit",
    "project.switchTitle": "Connect another project",
    "project.connect": "Connect",
    "project.switchHint": "Installed modpacks stay on disk. You can delete them in Settings.",
    "project.leaveTitle": "Leave project",
    "project.leaveSub": "The launcher forgets the project code. Profiles and your own modpacks stay — you can play without a project.",
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
    "unit.gb": "GB",
    "legal.notice": "NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.",
    "settings.aboutTitle": "About",
    "settings.updates": "Launcher updates",
    "settings.autoUpdate": "Automatically download and install launcher updates",
    "settings.autoUpdateHint": "When off, the launcher only shows a notification with the changelog, and you install it whenever it suits you.",
    "settings.checkUpdate": "Check for updates",
    "settings.version": "Current version: v{v}",
    "update.title": "A new launcher version is available",
    "update.later": "Later",
    "update.changelog": "Changelog",
    "update.download": "Download",
    "update.downloadFull": "Download update",
    "update.downloading": "Downloading the update…",
    "update.restarting": "Restarting the launcher…",
    "update.released": "released {date}",
    "update.modalTitle": "What's new in v{v}",
    "update.noNotes": "The release has no description.",
    "update.upToDate": "You have the latest launcher — v{v}",
    "update.checkFailed": "Couldn't check for updates: all sources are unreachable",
    "update.failed": "Couldn't install the update",
    "update.gameRunning": "Close the game to install the update",
    "update.autoOn": "Auto-update enabled",
    "update.autoOff": "Auto-update disabled — the launcher will notify you about new versions",
    "settings.network": "Network & mirrors",
    "settings.networkHint": "If the main source is unreachable (blocked, down, GitHub rate limit), the launcher switches to the next mirror by itself — for modpacks and for updates.",
    "mirrors.source.builtin": "Built-in mirror list",
    "mirrors.source.cache": "Saved mirror config",
    "mirrors.source.remote": "Up-to-date mirror config",
    "mirrors.source.custom": "Custom mirror file",
    "mirrors.content": "Modpack files",
    "mirrors.update": "Launcher updates",
    "mirrors.down": "not responding",
    "mirrors.custom": "Custom mirrors: create {file} in the format {format}. Templates: {owner}, {repo}, {branch} or {url}.",
    "settings.modSources": "Mod sources",
    "settings.modrinthOk": "Modrinth — works without a key",
    "settings.cfKey": "CurseForge API key",
    "settings.cfKeyPh": "Paste the key from console.curseforge.com",
    "settings.cfSave": "Save",
    "settings.cfClear": "Remove key",
    "settings.cfSet": "Key saved: {hint}",
    "settings.cfEnv": "Key taken from the CURSEFORGE_API_KEY environment variable",
    "settings.cfNone": "No key — CurseForge search is disabled",
    "settings.cfHint": "A free key is issued by CurseForge for Studios (console.curseforge.com → API keys). It is stored only on this computer and sent only to api.curseforge.com.",
    "settings.cfSaved": "CurseForge key saved",
    "settings.cfCleared": "CurseForge key removed",
    "dd.search": "Search…",
    "dd.nothing": "Nothing found",
    "dd.loading": "Loading…",
    "badge.release": "Release",
    "badge.snapshot": "Snapshot",
    "badge.beta": "Beta",
    "badge.alpha": "Alpha",
    "badge.latest": "Latest",
    "badge.recommended": "Recommended",
    "badge.auto": "Auto",
    "status.localOnly": "Offline mode: all mirrors are unreachable — showing installed modpacks",
    "home.offlineBadge": "Offline mode",
    "play.subOffline": "offline · from local cache",
    "play.subNeedNet": "needs internet or a .zip import",
    "step.debug": "Pack debugger",
    "stage.debug": "Debugger: checking mods",
    "stage.debug-download": "Debugger: downloading mods",
    "play.debugging": "Pack debugger…",
    "play.debugDownload": "Debugger: downloading — {p}%",
    "menu.debug": "Check modpack (Debugger)",
    "menu.import": "Import from .zip…",
    "menu.mods": "Modpack content",
    "debug.title": "Pack debugger — \"{name}\"",
    "debug.clean": "All good: every mod is in place, SHA-256 hashes match, no foreign .jar files in mods/.",
    "debug.checked": "Checked",
    "debug.ok": "OK",
    "debug.fixed": "Fixed",
    "debug.failed": "Failed",
    "debug.removed": "Removed extra mods (moved to {dir} — you can restore them)",
    "debug.downloaded": "Downloaded missing mods",
    "debug.replaced": "Replaced damaged mods",
    "debug.failedList": "Couldn't fix",
    "debug.offline": "Without a connection, missing and damaged mods are only reported — the launcher fixes them as soon as you're back online.",
    "debug.report": "Report",
    "import.button": "Import",
    "import.buttonTitle": "Install this modpack from a .zip archive — no internet needed",
    "import.newTitle": "Import a .zip as a new custom modpack",
    "import.confirmTitle": "Import the archive into \"{name}\"?",
    "import.confirmText": "Only manifest.json, mods/, config/, resourcepacks/, shaderpacks/ and servers.dat will be extracted from <b>{file}</b>. Worlds, screenshots and options.txt stay untouched.",
    "import.go": "Import",
    "import.running": "Importing the modpack archive…",
    "import.doneTitle": "\"{name}\" imported",
    "import.files": "Extracted",
    "import.verified": "Matched manifest.json",
    "import.skipped": "Skipped",
    "import.mismatched": "These files differ from manifest.json — the launcher will update them once you're online:",
    "import.ready": "The modpack can be started even without internet. Personal files (worlds, screenshots, options.txt) were not changed.",
    "mods.searchPhCurse": "Search CurseForge: JEI, Create, Biomes O' Plenty…",
    "mods.allCategories": "All categories",
    "mods.category": "Category",
    "mods.autoFilter": "Auto filter",
    "mods.latestCompatible": "Latest compatible",
    "mods.version": "File version",
    "mods.loadingVersions": "Loading versions…",
    "mods.cfNeedKey": "CurseForge needs an API key. Add it in Settings → \"Mod sources\". Modrinth works without a key.",
    "mods.cfOpenSettings": "Open settings",
    "mods.managedTitle": "Mods of \"{name}\"",
    "mods.managedLead": "The project's manifest.json defines this modpack. Before every launch the debugger downloads missing mods and removes foreign ones. To add your own mods, create a custom modpack.",
    "mods.managed": "Part of the modpack",
    "mods.managedMissing": "Missing — the debugger downloads it before launch",
    "mods.extra": "Foreign — the debugger removes it before launch",
    "loader.quilt": "Fabric fork: runs most Fabric mods",
    "ob.user": "Unique @username",
    "ob.userPh": "username",
    "ob.userHint": "Your account login — friends find you by it. 3–20 chars: Latin letters, digits, _",
    "ob.userBad": "@username: 3–20 characters — Latin letters, digits and _",
    "ob.offlineCreated": "Account @{user} was created offline — it will be registered once you're online",
    "ob.restored": "Welcome back, @{user}! Your account's nicknames and skins are restored",
    "mig.badge": "Terrapolis v0.4.1 update",
    "mig.title": "Choose your unique @username",
    "mig.sub": "Terrapolis now has one account with several game nicknames, friends and shared modpacks. Friends will find you by your @username.",
    "mig.hint": "3–20 chars: Latin letters, digits, _. We filled in your main nickname — feel free to change it.",
    "mig.keepNicks": "Game nicknames and skins",
    "mig.keepTheme": "Palette and settings",
    "mig.keepPacks": "Modpacks and projects",
    "mig.go": "Continue",
    "mig.conflictBadge": "Another name needed",
    "mig.conflictTitle": "@{user} is already taken",
    "mig.conflictSub": "While the launcher was offline, another player took this name. Pick another one — your nicknames, modpacks and settings stay as they are.",
    "mig.done": "Done! You are now @{user}",
    "account.logout": "Sign out",
    "account.logoutTitle": "Sign out of @{user}?",
    "account.logoutText": "The launcher returns to the welcome screen. Modpacks, the project and settings stay on this computer, and signing in again with the same @username brings back your game nicknames and skins.",
    "account.loggedOut": "Signed out",
    "account.synced": "Synced with the Terrapolis cloud",
    "account.pending": "Waiting to sync — the cloud is unreachable",
    "account.conflict": "Name taken — choose another",
    "ap.nicks": "Game nicknames",
    "ap.manage": "Manage account",
    "ap.addNick": "Add nickname…",
    "cloud.online": "Cloud: online",
    "cloud.offline": "Cloud unreachable",
    "cloud.pending": "Account not synced",
    "nav.friends": "Friends",
    "friends.title": "Friends",
    "friends.sub": "See which friends are in game and in which modpack — and join them: connect the same project or download their modpack.",
    "friends.addPh": "friend's username",
    "friends.add": "Add",
    "friends.addHint": "Enter your friend's @username — they'll get a friend request.",
    "friends.incoming": "Friend requests",
    "friends.access": "Modpack access requests",
    "friends.outgoing": "Sent requests",
    "friends.list": "Friends list",
    "friends.empty": "No friends yet. Add a friend by their @username to see what your friends are playing.",
    "friends.offlineState": "Can't reach the Terrapolis cloud. The game and custom modpacks work as usual — friends will appear once you're back online.",
    "friends.pendingState": "Account @{user} isn't synced with the cloud yet. Friends will appear here once you're online.",
    "friends.retry": "Retry",
    "friends.loading": "Loading friends…",
    "friends.accept": "Accept",
    "friends.decline": "Decline",
    "friends.allow": "Allow",
    "friends.deny": "Deny",
    "friends.cancel": "Cancel",
    "friends.wantsAccess": "asks for access to \"{pack}\"",
    "friends.wantsFriend": "wants to be your friend",
    "friends.waiting": "awaiting reply",
    "st.playing": "In game",
    "st.online": "Online",
    "st.offline": "Offline",
    "friends.playingPack": "Playing \"{pack}\"",
    "friends.playingProject": "\"{pack}\" · project {project}",
    "friends.playingPrivate": "Playing a custom modpack",
    "friends.playingAs": "as {nick}",
    "friends.timer": "Playing for {time}",
    "friends.lastSeen": "last seen {ago}",
    "time.justNow": "just now",
    "time.minAgo": "{n} min ago",
    "time.hourAgo": "{n} h ago",
    "time.dayAgo": "{n} d ago",
    "time.hm": "{h} h {m} min",
    "time.m": "{m} min",
    "time.s": "{s} s",
    "friends.openPack": "Open modpack",
    "friends.joinProject": "Join project",
    "friends.download": "Download modpack",
    "friends.requestAccess": "Request access",
    "friends.requestSent": "Request sent…",
    "friends.denied": "Access denied — ask again",
    "friends.packs": "Modpacks ({n})",
    "friends.packsTitle": "@{user}'s modpacks",
    "friends.packsLead": "Your friend's shared modpacks. Public ones download right away; \"On request\" ones after approval.",
    "friends.remove": "Remove friend",
    "friends.removeTitle": "Remove @{user} from friends?",
    "friends.removeText": "You'll stop seeing each other's status and modpack access will be revoked. You can add them again any time.",
    "friends.sent": "Friend request sent to @{user}",
    "friends.accepted": "You and @{user} are now friends",
    "friends.already": "@{user} is already your friend",
    "friends.alreadySent": "A request to @{user} was already sent",
    "friends.removed": "@{user} removed from friends",
    "friends.declined": "Request declined",
    "friends.accessAsked": "Access request for \"{pack}\" sent",
    "friends.accessGranted": "@{user} can now download \"{pack}\"",
    "friends.accessDenied": "@{user}'s request was denied",
    "friends.noResults": "No one found",
    "rel.friend": "Friend",
    "rel.outgoing": "Request sent",
    "rel.incoming": "Wants to be friends",
    "friends.joinTitle": "Join the \"{name}\" project?",
    "friends.joinText": "@{user} is playing \"{pack}\" from the \"{name}\" project (code <b>{code}</b>). The launcher will connect the project and open this modpack so you can play together.",
    "friends.joinReplace": "Your current project \"{cur}\" will be replaced. Installed modpacks stay on disk.",
    "friends.joinGo": "Join project",
    "friends.joined": "Project \"{name}\" connected",
    "friends.downloading": "Downloading \"{name}\"…",
    "friends.downloaded": "\"{name}\" from @{user} is installed",
    "friends.packMissing": "This modpack isn't in your list yet — refreshing the project",
    "vis.label": "4. Which friends see it",
    "vis.private": "Private",
    "vis.public": "Public to friends",
    "vis.on_request": "On request",
    "vis.privateHint": "Only you. Friends see \"Playing a custom modpack\" without a download button.",
    "vis.publicHint": "Any friend can download it in one click.",
    "vis.on_requestHint": "Friends see the name and a \"Request access\" button — they can download it after you approve.",
    "vis.title": "Visibility of \"{name}\"",
    "vis.lead": "Which friends can see and download this modpack. For public and on-request packs the launcher uploads a light copy to the cloud: the mod list from CDNs + config/, servers.dat (up to 45 MB).",
    "vis.save": "Save",
    "vis.update": "Save and refresh the copy",
    "vis.publishing": "Publishing \"{name}\" for friends…",
    "vis.published": "\"{name}\" is shared with friends · from CDN: {cdn}, in the archive: {bundled}",
    "vis.privateDone": "\"{name}\" is now private",
    "vis.pending": "Visibility saved — the modpack will be published once the cloud is reachable",
    "vis.skipped": "Didn't fit into 45 MB, so friends won't get: {files}",
    "vis.publishedTag": "published",
    "vis.failed": "Couldn't publish the modpack",
    "menu.visibility": "Visibility to friends…",
    "menu.icon": "Modpack icon…",
    "create.loaderVanilla": "Not required for Vanilla",
    "icon.step": "5. Pack icon (optional)",
    "icon.label": "Pack icon",
    "icon.hint": "PNG · JPG · WebP",
    "icon.hintFull": "PNG, JPG or WebP — the image is cropped to a square and scaled to 128×128",
    "icon.pick": "Choose file…",
    "icon.clear": "Remove icon",
    "icon.bad": "Couldn't read the image — try another file",
    "icon.title": "Icon for \"{name}\"",
    "icon.lead": "The icon shows in your custom modpacks list and for friends you share this modpack with.",
    "icon.save": "Save",
    "icon.saved": "Icon for \"{name}\" updated",
    "account.closeGame": "Close the game first — you can't sign out while playing",
    "friends.badProject": "Your friend's project code is invalid — connection cancelled",
    "friends.trustTitle": "Download \"{name}\" from @{user}?",
    "friends.trustText": "The launcher will create a new separate modpack: Modrinth and CurseForge mods download from their servers, while settings and manually added files come from @{user}'s archive. Nobody checks those files, so only download modpacks from friends you trust. Your worlds and other modpacks won't change."
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
  for (const node of document.querySelectorAll("[data-i18n-aria]")) node.setAttribute("aria-label", t(node.dataset.i18nAria));
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
  obSteps: $("obSteps"),
  obStep1: $("obStep1"),
  obNext: $("obNext"),
  obError1: $("obError1"),
  obThemes: $("obThemes"),
  obForm: $("obForm"),
  obCode: $("obCode"),
  obProjects: $("obProjects"),
  obNick: $("obNick"),
  obNickHint: $("obNickHint"),
  obError: $("obError"),
  obSubmit: $("obSubmit"),
  obBack: $("obBack"),
  obSkip: $("obSkip"),
  obLang: $("obLang"),
  app: $("app"),
  brand: $("brand"),
  topAvatar: $("topAvatar"),
  nav: $("nav"),
  navInd: $("navInd"),
  island: $("island"),
  status: $("statusText"),
  topProject: $("topProject"),
  topSync: $("topSync"),
  topDot: $("topDot"),
  topHead: $("topHead"),
  topNick: $("topNick"),
  hero: $("hero"),
  heroMedia: $("heroMedia"),
  ramNote: $("ramNote"),
  badgeMc: $("badgeMc"),
  badgeLoader: $("badgeLoader"),
  badgeServer: $("badgeServer"),
  serverText: $("serverText"),
  instName: $("instName"),
  instDesc: $("instDesc"),
  instMeta: $("instMeta"),
  play: $("play"),
  playWrap: $("playWrap"),
  playConfirm: $("playConfirm"),
  stopBtn: $("stopBtn"),
  stopYes: $("stopYes"),
  stopNo: $("stopNo"),
  playLabel: $("playLabel"),
  playSub: $("playSub"),
  playIco: $("playIco"),
  deleteBtn: $("deleteBtn"),
  repairBtn: $("repairBtn"),
  modsBtn: $("modsBtn"),
  heroMark: $("heroMark"),
  folderBtn: $("folderBtn"),
  progress: $("progress"),
  progressStage: $("progressStage"),
  progressPercent: $("progressPercent"),
  progressFill: $("progressFill"),
  progressFile: $("progressFile"),
  progressSteps: $("progressSteps"),
  progressTrack: $("progressTrack"),
  progressAmount: $("progressAmount"),
  progressStatus: $("progressStatus"),
  progressLog: $("progressLog"),
  nickNote: $("nickNote"),
  instCount: $("instCount"),
  instList: $("instList"),
  instMenu: $("instMenu"),
  skinBody: $("skinBody"),
  stageNick: $("stageNick"),
  skinUpload: $("skinUpload"),
  skinReset: $("skinReset"),
  addProfileForm: $("addProfileForm"),
  newNick: $("newNick"),
  newNickHint: $("newNickHint"),
  profileList: $("profileList"),
  projCard: $("projCard"),
  projBrand: $("projBrand"),
  projName: $("projName"),
  projMeta: $("projMeta"),
  projPicker: $("projPicker"),
  switchTitle: $("switchTitle"),
  switchHint: $("switchHint"),
  leaveCard: $("leaveCard"),
  projCode: $("projCode"),
  projCount: $("projCount"),
  projState: $("projState"),
  projRefresh: $("projRefresh"),
  clWrap: $("clWrap"),
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
  themeGrid: $("themeGrid"),
  dzReset: $("dzReset"),
  dzUninstall: $("dzUninstall"),
  presetSeg: $("presetSeg"),
  applyPreset: $("applyPreset"),
  uiLangSeg: $("uiLangSeg"),
  gameLangDd: $("gameLangDd"),
  versionText: $("versionText"),
  autoUpdate: $("autoUpdate"),
  checkUpdateBtn: $("checkUpdateBtn"),
  cfForm: $("cfForm"),
  cfKey: $("cfKey"),
  cfState: $("cfState"),
  cfClear: $("cfClear"),
  cfBlock: $("cfBlock"),
  mirrorSource: $("mirrorSource"),
  mirrorList: $("mirrorList"),
  mirrorCustom: $("mirrorCustom"),
  importBtn: $("importBtn"),
  badgeOffline: $("badgeOffline"),
  toasts: $("toasts"),
  tip: $("tip"),
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
  modalActions: $("modalActions"),
  obUser: $("obUser"),
  obUserHint: $("obUserHint"),
  obMigrate: $("obMigrate"),
  migBadge: $("migBadge"),
  migTitle: $("migTitle"),
  migSub: $("migSub"),
  migUser: $("migUser"),
  migHint: $("migHint"),
  migError: $("migError"),
  migSubmit: $("migSubmit"),
  topProfile: $("topProfile"),
  topUser: $("topUser"),
  topPresence: $("topPresence"),
  accountPop: $("accountPop"),
  apHead: $("apHead"),
  apUser: $("apUser"),
  apState: $("apState"),
  apNicks: $("apNicks"),
  acUser: $("acUser"),
  acState: $("acState"),
  logoutBtn: $("logoutBtn"),
  friendsBadge: $("friendsBadge"),
  cloudChip: $("cloudChip"),
  friendForm: $("friendForm"),
  friendQuery: $("friendQuery"),
  friendAdd: $("friendAdd"),
  friendSearch: $("friendSearch"),
  friendHint: $("friendHint"),
  incomingBlock: $("incomingBlock"),
  incomingList: $("incomingList"),
  incomingCount: $("incomingCount"),
  accessBlock: $("accessBlock"),
  accessList: $("accessList"),
  accessCount: $("accessCount"),
  outgoingBlock: $("outgoingBlock"),
  outgoingList: $("outgoingList"),
  friendList: $("friendList"),
  friendsCount: $("friendsCount")
};

const ICONS = {
  trash: '<svg viewBox="0 0 24 24"><path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6M10 11v6M14 11v6"/></svg>',
  wrench: '<svg viewBox="0 0 24 24"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.8-3.8a6 6 0 0 1-7.9 7.9l-6.9 6.9a2.1 2.1 0 0 1-3-3l6.9-6.9a6 6 0 0 1 7.9-7.9z"/></svg>',
  alert: '<svg viewBox="0 0 24 24"><path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0zM12 9v4M12 17h.01"/></svg>',
  folder: '<svg viewBox="0 0 24 24"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>',
  logout: '<svg viewBox="0 0 24 24"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/></svg>',
  palette: '<svg viewBox="0 0 24 24"><circle cx="13.5" cy="6.5" r="1"/><circle cx="17.5" cy="10.5" r="1"/><circle cx="8.5" cy="7.5" r="1"/><circle cx="6.5" cy="12.5" r="1"/><path d="M12 2a10 10 0 0 0 0 20c.9 0 1.7-.8 1.7-1.7 0-.4-.2-.8-.4-1.1-.3-.3-.4-.7-.4-1.1 0-.9.8-1.7 1.7-1.7H17a5 5 0 0 0 5-5C22 6 17.5 2 12 2z"/></svg>',
  box: '<svg viewBox="0 0 24 24"><path d="M21 16V8a2 2 0 0 0-1-1.7l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.7l7 4a2 2 0 0 0 2 0l7-4a2 2 0 0 0 1-1.7z"/><path d="M3.3 7 12 12l8.7-5M12 22V12"/></svg>',
  plus: '<svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg>',
  more: '<svg viewBox="0 0 24 24"><circle cx="12" cy="5" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="12" cy="19" r="1.2"/></svg>',
  memory: '<svg viewBox="0 0 24 24"><rect x="4" y="7" width="16" height="10" rx="1.5"/><path d="M8 7V4M12 7V4M16 7V4M8 20v-3M12 20v-3M16 20v-3"/></svg>',
  shirt: '<svg viewBox="0 0 24 24"><path d="M20.4 6.6 16 4a4 4 0 0 1-8 0L3.6 6.6a1 1 0 0 0-.5 1.1l.9 3.6a1 1 0 0 0 1 .7H7v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-8h2a1 1 0 0 0 1-.7l.9-3.6a1 1 0 0 0-.5-1.1z"/></svg>',
  archive: '<svg viewBox="0 0 24 24"><path d="M3 4h18v4H3z"/><path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8"/><path d="M12 11v6M9.5 14.5 12 17l2.5-2.5"/></svg>',
  bug: '<svg viewBox="0 0 24 24"><path d="M9 3h6M10 3v4M14 3v4"/><rect x="6" y="7" width="12" height="13" rx="6"/><path d="M12 11v5M3 13h3M18 13h3M4 7l2.5 2M20 7l-2.5 2M4 20l2.5-2M20 20l-2.5-2"/></svg>',
  update: '<svg viewBox="0 0 24 24"><path d="M12 3v12M7 10l5 5 5-5"/><path d="M5 21h14"/></svg>',
  sparkle: '<svg viewBox="0 0 24 24"><path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"/><path d="M19 15l.7 2 2 .7-2 .7-.7 2-.7-2-2-.7 2-.7z"/></svg>',
  search: '<svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>',
  chevron: '<svg viewBox="0 0 24 24"><path d="m6 9 6 6 6-6"/></svg>',
  check: '<svg viewBox="0 0 24 24"><path d="m5 12 5 5 9-10"/></svg>',
  close: '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18"/></svg>',
  layers: '<svg viewBox="0 0 24 24"><path d="m12 3 9 5-9 5-9-5z"/><path d="m3 13 9 5 9-5"/></svg>',
  image: '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="m21 16-5-5-9 9"/></svg>',
  sun: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>',
  link: '<svg viewBox="0 0 24 24"><path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/></svg>',
  users: '<svg viewBox="0 0 24 24"><circle cx="9" cy="8" r="3.5"/><path d="M2.5 20c0-3.6 2.9-6 6.5-6s6.5 2.4 6.5 6"/><path d="M15.5 4.6a3.5 3.5 0 0 1 0 6.8M18 14.4c2.1.8 3.5 2.8 3.5 5.6"/></svg>',
  userPlus: '<svg viewBox="0 0 24 24"><circle cx="9" cy="8" r="3.5"/><path d="M2.5 20c0-3.6 2.9-6 6.5-6s6.5 2.4 6.5 6M19 8v6M16 11h6"/></svg>',
  download: '<svg viewBox="0 0 24 24"><path d="M12 3v12M7 10l5 5 5-5"/><path d="M5 21h14"/></svg>',
  lock: '<svg viewBox="0 0 24 24"><rect x="4.5" y="10.5" width="15" height="10" rx="2"/><path d="M8 10.5V7.5a4 4 0 0 1 8 0v3"/></svg>',
  globe: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/></svg>',
  hand: '<svg viewBox="0 0 24 24"><path d="M8 11V5.5a1.5 1.5 0 0 1 3 0V10M11 9.5v-6a1.5 1.5 0 0 1 3 0V10M14 10V5a1.5 1.5 0 0 1 3 0v7"/><path d="M17 9.5a1.5 1.5 0 0 1 3 0V14a7 7 0 0 1-7 7h-1a7 7 0 0 1-5.6-2.8L3.6 15a1.5 1.5 0 0 1 2.3-1.9L8 15V11"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>',
  clock: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>'
};

/* ═══════════════════════════ ПАЛІТРИ ═══════════════════════════ */

/** Акцентні палітри (id збігаються з `config::THEMES` у Rust). rgb — для напівпрозорих відтінків. */
const THEMES = {
  emerald: { accent: "#3cc46f", hover: "#4ad27d", press: "#33ad61", ink: "#06170d", rgb: "60, 196, 111" },
  cobalt: { accent: "#4d8dff", hover: "#6a9fff", press: "#3f78e0", ink: "#04132e", rgb: "77, 141, 255" },
  amber: { accent: "#f0a73a", hover: "#f6b757", press: "#d6922c", ink: "#251602", rgb: "240, 167, 58" },
  ruby: { accent: "#e8457c", hover: "#ef6292", press: "#cc3a6b", ink: "#2b0613", rgb: "232, 69, 124" },
  amethyst: { accent: "#a47bff", hover: "#b593ff", press: "#8d63e6", ink: "#170834", rgb: "164, 123, 255" },
  graphite: { accent: "#cfd5dd", hover: "#e2e6eb", press: "#b6bdc7", ink: "#101317", rgb: "207, 213, 221" }
};

/** Миттєво перефарбовує весь інтерфейс: усі акцентні елементи беруть кольори з CSS-змінних. */
function applyTheme(id) {
  const theme = THEMES[id] || THEMES.emerald;
  const style = document.documentElement.style;
  style.setProperty("--accent", theme.accent);
  style.setProperty("--accent-hover", theme.hover);
  style.setProperty("--accent-press", theme.press);
  style.setProperty("--accent-ink", theme.ink);
  style.setProperty("--accent-soft", `rgba(${theme.rgb}, 0.12)`);
  style.setProperty("--accent-line", `rgba(${theme.rgb}, 0.45)`);
  style.setProperty("--accent-faint", `rgba(${theme.rgb}, 0.04)`);
  document.documentElement.dataset.theme = THEMES[id] ? id : "emerald";
}

/** Сітка кольорових зразків (онбординг і Налаштування). */
function renderThemeGrid(container, current) {
  if (!container) return;
  container.innerHTML = Object.entries(THEMES)
    .map(([id, theme]) => {
      const on = id === current;
      return `<button type="button" class="theme-swatch${on ? " on" : ""}" role="radio" aria-checked="${on}" data-theme="${id}" style="--sw:${theme.accent};--sw-ink:${theme.ink}"><i aria-hidden="true">${ICONS.check}</i><span>${esc(t("theme." + id))}</span></button>`;
    })
    .join("");
}

function markTheme(container, current) {
  for (const button of container?.querySelectorAll("[data-theme]") || []) {
    const on = button.dataset.theme === current;
    button.classList.toggle("on", on);
    button.setAttribute("aria-checked", String(on));
  }
}

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
  projectList: null,
  projectListError: "",
  pickedProject: "",
  view: null,
  viewError: null,
  activeId: "",
  busy: null,
  phase: null,
  percent: 0,
  running: null,
  /** Стан 3 розділеної кнопки: показано «Закрити гру? ✓ ✕». */
  stopConfirm: false,
  /** Гра закривається через «✓» (чекаємо stop_instance / подію game-state). */
  stopping: false,
  updating: false,
  disk: [],
  skins: new Map(),
  server: new Map(),
  menuFor: null,
  /** Остання перевірка оновлення лаунчера (UpdateCheck з бекенда). */
  update: null,
  /** «Пізніше» — до наступного запуску. */
  updateDismissed: false,
  /** Іде перевірка кнопкою «Синхронізувати» (повторні кліки ігноруються). */
  syncing: false,
  /** Шаблони дзеркал контенту — для картинок збірок із GitHub. */
  mirrors: null,
  /** Назва готової збірки, яка зараз встановлюється (для прогресу в «острові»). */
  modpackName: "",
  /** Відкрито системний діалог вибору файлу/папки (повторні кліки ігноруються). */
  picking: false,
  /** Акаунт Terrapolis (AccountView з бекенда): @username, синхронізація, конфлікт імені. */
  account: null,
  /** Остання відповідь social_snapshot (друзі, заявки, запити доступу). */
  social: null,
  /** performance.now() у момент отримання social — для живих таймерів гри. */
  socialAt: 0,
  /** Хмара Terrapolis відповідає (null — ще невідомо). */
  cloudOnline: null
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

/* Кольорові іконки завантажувачів (власні спрощені малюнки, не логотипи). */
const LOADER_ICONS = {
  vanilla:
    '<svg viewBox="0 0 24 24"><path fill="#6e4529" d="M21 7v10l-9 5V12z"/><path fill="#8a5a35" d="M3 7v10l9 5V12z"/><path fill="#5fb547" d="M12 2l9 5-9 5-9-5z"/><path fill="#4c9a38" d="M3 7l9 5v2.6L3 9.6z"/><path fill="#3f8530" d="M21 7l-9 5v2.6l9-5z"/></svg>',
  fabric:
    '<svg viewBox="0 0 24 24"><rect x="4.5" y="4.5" width="15" height="15" rx="2.5" transform="rotate(45 12 12)" fill="#dbd0b4" stroke="#38342a" stroke-width="1.4"/><rect x="7.3" y="7.3" width="9.4" height="9.4" rx="1.2" transform="rotate(45 12 12)" fill="none" stroke="#38342a" stroke-width="1.1" stroke-dasharray="1.6 1.4"/></svg>',
  forge:
    '<svg viewBox="0 0 24 24"><path fill="#8d99aa" d="M2 6h14c0 2.6 2.1 3.9 6 3.9v1.7c-3.6 0-6.4.9-8 2.4H9.2C7.6 12.4 5.2 11.2 2 10.8z"/><path fill="#6c7787" d="M9.2 14H14v3H9.2z"/><path fill="#4e5866" d="M6 17h11.2l1.6 3.2H4.4z"/></svg>',
  neoforge:
    '<svg viewBox="0 0 24 24"><path fill="#f0953f" d="M2 6h14c0 2.6 2.1 3.9 6 3.9v1.7c-3.6 0-6.4.9-8 2.4H9.2C7.6 12.4 5.2 11.2 2 10.8z"/><path fill="#cf6f26" d="M9.2 14H14v3H9.2z"/><path fill="#9c4f1b" d="M6 17h11.2l1.6 3.2H4.4z"/><path fill="#ffd27a" d="M19 1.5l.8 1.7 1.7.8-1.7.8-.8 1.7-.8-1.7-1.7-.8 1.7-.8z"/></svg>',
  quilt:
    '<svg viewBox="0 0 24 24"><rect x="3" y="3" width="8" height="8" rx="2" fill="#9b6fe0"/><rect x="13" y="3" width="8" height="8" rx="2" fill="#7a52c9"/><rect x="3" y="13" width="8" height="8" rx="2" fill="#7a52c9"/><rect x="13" y="13" width="8" height="8" rx="4" fill="#c4a8f3"/></svg>'
};

function loaderIcon(loader, cls = "") {
  const key = String(loader || "vanilla").toLowerCase();
  return `<span class="ldr${cls ? " " + cls : ""}" title="${esc(loaderName(key))}">${LOADER_ICONS[key] || LOADER_ICONS.vanilla}</span>`;
}

function formatCount(n) {
  if (n >= 1e6) return (Math.round(n / 1e5) / 10).toString().replace(".", lang === "uk" ? "," : ".") + "M";
  if (n >= 1e3) return Math.round(n / 1e3) + "K";
  return String(n || 0);
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
  else if (S.view?.local_only) setIsland("warn", t("status.localOnly"), { sticky: true });
  else if (S.view?.offline) setIsland("warn", t("status.offline"), { sticky: true });
  else if (S.view) setIsland("ok", t("status.ready"), { sticky: true });
}

function updateCalm() {
  document.body.classList.toggle("calm", Boolean(S.running) || document.hidden);
  // Слайд-шоу фону: пауза у згорнутому вікні та під час гри, відновлення після повернення.
  syncSlideshow();
}

/* ═══════════════════════════ КАРТИНКИ З РЕПОЗИТОРІЮ ═══════════════════════════ */

// Знайдену адресу пам'ятаємо між запусками (localStorage), а декодовані картинки —
// у пам'яті, тож перемикання збірок відбувається миттєво й без мерехтіння.
const ASSET_TTL = 6 * 60 * 60 * 1000;
const ASSET_MISS_TTL = 30 * 60 * 1000;
const assetJobs = new Map();
const assetResolved = new Map();
const assetReady = new Map();

function storageGet(key) {
  try {
    return JSON.parse(localStorage.getItem(key) || "null");
  } catch {
    return null;
  }
}

function storageSet(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* сховище недоступне — працюємо без нього */
  }
}

function loadImage(url) {
  if (assetReady.has(url)) return Promise.resolve(true);
  return new Promise((resolve) => {
    const image = new Image();
    image.decoding = "async";
    image.onload = () => {
      const done = () => {
        assetReady.set(url, image);
        resolve(true);
      };
      image.decode().then(done, done);
    };
    image.onerror = () => resolve(false);
    image.src = url;
  });
}

function assetList(candidates) {
  return candidates.filter((url) => typeof url === "string" && url.startsWith("https://"));
}

function assetKey(candidates) {
  return assetList(candidates).join("|");
}

const RAW_RE = /^https:\/\/raw\.githubusercontent\.com\/([\w.-]+)\/([\w.-]+)\/([\w.-]+)\/(.+)$/;

/** Ті самі картинки з дзеркал (jsDelivr тощо) — якщо raw.githubusercontent.com заблоковано. */
function mirrorVariants(url) {
  const match = RAW_RE.exec(url);
  if (!match || !S.mirrors?.content) return [];
  const [, owner, repo, branch, path] = match;
  return S.mirrors.content
    .map((template) =>
      template.includes("{url}")
        ? template.replace("{url}", url)
        : template.replace("{owner}", owner).replace("{repo}", repo).replace("{branch}", branch).replace(/\/+$/, "") + "/" + path
    )
    .filter((candidate) => candidate !== url && candidate.startsWith("https://"));
}

/** Спершу всі варіанти з основного джерела, потім — з дзеркал. */
function assetTries(list) {
  const out = [...list];
  for (const url of list) for (const mirror of mirrorVariants(url)) if (!out.includes(mirror)) out.push(mirror);
  return out;
}

/** Перша картинка зі списку, яка справді існує (решта — 404). */
function resolveAsset(candidates) {
  const list = assetList(candidates);
  const key = list.join("|");
  if (!list.length) {
    assetResolved.set(key, null);
    return Promise.resolve(null);
  }
  if (assetJobs.has(key)) return assetJobs.get(key);
  const job = (async () => {
    const saved = storageGet("asset:" + key);
    const age = saved ? Date.now() - saved.at : Infinity;
    let url = null;
    if (saved && saved.url === "" && age < ASSET_MISS_TTL) url = null;
    else if (saved?.url && age < ASSET_TTL && (await loadImage(saved.url))) url = saved.url;
    else {
      for (const candidate of assetTries(list)) {
        if (await loadImage(candidate)) {
          url = candidate;
          break;
        }
      }
      storageSet("asset:" + key, { url: url || "", at: Date.now() });
    }
    assetResolved.set(key, url);
    return url;
  })();
  assetJobs.set(key, job);
  return job;
}

/** Одиночні фони (пріоритет №2): `image` з instances.json, далі bg.png, bg.jpg, … — як FALLBACK_BACKGROUNDS у project.rs. */
const FALLBACK_BACKGROUNDS = ["bg.png", "bg.jpg", "background.png", "background.jpg", "background.webp", "banner.png", "banner.jpg"];
/** Кадр слайд-шоу (пріоритет №1): `1.png`, `2.png`, …, `10.png`. */
const FRAME_RE = /^[1-9]\d*\.png$/;

function instanceBackgrounds(inst) {
  const base = inst?.asset_base || "";
  return [inst?.image, ...(base ? FALLBACK_BACKGROUNDS.map((name) => base + name) : [])];
}

/* ── фони збірки: серія 1.png..N.png або один фон ── */

// Серії кешуються в пам'яті (між перемиканнями вкладок і збірок) і в localStorage (між запусками):
// знайдена серія — 6 годин, «серії немає» — 30 хвилин. Окремі кадри (і їхні 404) додатково
// кешує resolveAsset, тож повторні перевірки не ходять у мережу.
const bgSeries = new Map();

function backgroundKey(inst) {
  const listed = Array.isArray(inst?.backgrounds) ? inst.backgrounds.join(",") : "?";
  return [inst?.asset_base || "", inst?.image || "", listed].join("|");
}

/**
 * Опис фонів збірки. `frames` — адреси кадрів 1.png..N.png (raw; дзеркала підставляє resolveAsset),
 * `complete` — серію вже повністю знайдено (список із GitHub API, кеш або 404 на наступному номері),
 * `fallback` — кандидати одиночного фону, якщо серії немає.
 */
function seriesFor(inst) {
  const key = backgroundKey(inst);
  let series = bgSeries.get(key);
  if (series) return series;
  const base = inst?.asset_base || "";
  const listed = Array.isArray(inst?.backgrounds) ? inst.backgrounds : null;
  series = { key, base, frames: [], complete: true, probe: false, fallback: instanceBackgrounds(inst), first: null, firstUrl: undefined, probing: null };
  if (base && listed) {
    // Папку збірки видно в GitHub API — бекенд уже обрав фони, нічого не перевіряємо навмання.
    if (listed[0] === "1.png") series.frames = listed.filter((name) => FRAME_RE.test(name)).map((name) => base + name);
    else series.fallback = [inst.image, ...listed.map((name) => base + name)];
  } else if (base) {
    // Список файлів невідомий: 1.png, 2.png, … перевіряються по черзі до першого 404.
    series.probe = true;
    const saved = storageGet("bgset:" + key);
    const fresh = saved && Array.isArray(saved.frames) && Date.now() - saved.at < (saved.frames.length ? ASSET_TTL : ASSET_MISS_TTL);
    if (fresh) series.frames = saved.frames.filter((url) => typeof url === "string" && url.startsWith(base));
    else series.complete = false;
  }
  bgSeries.set(key, series);
  return series;
}

function saveSeries(series) {
  if (series.probe && series.complete) storageSet("bgset:" + series.key, { frames: series.frames, at: Date.now() });
}

/** Перевіряє наступний номер серії (N+1).png; 404 — серія закінчилась. */
function discoverNext(series) {
  if (series.complete) return Promise.resolve();
  if (series.probing) return series.probing;
  series.probing = (async () => {
    const raw = series.base + (series.frames.length + 1) + ".png";
    if (await resolveAsset([raw])) series.frames.push(raw);
    else {
      series.complete = true;
      saveSeries(series);
    }
    series.probing = null;
  })();
  return series.probing;
}

/** Перший кадр (1.png) або одиночний фон; `null` — фону немає. Результат кешується в series. */
function firstBackground(series) {
  if (!series.first) {
    series.first = (async () => {
      if (!series.frames.length && !series.complete) await discoverNext(series);
      if (series.frames.length) {
        const url = await resolveAsset([series.frames[0]]);
        if (url) return url;
        // 1.png зник із репозиторію — серії більше немає.
        series.frames = [];
        series.complete = true;
        saveSeries(series);
      }
      return resolveAsset(series.fallback);
    })().then((url) => (series.firstUrl = url || null));
  }
  return series.first;
}

function instanceIcons(inst) {
  return inst?.asset_base ? [inst.asset_base + "icon.png"] : [];
}

function projectIcons() {
  if (S.view?.icon) return [S.view.icon];
  const index = S.binding?.index_url || "";
  return index.startsWith("https://") ? [index.replace(/[^/]*$/, "icon.png")] : [];
}

/** Іконка проєкту/збірки; поки картинки немає (або 404) — акуратна літера. */
function paintAvatar(node, letter, candidates) {
  if (!node) return;
  const key = assetKey(candidates);
  node.dataset.asset = key;
  let label = node.querySelector(".av-l");
  if (!label) {
    node.textContent = "";
    label = document.createElement("span");
    label.className = "av-l";
    node.append(label);
  }
  label.textContent = letter;
  const apply = (url) => {
    if (node.dataset.asset !== key) return;
    let img = node.querySelector("img");
    if (!url) {
      img?.remove();
      node.classList.remove("has-img");
      return;
    }
    if (!img) {
      img = document.createElement("img");
      img.alt = "";
      img.draggable = false;
      node.append(img);
    }
    if (img.getAttribute("src") !== url) {
      img.classList.toggle("ready", assetReady.has(url));
      img.onload = () => img.classList.add("ready");
      img.src = url;
    }
    node.classList.add("has-img");
  };
  if (assetResolved.has(key)) apply(assetResolved.get(key));
  else {
    apply(null);
    resolveAsset(candidates).then(apply);
  }
}

/** Приглушена мініатюра фону збірки в картці списку. */
function paintArt(node, inst) {
  if (!node) return;
  const apply = (url) => {
    if (!url || !node.isConnected) return;
    node.style.backgroundImage = 'url("' + url.replace(/["\\\n]/g, "") + '")';
    node.classList.add("ready");
  };
  // У картці — лише перший кадр серії (1.png) або одиночний фон, без слайд-шоу.
  const series = seriesFor(inst);
  if (series.firstUrl !== undefined) apply(series.firstUrl);
  else firstBackground(series).then(apply);
}

function initial(text) {
  return (String(text || "").trim().slice(0, 1) || "T").toUpperCase();
}

/** Фони й іконки всіх збірок — заздалегідь, щоб перемикання було миттєвим. */
function prefetchAssets() {
  resolveAsset(projectIcons());
  for (const inst of S.view?.instances || []) {
    resolveAsset(instanceIcons(inst));
    // Лише перший кадр: решту серії слайд-шоу довантажує поступово, для активної збірки.
    firstBackground(seriesFor(inst));
  }
}

/* фон Hero: два шари (подвійний буфер), перехід лише через opacity/transform */
let heroLayer = 0;
let heroUrl = null;
let heroHideTimer = 0;
/** Скільки триває кросфейд кадру (має збігатися з .hero-media.is-slideshow .hero-bg у style.css) + запас. */
const HERO_FADE_MS = 1300;

/**
 * Справжній кросфейд: новий кадр проявляється ПОВЕРХ старого, а старий шар ховається лише після
 * завершення переходу — без «провалу» в темряву посередині. Картинка до цього вже декодована (loadImage).
 */
function showHeroImage(url) {
  if (url === heroUrl) return;
  heroUrl = url;
  const layers = el.heroMedia.children;
  const previous = layers[heroLayer];
  clearTimeout(heroHideTimer);
  if (!url) {
    previous.classList.remove("ready");
    el.hero.classList.remove("has-image");
    return;
  }
  heroLayer ^= 1;
  const next = layers[heroLayer];
  // Шар, що повертається з-під низу, миттєво скидаємо в opacity: 0 — без зворотного переходу.
  next.classList.add("instant");
  next.classList.remove("ready", "front");
  next.style.backgroundImage = 'url("' + url.replace(/["\\\n]/g, "") + '")';
  void next.offsetWidth;
  next.classList.remove("instant");
  next.classList.add("front", "ready");
  previous.classList.remove("front");
  heroHideTimer = setTimeout(() => {
    if (layers[heroLayer] === next) previous.classList.remove("ready");
  }, HERO_FADE_MS);
  el.hero.classList.add("has-image");
}

/* ── слайд-шоу з 1.png..N.png ── */

const SLIDE_MS = 7000;
const SLIDE_RETRY_MS = 2500;
const slide = { series: null, index: 0, timer: 0, token: 0, busy: false };

/** Таймер працює лише у видимому вікні, без запущеної гри і без prefers-reduced-motion. */
function slideshowAllowed() {
  return !document.hidden && !S.running && !reduceMotion.matches;
}

function stopSlideshow() {
  clearTimeout(slide.timer);
  slide.timer = 0;
}

function scheduleSlide(delay = SLIDE_MS) {
  stopSlideshow();
  const series = slide.series;
  if (!series || series.frames.length < 2 || !slideshowAllowed()) return;
  slide.timer = setTimeout(nextSlide, delay);
}

/** Пауза/відновлення: виклик із updateCalm (згортання вікна, гра) і зміни prefers-reduced-motion. */
function syncSlideshow() {
  el.heroMedia.classList.toggle("is-slideshow", (slide.series?.frames.length || 0) >= 2);
  if (!slideshowAllowed()) stopSlideshow();
  else if (!slide.timer && !slide.busy) scheduleSlide();
}

/** Індекс наступного кадру; за потреби спершу перевіряє, чи існує (N+1).png. */
async function nextIndex(series, from) {
  const next = from + 1;
  if (next >= series.frames.length && !series.complete) await discoverNext(series);
  return next < series.frames.length ? next : 0;
}

/** Попереднє завантаження наступного кадру у фоні, поки показується поточний. */
async function preloadAhead(series, token) {
  const next = await nextIndex(series, slide.index);
  if (slide.token === token && series.frames[next]) resolveAsset([series.frames[next]]);
  if (slide.token === token) syncSlideshow();
}

async function nextSlide() {
  slide.timer = 0;
  const { series, token } = slide;
  if (!series || slide.busy || !slideshowAllowed()) return;
  slide.busy = true;
  try {
    const next = await nextIndex(series, slide.index);
    // Перехід стартує лише тоді, коли кадр уже завантажено й декодовано (new Image() у loadImage).
    const url = await resolveAsset([series.frames[next]]);
    if (slide.token !== token) return;
    slide.index = next;
    if (!url) {
      scheduleSlide(SLIDE_RETRY_MS);
      return;
    }
    if (!slideshowAllowed()) return;
    showHeroImage(url);
    preloadAhead(series, token);
    scheduleSlide();
  } finally {
    if (slide.token === token) slide.busy = false;
  }
}

function paintHeroImage(inst) {
  if (!inst) {
    stopSlideshow();
    slide.series = null;
    slide.token++;
    slide.busy = false;
    syncSlideshow();
    return showHeroImage(null);
  }
  const series = seriesFor(inst);
  // Та сама збірка (перемальовування картки, оновлення стану) — слайд-шоу не перезапускаємо.
  if (slide.series === series) return syncSlideshow();
  stopSlideshow();
  slide.series = series;
  slide.index = 0;
  slide.busy = false;
  const token = ++slide.token;
  syncSlideshow();
  const show = (url) => {
    if (slide.token !== token) return;
    showHeroImage(url);
    if (series.frames.length) preloadAhead(series, token);
  };
  // Попередній фон лишається, доки новий не готовий: кросфейд без «провалу» в темряву.
  if (series.firstUrl !== undefined) show(series.firstUrl);
  else firstBackground(series).then(show);
}

reduceMotion.addEventListener?.("change", syncSlideshow);

/* ═══════════════════════════ ЛЮДСЬКІ ПОМИЛКИ ═══════════════════════════ */

const ERROR_RULES = [
  [/os error 112|not enough space|no space left|недостатньо місця/i, "err.disk"],
  [/os error 5\b|os error 32\b|access is denied|permission denied|відмовлено в доступі|being used by another process/i, "err.access"],
  [/контрольна сума|checksum|hash mismatch/i, "err.corrupt"],
  [/\b451\b/, "err.blocked"],
  [/\b(403|429)\b|rate limit/i, "err.rateLimit"],
  [/\b404\b/, "err.notFound"],
  [/server error|\b50[0-9]\b/i, "err.server"],
  [/timed out|timeout/i, "err.timeout"],
  [/error sending request|dns error|failed to lookup|connection refused|connection reset|connect error|network is unreachable|os error 1100[1-4]|os error 1006[0-5]/i, "err.offline"]
];

/** Технічну помилку (Rust/HTTP) перекладаємо людською мовою; оригінал — у «подробицях». */
function humanError(error) {
  const raw = errText(error);
  for (const [pattern, key] of ERROR_RULES) {
    if (pattern.test(raw)) return { text: t(key), raw };
  }
  return { text: raw, raw: "" };
}

function errHuman(error) {
  return humanError(error).text;
}

/* ═══════════════════════════ МОДАЛЬНЕ ВІКНО ═══════════════════════════ */

let modalResolve = null;
let modalReturnFocus = null;
let modalCloseTimer = 0;

function closeModal(result) {
  if (el.modal.hidden || el.modal.classList.contains("closing")) return;
  const resolve = modalResolve;
  modalResolve = null;
  Dropdown.closeAll();
  hideTip();
  const finish = () => {
    modalCloseTimer = 0;
    el.modal.hidden = true;
    el.modal.classList.remove("closing", "xl");
    el.modal.dataset.kind = "";
    modalReturnFocus?.focus?.();
  };
  // Таймер, а не animationend: у згорнутому вікні кадри не малюються, і подія може не прийти.
  if (reduceMotion.matches || document.hidden) finish();
  else {
    el.modal.classList.add("closing");
    modalCloseTimer = setTimeout(finish, 180);
  }
  resolve?.(result);
}

/**
 * openModal({icon, tone, title, html, buttons:[{id,label,kind,left,onClick}]}) → Promise<id|null>
 * onClick може повернути false, щоб вікно лишилося відкритим.
 */
function openModal({ icon = "", tone = "", title, html, buttons, wide = false, xl = false, kind = "" }) {
  if (modalResolve) closeModal(null);
  // Нове вікно під час анімації закриття попереднього: скасовуємо приховування.
  clearTimeout(modalCloseTimer);
  modalCloseTimer = 0;
  el.modal.classList.remove("closing");
  if (el.modal.hidden) modalReturnFocus = document.activeElement;
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
  el.modal.classList.toggle("wide", wide || xl);
  el.modal.classList.toggle("xl", xl);
  el.modal.dataset.kind = kind;
  el.modal.hidden = false;
  el.modalText.scrollTop = 0;
  el.modal.querySelector(".modal-card").scrollTop = 0;
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
  const { text, raw } = humanError(error);
  const details = raw ? `<details class="err-details"><summary>${esc(t("err.details"))}</summary><pre>${esc(raw)}</pre></details>` : "";
  return openModal({
    icon: "alert",
    tone: "danger",
    title: t("error.title"),
    html: `<p>${esc(text)}</p>${details}`,
    buttons: [
      {
        id: "copy",
        label: t("error.copy"),
        kind: "ghost",
        left: true,
        onClick: async (button) => {
          if (await copyText(raw || text)) button.textContent = t("crash.copied");
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

/* ═══════════════════════════ АНІМАЦІЇ: ДОПОМІЖНЕ ═══════════════════════════ */

/** Плавне зникнення через CSS-клас; завершення — за таймером, тож у згорнутому
 *  вікні (де кадри не малюються й animationend не приходить) нічого не «зависає». */
function leave(node, className, ms, done) {
  if (!node) return;
  if (reduceMotion.matches || document.hidden) {
    done?.();
    return;
  }
  node.classList.add(className);
  setTimeout(() => done?.(), ms);
}

/* ═══════════════════════════ ВИПАДНІ СПИСКИ ═══════════════════════════ */

const BADGE_KINDS = new Set(["release", "snapshot", "beta", "alpha", "latest", "recommended", "auto"]);

function badgesHtml(badges) {
  if (!badges?.length) return "";
  return `<span class="vbadges">${badges
    .map((badge) => `<span class="vbadge ${BADGE_KINDS.has(badge.kind) ? badge.kind : ""}">${esc(badge.text)}</span>`)
    .join("")}</span>`;
}

/**
 * Стилізований список вибору замість системного <select>: пошук/фільтр, бейджі типу
 * релізу, підсвічена активна версія, клавіатура (↑↓ Enter Esc Home End), плавне відкриття.
 * items: [{ value, label, sub?, badges?: [{text, kind}], search? }]
 */
class Dropdown {
  static current = null;

  static closeAll() {
    Dropdown.current?.close();
  }

  constructor(opts = {}) {
    this.opts = { placeholder: "—", search: false, ...opts };
    this.items = opts.items || [];
    this.value = opts.value ?? "";
    this.loading = false;
    this.pending = false;
    this.disabled = false;
    this.panel = null;
    this.active = -1;
    this.filter = "";
    this.root = document.createElement("div");
    this.root.className = "dd" + (opts.className ? " " + opts.className : "");
    this.trigger = document.createElement("button");
    this.trigger.type = "button";
    this.trigger.className = "dd-trigger";
    this.trigger.setAttribute("aria-haspopup", "listbox");
    this.trigger.setAttribute("aria-expanded", "false");
    if (opts.label) this.trigger.setAttribute("aria-label", opts.label);
    this.root.append(this.trigger);
    this.trigger.addEventListener("click", () => (this.panel ? this.close() : this.open()));
    this.trigger.addEventListener("keydown", (event) => {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(event.key) && !this.panel) {
        event.preventDefault();
        this.open();
      }
    });
    this.render();
  }

  mount(node) {
    node.replaceChildren(this.root);
    return this;
  }

  get selected() {
    return this.items.find((item) => item.value === this.value) || null;
  }

  setItems(items, value) {
    this.items = items || [];
    if (value !== undefined) this.value = value;
    else if (!this.items.some((item) => item.value === this.value)) this.value = this.items[0]?.value ?? "";
    this.loading = false;
    this.pending = false;
    this.render();
    if (this.panel) {
      this.active = Math.max(0, this.visible().findIndex((item) => item.value === this.value));
      this.renderList();
      this.position();
    }
  }

  setValue(value) {
    this.value = value;
    this.render();
  }

  /** Триггер у стані «завантаження» (список ще не отримано). */
  setLoading(on) {
    this.loading = on;
    if (on) this.close();
    this.render();
  }

  /** Список уже відкрито, а елементи довантажуються. */
  setPending(on) {
    this.pending = on;
    if (this.panel) this.renderList();
  }

  setDisabled(on) {
    this.disabled = on;
    if (on) this.close();
    this.render();
  }

  render() {
    const item = this.selected;
    const text = this.loading ? this.opts.loadingText || t("dd.loading") : item ? item.label : this.items.length ? this.opts.placeholder : this.opts.empty || this.opts.placeholder;
    this.trigger.disabled = this.disabled || this.loading || (!this.items.length && !this.opts.onOpen);
    this.root.classList.toggle("is-loading", this.loading);
    const shown = item && !this.loading;
    // Іконка (HTML лише з коду лаунчера, напр. loaderIcon) і короткий опис під назвою — для «багатих» списків.
    const icon = shown && item.icon ? `<span class="dd-ico" aria-hidden="true">${item.icon}</span>` : "";
    const label = shown && this.opts.showSub && item.sub ? `<span class="dd-stack"><span class="dd-text">${esc(text)}</span><small class="dd-sub">${esc(item.sub)}</small></span>` : `<span class="dd-text">${esc(text)}</span>`;
    this.trigger.innerHTML = `<span class="dd-value">${icon}${label}${shown && this.opts.triggerBadges !== false ? badgesHtml(item.badges) : ""}</span>${
      this.loading ? '<span class="btn-spin" aria-hidden="true"></span>' : `<span class="dd-chev" aria-hidden="true">${ICONS.chevron}</span>`
    }`;
  }

  visible() {
    const query = this.filter.trim().toLowerCase();
    if (!query) return this.items;
    return this.items.filter((item) => [item.label, item.sub, item.search].filter(Boolean).join(" ").toLowerCase().includes(query));
  }

  open() {
    if (this.panel || this.trigger.disabled) return;
    Dropdown.closeAll();
    hideTip();
    const panel = document.createElement("div");
    panel.className = "dd-panel" + (this.opts.className ? " " + this.opts.className + "-panel" : "");
    panel.innerHTML =
      (this.opts.search
        ? `<div class="dd-search">${ICONS.search}<input type="text" spellcheck="false" autocomplete="off" aria-label="${esc(this.opts.searchPh || t("dd.search"))}" placeholder="${esc(this.opts.searchPh || t("dd.search"))}"></div>`
        : "") + '<div class="dd-list" role="listbox" tabindex="-1"></div>';
    document.body.append(panel);
    this.panel = panel;
    this.list = panel.querySelector(".dd-list");
    this.filter = "";
    Dropdown.current = this;
    this.trigger.setAttribute("aria-expanded", "true");
    this.root.classList.add("open");
    this.opts.onOpen?.(this);
    this.active = Math.max(0, this.visible().findIndex((item) => item.value === this.value));
    this.renderList();
    this.position();

    const input = panel.querySelector("input");
    if (input) {
      input.addEventListener("input", () => {
        this.filter = input.value;
        this.active = 0;
        this.renderList();
      });
      input.focus();
    } else this.list.focus();
    panel.addEventListener("keydown", (event) => this.onKey(event));
    this.list.addEventListener("click", (event) => {
      const option = event.target.closest("[data-index]");
      if (option) this.pick(this.shown[Number(option.dataset.index)]);
    });
    this.list.addEventListener("pointermove", (event) => {
      const option = event.target.closest("[data-index]");
      if (!option) return;
      const index = Number(option.dataset.index);
      if (index !== this.active) this.highlight(index, false);
    });
    const selected = this.list.querySelector(".dd-opt.on");
    if (selected) this.list.scrollTop = selected.offsetTop - this.list.clientHeight / 2 + selected.offsetHeight / 2;
  }

  position() {
    const panel = this.panel;
    if (!panel) return;
    const rect = this.trigger.getBoundingClientRect();
    const width = Math.min(Math.max(rect.width, this.opts.minWidth || 0), window.innerWidth - 16);
    panel.style.width = width + "px";
    panel.style.maxHeight = "";
    const natural = panel.offsetHeight;
    const below = window.innerHeight - rect.bottom - 10;
    const above = rect.top - 10;
    const up = natural > below && above > below;
    const room = Math.max(140, up ? above : below);
    if (natural > room) panel.style.maxHeight = room + "px";
    const height = Math.min(natural, room);
    let left = rect.left;
    if (left + width > window.innerWidth - 8) left = rect.right - width;
    left = Math.max(8, Math.min(left, window.innerWidth - width - 8));
    panel.style.left = left + "px";
    panel.style.top = (up ? rect.top - height - 6 : rect.bottom + 6) + "px";
    panel.classList.toggle("up", up);
  }

  renderList() {
    if (!this.list) return;
    const items = this.visible();
    this.shown = items.slice(0, 500);
    if (this.pending && !items.length) {
      this.list.innerHTML = `<div class="dd-empty"><span class="btn-spin" aria-hidden="true"></span>${esc(this.opts.pendingText || t("dd.loading"))}</div>`;
      return;
    }
    if (!items.length) {
      this.list.innerHTML = `<div class="dd-empty">${esc(this.opts.emptyList || t("dd.nothing"))}</div>`;
      return;
    }
    this.list.innerHTML =
      this.shown
        .map((item, index) => {
          const on = item.value === this.value;
          return `<div class="dd-opt${on ? " on" : ""}${index === this.active ? " active" : ""}" role="option" aria-selected="${on}" data-index="${index}">
            ${item.icon ? `<span class="dd-ico" aria-hidden="true">${item.icon}</span>` : ""}<span class="dd-opt-main"><span class="dd-opt-label">${esc(item.label)}</span>${item.sub ? `<small>${esc(item.sub)}</small>` : ""}</span>
            ${badgesHtml(item.badges)}<span class="dd-check" aria-hidden="true">${on ? ICONS.check : ""}</span>
          </div>`;
        })
        .join("") + (this.pending ? `<div class="dd-empty"><span class="btn-spin" aria-hidden="true"></span>${esc(this.opts.pendingText || t("dd.loading"))}</div>` : "");
  }

  highlight(index, scroll = true) {
    const options = this.list.querySelectorAll(".dd-opt");
    if (!options.length) return;
    this.active = Math.max(0, Math.min(options.length - 1, index));
    options.forEach((node, i) => node.classList.toggle("active", i === this.active));
    if (!scroll) return;
    // Прокручуємо лише сам список (scrollIntoView зачепив би й зовнішні контейнери).
    const node = options[this.active];
    const list = this.list;
    if (node.offsetTop < list.scrollTop) list.scrollTop = node.offsetTop;
    else if (node.offsetTop + node.offsetHeight > list.scrollTop + list.clientHeight) list.scrollTop = node.offsetTop + node.offsetHeight - list.clientHeight;
  }

  onKey(event) {
    const page = 8;
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        this.highlight(this.active + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        this.highlight(this.active - 1);
        break;
      case "PageDown":
        event.preventDefault();
        this.highlight(this.active + page);
        break;
      case "PageUp":
        event.preventDefault();
        this.highlight(this.active - page);
        break;
      case "Home":
        if (!this.opts.search) {
          event.preventDefault();
          this.highlight(0);
        }
        break;
      case "End":
        if (!this.opts.search) {
          event.preventDefault();
          this.highlight(Infinity);
        }
        break;
      case "Enter":
        event.preventDefault();
        if (this.shown?.[this.active]) this.pick(this.shown[this.active]);
        break;
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        this.close(true);
        break;
      case "Tab":
        this.close();
        break;
    }
  }

  pick(item) {
    if (!item) return;
    const changed = item.value !== this.value;
    this.value = item.value;
    this.render();
    this.close(true);
    if (changed) this.opts.onChange?.(item.value, item);
  }

  close(focusTrigger = false) {
    const panel = this.panel;
    if (!panel) return;
    this.panel = null;
    this.list = null;
    if (Dropdown.current === this) Dropdown.current = null;
    this.trigger.setAttribute("aria-expanded", "false");
    this.root.classList.remove("open");
    leave(panel, "closing", 150, () => panel.remove());
    if (focusTrigger) this.trigger.focus();
  }
}

document.addEventListener(
  "pointerdown",
  (event) => {
    const open = Dropdown.current;
    if (open && !open.panel?.contains(event.target) && !open.root.contains(event.target)) open.close();
  },
  true
);
document.addEventListener(
  "scroll",
  (event) => {
    const open = Dropdown.current;
    if (open && !open.panel?.contains(event.target)) open.close();
  },
  true
);

/* ═══════════════════════════ ПІДКАЗКИ ═══════════════════════════ */

// Системні title-підказки не анімуються, тож замінюємо їх власними: затримка, плавна поява.
let tipTarget = null;
let tipTimer = 0;

function hideTip() {
  clearTimeout(tipTimer);
  tipTarget = null;
  if (el.tip.hidden || el.tip.classList.contains("leaving")) return;
  leave(el.tip, "leaving", 120, () => {
    if (!tipTarget) el.tip.hidden = true;
    el.tip.classList.remove("leaving");
  });
}

function showTip(target) {
  const text = target.dataset.tip;
  if (!text || !target.isConnected) return;
  el.tip.textContent = text;
  el.tip.classList.remove("leaving", "below");
  el.tip.hidden = false;
  const rect = target.getBoundingClientRect();
  const width = el.tip.offsetWidth;
  const height = el.tip.offsetHeight;
  let top = rect.top - height - 8;
  if (top < 6) {
    top = rect.bottom + 8;
    el.tip.classList.add("below");
  }
  const left = Math.min(Math.max(6, rect.left + rect.width / 2 - width / 2), window.innerWidth - width - 6);
  el.tip.style.transform = `translate3d(${Math.round(left)}px, ${Math.round(top)}px, 0)`;
  el.tip.classList.remove("show");
  void el.tip.offsetWidth;
  el.tip.classList.add("show");
}

document.addEventListener("pointerover", (event) => {
  const target = event.target.closest?.("[title], [data-tip]");
  if (!target || target === tipTarget || target.closest(".dd-panel")) return;
  if (target.hasAttribute("title")) {
    const text = target.getAttribute("title");
    target.removeAttribute("title");
    if (!text) return;
    target.dataset.tip = text;
    if (!target.hasAttribute("aria-label") && !target.textContent.trim()) target.setAttribute("aria-label", text);
  }
  clearTimeout(tipTimer);
  if (!el.tip.hidden) el.tip.hidden = true;
  tipTarget = target;
  tipTimer = setTimeout(() => tipTarget === target && showTip(target), 420);
});
document.addEventListener("pointerout", (event) => {
  if (tipTarget && !tipTarget.contains(event.relatedTarget)) hideTip();
});
document.addEventListener("pointerdown", hideTip, true);
document.addEventListener("keydown", hideTip, true);
document.addEventListener("wheel", hideTip, { passive: true, capture: true });

/* ═══════════════════════════ СПОВІЩЕННЯ ═══════════════════════════ */

/**
 * toast({ id, icon, tone, title, text, html, actions:[{label, kind, onClick}], timeout })
 * onClick може повернути false, щоб сповіщення лишилось.
 */
function toast({ id = "", icon = "", tone = "", title = "", text = "", html = "", actions = [], timeout = 0 }) {
  if (id) dismissToast($("toast-" + id), true);
  const node = document.createElement("div");
  node.className = "toast" + (tone ? " " + tone : "");
  if (id) node.id = "toast-" + id;
  node.setAttribute("role", "status");
  node.innerHTML = `${icon ? `<span class="toast-icon">${ICONS[icon] || ""}</span>` : ""}
    <div class="toast-body">${title ? `<b class="toast-title">${esc(title)}</b>` : ""}${text ? `<p class="toast-text">${esc(text)}</p>` : ""}${html}
      ${actions.length ? '<div class="toast-actions"></div>' : ""}</div>
    <button class="toast-x" type="button" aria-label="${esc(t("common.close"))}">${ICONS.close}</button>`;
  const row = node.querySelector(".toast-actions");
  for (const spec of actions) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn btn-sm " + ({ primary: "btn-primary", ghost: "btn-ghost", plain: "btn-plain" }[spec.kind] || "btn-ghost");
    button.textContent = spec.label;
    if (spec.id) button.dataset.id = spec.id;
    button.addEventListener("click", async () => {
      const keep = (await spec.onClick?.(button, node)) === false;
      if (!keep) dismissToast(node);
    });
    row.append(button);
  }
  node.querySelector(".toast-x").addEventListener("click", () => dismissToast(node));
  el.toasts.append(node);
  if (timeout) node.dataset.timer = String(setTimeout(() => dismissToast(node), timeout));
  return node;
}

function dismissToast(node, instant = false) {
  if (!node || node.classList.contains("leaving")) return;
  clearTimeout(Number(node.dataset.timer || 0));
  node.dataset.dismissed = "1";
  if (instant) {
    node.remove();
    return;
  }
  leave(node, "leaving", 260, () => node.remove());
}

/* ═══════════════════════════ MARKDOWN (CHANGELOG) ═══════════════════════════ */

function mdInline(text) {
  return esc(text)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
    .replace(/__([^_]+)__/g, "<b>$1</b>")
    .replace(/(^|[\s(])\*([^*\s][^*]*?)\*(?=[\s).,:;!?]|$)/g, "$1<i>$2</i>")
    .replace(/~~([^~]+)~~/g, "<s>$1</s>")
    .replace(/\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g, '<span class="md-link" title="$2">$1</span>');
}

/** Безпечний рендер Markdown релізу: спершу екранування, потім лише базова розмітка. */
function renderMarkdown(source) {
  const lines = String(source || "").replace(/\r\n?/g, "\n").split("\n");
  const out = [];
  let paragraph = [];
  let list = null;
  let code = null;
  const flushParagraph = () => {
    if (paragraph.length) out.push(`<p>${paragraph.map(mdInline).join("<br>")}</p>`);
    paragraph = [];
  };
  const flushList = () => {
    if (list) out.push(`<${list.tag}>${list.items.map((item) => `<li>${mdInline(item)}</li>`).join("")}</${list.tag}>`);
    list = null;
  };
  for (const raw of lines) {
    const line = raw.replace(/\s+$/, "");
    if (code) {
      if (/^```/.test(line.trim())) {
        out.push(`<pre class="md-code">${esc(code.join("\n"))}</pre>`);
        code = null;
      } else code.push(raw);
      continue;
    }
    if (/^```/.test(line.trim())) {
      flushParagraph();
      flushList();
      code = [];
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    const bullet = /^\s*[-*+]\s+(.*)$/.exec(line);
    const ordered = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    if (!line.trim()) {
      flushParagraph();
      flushList();
    } else if (heading) {
      flushParagraph();
      flushList();
      const level = Math.min(3, heading[1].length);
      out.push(`<h${level + 3} class="md-h">${mdInline(heading[2].replace(/#+$/, ""))}</h${level + 3}>`);
    } else if (/^(-{3,}|\*{3,}|_{3,})$/.test(line.trim())) {
      flushParagraph();
      flushList();
      out.push("<hr>");
    } else if (bullet || ordered) {
      flushParagraph();
      const tag = bullet ? "ul" : "ol";
      if (list && list.tag !== tag) flushList();
      if (!list) list = { tag, items: [] };
      list.items.push((bullet || ordered)[1]);
    } else if (/^>\s?/.test(line)) {
      flushParagraph();
      flushList();
      out.push(`<blockquote>${mdInline(line.replace(/^>\s?/, ""))}</blockquote>`);
    } else if (list && /^\s{2,}\S/.test(raw)) {
      list.items[list.items.length - 1] += " " + line.trim();
    } else {
      flushList();
      paragraph.push(line.trim());
    }
  }
  if (code) out.push(`<pre class="md-code">${esc(code.join("\n"))}</pre>`);
  flushParagraph();
  flushList();
  return out.join("");
}

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
  el.topUser.textContent = S.account?.username ? "@" + S.account.username : "@—";
  renderPresenceDot();
  if (!el.accountPop.hidden) renderAccountPop();
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
  closeAccountPop();
  Dropdown.closeAll();
  hideTip();
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
  if (name === "changelog") loadChangelog();
  if (name === "home") scheduleServerPing(true);
  if (name === "friends") {
    renderFriends();
    loadSocial({ quiet: Boolean(S.social) });
  }
  syncFriendTicker();
}

/* ═══════════════════════════ ЗБІРКИ ═══════════════════════════ */

/* ═══════════════════════════ ПАНЕЛЬ ПРОГРЕСУ ═══════════════════════════ */

const STEPS = ["manifest", "files", "debug", "minecraft", "java", "loader", "libraries", "assets", "launch"];
const STEP_OF_STAGE = {
  verify: "files",
  download: "files",
  debug: "debug",
  "debug-download": "debug",
  client: "minecraft",
  java: "java",
  libraries: "libraries",
  assets: "assets"
};
const BYTE_STAGES = new Set(["client", "java", "download", "debug-download", "launcher-update"]);
const LOG_LIMIT = 120;
let progressHideTimer = 0;

function stageText(stage) {
  return DICT[lang]["stage." + stage] ? t("stage." + stage) : t("play.checking");
}

function decimal(value, digits = 1) {
  const text = value.toFixed(digits);
  return lang === "uk" ? text.replace(".", ",") : text;
}

function formatDuration(ms) {
  const sec = ms / 1000;
  if (sec < 60) return decimal(sec, sec < 10 ? 1 : 0) + " " + t("unit.sec");
  return Math.floor(sec / 60) + " " + t("unit.min") + " " + Math.round(sec % 60) + " " + t("unit.sec");
}

function megabytes(bytes) {
  return decimal(bytes / MB, bytes >= 100 * MB ? 0 : 1);
}

/* ═══ плавна поява/зникнення панелі прогресу ═══ */

const EASE = "cubic-bezier(0.22, 1, 0.36, 1)";
let progressLeaveTimer = 0;

/** Анімація лише transform/opacity. Якщо вікно згорнуте й кадри не малюються —
 *  через тривалість анімації елемент гарантовано стає в кінцевий стан. */
function motion(node, keyframes, duration, delay = 0) {
  if (!node || reduceMotion.matches || document.hidden) return;
  const animation = node.animate(keyframes, { duration, delay, easing: EASE, fill: "backwards" });
  setTimeout(() => animation.cancel(), delay + duration + 150);
}

/** Сусіди панелі в Hero не стрибають, а плавно «підіймаються» (FLIP, лише transform). */
function flipHero(change) {
  const nodes = [...el.progress.parentElement.children].filter((node) => node !== el.progress && !node.hidden);
  const before = nodes.map((node) => node.getBoundingClientRect().top);
  change();
  nodes.forEach((node, index) => {
    const dy = before[index] - node.getBoundingClientRect().top;
    if (Math.abs(dy) < 1) return;
    motion(node, [{ transform: `translate3d(0, ${dy}px, 0)` }, { transform: "none" }], 520);
  });
}

/** Короткий «підйом» тексту, коли змінюється етап. */
function liftIn(node) {
  motion(
    node,
    [
      { opacity: 0, transform: "translate3d(0, 6px, 0)" },
      { opacity: 1, transform: "none" }
    ],
    320
  );
}

function setProgressStatus(text) {
  if (el.progressStatus.textContent === text) return;
  el.progressStatus.textContent = text;
  liftIn(el.progressStatus);
}

function showProgress() {
  clearTimeout(progressLeaveTimer);
  const panel = el.progress;
  if (panel.classList.contains("leaving")) {
    panel.classList.remove("leaving");
    return;
  }
  if (!panel.hidden) return;
  flipHero(() => {
    panel.hidden = false;
  });
}

function resetProgressPanel() {
  el.progress.classList.remove("simple", "failed", "finished", "leaving");
  el.progressFill.style.transform = "scaleX(0)";
}

function stepsFor(mode) {
  if (mode === "repair") return STEPS.filter((step) => step !== "debug" && step !== "launch");
  if (mode === "debug") return ["debug"];
  // Власні збірки дебагер не чіпає.
  if (mode === "custom") return STEPS.filter((step) => step !== "debug");
  return STEPS;
}

/** Почати показ етапів: mode = "play" | "custom" | "repair" | "debug". */
function startProgress(mode) {
  clearTimeout(progressHideTimer);
  S.run = {
    steps: stepsFor(mode),
    step: "",
    stepStart: performance.now(),
    start: performance.now(),
    done: new Set(),
    failed: "",
    finished: false,
    stage: "",
    percent: 0,
    measured: false,
    speed: 0,
    sample: null
  };
  S.percent = 0;
  el.progressLog.replaceChildren();
  el.progress.classList.remove("simple", "failed", "finished");
  showProgress();
  el.progressFile.textContent = " ";
  el.progressAmount.textContent = "";
  el.progressStatus.textContent = t("progress.starting");
  setBar(null);
  renderSteps();
}

function setBar(percent) {
  const indeterminate = percent === null;
  const wasIndeterminate = el.progressTrack.classList.contains("indeterminate");
  el.progressTrack.classList.toggle("indeterminate", indeterminate);
  if (indeterminate) {
    el.progressFill.style.transform = "";
    el.progressPercent.textContent = "";
    return;
  }
  // З «бігунка» — одразу на потрібний відсоток, без перетікання з анімації.
  if (wasIndeterminate) el.progressFill.style.transition = "none";
  el.progressFill.style.transform = "scaleX(" + percent / 100 + ")";
  if (wasIndeterminate) {
    void el.progressFill.offsetWidth;
    el.progressFill.style.transition = "";
  }
  el.progressPercent.textContent = Math.floor(percent) + "%";
}

function renderSteps() {
  const run = S.run;
  if (!run) return;
  const index = run.steps.indexOf(run.step);
  el.progressSteps.style.setProperty("--steps", run.steps.length);
  el.progressSteps.innerHTML = run.steps
    .map((name) => {
      let state = "";
      if (run.failed === name) state = "failed";
      else if (run.done.has(name)) state = "done";
      else if (name === run.step) state = "active";
      return '<li class="' + state + '" title="' + esc(t("step." + name)) + '"></li>';
    })
    .join("");
  if (run.failed) el.progressStage.textContent = t("progress.failedAt", { step: t("step." + run.failed) });
  else if (index < 0) el.progressStage.textContent = t("progress.starting");
  else el.progressStage.textContent = t("progress.step", { n: index + 1, total: run.steps.length }) + " · " + t("step." + run.step);
}

function addLog(text, kind = "") {
  const run = S.run;
  if (!run) return;
  const item = document.createElement("li");
  if (kind) item.className = kind;
  const time = document.createElement("span");
  time.className = "tm";
  time.textContent = "+" + formatDuration(performance.now() - run.start);
  item.append(time, document.createTextNode(text));
  el.progressLog.append(item);
  while (el.progressLog.childElementCount > LOG_LIMIT) el.progressLog.firstElementChild.remove();
  el.progressLog.scrollTop = el.progressLog.scrollHeight;
}

function completeStep(name) {
  const run = S.run;
  if (!name || run.done.has(name)) return;
  run.done.add(name);
  addLog("✓ " + t("step." + name) + " · " + formatDuration(performance.now() - run.stepStart), "ok");
}

function setStep(name) {
  const run = S.run;
  if (!run || run.finished || name === run.step) return;
  const next = run.steps.indexOf(name);
  if (next < 0 || next < run.steps.indexOf(run.step)) return;
  completeStep(run.step);
  // Етапи, які бекенд пропустив (наприклад, без завантажувача), теж готові.
  for (const earlier of run.steps.slice(0, next)) run.done.add(earlier);
  run.step = name;
  run.counts = null;
  run.stepStart = performance.now();
  run.measured = false;
  run.stage = "";
  run.sample = null;
  run.speed = 0;
  el.progressFile.textContent = " ";
  el.progressAmount.textContent = "";
  setBar(null);
  renderSteps();
  schedulePlay();
}

function amountText(payload) {
  const run = S.run;
  if (!BYTE_STAGES.has(payload.stage)) {
    return payload.total ? payload.current + " / " + payload.total + " " + t("unit.files") : "";
  }
  let text = megabytes(payload.current) + " / " + megabytes(payload.total) + " " + t("unit.mb");
  if (!run) return text;
  const now = performance.now();
  if (!run.sample || payload.current < run.sample.bytes) run.sample = { at: now, bytes: payload.current };
  const elapsed = now - run.sample.at;
  if (elapsed >= 600) {
    const instant = ((payload.current - run.sample.bytes) / elapsed) * 1000;
    run.speed = run.speed ? run.speed * 0.7 + instant * 0.3 : instant;
    run.sample = { at: now, bytes: payload.current };
  }
  if (run.speed > 1024 && payload.current < payload.total) {
    text += " · " + decimal(run.speed / MB) + " " + t("unit.mbps");
    text += " · " + t("progress.left", { time: formatDuration(((payload.total - payload.current) / run.speed) * 1000) });
  }
  return text;
}

function setProgress(payload) {
  const percent = Math.max(0, Math.min(100, Number(payload.percent) || 0));
  S.percent = percent;
  const run = S.run;
  if (!run) {
    // Оновлення лаунчера чи інша дія без етапів — лише смужка.
    el.progress.classList.add("simple");
    showProgress();
    el.progressStage.textContent = stageText(payload.stage);
    setBar(percent);
    el.progressFile.textContent = payload.file || " ";
    el.progressAmount.textContent = amountText(payload);
    return;
  }
  if (run.finished) return;
  const owner = STEP_OF_STAGE[payload.stage];
  if (owner) setStep(owner);
  if (run.stage !== payload.stage) {
    run.stage = payload.stage;
    run.sample = null;
    run.speed = 0;
  }
  run.measured = true;
  run.percent = percent;
  run.counts = { current: payload.current, total: payload.total };
  setBar(percent);
  el.progressFile.textContent = payload.file || " ";
  el.progressAmount.textContent = amountText(payload);
  schedulePlay();
}

function onLauncherStatus(text) {
  const run = S.run;
  if (!run || run.finished) return;
  setProgressStatus(text);
  addLog(text);
  // Після 100% триває робота без лічильника (розпакування, інсталятор) — «бігунок».
  if (run.measured && run.percent >= 100) {
    run.measured = false;
    setBar(null);
    el.progressAmount.textContent = "";
  }
}

function finishProgress(ok, text) {
  const run = S.run;
  if (!run) return hideProgress();
  run.finished = true;
  if (ok) {
    completeStep(run.step);
    for (const name of run.steps) run.done.add(name);
    run.step = run.steps[run.steps.length - 1];
    el.progress.classList.add("finished");
    setBar(100);
    el.progressFile.textContent = " ";
    el.progressAmount.textContent = formatDuration(performance.now() - run.start);
    setProgressStatus(text);
    addLog("✓ " + text, "ok");
    renderSteps();
    progressHideTimer = setTimeout(() => {
      if (S.run === run) hideProgress();
    }, 2600);
    return;
  }
  run.failed = run.step || run.steps[0];
  el.progress.classList.add("failed");
  if (!run.measured) setBar(0);
  setProgressStatus(text);
  addLog("✕ " + text, "err");
  renderSteps();
}

function hideProgress() {
  clearTimeout(progressHideTimer);
  S.run = null;
  S.percent = 0;
  const panel = el.progress;
  if (panel.hidden || reduceMotion.matches) {
    panel.hidden = true;
    resetProgressPanel();
    return;
  }
  // Спершу панель плавно опускається й зникає, потім решта Hero м'яко сідає на місце.
  panel.classList.add("leaving");
  clearTimeout(progressLeaveTimer);
  progressLeaveTimer = setTimeout(() => {
    flipHero(() => {
      panel.hidden = true;
      resetProgressPanel();
    });
  }, 260);
}

const PLAY_ICONS = {
  play: '<svg viewBox="0 0 24 24"><path class="fill" d="M7 4.6v14.8L19 12z"/></svg>',
  install: '<svg viewBox="0 0 24 24"><path d="M12 4v11M7 10l5 5 5-5M5 20h14"/></svg>',
  update: '<svg viewBox="0 0 24 24"><path d="M20 11a8 8 0 0 0-14.9-3M4 4v4h4M4 13a8 8 0 0 0 14.9 3M20 20v-4h-4"/></svg>',
  running: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/></svg>',
  busy: '<span class="btn-spin"></span>'
};

/** Підпис кнопки під час встановлення — що саме зараз відбувається. */
function busyLabel() {
  const run = S.run;
  if (!["play", "repair", "debug"].includes(S.busy)) return t("play.wait");
  const p = Math.round(S.percent);
  const measuring = run?.measured && run.percent < 100;
  switch (run?.step) {
    case "files":
      if (run.stage === "download" && measuring) return t("play.downloadMods", { p });
      if (run.stage === "verify" && run.counts?.total) return t("play.verifyN", { n: run.counts.current, total: run.counts.total });
      return S.busy === "repair" ? t("play.repairing") : t("play.checking");
    case "debug":
      if (run.stage === "debug-download" && measuring) return t("play.debugDownload", { p });
      if (run.stage === "debug" && run.counts?.total && measuring) return t("play.verifyN", { n: run.counts.current, total: run.counts.total });
      return t("play.debugging");
    case "minecraft":
      return measuring ? t("play.gameFiles", { p }) : t("play.prepMinecraft");
    case "java":
      return t("play.prepJava") + (measuring ? " — " + p + "%" : "");
    case "loader":
      return t("play.prepLoader");
    case "libraries":
    case "assets":
      return measuring ? t("play.gameFiles", { p }) : t("play.preparing");
    case "launch":
      return t("play.launching");
    default:
      if (S.busy === "debug") return t("play.debugging");
      return S.busy === "repair" ? t("play.repairing") : t("play.checking");
  }
}

let playFrame = 0;

function schedulePlay() {
  if (!playFrame) {
    playFrame = requestAnimationFrame(() => {
      playFrame = 0;
      renderPlay();
    });
  }
}

/* Розділена кнопка під час гри:
 *   стан 1 — одна кнопка «Встановити / Оновити / Грати»;
 *   стан 2 — [Гра запущена | бар'єр «Закрити гру»];
 *   стан 3 — [Закрити гру? ✓ ✕ | бар'єр]: ✓ викликає stop_instance, ✕ повертає у стан 2.
 * Якщо гра закрилася сама (подія game-state: exited), кнопка повертається у стан 1. */
const STOP_YES_ICON = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12.5 4.5 4.5L19 7.5"/></svg>';
let splitTimer = 0;

/** Сусідні кнопки рядка дій не стрибають, коли головна кнопка змінює ширину (FLIP, лише transform). */
function flipActions(change) {
  const nodes = [...el.playWrap.parentElement.children].filter((node) => node !== el.playWrap && !node.hidden);
  const before = nodes.map((node) => node.getBoundingClientRect());
  change();
  nodes.forEach((node, index) => {
    const rect = node.getBoundingClientRect();
    const dx = before[index].left - rect.left;
    const dy = before[index].top - rect.top;
    if (Math.abs(dx) < 1 && Math.abs(dy) < 1) return;
    motion(node, [{ transform: `translate3d(${dx}px, ${dy}px, 0)` }, { transform: "none" }], 420);
  });
}

function renderPlay() {
  const wrap = el.playWrap;
  const split = Boolean(S.running);
  const shown = wrap.classList.contains("split") && !wrap.classList.contains("closing");
  // Поява / зникнення бар'єра змінює ширину рядка дій — сусідів зсуваємо плавно.
  if (split !== shown) flipActions(renderPlayNow);
  else renderPlayNow();
}

function renderPlayNow() {
  renderPlayButton();
  renderSplit();
}

function renderSplit() {
  const wrap = el.playWrap;
  const split = Boolean(S.running);
  if (!split) {
    S.stopConfirm = false;
    S.stopping = false;
  }
  const confirm = split && S.stopConfirm;
  wrap.classList.toggle("confirm", confirm);
  el.playConfirm.inert = !confirm;
  el.stopBtn.inert = !split;
  el.stopBtn.setAttribute("aria-label", t("play.stop"));
  el.stopBtn.setAttribute("aria-expanded", String(confirm));
  el.stopYes.setAttribute("aria-label", t("play.stopYes"));
  el.stopNo.setAttribute("aria-label", t("play.stopNo"));
  el.stopYes.disabled = S.stopping;
  el.stopNo.disabled = S.stopping;
  el.stopBtn.disabled = S.stopping;
  const yesMode = S.stopping ? "busy" : "idle";
  if (el.stopYes.dataset.mode !== yesMode) {
    el.stopYes.dataset.mode = yesMode;
    el.stopYes.innerHTML = S.stopping ? '<span class="btn-spin" aria-hidden="true"></span>' : STOP_YES_ICON;
  }
  el.stopYes.setAttribute("aria-busy", String(S.stopping));

  const shown = wrap.classList.contains("split") && !wrap.classList.contains("closing");
  if (split === shown) return;
  clearTimeout(splitTimer);
  if (split) {
    wrap.classList.remove("closing");
    wrap.classList.add("split");
  } else {
    // Спершу бар'єр тане на місці, потім контейнер повертає звичайну ширину.
    wrap.classList.add("closing");
    splitTimer = setTimeout(() => {
      if (!wrap.classList.contains("closing")) return;
      flipActions(() => wrap.classList.remove("split", "closing"));
    }, reduceMotion.matches ? 0 : 170);
  }
}

/** Бар'єр: відкрити / сховати підтвердження «Закрити гру?». */
function onStopToggle() {
  if (!S.running || S.stopping) return;
  S.stopConfirm = !S.stopConfirm;
  renderPlay();
  if (S.stopConfirm) el.stopNo.focus({ preventScroll: true });
  else if (el.playConfirm.contains(document.activeElement)) el.stopBtn.focus({ preventScroll: true });
}

function onStopCancel() {
  if (!S.stopConfirm || S.stopping) return;
  const refocus = el.playConfirm.contains(document.activeElement);
  S.stopConfirm = false;
  renderPlay();
  if (refocus) el.stopBtn.focus({ preventScroll: true });
}

/** ✓ — примусово закрити запущену гру (stop_instance завершує дерево процесів). */
async function onStopConfirm() {
  const id = S.running;
  if (!id || !S.stopConfirm || S.stopping) return;
  S.stopping = true;
  renderPlay();
  try {
    await invoke("stop_instance", { instanceId: id });
    // Бекенд уже звільнив стан гри; подія game-state: exited може прийти трохи пізніше.
    if (S.running === id) {
      S.running = null;
      updateCalm();
      renderList();
      if (S.tab === "settings") renderDisk();
    }
    S.stopConfirm = false;
  } catch (error) {
    // Гра не закрилася — залишаємось у стані 2, щоб можна було спробувати ще раз.
    S.stopConfirm = false;
    setIsland("error", errHuman(error));
  } finally {
    S.stopping = false;
    renderPlay();
  }
}

function renderPlayButton() {
  const inst = current();
  const play = el.play;
  let label = t("play.play");
  let sub = "";
  let disabled = false;
  let mode = "play";

  if (S.viewError) {
    label = t("common.retry");
    mode = "update";
  } else if (!inst) {
    label = S.view ? t("play.choose") : t("play.wait");
    disabled = true;
    mode = "";
  } else if (S.running) {
    label = S.running === inst.id ? t("play.running") : t("play.otherRunning");
    sub = t("play.subRunning");
    disabled = true;
    mode = "running";
  } else if (S.busy) {
    disabled = true;
    mode = "busy";
    label = busyLabel();
    sub = ["play", "repair", "debug"].includes(S.busy) ? t("play.subWait") : "";
  } else if (S.updating) {
    label = t("play.wait");
    disabled = true;
    mode = "busy";
  } else if (!activeProfile()) {
    label = t("play.addNick");
    mode = "";
  } else if (!inst.installed) {
    label = t("play.install");
    mode = "install";
    const size = formatSize(inst.size_bytes);
    sub = S.view?.offline && !inst.custom ? t("play.subNeedNet") : size ? t("play.subInstall", { size: "~" + size }) : "";
  } else if (inst.update_available && !S.view?.offline) {
    label = t("play.update");
    mode = "update";
    sub = t("play.subUpdate", { v: inst.remote_version });
  } else if (S.view?.offline && !inst.custom) {
    sub = t("play.subOffline");
  }

  play.disabled = disabled;
  for (const name of ["install", "update", "busy", "running"]) play.classList.toggle(name, mode === name);
  const icon = PLAY_ICONS[mode] || "";
  if (el.playIco.dataset.mode !== mode) {
    const first = el.playIco.dataset.mode === undefined;
    el.playIco.dataset.mode = mode;
    el.playIco.innerHTML = icon;
    // Іконка стану з'являється з легким «пружинним» масштабом (лише transform/opacity).
    if (!first) {
      motion(
        el.playIco,
        [
          { opacity: 0, transform: "scale(0.6) rotate(-20deg)" },
          { opacity: 1, transform: "none" }
        ],
        360
      );
    }
  }
  const shape = label.replace(/[d.,]+/g, "#");
  if (el.playLabel.dataset.shape !== shape) {
    if (el.playLabel.dataset.shape !== undefined) liftIn(el.playLabel.parentElement);
    el.playLabel.dataset.shape = shape;
  }
  el.playLabel.textContent = label;
  el.playSub.textContent = sub;
  play.setAttribute("aria-busy", mode === "busy");

  const locked = !inst || Boolean(S.busy) || Boolean(S.running);
  el.repairBtn.disabled = locked || !inst?.installed;
  el.folderBtn.disabled = !inst?.installed;
  el.deleteBtn.disabled = !inst?.installed || Boolean(S.busy) || S.running === inst?.id;
  el.importBtn.hidden = !inst || inst.custom;
  el.importBtn.disabled = !inst || Boolean(S.busy) || Boolean(S.updating) || S.running === inst?.id;
}

function renderRamNote() {
  const inst = current();
  const rec = Math.min(inst?.recommended_ram_mb || 0, S.maxRam);
  const cur = S.settings?.ram_mb || 0;
  const show = Boolean(inst && rec && cur && cur < rec && !S.busy);
  el.ramNote.hidden = !show;
  if (show) {
    el.ramNote.innerHTML = ICONS.memory + "<span>" + esc(t("home.ramLow", { cur: formatRam(cur), rec: formatRam(rec) })) + '</span><button type="button" data-goto="settings">' + esc(t("home.ramFix")) + "</button>";
  }
}

function renderNickNote() {
  const profile = activeProfile();
  el.nickNote.innerHTML = profile ? t("home.playingAs", { nick: esc(profile.nickname) }) : t("home.noNick");
}

let heroShownId = null;

/** Інша збірка — заголовок, опис і бейджі м'яко «підіймаються» з невеликою затримкою один за одним. */
function heroSwap(inst) {
  const id = inst?.id || "";
  if (heroShownId === id) return;
  const first = heroShownId === null;
  heroShownId = id;
  if (first) return;
  [el.badgeMc.parentElement, el.instName, el.instDesc, el.instMeta].forEach((node, index) =>
    motion(
      node,
      [
        { opacity: 0, transform: "translate3d(0, 8px, 0)" },
        { opacity: 1, transform: "none" }
      ],
      420,
      index * 35
    )
  );
}

function renderHero() {
  const inst = current();
  heroSwap(inst);
  el.badgeOffline.hidden = !(inst && S.view?.offline && !inst.custom);
  if (!inst) {
    el.instName.textContent = S.viewError ? t("list.error") : S.view ? t("home.empty") : "";
    el.instName.classList.toggle("small", Boolean(S.viewError));
    el.instDesc.hidden = !S.viewError;
    el.instDesc.textContent = S.viewError || "";
    el.instMeta.textContent = "";
    el.badgeMc.hidden = true;
    el.badgeLoader.hidden = true;
    el.badgeServer.hidden = true;
    el.ramNote.hidden = true;
    el.modsBtn.hidden = true;
    el.heroMark.hidden = true;
    paintHeroImage(null);
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
  el.badgeLoader.hidden = !loader || (loader === "Vanilla" && !inst.custom);
  el.badgeLoader.innerHTML = loaderIcon(inst.loader) + esc(loader + (inst.loader_version ? " " + inst.loader_version : ""));

  const vanilla = !inst.loader || inst.loader === "vanilla";
  if (inst.custom) {
    // Підказка «що робити далі» замість опису з репозиторію.
    const key = vanilla ? "home.customDescVanilla" : inst.installed ? "home.customDescMods" : "home.customDescNew";
    el.instDesc.textContent = t(key, { mc: inst.minecraft_version, loader });
    el.instDesc.hidden = false;
  }
  el.heroMark.hidden = !inst.custom;
  el.heroMark.innerHTML = inst.custom ? loaderIcon(inst.loader) : "";
  // «Контент» — моди, ресурс-паки й шейдери; ванільній збірці доступні ресурс-паки.
  el.modsBtn.hidden = !inst.custom;
  el.modsBtn.disabled = false;
  el.modsBtn.title = t("home.modsTitle");

  const meta = [];
  if (inst.custom) {
    meta.push(inst.origin || t("home.customMeta"), inst.installed ? t("inst.installed") : t("home.notInstalledMeta"));
    if (!vanilla) meta.push(plural(inst.mods_count || 0, "plural.mods"));
    if (VISIBILITIES.includes(inst.visibility)) {
      meta.push(t("vis." + inst.visibility) + (inst.visibility !== "private" && inst.published ? " (" + t("vis.publishedTag") + ")" : ""));
    }
  }
  else if (inst.update_available) meta.push(t("home.updateMeta", { from: inst.local_version || "?", to: inst.remote_version }));
  else if (inst.installed) meta.push(t("home.installedMeta", { v: inst.local_version || "—" }));
  else meta.push(t("home.notInstalledMeta"));
  if (inst.size_bytes) meta.push(formatSize(inst.size_bytes));
  if (!inst.installed) meta.push(t("play.subFirst"));
  el.instMeta.textContent = meta.join(" · ");

  paintHeroImage(inst);
  renderRamNote();
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
  if (S.viewError || !S.view) list.dataset.layout = "";
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

  // Той самий набір збірок — оновлюємо картки на місці: так вибір іншої збірки
  // анімується (рамка й акцентна смужка — через opacity/transform), а не «перемальовується».
  const hasProject = S.view.has_project !== false;
  const layout = [lang, hasProject ? "P" : "L", ...items.map((inst) => (inst.custom ? "c:" : "p:") + inst.id)].join("|");
  if (list.dataset.layout === layout && patchList(items)) return;

  const projectItems = items.filter((inst) => !inst.custom);
  const customItems = items.filter((inst) => inst.custom);
  const intro = !list.querySelector(".inst-card");
  list.classList.toggle("intro", intro);
  let index = 0;
  const card = (inst) => {
    const on = inst.id === S.activeId;
    const delay = Math.min(index++, 8) * 30;
    return `<div class="inst-card${on ? " on" : ""}" role="option" tabindex="0" aria-selected="${on}" data-id="${esc(inst.id)}" style="animation-delay:${delay}ms">${cardInner(inst)}</div>`;
  };
  const addButton = `<button class="inst-add" type="button" data-act="create" aria-label="${esc(t("create.button"))}" title="${esc(t("create.button"))}">${ICONS.plus}</button>`;
  const importNew = `<button class="inst-add" type="button" data-act="import-new" aria-label="${esc(t("import.newTitle"))}" title="${esc(t("import.newTitle"))}">${ICONS.archive}</button>`;
  const browsePacks = `<button class="inst-add" type="button" data-act="modpacks" aria-label="${esc(t("home.browsePacks"))}" title="${esc(t("home.browsePacks"))}">${ICONS.layers}</button>`;
  const cta = `<button class="inst-cta" type="button" data-act="create">${ICONS.plus}<span><b>${esc(t("create.ctaTitle"))}</b><small>${esc(t(ENABLE_CURSEFORGE ? "create.ctaText" : "create.ctaTextModrinth"))}</small></span></button>`;
  const connect = `<button class="inst-cta inst-cta-soft" type="button" data-goto="project">${ICONS.link}<span><b>${esc(t("home.connectTitle"))}</b><small>${esc(t("home.connectText"))}</small></span></button>`;
  const projectPart = hasProject
    ? `<h3 class="inst-group"><span>${esc(t("home.projectTitle"))}</span></h3>` +
      (projectItems.length ? projectItems.map(card).join("") : `<div class="empty small">${esc(t("list.empty"))}</div>`)
    : "";
  const customPart =
    `<h3 class="inst-group"><span>${esc(t("home.customTitle"))}</span><span class="inst-group-acts">${browsePacks}${importNew}${customItems.length ? addButton : ""}</span></h3>` +
    (customItems.length ? customItems.map(card).join("") : cta);
  // Без проєкту — спершу власні збірки, а внизу ненав'язлива пропозиція підключити проєкт.
  list.innerHTML = hasProject ? projectPart + customPart : customPart + connect;
  list.dataset.layout = layout;
  cardSigs.clear();
  for (const inst of items) {
    const node = list.querySelector(`.inst-card[data-id="${CSS.escape(inst.id)}"]`);
    cardSigs.set(inst.id, cardInner(inst));
    paintCard(node, inst);
  }
}

const cardSigs = new Map();

function cardInner(inst) {
  const mods = inst.custom && inst.loader && inst.loader !== "vanilla" ? plural(inst.mods_count || 0, "plural.mods") : "";
  // Назву завантажувача показує іконка (з підказкою), тож у тексті — лише версія та розмір.
  const details = [inst.minecraft_version, mods || formatSize(inst.size_bytes)].filter(Boolean).join(" · ");
  const icon = inst.custom
    ? customIcon(inst)
    : `<span class="inst-icon avatar" data-icon="${esc(inst.id)}"></span>`;
  const more = `<button class="more" type="button" data-more="${esc(inst.id)}" aria-label="${esc(t("inst.more"))}" title="${esc(t("inst.more"))}">${ICONS.more}</button>`;
  return `<span class="inst-art" data-art="${esc(inst.id)}"></span>
      ${icon}
      <span class="inst-info"><b>${esc(inst.name)}</b><span class="inst-sub">${statusTag(inst)}${inst.loader ? loaderIcon(inst.loader) : ""}<small>${esc(details)}</small></span></span>
      <span class="inst-side">${more}</span>`;
}

/** Значок власної збірки: обрана гравцем іконка або значок завантажувача. */
function customIcon(inst) {
  return isIconUrl(inst.icon)
    ? `<span class="inst-icon avatar has-img"><img class="ready" src="${esc(inst.icon)}" alt="" draggable="false"></span>`
    : `<span class="inst-icon avatar avatar-loader">${loaderIcon(inst.loader)}</span>`;
}

function isIconUrl(url) {
  return typeof url === "string" && url.startsWith("data:image/png;base64,");
}

function paintCard(node, inst) {
  if (!node || inst.custom) return;
  paintAvatar(node.querySelector("[data-icon]"), initial(inst.name), instanceIcons(inst));
  paintArt(node.querySelector("[data-art]"), inst);
}

/** Оновлює існуючі картки; false — структура змінилась і потрібна повна перебудова. */
function patchList(items) {
  const list = el.instList;
  for (const inst of items) {
    const node = list.querySelector(`.inst-card[data-id="${CSS.escape(inst.id)}"]`);
    if (!node) return false;
    const inner = cardInner(inst);
    if (cardSigs.get(inst.id) !== inner) {
      node.innerHTML = inner;
      cardSigs.set(inst.id, inner);
      paintCard(node, inst);
    }
    const on = inst.id === S.activeId;
    node.classList.toggle("on", on);
    node.setAttribute("aria-selected", String(on));
  }
  list.classList.remove("intro");
  return true;
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

/** Номер останнього вибору збірки: відповідь instance_details для попереднього кліку
 *  (повільна мережа) не перезапише заголовок, опис, кнопки й фон щойно обраної збірки. */
let selectGeneration = 0;

async function selectInstance(id, { save = true } = {}) {
  if (!S.view?.instances.some((i) => i.id === id)) return;
  const gen = ++selectGeneration;
  const changed = S.activeId !== id;
  if (changed && !S.busy) hideProgress();
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
    if (gen !== selectGeneration || S.activeId !== id || fresh?.id !== id) return;
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

/** Кожне завантаження списку має номер: застаріла відповідь (після виходу з проєкту,
 *  підключення іншого чи очищення даних) не перезапише свіжий стан. */
let loadSeq = 0;

async function loadProject({ quiet = false } = {}) {
  const my = ++loadSeq;
  if (!quiet) {
    S.view = null;
    S.viewError = null;
    renderList();
    setIsland("busy", t("status.loading"));
  }
  try {
    const view = await invoke("load_project");
    if (my !== loadSeq) return;
    S.view = view;
    S.viewError = null;
    S.activeId = view.active_instance;
    if (S.binding && view.name) S.binding.name = view.name;
    prefetchAssets();
    renderTop();
    renderList();
    renderHero();
    if (!S.busy && !S.running) idleIsland();
    selectInstance(S.activeId, { save: false });
  } catch (error) {
    if (my !== loadSeq) return;
    S.view = null;
    S.viewError = errHuman(error);
    renderList();
    renderHero();
    setIsland("error", errHuman(error));
  }
}

function hasProject() {
  return Boolean(S.binding) && S.view?.has_project !== false;
}

function renderTop() {
  const linked = hasProject();
  const name = linked ? S.view?.name || S.binding?.name || S.binding?.code || "—" : "Terrapolis";
  el.topProject.textContent = name;
  el.topProject.parentElement.title = linked ? t("nav.project") : t("top.noProject");
  el.topDot.hidden = !linked;
  el.topDot.classList.toggle("off", Boolean(S.view?.offline));
  const icons = linked ? projectIcons() : [];
  paintAvatar(el.brand, initial(name), icons);
  paintAvatar(el.topAvatar, initial(name), icons);
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
  startProgress(inst.custom ? "custom" : "play");
  renderPlay();
  try {
    const result = await invoke("play", { instanceId: inst.id });
    finishProgress(true, t("progress.launched"));
    if (!S.running) S.running = inst.id;
    updateCalm();
    if (result.offline) setIsland("warn", t("status.offlinePlay"));
    else if (result.first_install) setIsland("ok", t("status.installed"));
    if (result.debug && !debugClean(result.debug)) showDebugToast(inst, result.debug);
    const fresh = current();
    if (fresh) {
      fresh.installed = true;
      if (!result.offline) {
        fresh.update_available = false;
        if (fresh.remote_version) fresh.local_version = fresh.remote_version;
      }
    }
  } catch (error) {
    finishProgress(false, errHuman(error));
    setIsland("error", errHuman(error));
    const extra = [{ id: "retry", label: t("common.retry"), kind: "ghost", onClick: () => setTimeout(onPlay, 220) }];
    // Без мережі встановити збірку не вийшло — підказуємо імпорт з архіву.
    if (!inst.custom && !inst.installed) extra.unshift({ id: "import", label: t("menu.import"), kind: "ghost", onClick: () => setTimeout(() => onImport(inst.id), 220) });
    showError(error, extra);
  } finally {
    S.busy = null;
    S.phase = null;
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
  startProgress("repair");
  renderPlay();
  try {
    const report = await invoke("repair_instance", { instanceId: inst.id });
    finishProgress(true, t("progress.checked"));
    const nothing = report.downloaded === 0 && report.removed === 0;
    setIsland("ok", nothing ? t("repair.ok") : t("repair.fixed", { d: report.downloaded, r: report.removed }));
    inst.installed = true;
    inst.update_available = false;
    if (inst.remote_version) inst.local_version = inst.remote_version;
  } catch (error) {
    finishProgress(false, errHuman(error));
    setIsland("error", errHuman(error));
    showError(error);
  } finally {
    S.busy = null;
    S.phase = null;
    renderList();
    renderHero();
  }
}

async function onDelete(id) {
  const inst = S.view?.instances.find((i) => i.id === id) || S.disk.find((d) => d.id === id);
  if (!inst || S.busy) return;
  if (S.running === id) {
    setIsland("warn", t("delete.running"));
    return;
  }
  const name = inst.name || id;
  let usage = null;
  try {
    usage = await invoke("instance_disk_usage", { instanceId: id });
  } catch {
    usage = null;
  }
  const hasPersonal = Boolean(usage && (usage.saves_count > 0 || usage.screenshots_count > 0));
  const cell = (label, value, cellId) => '<div><small>' + esc(label) + "</small><b" + (cellId ? ' id="' + cellId + '"' : "") + ">" + esc(value) + "</b></div>";
  const freeWith = (keep) => formatSize(Math.max(0, usage.total_bytes - (keep ? usage.personal_bytes : 0)));
  const stats = usage
    ? '<div class="usage">' + cell(t("delete.free"), freeWith(hasPersonal), "freeValue") + cell(t("delete.worlds"), String(usage.saves_count)) + cell(t("delete.shots"), String(usage.screenshots_count)) + "</div>"
    : "";
  const keepBox = hasPersonal
    ? '<label class="check"><input type="checkbox" id="keepPersonal" checked><span>' + esc(t("delete.keep")) + "<small>" + esc(t("delete.keepHint", { size: formatSize(usage.personal_bytes) })) + "</small></span></label>"
    : usage
      ? "<p>" + esc(t("delete.noPersonal")) + "</p>"
      : "";
  let keep = false;
  const choice = openModal({
    icon: "trash",
    tone: "danger",
    title: t("delete.title", { name }),
    html: "<p>" + esc(t(inst.custom ? "delete.leadCustom" : "delete.lead")) + "</p>" + stats + keepBox,
    buttons: [
      { id: "cancel", label: t("common.cancel"), kind: "ghost" },
      {
        id: "ok",
        label: t("delete.go"),
        kind: "danger",
        onClick: () => {
          keep = Boolean($("keepPersonal")?.checked);
        }
      }
    ]
  });
  $("keepPersonal")?.addEventListener("change", (event) => {
    const free = $("freeValue");
    if (free) free.textContent = freeWith(event.target.checked);
  });
  if ((await choice) !== "ok") return;
  try {
    await invoke("delete_instance", { instanceId: id, keepPersonal: keep });
    setIsland("ok", keep ? t("delete.doneKept", { name }) : t("delete.done", { name }));
    const local = S.view?.instances.find((i) => i.id === id);
    if (local?.custom && !keep) {
      // Власна збірка зникає повністю — переходимо на іншу.
      S.view.instances = S.view.instances.filter((i) => i.id !== id);
      if (id === S.activeId) {
        hideProgress();
        const next = S.view.instances[0]?.id;
        S.activeId = "";
        if (next) selectInstance(next);
      }
    } else if (local) {
      local.installed = false;
      local.update_available = false;
      local.local_version = "";
    }
    if (id === S.activeId) hideProgress();
    renderList();
    renderHero();
    if (S.tab === "settings") refreshSettings();
  } catch (error) {
    showError(error);
  }
}

/* дебагер збірки проєкту */

function debugClean(report) {
  return !report.missing.length && !report.corrupt.length && !report.extra.length;
}

function statCell(label, value, tone = "") {
  return `<div${tone ? ` class="${tone}"` : ""}><small>${esc(label)}</small><b>${esc(String(value))}</b></div>`;
}

/** Після pre-launch перевірки, якщо дебагер щось виправив, — коротке сповіщення з кнопкою «Звіт». */
function showDebugToast(inst, report) {
  toast({
    id: "debug",
    icon: "bug",
    tone: report.failed.length ? "warn" : "",
    title: t("step.debug"),
    text: report.summary,
    actions: [{ label: t("debug.report"), kind: "ghost", onClick: () => showDebugReport(inst, report) }],
    timeout: 15000
  });
}

function showDebugReport(inst, report) {
  const fixed = report.downloaded.length + report.replaced.length + report.removed.length;
  const stats = `<div class="usage">${statCell(t("debug.checked"), report.checked)}${statCell(t("debug.ok"), report.ok)}${statCell(t("debug.fixed"), fixed, fixed ? "good" : "")}${statCell(
    t("debug.failed"),
    report.failed.length,
    report.failed.length ? "bad" : ""
  )}</div>`;
  const section = (title, list, tone) =>
    list.length
      ? `<section class="dbg-sec ${tone}"><h5><span>${esc(title)}</span><span class="count">${list.length}</span></h5><ul class="dbg-list">${list.map((name) => `<li>${esc(name)}</li>`).join("")}</ul></section>`
      : "";
  const body = debugClean(report)
    ? `<p class="dbg-clean">${ICONS.check}<span>${esc(t("debug.clean"))}</span></p>`
    : section(t("debug.removed", { dir: report.quarantine }), report.removed, "warn") +
      section(t("debug.downloaded"), report.downloaded, "ok") +
      section(t("debug.replaced"), report.replaced, "ok") +
      section(t("debug.failedList"), report.failed, "bad") +
      (report.offline ? `<p class="hint warn">${esc(t("debug.offline"))}</p>` : "");
  openModal({
    icon: "bug",
    tone: report.failed.length ? "warn" : "",
    title: t("debug.title", { name: inst?.name || "" }),
    html: `<p>${esc(report.summary)}</p>${stats}<div class="dbg">${body}</div>`,
    wide: true,
    buttons: [
      { id: "folder", label: t("crash.folder"), kind: "ghost", left: true, onClick: () => (openFolder(inst?.id), false) },
      { id: "close", label: t("common.close"), kind: "primary" }
    ]
  });
}

/** «Перевірити збірку (Дебагер)» з меню дій збірки. */
async function onDebug(id) {
  const inst = S.view?.instances.find((i) => i.id === (id || S.activeId));
  if (!inst || inst.custom || !inst.installed || S.busy || S.updating || S.running === inst.id) return;
  if (inst.id !== S.activeId) await selectInstance(inst.id);
  S.busy = "debug";
  S.phase = "checking";
  startProgress("debug");
  renderPlay();
  try {
    const report = await invoke("debug_instance", { instanceId: inst.id });
    finishProgress(true, report.summary);
    setIsland(report.failed.length ? "warn" : "ok", report.summary);
    showDebugReport(inst, report);
  } catch (error) {
    finishProgress(false, errHuman(error));
    setIsland("error", errHuman(error));
    showError(error);
  } finally {
    S.busy = null;
    S.phase = null;
    renderList();
    renderHero();
  }
}

/* імпорт збірки з .zip */

function fileName(path) {
  return String(path || "").split(/[\\/]/).pop();
}

/** targetId — збірка, в яку імпортувати; порожньо — нова власна збірка з архіву. */
async function onImport(targetId) {
  if (S.busy || S.updating) return;
  const inst = targetId ? S.view?.instances.find((i) => i.id === targetId) : null;
  if (targetId && !inst) return;
  if (targetId && S.running === targetId) {
    setIsland("warn", t("mods.running"));
    return;
  }
  if (S.picking) return;
  S.picking = true;
  let path = null;
  try {
    path = await invoke("pick_zip");
  } catch (error) {
    showError(error);
    return;
  } finally {
    S.picking = false;
  }
  if (!path || S.busy) return;
  const ok = await confirmDialog({
    icon: "archive",
    title: inst ? t("import.confirmTitle", { name: inst.name }) : t("import.newTitle"),
    html: t("import.confirmText", { file: esc(fileName(path)) }),
    ok: t("import.go")
  });
  if (!ok) return;
  S.busy = "import";
  renderPlay();
  setIsland("busy", t("import.running"));
  try {
    const report = await invoke("import_local_pack", { zipPath: path, instanceId: targetId || null, basePath: null });
    setIsland("ok", t("import.doneTitle", { name: report.name }));
    if (report.created) {
      await loadProject({ quiet: true });
      selectInstance(report.instance_id, { save: false });
    } else {
      try {
        mergeInstance(await invoke("instance_details", { instanceId: report.instance_id }));
      } catch {
        if (inst) {
          inst.installed = true;
          inst.local_version = report.pack_version;
        }
      }
    }
    showImportReport(report);
  } catch (error) {
    setIsland("error", errHuman(error));
    showError(error);
  } finally {
    S.busy = null;
    renderList();
    renderHero();
    if (S.tab === "settings") refreshSettings();
  }
}

function showImportReport(report) {
  const stats = `<div class="usage">${statCell(t("import.files"), report.files)}${statCell(t("import.verified"), report.verified, report.verified ? "good" : "")}${statCell(t("import.skipped"), report.skipped)}</div>`;
  const mismatched = report.mismatched.length
    ? `<p>${esc(t("import.mismatched"))}</p><ul class="dbg-list">${report.mismatched
        .slice(0, 40)
        .map((path) => `<li>${esc(path)}</li>`)
        .join("")}</ul>`
    : "";
  openModal({
    icon: "archive",
    title: t("import.doneTitle", { name: report.name }),
    html: `<p>${esc(t("import.ready"))}</p>${stats}${mismatched}`,
    buttons: [{ id: "close", label: t("common.close"), kind: "primary" }]
  });
}

/* власна збірка: будь-яка версія Minecraft + завантажувач */

const CUSTOM_LOADERS = ["vanilla", "fabric", "forge", "neoforge", "quilt"];

/** Версії Minecraft для списку: бейджі Release/Snapshot і Latest на найновішій. */
function mcVersionItems(list) {
  const latest = new Set();
  return list.map((version) => {
    const badges = [];
    const kinds = { release: "release", snapshot: "snapshot", old_beta: "beta", old_alpha: "alpha" };
    const kind = kinds[version.kind] || "release";
    badges.push({ text: t("badge." + kind), kind });
    if ((kind === "release" || kind === "snapshot") && !latest.has(kind)) {
      latest.add(kind);
      badges.push({ text: t("badge.latest"), kind: "latest" });
    }
    return { value: version.id, label: version.id, badges, search: version.kind };
  });
}

/** Версії завантажувача: Latest (найновіша), Recommended, Beta для нестабільних. */
function loaderVersionItems(list) {
  return list.map((version, index) => {
    const badges = [];
    if (version.recommended) badges.push({ text: t("badge.recommended"), kind: "recommended" });
    if (index === 0) badges.push({ text: t("badge.latest"), kind: "latest" });
    if (!version.stable) badges.push({ text: t("badge.beta"), kind: "beta" });
    return { value: version.id, label: version.id, badges };
  });
}

/** Нова власна збірка в списку: перед іншими власними, після збірок проєкту. */
function addCustomInstance(created) {
  if (!S.view || !created) return;
  S.view.instances = S.view.instances.filter((i) => i.id !== created.id);
  const firstCustom = S.view.instances.findIndex((i) => i.custom);
  S.view.instances.splice(firstCustom < 0 ? S.view.instances.length : firstCustom, 0, created);
}

/** Пункти списку завантажувачів: іконка, назва, короткий опис і бейдж «Рекомендовано» для Fabric. */
function loaderItems() {
  return CUSTOM_LOADERS.map((loader) => ({
    value: loader,
    label: loaderName(loader),
    sub: t("loader." + loader),
    icon: loaderIcon(loader, "ldr-md"),
    badges: loader === "fabric" ? [{ text: t("create.recommended"), kind: "recommended" }] : []
  }));
}

async function openCreateDialog() {
  if (!S.view || S.busy) return;
  // Vanilla за замовчуванням: найпростіший старт, завантажувач — лише якщо гравець його обрав.
  const form = { loader: "vanilla", snapshots: false, visibility: "private", icon: null };
  let created = null;
  let loaderSeq = 0;
  let mcSeq = 0;
  const mcDd = new Dropdown({ search: true, label: t("create.mc"), placeholder: "—", searchPh: "1.21.1, 1.20.1…", minWidth: 260 });
  const lvDd = new Dropdown({ search: true, label: t("create.loaderVersion"), placeholder: "—", empty: t("create.noLoader"), minWidth: 260 });
  const loaderDd = new Dropdown({ className: "dd-rich", label: t("create.loader"), showSub: true, triggerBadges: false, minWidth: 300, items: loaderItems(), value: form.loader });
  const html = `<p class="create-lead">${esc(t("create.lead"))}</p>
    <div class="create-form">
      <div class="create-grid">
        <div class="create-col">
        <section class="create-step cs-mc">
          <span class="label">${esc(t("create.step1"))}</span>
          <div class="dd-mount" id="cMc"></div>
          <div class="create-row create-sub"><span class="hint">${esc(t("create.mcHint"))}</span>
          <label class="toggle-inline" for="cSnap"><span class="switch"><input type="checkbox" id="cSnap"><span></span></span><span>${esc(t("create.snapshots"))}</span></label></div>
        </section>
        <section class="create-step cs-name">
          <label class="label" for="cName">${esc(t("create.step3"))}</label>
          <input type="text" id="cName" maxlength="40" spellcheck="false" autocomplete="off" placeholder="${esc(t("create.namePh"))}">
        </section>
        <section class="create-step cs-vis" id="cVis">
          <span class="label">${esc(t("vis.label"))}</span>
          ${visibilitySeg(form.visibility)}
        </section>
        <section class="create-step cs-icon">
          ${packIconPicker("cIcon", "", "V", t("icon.step"))}
        </section>
        </div>
        <div class="create-col">
        <section class="create-step cs-loader">
          <span class="label">${esc(t("create.step2"))}</span>
          <div class="dd-mount" id="cLoader"></div>
          <div class="lv-field is-off" id="cLvField">
            <span class="label">${esc(t("create.loaderVersion"))}</span>
            <div class="lv-dd" id="cLvInner" inert><div class="dd-mount" id="cLv"></div></div>
            <div class="swap-hints"><span class="hint" data-when="on">${esc(t("create.loaderHint"))}</span><span class="hint" data-when="off">${esc(t("create.loaderVanilla"))}</span></div>
          </div>
        </section>
        </div>
      </div>
      <p class="form-error" id="cError" hidden></p>
    </div>`;
  const updateNamePlaceholder = () => {
    const mc = mcDd.value;
    const input = $("cName");
    if (input && mc) input.placeholder = `${loaderName(form.loader)} ${mc}`;
    // Без обраної картинки прев'ю іконки показує першу літеру майбутньої назви.
    if (input) setPackIconLetter($("cIcon"), initial(input.value.trim() || input.placeholder));
  };

  const showError = (text) => {
    const node = $("cError");
    if (!node) return;
    node.textContent = text || "";
    node.hidden = !text;
  };
  /** «Версія завантажувача» завжди на своєму місці: для Vanilla — приглушена й неактивна,
   *  для Fabric/Forge/NeoForge/Quilt — плавно активується (opacity/transform), висота модалки не змінюється. */
  const setLoaderVersionActive = (on) => {
    const field = $("cLvField");
    if (!field) return;
    field.classList.toggle("is-off", !on);
    $("cLvInner").inert = !on;
    lvDd.setDisabled(!on);
    if (!on) {
      lvDd.opts.empty = "—";
      lvDd.setItems([], "");
    } else lvDd.opts.empty = t("create.noLoader");
  };
  const loadLoaderVersions = async () => {
    if (!$("cLvField")) return;
    const vanilla = form.loader === "vanilla";
    setLoaderVersionActive(!vanilla);
    updateNamePlaceholder();
    const mc = mcDd.value;
    const my = ++loaderSeq;
    if (vanilla || !mc) return;
    lvDd.setLoading(true);
    try {
      const list = await invoke("loader_versions", { loader: form.loader, minecraftVersion: mc });
      if (my !== loaderSeq) return;
      const preferred = list.find((v) => v.recommended) || list[0];
      lvDd.setItems(loaderVersionItems(list), preferred?.id ?? "");
    } catch (error) {
      if (my !== loaderSeq) return;
      lvDd.setItems([], "");
      showError(errHuman(error));
    }
  };
  const loadMc = async () => {
    const previous = mcDd.value;
    const my = ++mcSeq;
    mcDd.setLoading(true);
    try {
      const list = await invoke("minecraft_versions", { snapshots: form.snapshots });
      if (my !== mcSeq) return;
      const keep = previous && list.some((v) => v.id === previous) ? previous : list.find((v) => v.kind === "release")?.id ?? list[0]?.id ?? "";
      mcDd.setItems(mcVersionItems(list), keep);
      showError("");
    } catch (error) {
      if (my !== mcSeq) return;
      mcDd.setItems([], "");
      showError(errHuman(error));
    }
    if (my === mcSeq) await loadLoaderVersions();
  };
  mcDd.opts.onChange = () => {
    showError("");
    loadLoaderVersions();
  };
  loaderDd.opts.onChange = (value) => {
    form.loader = value;
    showError("");
    loadLoaderVersions();
  };

  const choice = openModal({
    icon: "plus",
    title: t("create.title"),
    html,
    wide: true,
    kind: "create",
    buttons: [
      { id: "browse", label: t("create.browse"), kind: "ghost", left: true, onClick: () => void setTimeout(() => openContentDialog("", "modpack"), 220) },
      { id: "cancel", label: t("common.cancel"), kind: "ghost" },
      {
        id: "ok",
        label: t("create.go"),
        kind: "primary",
        onClick: async (button) => {
          const mc = mcDd.value;
          const loaderVersion = form.loader === "vanilla" ? "" : lvDd.value;
          if (!mc || (form.loader !== "vanilla" && !loaderVersion)) {
            showError(t(!mc ? "create.mc" : "create.noLoader"));
            return false;
          }
          const name = $("cName").value.trim() || `${loaderName(form.loader)} ${mc}`;
          button.disabled = true;
          try {
            created = await invoke("create_custom_instance", {
              spec: { name, minecraft_version: mc, loader: form.loader, loader_version: loaderVersion, visibility: form.visibility },
              icon: form.icon ? Array.from(form.icon.bytes) : null
            });
          } catch (error) {
            showError(errHuman(error));
            button.disabled = false;
            return false;
          }
        }
      }
    ]
  });

  mcDd.mount($("cMc"));
  loaderDd.mount($("cLoader"));
  lvDd.mount($("cLv"));
  bindVisibilitySeg($("cVis"), (value) => (form.visibility = value));
  bindPackIconPicker($("cIcon"), {
    onChange: (icon) => {
      form.icon = icon;
      showError("");
    },
    onError: showError
  });
  $("cName")?.addEventListener("input", updateNamePlaceholder);
  $("cSnap")?.addEventListener("change", (event) => {
    form.snapshots = event.target.checked;
    loadMc();
  });
  loadMc();

  if ((await choice) !== "ok" || !created || !S.view) return;
  addCustomInstance(created);
  setIsland("ok", t("create.done", { name: created.name }));
  selectInstance(created.id, { save: false });
  // Публічна / за проханням: одразу публікуємо легку копію для друзів (у фоні, гра не блокується).
  if (form.visibility !== "private") applyVisibility(created.id, form.visibility);
}

/* каталог контенту (Modrinth / CurseForge) у стилі Prism Launcher: моди, ресурс-паки, шейдери, готові збірки */

const MOD_PLACEHOLDER = '<span class="mod-icon mod-icon-empty" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M21 16V8a2 2 0 0 0-1-1.7l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.7l7 4a2 2 0 0 0 2 0l7-4a2 2 0 0 0 1-1.7z"/></svg></span>';

function modIcon(url) {
  if (typeof url !== "string" || !url.startsWith("https://")) return MOD_PLACEHOLDER;
  return `<img class="mod-icon" src="${esc(url)}" alt="" loading="lazy" draggable="false">`;
}

const ALL_MOD_SOURCES = [
  { id: "modrinth", label: "Modrinth" },
  { id: "curseforge", label: "CurseForge" }
];
/** Платформи в перемикачі менеджера модів (CurseForge — лише з ENABLE_CURSEFORGE). */
const MOD_SOURCES = ALL_MOD_SOURCES.filter((source) => ENABLE_CURSEFORGE || source.id !== "curseforge");
const modCategoryCache = new Map();

/** Підпис джерела мода (і для вже встановлених раніше модів з CurseForge). */
function sourceLabel(id) {
  return ALL_MOD_SOURCES.find((source) => source.id === (id || "modrinth"))?.label || "";
}

/** Версії файлу мода: «Найновіша сумісна (Авто)» + конкретні файли з бейджами типу релізу. */
function modVersionItems(list, autoLabel = t("mods.latestCompatible")) {
  const items = autoVersionItem(autoLabel);
  list.forEach((version, index) => {
    const kind = ["release", "beta", "alpha"].includes(version.kind) ? version.kind : "release";
    const badges = [{ text: t("badge." + kind), kind }];
    if (index === 0) badges.push({ text: t("badge.latest"), kind: "latest" });
    items.push({
      value: version.id,
      label: version.version_number || version.name || version.filename,
      sub: [version.date ? changelogDate(version.date) : "", version.filename].filter(Boolean).join(" · "),
      badges,
      search: version.filename
    });
  });
  return items;
}

function autoVersionItem(label = t("mods.latestCompatible")) {
  return [{ value: "", label, badges: [{ text: t("badge.auto"), kind: "auto" }] }];
}

const CONTENT_KINDS = ["mod", "resourcepack", "shader", "modpack"];
const KIND_LABEL = { mod: "content.mods", resourcepack: "content.resourcepacks", shader: "content.shaders", modpack: "content.modpacks" };
const KIND_ICON = { mod: "box", resourcepack: "image", shader: "sun", modpack: "layers" };
const KIND_FOLDER = { mod: "mods.folder", resourcepack: "content.folderRp", shader: "content.folderSh" };

function contentPlaceholder(kind, source) {
  if (kind === "mod") return t(source === "curseforge" ? "mods.searchPhCurse" : "mods.searchPh");
  return t({ resourcepack: "content.searchPhRp", shader: "content.searchPhSh", modpack: "content.searchPhMp" }[kind]);
}

/**
 * Каталог контенту. `id` — власна збірка (моди, ресурс-паки, шейдери й готові збірки);
 * збірка проєкту — лише перегляд складу модів; порожній `id` — лише каталог готових збірок.
 */
async function openContentDialog(id = S.activeId, startKind = "") {
  const inst = id ? S.view?.instances.find((i) => i.id === id) : null;
  if (id && !inst) return;
  const vanilla = Boolean(inst) && (!inst.loader || inst.loader === "vanilla");
  // Збірка проєкту: склад визначає manifest.json, тож лише перегляд модів.
  const managed = Boolean(inst) && !inst.custom;
  if (managed && vanilla) return;
  const kinds = managed ? ["mod"] : inst ? CONTENT_KINDS : ["modpack"];
  const fallback = managed ? "mod" : !inst ? "modpack" : vanilla ? "resourcepack" : "mod";
  const M = {
    kind: kinds.includes(startKind) ? startKind : fallback,
    source: "modrinth",
    tab: managed ? "installed" : "browse",
    query: "",
    category: "",
    offset: 0,
    total: 0,
    hits: [],
    installed: [],
    busy: new Set(),
    seq: 0,
    listSeq: 0,
    versions: new Map(),
    picked: new Map(),
    pickers: [],
    cf: false,
    shaderMod: true
  };
  const keyOf = (projectId) => M.source + ":" + M.kind + ":" + projectId;
  const installedKeys = () => new Set(M.installed.filter((m) => m.project_id).map((m) => (m.source || "modrinth") + ":" + M.kind + ":" + m.project_id));
  const loaderText = inst ? loaderName(inst.loader) + (inst.loader_version ? " " + inst.loader_version : "") : "";
  const chipsFor = (kind) => {
    if (!inst || kind === "modpack") return "";
    const mc = `<span class="chip">Minecraft ${esc(inst.minecraft_version)}</span>`;
    return kind === "mod" ? mc + `<span class="chip">${loaderIcon(inst.loader)}${esc(loaderName(inst.loader))}</span>` : mc;
  };
  const leadFor = (kind) => {
    if (managed) return t("mods.managedLead");
    if (kind === "mod") return t("mods.lead", { loader: loaderText, mc: inst.minecraft_version });
    if (kind === "resourcepack") return t("content.leadResourcepacks", { mc: inst.minecraft_version });
    if (kind === "shader") return t("content.leadShaders", { mc: inst.minecraft_version });
    return t("content.leadModpacks");
  };
  const shaderModName = () => (inst?.loader === "forge" ? "Oculus" : "Iris");
  /** Для ванільної збірки моди й шейдери недоступні — показуємо пояснення замість пошуку. */
  const blocked = () => Boolean(inst) && !managed && vanilla && (M.kind === "mod" || M.kind === "shader");
  const cfLocked = () => M.source === "curseforge" && !M.cf;
  const skeletons = '<div class="skeleton"></div><div class="skeleton"></div><div class="skeleton"></div>';

  const kindTabs =
    kinds.length > 1
      ? `<div class="kind-tabs" id="mKinds" role="tablist">${kinds
          .map((kind) => `<button type="button" role="tab" data-kind="${kind}">${ICONS[KIND_ICON[kind]]}<span>${esc(t(KIND_LABEL[kind]))}</span></button>`)
          .join("")}</div>`
      : "";
  const html = `${kindTabs}<p class="content-lead" id="mLead"></p>
      <div class="mods">
        <div class="mods-head">
          <div class="seg seg-sm" id="mTabs" role="tablist"${managed ? " hidden" : ""}>
            <button type="button" role="tab" data-mtab="browse">${esc(t("content.browse"))}</button>
            <button type="button" role="tab" data-mtab="installed">${esc(t("mods.installed"))} <span class="mods-count" id="mCount"></span></button>
          </div>
          <span class="chips" id="mChipsHead"${managed ? "" : " hidden"}></span>
          <button class="btn btn-ghost btn-sm" type="button" id="mFolder">${ICONS.folder}<span id="mFolderText"></span></button>
        </div>
        <div class="cf-note" id="mNote" hidden>${ICONS.alert}<span id="mNoteText"></span><button class="btn btn-primary btn-sm" type="button" id="mNoteAct" hidden><span class="btn-spin" aria-hidden="true"></span><span class="btn-text"></span></button></div>
        <div id="mBrowse" class="mods-pane"${managed ? " hidden" : ""}>
          <div class="mods-tools">
            <div class="seg seg-sm seg-src" id="mSource" role="radiogroup">${MOD_SOURCES.map((s) => `<button type="button" role="radio" data-src="${s.id}">${s.label}</button>`).join("")}</div>
            <label class="mods-search">${ICONS.search}<input type="text" id="mQuery" spellcheck="false" autocomplete="off"></label>
            <div class="dd-mount mods-cat" id="mCat"></div>
          </div>
          <div class="mods-filter" id="mFilter"><span class="mods-filter-label">${esc(t("mods.autoFilter"))}</span><span class="chips" id="mChips"></span></div>
          <div class="cf-note" id="mCfNote" hidden>${ICONS.alert}<span>${esc(t("mods.cfNeedKey"))}</span><button class="btn btn-ghost btn-sm" type="button" id="mCfSettings">${esc(t("mods.cfOpenSettings"))}</button></div>
          <span class="hint" id="mHint"></span>
          <div class="mod-list" id="mResults"></div>
          <button class="btn btn-ghost btn-sm mods-more" type="button" id="mMore" hidden><span class="btn-spin" aria-hidden="true"></span><span>${esc(t("mods.more"))}</span></button>
        </div>
        <div id="mInstalled" class="mods-pane"${managed ? "" : " hidden"}><div class="mod-list" id="mList">${skeletons}</div></div>
      </div>`;

  const choice = openModal({
    icon: KIND_ICON[M.kind] === "layers" && !inst ? "layers" : "box",
    title: managed ? t("mods.managedTitle", { name: inst.name }) : inst ? t("content.title", { name: inst.name }) : t("content.modpacks"),
    html,
    xl: true,
    kind: "content",
    buttons: [{ id: "done", label: t("common.done"), kind: "primary" }]
  });

  const catDd = managed
    ? null
    : new Dropdown({
        className: "dd-sm",
        label: t("mods.category"),
        placeholder: t("mods.allCategories"),
        search: true,
        minWidth: 240,
        items: [{ value: "", label: t("mods.allCategories") }],
        value: "",
        onChange: (value) => {
          M.category = value;
          search(false);
        }
      });
  if (catDd) catDd.mount($("mCat"));

  const renderTabs = () => {
    if (!$("mList")) return;
    if (!managed) {
      for (const button of $("mTabs").querySelectorAll("[data-mtab]")) {
        const on = button.dataset.mtab === M.tab;
        button.classList.toggle("on", on);
        button.setAttribute("aria-selected", String(on));
      }
      $("mBrowse").hidden = M.tab !== "browse";
      $("mInstalled").hidden = M.tab !== "installed";
    }
    $("mCount").textContent = M.installed.length ? String(M.installed.length) : "";
  };

  const renderNote = () => {
    const note = $("mNote");
    if (!note) return;
    let text = "";
    let action = "";
    if (!managed && inst && M.kind === "mod" && vanilla) text = t("content.modsVanilla");
    else if (!managed && inst && M.kind === "shader" && vanilla) text = t("content.shaderVanilla");
    else if (!managed && inst && M.kind === "shader" && !M.shaderMod) {
      text = t("content.shaderNeed");
      action = t("content.shaderInstall", { mod: shaderModName() });
    }
    note.hidden = !text;
    $("mNoteText").textContent = text;
    const act = $("mNoteAct");
    act.hidden = !action;
    act.querySelector(".btn-text").textContent = action;
  };

  const renderSource = () => {
    if (managed || !$("mSource")) return;
    for (const button of $("mSource").querySelectorAll("[data-src]")) {
      const on = button.dataset.src === M.source;
      button.classList.toggle("on", on);
      button.setAttribute("aria-checked", String(on));
    }
    const locked = cfLocked();
    const off = locked || blocked();
    $("mQuery").placeholder = contentPlaceholder(M.kind, M.source);
    $("mQuery").disabled = off;
    $("mCfNote").hidden = !locked || blocked();
    $("mResults").hidden = off;
    $("mHint").hidden = off;
    if (off) $("mMore").hidden = true;
    catDd.setDisabled(off);
  };

  const renderKind = () => {
    if (!$("mLead")) return;
    $("mLead").textContent = leadFor(M.kind);
    for (const button of $("mKinds")?.querySelectorAll("[data-kind]") || []) {
      const on = button.dataset.kind === M.kind;
      button.classList.toggle("on", on);
      button.setAttribute("aria-selected", String(on));
    }
    const packs = M.kind === "modpack";
    if (packs) M.tab = "browse";
    $("mFolder").hidden = packs || !inst;
    if (!packs) $("mFolderText").textContent = t(KIND_FOLDER[M.kind]);
    if (!managed) {
      $("mTabs").hidden = packs;
      $("mFilter").hidden = packs || !inst;
      $("mChips").innerHTML = chipsFor(M.kind);
    } else $("mChipsHead").innerHTML = chipsFor("mod");
    renderNote();
    renderSource();
    renderTabs();
  };

  const loadCategories = async () => {
    const source = M.source;
    const kind = M.kind;
    if (managed || cfLocked() || blocked()) return;
    const cacheKey = source + ":" + kind;
    let list = modCategoryCache.get(cacheKey);
    if (!list) {
      try {
        list = await invoke("mod_categories", { source, kind });
        modCategoryCache.set(cacheKey, list);
      } catch {
        list = [];
      }
    }
    if (source !== M.source || kind !== M.kind || !$("mCat")) return;
    catDd.setItems([{ value: "", label: t("mods.allCategories") }, ...list.map((c) => ({ value: c.id, label: c.name }))], "");
  };

  const loadStatus = async () => {
    if (!inst || managed || vanilla) return;
    try {
      M.shaderMod = Boolean((await invoke("content_status", { instanceId: id })).shader_mod);
    } catch {
      M.shaderMod = true;
    }
    renderNote();
  };

  const closePickers = () => {
    for (const picker of M.pickers) picker.close();
  };

  const rowHtml = (hit, index, have) => {
    const key = keyOf(hit.id);
    const done = have.has(key);
    const busy = M.busy.has(key);
    const label = busy ? t("mods.installing") : done ? t("mods.isInstalled") : t("mods.install");
    const mc = M.kind === "modpack" && hit.game_versions?.length ? " · " + t("mp.mc", { v: hit.game_versions[0] }) : "";
    return `<div class="mod-row enter" data-pid="${esc(hit.id)}" style="--i:${Math.min(index, 10)}">
        ${modIcon(hit.icon_url)}
        <span class="mod-info"><b>${esc(hit.title)}</b><small>${esc(t("mods.by", { author: hit.author || "?" }))} · ↓ ${esc(formatCount(hit.downloads))}${esc(mc)}</small><span class="mod-desc">${esc(hit.description)}</span><span class="mod-err" hidden></span></span>
        <span class="mod-act">
          <span class="dd-mount" data-ver="${esc(hit.id)}"></span>
          <button class="btn btn-sm ${done ? "btn-ghost" : "btn-primary"}${busy ? " loading" : ""}" type="button" data-install="${esc(hit.id)}" ${done || busy ? "disabled" : ""}><span class="btn-spin" aria-hidden="true"></span><span class="btn-text">${esc(label)}</span></button>
        </span>
      </div>`;
  };

  /** Рядки результатів; from > 0 — додати лише нові (без повторної анімації старих). */
  const renderResults = (from = 0) => {
    const box = $("mResults");
    if (!box) return;
    const have = installedKeys();
    if (from === 0) {
      closePickers();
      M.pickers = [];
      box.innerHTML = "";
    }
    box.insertAdjacentHTML("beforeend", M.hits.slice(from).map((hit, index) => rowHtml(hit, index, have)).join(""));
    const autoLabel = M.kind === "modpack" ? t("mp.latest") : t("mods.latestCompatible");
    for (const hit of M.hits.slice(from)) {
      const mount = box.querySelector(`[data-ver="${CSS.escape(hit.id)}"]`);
      if (!mount) continue;
      const key = keyOf(hit.id);
      const known = M.versions.get(key);
      const picker = new Dropdown({
        className: "dd-sm dd-ver",
        label: t("mods.version"),
        search: true,
        minWidth: 330,
        items: known ? modVersionItems(known, autoLabel) : autoVersionItem(autoLabel),
        value: M.picked.get(key) || "",
        pendingText: t("mods.loadingVersions"),
        onOpen: (dd) => loadVersions(hit.id, dd),
        onChange: (value) => M.picked.set(key, value)
      });
      picker.mount(mount);
      M.pickers.push(picker);
    }
    $("mMore").hidden = M.hits.length >= M.total || !M.hits.length;
  };

  const refreshInstallButtons = () => {
    const box = $("mResults");
    if (!box) return;
    const have = installedKeys();
    for (const button of box.querySelectorAll("[data-install]")) {
      const key = keyOf(button.dataset.install);
      const done = have.has(key);
      const busy = M.busy.has(key);
      button.disabled = done || busy || (M.kind === "modpack" && Boolean(S.busy) && !busy);
      button.classList.toggle("loading", busy);
      button.classList.toggle("btn-primary", !done);
      button.classList.toggle("btn-ghost", done);
      if (!busy || M.kind !== "modpack") button.querySelector(".btn-text").textContent = busy ? t("mods.installing") : done ? t("mods.isInstalled") : t("mods.install");
    }
  };

  const loadVersions = async (projectId, picker) => {
    const key = keyOf(projectId);
    const kind = M.kind;
    if (M.versions.has(key)) return;
    picker.setPending(true);
    try {
      const list =
        kind === "modpack"
          ? await invoke("modpack_versions", { source: M.source, projectId })
          : await invoke("mod_versions", { instanceId: id, source: M.source, projectId, kind });
      M.versions.set(key, list);
      picker.setItems(modVersionItems(list, kind === "modpack" ? t("mp.latest") : t("mods.latestCompatible")), M.picked.get(key) || "");
    } catch (error) {
      picker.setPending(false);
      setIsland("error", errHuman(error));
    }
  };

  const renderInstalled = (animate = false) => {
    const box = $("mList");
    if (!box) return;
    renderTabs();
    if (!M.installed.length) {
      box.innerHTML = `<div class="empty small">${esc(t(managed || M.kind === "mod" ? "mods.none" : "content.none"))}</div>`;
      return;
    }
    box.innerHTML = M.installed
      .map((mod, index) => {
        let sub;
        if (mod.managed) sub = mod.enabled ? t("mods.managed") : t("mods.managedMissing");
        else if (managed) sub = t("mods.extra");
        else sub = mod.manual ? t("mods.manual") : [sourceLabel(mod.source), mod.version_number, mod.dependency ? t("mods.dependency") : ""].filter(Boolean).join(" · ");
        const tone = managed ? (mod.managed ? (mod.enabled ? "" : " missing") : " extra") : mod.enabled ? "" : " off";
        const controls = managed
          ? `<span class="mod-state${mod.managed && mod.enabled ? " ok" : ""}" aria-hidden="true"></span>`
          : `<label class="switch" title="${esc(t("content.enable"))}"><input type="checkbox" data-toggle="${esc(mod.filename)}" ${mod.enabled ? "checked" : ""}><span></span></label>
             <button class="more" type="button" data-remove="${esc(mod.filename)}" title="${esc(t("content.remove"))}" aria-label="${esc(t("content.remove"))}">${ICONS.trash}</button>`;
        return `<div class="mod-row${tone}${animate ? " enter" : ""}" style="--i:${Math.min(index, 10)}">
          ${modIcon(mod.icon_url)}
          <span class="mod-info"><b>${esc(mod.title)}</b><small>${esc(sub || mod.filename)}</small></span>
          ${controls}
        </div>`;
      })
      .join("");
  };

  const loadInstalled = async (animate = false) => {
    if (!inst || M.kind === "modpack") {
      M.installed = [];
      renderTabs();
      return;
    }
    const kind = M.kind;
    const my = ++M.listSeq;
    let list = [];
    try {
      list = await invoke("list_mods", { instanceId: id, kind });
    } catch (error) {
      setIsland("error", errHuman(error));
    }
    if (my !== M.listSeq || kind !== M.kind) return;
    M.installed = list;
    renderInstalled(animate);
    refreshInstallButtons();
  };

  const search = async (append = false) => {
    if (managed || cfLocked() || blocked() || !$("mResults")) return;
    const my = ++M.seq;
    const kind = M.kind;
    const hint = $("mHint");
    const more = $("mMore");
    if (!append) {
      M.offset = 0;
      closePickers();
      $("mResults").innerHTML = skeletons;
      more.hidden = true;
    } else setLoading(more, true);
    try {
      const args = { source: M.source, query: M.query, category: M.category || null, offset: M.offset };
      const page = kind === "modpack" ? await invoke("search_modpacks", args) : await invoke("search_mods", { ...args, instanceId: id, kind });
      if (my !== M.seq || !$("mResults")) return;
      M.total = page.total_hits;
      const from = append ? M.hits.length : 0;
      M.hits = append ? M.hits.concat(page.hits) : page.hits;
      M.offset = M.hits.length;
      if (hint) hint.textContent = M.query || M.category ? "" : t(kind === "modpack" ? "content.popularPacks" : kind === "mod" ? "mods.popular" : "content.popular");
      renderResults(from);
      if (!M.hits.length) $("mResults").innerHTML = `<div class="empty small">${esc(t("mods.nothing"))}</div>`;
    } catch (error) {
      if (my !== M.seq || !$("mResults")) return;
      if (!append) $("mResults").innerHTML = `<div class="empty small">${esc(errHuman(error))}</div>`;
      else setIsland("error", errHuman(error));
    } finally {
      if ($("mMore")) setLoading($("mMore"), false);
    }
  };

  const showRowError = (row, error) => {
    const node = row?.querySelector(".mod-err");
    if (!node || !node.isConnected) return;
    node.textContent = errHuman(error);
    node.hidden = false;
  };

  /** Готова збірка → нова окрема збірка (команда тримає «важке» блокування лаунчера). */
  const installPack = async (projectId, row) => {
    if (S.busy || S.updating) {
      setIsland("warn", t("play.wait"));
      return;
    }
    const hit = M.hits.find((h) => h.id === projectId);
    const key = keyOf(projectId);
    const source = M.source;
    row?.querySelector(".mod-err")?.setAttribute("hidden", "");
    M.busy.add(key);
    S.busy = "modpack";
    S.modpackName = hit?.title || "";
    refreshInstallButtons();
    renderPlay();
    setIsland("busy", t("mp.installing"));
    try {
      const result = await invoke("install_modpack", { source, projectId, versionId: M.picked.get(key) || null });
      addCustomInstance(result.instance);
      setIsland("ok", t("mp.doneTitle", { name: result.report.name }));
      showModpackReport(result.report);
      selectInstance(result.instance.id, { save: false });
    } catch (error) {
      setIsland("error", errHuman(error));
      showRowError(row, error);
    } finally {
      S.busy = null;
      S.modpackName = "";
      M.busy.delete(key);
      refreshInstallButtons();
      renderList();
      renderHero();
    }
  };

  const install = async (projectId, row) => {
    if (M.kind === "modpack") return installPack(projectId, row);
    if (S.running === id) {
      setIsland("warn", t("mods.running"));
      return;
    }
    const key = keyOf(projectId);
    const kind = M.kind;
    const source = M.source;
    row?.querySelector(".mod-err")?.setAttribute("hidden", "");
    M.busy.add(key);
    refreshInstallButtons();
    try {
      const report = await invoke("install_mod", { instanceId: id, source, projectId, versionId: M.picked.get(key) || null, kind });
      setIsland(
        report.missing.length ? "warn" : "ok",
        report.missing.length ? t("mods.missing", { list: report.missing.join(", ") }) : t("mods.done", { list: report.installed.join(", ") })
      );
    } catch (error) {
      setIsland("error", errHuman(error));
      showRowError(row, error);
    } finally {
      M.busy.delete(key);
      if (kind === M.kind) await loadInstalled();
      else refreshInstallButtons();
      // Iris/Oculus могли поставити вручну з вкладки модів — підказку для шейдерів оновлюємо.
      if (kind === "mod") loadStatus();
    }
  };

  /** Перемикання типу контенту: пошук, категорії й встановлене — для нового типу. */
  const switchKind = (kind) => {
    if (kind === M.kind || !kinds.includes(kind)) return;
    closePickers();
    catDd?.close();
    M.kind = kind;
    M.category = "";
    M.query = "";
    M.hits = [];
    M.offset = 0;
    M.total = 0;
    M.seq++;
    M.listSeq++;
    M.installed = [];
    if ($("mQuery")) $("mQuery").value = "";
    if ($("mList")) $("mList").innerHTML = skeletons;
    catDd?.setItems([{ value: "", label: t("mods.allCategories") }], "");
    renderKind();
    loadCategories();
    loadInstalled(true);
    search(false);
  };

  // Іконка не завантажилась — показуємо заглушку (error не спливає, тому capture).
  el.modalText.querySelector(".mods").addEventListener(
    "error",
    (event) => {
      if (event.target.matches?.("img.mod-icon")) event.target.outerHTML = MOD_PLACEHOLDER;
    },
    true
  );
  $("mFolder").addEventListener("click", () => invoke("open_mods_folder", { instanceId: id, kind: M.kind }).catch((e) => setIsland("error", errText(e))));
  $("mKinds")?.addEventListener("click", (event) => {
    const button = event.target.closest("[data-kind]");
    if (button) switchKind(button.dataset.kind);
  });
  $("mNoteAct").addEventListener("click", async (event) => {
    const button = event.currentTarget;
    if (button.disabled) return;
    if (S.running === id) {
      setIsland("warn", t("mods.running"));
      return;
    }
    setLoading(button, true);
    try {
      const report = await invoke("install_shader_mod", { instanceId: id });
      M.shaderMod = true;
      setIsland("ok", t("content.shaderModDone", { mod: report.installed[0] || shaderModName() }));
      renderNote();
    } catch (error) {
      setIsland("error", errHuman(error));
    } finally {
      if (button.isConnected) setLoading(button, false);
    }
  });

  if (!managed) {
    $("mTabs").addEventListener("click", (event) => {
      const button = event.target.closest("[data-mtab]");
      if (!button || button.dataset.mtab === M.tab) return;
      closePickers();
      M.tab = button.dataset.mtab;
      renderTabs();
      if (M.tab === "browse" && !$("mQuery").disabled) $("mQuery").focus();
    });
    $("mSource").addEventListener("click", (event) => {
      const button = event.target.closest("[data-src]");
      if (!button || button.dataset.src === M.source) return;
      M.source = button.dataset.src;
      M.category = "";
      M.hits = [];
      M.seq++;
      renderSource();
      loadCategories();
      search(false);
    });
    $("mCfSettings").addEventListener("click", () => {
      closeModal("settings");
      setTimeout(() => {
        switchTab("settings");
        el.cfKey.focus();
      }, 220);
    });
    let searchTimer = 0;
    $("mQuery").addEventListener("input", (event) => {
      M.query = event.target.value.trim();
      clearTimeout(searchTimer);
      searchTimer = setTimeout(() => search(false), 350);
    });
    $("mQuery").addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        clearTimeout(searchTimer);
        search(false);
      }
    });
    $("mMore").addEventListener("click", () => search(true));
    $("mResults").addEventListener("click", (event) => {
      const button = event.target.closest("[data-install]");
      if (button && !button.disabled) install(button.dataset.install, button.closest(".mod-row"));
    });
    // Вмикання/вимикання: рядок одразу плавно «гасне», файл перейменовується у фоні.
    $("mList").addEventListener("change", async (event) => {
      const input = event.target.closest("[data-toggle]");
      if (!input) return;
      const kind = M.kind;
      const row = input.closest(".mod-row");
      const mod = M.installed.find((m) => m.filename === input.dataset.toggle);
      const enabled = input.checked;
      row?.classList.toggle("off", !enabled);
      input.disabled = true;
      try {
        await invoke("set_mod_enabled", { instanceId: id, filename: input.dataset.toggle, enabled, kind });
        if (mod) mod.enabled = enabled;
        if (kind === "mod") loadStatus();
      } catch (error) {
        input.checked = !enabled;
        row?.classList.toggle("off", enabled);
        setIsland("error", errHuman(error));
      } finally {
        input.disabled = false;
      }
    });
    $("mList").addEventListener("click", async (event) => {
      const button = event.target.closest("[data-remove]");
      if (!button || button.disabled) return;
      const kind = M.kind;
      const filename = button.dataset.remove;
      const mod = M.installed.find((m) => m.filename === filename);
      const row = button.closest(".mod-row");
      button.disabled = true;
      try {
        await invoke("remove_mod", { instanceId: id, filename, kind });
        setIsland("ok", t("content.removed", { name: mod?.title || filename }));
        if (kind !== M.kind) return;
        M.installed = M.installed.filter((m) => m.filename !== filename);
        leave(row, "leaving", 240, () => {
          renderInstalled();
          refreshInstallButtons();
        });
        if (kind === "mod") loadStatus();
      } catch (error) {
        button.disabled = false;
        setIsland("error", errHuman(error));
      }
    });
  }

  renderKind();
  try {
    M.cf = Boolean((await invoke("mod_sources")).curseforge);
  } catch {
    M.cf = false;
  }
  renderSource();
  loadCategories();
  loadStatus();
  await loadInstalled(true);
  if (!managed && !startKind && M.kind !== "modpack" && M.installed.length) {
    M.tab = "installed";
    renderTabs();
  }
  search(false);

  await choice;
  closePickers();
  catDd?.close();
  if (!inst) return;
  try {
    mergeInstance(await invoke("instance_details", { instanceId: id }));
  } catch {
    /* лічильник модів оновиться пізніше */
  }
}

/** Звіт про встановлену готову збірку (і файли, які треба докачати вручну). */
function showModpackReport(report) {
  const stats = `<div class="usage">${statCell(t("mp.files"), report.files, "good")}${statCell(t("mp.overrides"), report.overrides)}${statCell(t("mp.skipped"), report.skipped)}</div>`;
  const missing = report.missing.length
    ? `<p>${esc(t("mp.missing"))}</p><ul class="dbg-list">${report.missing
        .slice(0, 40)
        .map((name) => `<li>${esc(name)}</li>`)
        .join("")}</ul>`
    : "";
  const loader = loaderName(report.loader) + (report.loader_version ? " " + report.loader_version : "");
  openModal({
    icon: "layers",
    tone: report.missing.length ? "warn" : "",
    title: t("mp.doneTitle", { name: report.name }),
    html: `<p>${esc(t("mp.doneText", { mc: report.minecraft_version, loader }))}</p>${stats}${missing}`,
    buttons: [{ id: "close", label: t("common.close"), kind: "primary" }]
  });
}

/** Прогрес завантаження файлів готової збірки: «острів» і кнопка в рядку каталогу. */
function onModpackProgress(payload) {
  if (S.busy !== "modpack") return;
  const p = Math.max(0, Math.min(100, Math.floor(Number(payload.percent) || 0)));
  setIsland("busy", t("mp.progress", { name: S.modpackName || "…", p }));
  const label = document.querySelector("#mResults [data-install].loading .btn-text");
  if (label) label.textContent = p + "%";
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
  if (el.instMenu.hidden || el.instMenu.classList.contains("closing")) return;
  S.menuFor = null;
  leave(el.instMenu, "closing", 140, () => {
    if (S.menuFor === null) el.instMenu.hidden = true;
    el.instMenu.classList.remove("closing");
  });
}

function openMenu(button, id) {
  if (S.menuFor === id && !el.instMenu.hidden) {
    closeMenu();
    return;
  }
  S.menuFor = id;
  hideTip();
  const locked = Boolean(S.busy) || Boolean(S.running) || S.updating;
  const target = S.view?.instances.find((i) => i.id === id);
  const item = (act) => el.instMenu.querySelector(`[data-act="${act}"]`);
  const vanilla = !target?.loader || target.loader === "vanilla";
  item("folder").disabled = !target?.installed && !target?.custom;
  // Контент: власна збірка — повний каталог (ванільна — ресурс-паки); збірка проєкту — перегляд модів.
  item("mods").hidden = vanilla && !target?.custom;
  item("mods").disabled = !target?.custom && !target?.installed;
  item("repair").disabled = locked || !target?.installed;
  item("debug").hidden = Boolean(target?.custom);
  item("debug").disabled = Boolean(S.busy) || S.updating || S.running === id || !target?.installed;
  item("import").disabled = Boolean(S.busy) || S.updating || S.running === id;
  // Видимість для друзів — лише для власних збірок.
  item("visibility").hidden = !target?.custom;
  item("visibility").disabled = Boolean(S.busy) || S.updating;
  item("icon").hidden = !target?.custom;
  item("delete").disabled = Boolean(S.busy) || S.running === id || (!target?.installed && !target?.custom);

  el.instMenu.classList.remove("closing");
  el.instMenu.hidden = false;
  // Анімація з'являється заново при кожному відкритті.
  el.instMenu.style.animation = "none";
  void el.instMenu.offsetWidth;
  el.instMenu.style.animation = "";
  const rect = button.getBoundingClientRect();
  const width = el.instMenu.offsetWidth;
  const height = el.instMenu.offsetHeight;
  const below = rect.bottom + 6 + height <= window.innerHeight - 8;
  const top = below ? rect.bottom + 6 : Math.max(8, rect.top - height - 6);
  el.instMenu.classList.toggle("up", !below);
  el.instMenu.style.left = Math.max(8, rect.right - width) + "px";
  el.instMenu.style.top = top + "px";
  el.instMenu.querySelector("button:not(:disabled):not([hidden])")?.focus();
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
  renderAccountCard();
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

  // Активний профіль змінився: рамка «перетікає» зі старої картки на нову (лише opacity/transform).
  const previous = profilesActiveShown;
  profilesActiveShown = S.activeProfile;
  if (previous && previous !== S.activeProfile) {
    const oldRow = el.profileList.querySelector(`[data-profile="${CSS.escape(previous)}"]`);
    const newRow = el.profileList.querySelector(`[data-profile="${CSS.escape(S.activeProfile)}"]`);
    oldRow?.classList.add("was-on");
    newRow?.classList.remove("on");
    void el.profileList.offsetWidth;
    oldRow?.classList.remove("was-on");
    newRow?.classList.add("on");
  }
}

let profilesActiveShown = null;

/** Нативний діалог вибору .png (IFileOpenDialog у фоновому STA-потоці): перевірку розміру
 *  й збереження робить бекенд — без передачі мегабайтного масиву байтів через IPC. */
async function pickSkin(profileId) {
  if (!profileId || S.picking) return;
  S.picking = true;
  try {
    const view = await invoke("pick_skin", { profileId });
    if (!view) return;
    S.skins.delete(profileId);
    applyProfiles(view);
    renderProfiles();
    setIsland("ok", t("profiles.skinSaved"));
  } catch (error) {
    setIsland("error", errHuman(error));
  } finally {
    S.picking = false;
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
      const view = await invoke("remove_profile", { profileId: id });
      S.skins.delete(id);
      // Картка спершу плавно зникає, потім список перебудовується.
      await new Promise((resolve) => leave(row, "leaving", 240, resolve));
      applyProfiles(view);
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
  const linked = Boolean(S.binding);
  el.leaveCard.hidden = !linked;
  el.projRefresh.hidden = !linked;
  el.projMeta.hidden = !linked;
  el.projPicker.hidden = linked;
  el.switchTitle.dataset.i18n = linked ? "project.switchTitle" : "project.connectTitle";
  el.switchTitle.textContent = t(el.switchTitle.dataset.i18n);
  el.switchHint.dataset.i18n = linked ? "project.switchHint" : "project.connectHint";
  el.switchHint.textContent = t(el.switchHint.dataset.i18n);
  if (!linked) {
    // Без проєкту: пояснення + список публічних проєктів і поле для коду / owner/repo.
    el.projName.textContent = t("project.none");
    el.projState.textContent = t("project.noneSub");
    paintAvatar(el.projBrand, "T", []);
    if (!S.projectList) loadProjectList();
    else renderProjectList();
    return;
  }
  const name = S.view?.name || S.binding?.name || "—";
  el.projName.textContent = name;
  paintAvatar(el.projBrand, initial(name), projectIcons());
  el.projCode.textContent = S.binding?.code || "—";
  el.projCount.textContent = S.view ? plural(S.view.instances.filter((i) => !i.custom).length, "plural.packs") : "";
  el.projState.textContent = S.view ? (S.view.offline ? t("project.offline") : t("project.online")) : S.viewError || "";
}

function setLoading(button, on) {
  button.classList.toggle("loading", on);
  button.disabled = on;
}

let changelogToken = 0;

function changelogDate(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleDateString(lang === "en" ? "en-GB" : "uk-UA", { day: "2-digit", month: "short", year: "numeric" });
}

const CL_ICONS = {
  github: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 4h6v6M20 4l-9 9M19 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4"/></svg>',
  expand: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/></svg>'
};

function changelogButton(action, label) {
  return `<button class="cl-btn" type="button" data-cl="${action}" title="${esc(label)}" aria-label="${esc(label)}">${CL_ICONS[action]}</button>`;
}

function changelogTag(item) {
  return `${item.kind === "release" ? t("cl.release") : t("cl.commit")} · ${item.tag}`;
}

function changelogMeta(item) {
  return [changelogDate(item.date), item.author].filter(Boolean).join(" · ");
}

function changelogItem(item, column, index) {
  const release = item.kind === "release";
  const body = item.body ? `<div class="cl-body">${esc(item.body)}</div>` : "";
  return `<article class="cl-item${release ? " release" : ""}" data-index="${index}"><div class="cl-top"><span class="cl-tag">${esc(changelogTag(item))}</span><span class="cl-date">${esc(changelogMeta(item))}</span></div><div class="cl-title">${esc(item.title)}</div>${body}<div class="cl-actions">${changelogButton("expand", t("cl.read"))}${changelogButton("github", t("cl.github"))}</div></article>`;
}

function changelogColumn(key, label, column) {
  let body;
  if (column.error) body = `<div class="cl-state">${esc(column.error)}</div>`;
  else if (!column.repo) body = `<div class="cl-state">${esc(t("cl.noRepo"))}</div>`;
  else if (!column.items.length) body = `<div class="cl-state">${esc(t("cl.empty"))}</div>`;
  else body = column.items.map((item, index) => changelogItem(item, key, index)).join("");
  const link = column.repo ? changelogButton("github", t("cl.github")) : "";
  return `<section class="cl-col glass" data-column="${key}"><header><div class="cl-head"><small>${esc(label)}</small><h4 class="mono selectable" title="${esc(column.repo)}">${esc(column.repo || "—")}</h4></div>${link}</header><div class="cl-list">${body}</div></section>`;
}

function paintChangelog() {
  const data = S.changelog;
  if (!data) return;
  el.clWrap.innerHTML = `<div class="cl-grid">${changelogColumn("launcher", t("cl.launcher"), data.launcher)}${changelogColumn("project", t("cl.project"), data.project)}</div>`;
}

async function loadChangelog() {
  const token = ++changelogToken;
  // Записи іншого проєкту не показуємо, поки вантажаться нові.
  const key = S.binding?.index_url || "";
  if (S.changelogKey !== key) S.changelog = null;
  S.changelogKey = key;
  if (S.changelog) paintChangelog();
  else el.clWrap.innerHTML = `<div class="cl-state">${esc(t("cl.loading"))}</div>`;
  try {
    const data = await invoke("load_changelog");
    if (token !== changelogToken) return;
    S.changelog = data;
    paintChangelog();
  } catch (error) {
    if (token === changelogToken) el.clWrap.innerHTML = `<div class="cl-state">${esc(errText(error))}</div>`;
  }
}

function openOnGithub(repo, item) {
  invoke("open_github", { repo, kind: item?.kind || "", tag: item?.tag || "" }).catch((e) => setIsland("error", errText(e)));
}

function openChangeItem(repo, item) {
  const meta = [changelogTag(item), changelogMeta(item)].filter(Boolean).join(" · ");
  openModal({
    title: item.title,
    html: `<p class="cl-meta">${esc(repo)}<br>${esc(meta)}</p><div class="md cl-full selectable">${item.body ? renderMarkdown(item.body) : `<p>${esc(t("cl.noBody"))}</p>`}</div>`,
    wide: true,
    buttons: [
      { id: "github", label: t("cl.github"), kind: "ghost", left: true, onClick: () => (openOnGithub(repo, item), false) },
      { id: "close", label: t("common.close"), kind: "primary" }
    ]
  });
}

function onChangelogClick(event) {
  const holder = event.target.closest("[data-column]");
  if (!holder || !S.changelog) return;
  const column = S.changelog[holder.dataset.column];
  const itemEl = event.target.closest(".cl-item");
  const item = itemEl ? column.items[Number(itemEl.dataset.index)] : null;
  const action = event.target.closest("[data-cl]")?.dataset.cl;
  if (action === "github") openOnGithub(column.repo, item);
  else if (item) openChangeItem(column.repo, item);
}

async function onSwitchProject(event) {
  event.preventDefault();
  if (el.switchSubmit.disabled) return;
  const code = el.switchCode.value.trim() || (S.binding ? "" : S.pickedProject);
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
    S.pickedProject = "";
    S.changelog = null;
    await loadProject();
    renderProject();
    setIsland("ok", t("project.switched", { name: S.binding.name || code }));
  } catch (error) {
    el.switchError.textContent = errHuman(error);
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
    // Лаунчер лишається відкритим: без проєкту показуються власні збірки.
    S.binding = null;
    S.changelog = null;
    S.pickedProject = "";
    await loadProject();
    renderTop();
    renderProject();
    setIsland("ok", t("project.left"));
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
    renderRamNote();
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

const GAME_LANGS = [
  ["uk_ua", "Українська"],
  ["en_us", "English (US)"],
  ["en_gb", "English (UK)"],
  ["pl_pl", "Polski"],
  ["de_de", "Deutsch"],
  ["fr_fr", "Français"],
  ["es_es", "Español"],
  ["cs_cz", "Čeština"],
  ["be_by", "Беларуская"],
  ["ru_ru", "Русский"]
];

const gameLangDd = new Dropdown({
  className: "dd-sm",
  minWidth: 200,
  items: GAME_LANGS.map(([value, label]) => ({ value, label, sub: value })),
  onChange: (value) => {
    S.settings.game_lang = value;
    invoke("save_settings", { patch: { game_lang: value } }).catch((e) => setIsland("error", errText(e)));
  }
});

async function refreshSettings() {
  renderRamHint();
  renderPreset();
  if (!el.gameLangDd.firstChild) gameLangDd.mount(el.gameLangDd);
  gameLangDd.trigger.setAttribute("aria-label", t("settings.gameLang"));
  gameLangDd.setValue(S.settings.game_lang);
  el.dataDir.textContent = S.settings.data_dir;
  el.resetDir.hidden = S.settings.data_dir === S.settings.default_data_dir;
  el.aboutText.textContent = t("settings.about", { v: S.launcherVersion });
  el.versionText.textContent = "v" + S.launcherVersion;
  el.autoUpdate.checked = Boolean(S.settings.auto_update_launcher);
  if (!el.themeGrid.firstChild || el.themeGrid.dataset.lang !== lang) {
    renderThemeGrid(el.themeGrid, S.settings.theme);
    el.themeGrid.dataset.lang = lang;
  } else markTheme(el.themeGrid, S.settings.theme);
  renderDanger();
  renderCurseForge();
  renderMirrors();
  invoke("mirrors_info")
    .then((info) => {
      S.mirrors = info;
      renderMirrors();
    })
    .catch(() => {});
  try {
    S.disk = await invoke("list_installed");
  } catch {
    S.disk = [];
  }
  renderDisk();
}

function renderCurseForge() {
  el.cfBlock.hidden = !ENABLE_CURSEFORGE;
  if (!ENABLE_CURSEFORGE) return;
  const s = S.settings;
  const dot = el.cfState.querySelector(".src-dot");
  const text = el.cfState.lastElementChild;
  const ready = s.curseforge_key_set || s.curseforge_key_env;
  dot.classList.toggle("ok", ready);
  text.textContent = s.curseforge_key_set ? t("settings.cfSet", { hint: s.curseforge_key_hint }) : s.curseforge_key_env ? t("settings.cfEnv") : t("settings.cfNone");
  el.cfClear.hidden = !s.curseforge_key_set;
}

function hostOf(template) {
  return String(template).replace(/^https:\/\//, "").split("/")[0];
}

function renderMirrors() {
  const info = S.mirrors;
  if (!info) {
    el.mirrorSource.textContent = "";
    el.mirrorList.innerHTML = "";
    el.mirrorCustom.textContent = "";
    return;
  }
  el.mirrorSource.textContent = t("mirrors.source." + info.source);
  const down = new Set(info.down || []);
  const group = (label, list) => {
    const hosts = [...new Set(list.map(hostOf))];
    return `<div class="mirror-group"><small>${esc(label)}</small><div class="mirror-chips">${hosts
      .map((host) => `<span class="mirror${down.has(host) ? " down" : ""}" title="${esc(down.has(host) ? t("mirrors.down") : host)}"><i></i>${esc(host)}</span>`)
      .join("")}</div></div>`;
  };
  el.mirrorList.innerHTML = group(t("mirrors.content"), info.content) + group(t("mirrors.update"), info.update);
  el.mirrorCustom.textContent = t("mirrors.custom", { file: info.custom_file, format: '{"content": [...], "update": [...]}' });
}

async function saveSettingsPatch(patch) {
  const view = await invoke("save_settings", { patch });
  if (view) S.settings = { ...S.settings, ...view };
  return view;
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
      if (d.kept_personal) notes.push(t("settings.kept"));
      else if (!d.installed) notes.push(t("settings.partial"));
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

async function onThemePick(event) {
  const button = event.target.closest("[data-theme]");
  if (!button || !S.settings || button.dataset.theme === S.settings.theme) return;
  const previous = S.settings.theme;
  const next = button.dataset.theme;
  S.settings.theme = next;
  applyTheme(next);
  markTheme(el.themeGrid, next);
  try {
    await saveSettingsPatch({ theme: next });
  } catch (error) {
    S.settings.theme = previous;
    applyTheme(previous);
    markTheme(el.themeGrid, previous);
    setIsland("error", errText(error));
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
  if (S.busy || S.running || S.picking) return;
  S.picking = true;
  let picked = null;
  try {
    picked = await invoke("pick_folder");
  } catch (error) {
    showError(error);
  } finally {
    S.picking = false;
  }
  if (picked) await changeDataDir(picked);
}

/* ═══════════════════════════ ПЕРШИЙ ЗАПУСК ═══════════════════════════ */

/** Майстер першого запуску: крок 1 — нік і палітра, крок 2 — проєкт (можна пропустити). */
const OB = { step: 1, theme: "emerald", busy: false, nickTouched: false };

/** Списки проєктів: у майстрі та у вкладці «Проєкт», коли проєкт не підключено. */
function projectPickers() {
  return [el.obProjects, el.projPicker].filter(Boolean);
}

function markPicked() {
  for (const container of projectPickers()) {
    for (const button of container.querySelectorAll(".pl-item")) {
      const on = button.dataset.code === S.pickedProject;
      button.classList.toggle("on", on);
      button.setAttribute("aria-checked", on);
    }
  }
}

function renderProjectList() {
  const list = S.projectList;
  for (const container of projectPickers()) {
    if (!list) {
      container.innerHTML = `<div class="pl-state">${esc(t("ob.loadingProjects"))}</div>`;
      continue;
    }
    if (!list.length) {
      container.innerHTML = `<div class="pl-state">${esc(S.projectListError || t("ob.noProjects"))}</div>`;
      continue;
    }
    container.innerHTML = list
      .map((project, index) => `<button type="button" class="pl-item" role="radio" aria-checked="false" data-code="${esc(project.code)}"><span class="avatar" data-pl="${index}"></span><span class="pl-text"><b>${esc(project.title)}</b><span>${esc(project.description)}</span></span></button>`)
      .join("");
    list.forEach((project, index) => paintAvatar(container.querySelector(`[data-pl="${index}"]`), initial(project.title), [project.icon]));
  }
  markPicked();
}

let projectListSeq = 0;

async function loadProjectList() {
  const my = ++projectListSeq;
  S.projectList = null;
  S.projectListError = "";
  S.pickedProject = "";
  renderProjectList();
  let list = [];
  let error = "";
  try {
    list = await invoke("list_projects");
  } catch (failure) {
    error = errHuman(failure);
  }
  if (my !== projectListSeq) return;
  S.projectList = list;
  S.projectListError = error;
  if (list.length === 1) S.pickedProject = list[0].code;
  renderProjectList();
}

function onProjectPick(event, input) {
  const button = event.target.closest(".pl-item");
  if (!button) return;
  S.pickedProject = button.dataset.code;
  input.value = "";
  input.classList.remove("invalid");
  markPicked();
}

/** Кроки майстра: 1 — @username, нік і палітра; 2 — проєкт; 3 — вікно оновлення до v0.4.1
 *  (лише @username для гравців, які вже користувалися лаунчером). */
function setObStep(step, animate = true) {
  const back = step < OB.step && OB.step !== 3;
  OB.step = step;
  el.obStep1.hidden = step !== 1;
  el.obForm.hidden = step !== 2;
  el.obMigrate.hidden = step !== 3;
  el.obSteps.hidden = step === 3;
  el.obSteps.querySelectorAll("li").forEach((li, index) => {
    li.classList.toggle("on", index + 1 === step);
    li.classList.toggle("done", index + 1 < step);
  });
  const shown = step === 1 ? el.obStep1 : step === 2 ? el.obForm : el.obMigrate;
  if (animate) {
    motion(
      shown,
      [
        { opacity: 0, transform: `translate3d(${back ? -18 : 18}px, 0, 0)` },
        { opacity: 1, transform: "none" }
      ],
      360
    );
  }
  if (step === 2) {
    el.obError.hidden = true;
    if (!S.projectList) loadProjectList();
    else renderProjectList();
  } else if (step === 3) requestAnimationFrame(() => el.migUser.select());
  else requestAnimationFrame(() => (el.obUser.value ? el.obNick : el.obUser).focus());
}

function prepareOnboardingScreen() {
  closeMenu();
  closeAccountPop();
  clearTimeout(islandTimer);
  clearInterval(serverTimer);
  el.app.hidden = true;
  el.onboarding.hidden = false;
  stopSocial();
  OB.busy = false;
  applyI18n();
  el.boot.classList.add("gone");
}

function showOnboarding() {
  prepareOnboardingScreen();
  OB.theme = THEMES[S.settings?.theme] ? S.settings.theme : "emerald";
  OB.nickTouched = false;
  applyTheme(OB.theme);
  renderThemeGrid(el.obThemes, OB.theme);
  const profile = activeProfile();
  if (profile && !el.obNick.value) {
    el.obNick.value = profile.nickname;
    OB.nickTouched = true;
  }
  el.obError1.hidden = true;
  el.obError.hidden = true;
  el.obCode.value = "";
  userState(el.obUser, el.obUserHint, "ob.userHint");
  nickState(el.obNick, el.obNickHint, "ob.nickHint");
  setObStep(1, false);
}

/** Оновлення з v0.4.0 (або офлайн-ім'я виявилося зайнятим): фокусоване вікно вибору @username.
 *  Ніки, скіни, палітра, збірки й проєкт уже на місці — після підтвердження одразу звичний інтерфейс. */
function showMigration() {
  prepareOnboardingScreen();
  applyTheme(S.settings?.theme);
  const conflict = Boolean(S.account?.conflict);
  el.migUser.value = conflict ? "" : S.account?.suggested || activeProfile()?.nickname || "";
  el.migError.hidden = true;
  paintMigration();
  setObStep(3, false);
}

/** Тексти вікна оновлення: звичайне оновлення до v0.4.1 або «ім'я вже зайняте». */
function paintMigration() {
  const conflict = Boolean(S.account?.conflict);
  el.migBadge.textContent = conflict ? t("mig.conflictBadge") : t("mig.badge");
  el.migTitle.textContent = conflict ? t("mig.conflictTitle", { user: S.account?.username || "" }) : t("mig.title");
  el.migSub.textContent = conflict ? t("mig.conflictSub") : t("mig.sub");
  userState(el.migUser, el.migHint, "mig.hint");
}

function setObBusy(on, button) {
  OB.busy = on;
  if (button) setLoading(button, on);
  for (const node of [el.obBack, el.obSkip, el.obNext, el.obSubmit, el.migSubmit]) if (node !== button) node.disabled = on;
}

/** Підсвічування поля @username (3–20 символів). */
function userState(input, hint, defaultKey) {
  const value = input.value.trim().replace(/^@/, "");
  const invalid = value.length > 0 && !USER_RE.test(value);
  input.classList.toggle("invalid", invalid);
  hint.classList.toggle("error", invalid);
  hint.textContent = invalid ? t("ob.userBad") : t(defaultKey);
  return !invalid && value.length > 0;
}

function rejectField(input, hint, key) {
  input.classList.add("invalid");
  hint.classList.add("error");
  hint.textContent = t(key);
  input.focus();
}

/** Результат реєстрації @username (ClaimOutcome): акаунт, ніки, палітра. */
function applyClaim(outcome) {
  S.account = outcome.account;
  applyProfiles(outcome.profiles);
  if (S.settings && outcome.theme) S.settings.theme = outcome.theme;
  const user = outcome.account.username;
  if (outcome.restored) toast({ icon: "users", title: t("ob.restored", { user }), timeout: 6000 });
  else if (outcome.offline) toast({ icon: "alert", tone: "warn", title: t("ob.offlineCreated", { user }), timeout: 9000 });
}

/** Крок 1 → 2: @username (перевірка унікальності — `register_account` у Supabase; без інтернету —
 *  офлайн-створення з відкладеною синхронізацією), перший ігровий нік і палітра. */
async function onObStep1(event) {
  event.preventDefault();
  if (OB.busy) return;
  el.obError1.hidden = true;
  const username = el.obUser.value.trim().replace(/^@/, "");
  const nickname = el.obNick.value.trim() || username.slice(0, 16);
  if (!USER_RE.test(username)) return rejectField(el.obUser, el.obUserHint, "ob.userBad");
  if (!NICK_RE.test(nickname)) return rejectField(el.obNick, el.obNickHint, "profiles.nickBad");
  setObBusy(true, el.obNext);
  try {
    applyClaim(await invoke("onboarding_account", { username, nickname, theme: OB.theme }));
    el.obNick.value = nickname;
    setObStep(2);
  } catch (error) {
    el.obError1.textContent = errHuman(error);
    el.obError1.hidden = false;
  } finally {
    setObBusy(false, el.obNext);
  }
}

/** Вікно оновлення: зареєструвати @username і повернутися до звичного інтерфейсу. */
async function onMigrate(event) {
  event.preventDefault();
  if (OB.busy) return;
  el.migError.hidden = true;
  const username = el.migUser.value.trim().replace(/^@/, "");
  if (!USER_RE.test(username)) return rejectField(el.migUser, el.migHint, "ob.userBad");
  setObBusy(true, el.migSubmit);
  try {
    applyClaim(await invoke("claim_username", { username }));
    showApp();
    setIsland("ok", t("mig.done", { user: S.account.username }));
  } catch (error) {
    el.migError.textContent = errHuman(error);
    el.migError.hidden = false;
  } finally {
    setObBusy(false, el.migSubmit);
  }
}

/** Крок 2: підключити обраний проєкт або введений код / owner/repo. */
async function onOnboardingSubmit(event) {
  event.preventDefault();
  if (OB.busy) return;
  el.obError.hidden = true;
  const code = el.obCode.value.trim() || S.pickedProject;
  if (!code) {
    el.obCode.classList.add("invalid");
    el.obError.textContent = t("ob.needCode");
    el.obError.hidden = false;
    el.obCode.focus();
    return;
  }
  setObBusy(true, el.obSubmit);
  el.obSubmit.querySelector(".btn-text").textContent = t("ob.checking");
  try {
    S.binding = await invoke("connect_project", { code });
    await finishOnboarding();
  } catch (error) {
    el.obError.textContent = errHuman(error);
    el.obError.hidden = false;
  } finally {
    setObBusy(false, el.obSubmit);
    el.obSubmit.querySelector(".btn-text").textContent = t("ob.submit");
  }
}

/** «Пропустити»: увійти без проєкту — власні збірки можна створювати й запускати одразу. */
async function onObSkip() {
  if (OB.busy) return;
  setObBusy(true, el.obSkip);
  try {
    await finishOnboarding();
  } catch (error) {
    el.obError.textContent = errHuman(error);
    el.obError.hidden = false;
  } finally {
    setObBusy(false, el.obSkip);
  }
}

async function finishOnboarding() {
  await invoke("finish_onboarding");
  showApp();
}

function onObTheme(event) {
  const button = event.target.closest("[data-theme]");
  if (!button || button.dataset.theme === OB.theme) return;
  OB.theme = button.dataset.theme;
  applyTheme(OB.theme);
  markTheme(el.obThemes, OB.theme);
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
  // Друзі та бейдж заявок: перший знімок за мить, далі — фонове оновлення.
  startSocial();
}

/* ═══════════════════════════ АКАУНТ ═══════════════════════════ */

/** Стан хмари для акаунта: online | offline | pending (не синхронізовано) | conflict. */
function cloudState() {
  if (!S.account?.username) return "pending";
  if (S.account.conflict) return "conflict";
  if (!S.account.synced) return "pending";
  return S.cloudOnline === false ? "offline" : "online";
}

/** Крапка + підпис стану хмари (картка акаунта, поповер). */
function paintCloud(node) {
  if (!node) return;
  const state = cloudState();
  const dot = node.querySelector(".cloud-dot");
  const text = node.querySelector("span");
  if (dot) dot.dataset.state = state;
  if (text) {
    text.textContent =
      state === "online" ? t("account.synced") : state === "offline" ? t("cloud.offline") : state === "conflict" ? t("account.conflict") : t("account.pending");
  }
}

function renderAccountCard() {
  el.acUser.textContent = "@" + (S.account?.username || "—");
  paintCloud(el.acState);
  el.logoutBtn.disabled = Boolean(S.running);
}

function renderPresenceDot() {
  const state = S.running ? "playing" : cloudState() === "online" ? "online" : "off";
  el.topPresence.dataset.state = state;
}

function renderCloudChip() {
  const state = cloudState();
  el.cloudChip.querySelector(".cloud-dot").dataset.state = state;
  el.cloudChip.querySelector("span").textContent = state === "online" ? t("cloud.online") : state === "offline" ? t("cloud.offline") : t("cloud.pending");
}

/* поповер міні-картки профілю */

let popToken = 0;

function renderAccountPop() {
  el.apUser.textContent = "@" + (S.account?.username || "—");
  paintCloud(el.apState);
  const profile = activeProfile();
  loadSkin(profile).then((img) => drawHead(el.apHead, img, profile?.nickname, 40));
  el.apNicks.innerHTML =
    S.profiles
      .map((p) => {
        const on = p.id === S.activeProfile;
        return `<button type="button" role="menuitemradio" aria-checked="${on}" class="ap-nick${on ? " on" : ""}" data-nick="${esc(p.id)}"><canvas class="head" data-ap-head="${esc(p.id)}" width="24" height="24" aria-hidden="true"></canvas><span>${esc(p.nickname)}</span>${on ? `<i>${ICONS.check}</i>` : ""}</button>`;
      })
      .join("") + `<button type="button" class="ap-nick ap-add" data-act="add-nick">${ICONS.plus}<span>${esc(t("ap.addNick"))}</span></button>`;
  for (const canvas of el.apNicks.querySelectorAll("canvas[data-ap-head]")) {
    const p = S.profiles.find((x) => x.id === canvas.dataset.apHead);
    loadSkin(p).then((img) => drawHead(canvas, img, p?.nickname, 24));
  }
  el.accountPop.querySelector('[data-act="logout"]').disabled = Boolean(S.running);
}

function openAccountPop() {
  if (!el.accountPop.hidden && !el.accountPop.classList.contains("closing")) {
    closeAccountPop();
    return;
  }
  popToken++;
  closeMenu();
  hideTip();
  Dropdown.closeAll();
  renderAccountPop();
  el.accountPop.classList.remove("closing");
  el.accountPop.hidden = false;
  el.accountPop.style.animation = "none";
  void el.accountPop.offsetWidth;
  el.accountPop.style.animation = "";
  const rect = el.topProfile.getBoundingClientRect();
  el.accountPop.style.top = rect.bottom + 6 + "px";
  el.accountPop.style.left = Math.max(8, rect.right - el.accountPop.offsetWidth) + "px";
  el.topProfile.setAttribute("aria-expanded", "true");
  el.accountPop.querySelector(".ap-nick.on, button:not(:disabled)")?.focus();
}

function closeAccountPop() {
  if (el.accountPop.hidden || el.accountPop.classList.contains("closing")) return;
  const my = ++popToken;
  el.topProfile.setAttribute("aria-expanded", "false");
  leave(el.accountPop, "closing", 140, () => {
    if (my !== popToken) return;
    el.accountPop.hidden = true;
    el.accountPop.classList.remove("closing");
  });
}

async function onAccountPopClick(event) {
  const nick = event.target.closest("[data-nick]");
  if (nick) {
    closeAccountPop();
    if (nick.dataset.nick === S.activeProfile) return;
    try {
      applyProfiles(await invoke("set_active_profile", { profileId: nick.dataset.nick }));
    } catch (error) {
      setIsland("error", errHuman(error));
    }
    return;
  }
  const button = event.target.closest("[data-act]");
  if (!button || button.disabled) return;
  closeAccountPop();
  const act = button.dataset.act;
  if (act === "account") switchTab("profiles");
  else if (act === "add-nick") {
    switchTab("profiles");
    requestAnimationFrame(() => el.newNick.focus());
  } else if (act === "friends") switchTab("friends");
  else if (act === "logout") onLogout();
}

/** «Вийти з акаунта»: статус offline на сервері, очищення акаунта й повернення до онбордингу. */
let loggingOut = false;

async function onLogout() {
  if (loggingOut) return;
  if (S.running) {
    setIsland("warn", t("account.closeGame"));
    return;
  }
  if (S.busy || S.updating) {
    setIsland("warn", t("play.wait"));
    return;
  }
  const user = S.account?.username || "";
  const ok = await confirmDialog({
    icon: "logout",
    tone: "warn",
    title: t("account.logoutTitle", { user }),
    html: esc(t("account.logoutText")),
    ok: t("account.logout"),
    danger: true
  });
  if (!ok || loggingOut) return;
  loggingOut = true;
  try {
    await invoke("logout_account");
  } catch (error) {
    showError(error);
    return;
  } finally {
    loggingOut = false;
  }
  S.account = { account_id: "", username: "", synced: false, conflict: false, needs_username: false, suggested: "" };
  S.profiles = [];
  S.activeProfile = "";
  S.skins.clear();
  S.social = null;
  S.cloudOnline = null;
  renderFriendsBadge();
  el.obUser.value = "";
  el.obNick.value = "";
  showOnboarding();
  toast({ icon: "logout", title: t("account.loggedOut"), timeout: 4000 });
}

/* ═══════════════════════════ ДРУЗІ ═══════════════════════════ */

let socialTimer = 0;
let socialSeq = 0;
let tickerTimer = 0;
let friendsSig = "";
let searchTimer = 0;
let searchSeq = 0;

/** Друзі оновлюються кожні 15 с у вкладці «Друзі» і раз на хвилину у фоні (бейдж заявок). */
function socialDelay() {
  return !el.app.hidden && S.tab === "friends" && !document.hidden ? 15000 : 60000;
}

function scheduleSocial(delay = socialDelay()) {
  clearTimeout(socialTimer);
  socialTimer = el.app.hidden ? 0 : setTimeout(() => loadSocial({ quiet: true }), delay);
}

function startSocial() {
  scheduleSocial(1500);
}

function stopSocial() {
  clearTimeout(socialTimer);
  socialTimer = 0;
  socialSeq++;
  syncFriendTicker();
}

async function loadSocial({ quiet = false } = {}) {
  if (el.app.hidden || !S.account?.username) return;
  const my = ++socialSeq;
  if (!quiet && S.tab === "friends") renderFriends();
  let snap;
  try {
    snap = await invoke("social_snapshot");
  } catch (error) {
    snap = { online: false, pending: false, error: errHuman(error), data: null };
  }
  if (my !== socialSeq) return;
  if (snap.online && snap.data) {
    S.social = snap.data;
    S.socialAt = performance.now();
    S.cloudOnline = true;
  } else if (!snap.pending) S.cloudOnline = false;
  renderFriendsBadge();
  renderPresenceDot();
  if (S.tab === "friends") renderFriends();
  if (S.tab === "profiles") renderAccountCard();
  scheduleSocial();
}

function renderFriendsBadge() {
  const count = (S.social?.incoming?.length || 0) + (S.social?.pack_requests?.length || 0);
  const shown = !el.friendsBadge.hidden;
  el.friendsBadge.hidden = !count;
  el.friendsBadge.textContent = count > 9 ? "9+" : String(count);
  if (count && !shown) {
    el.friendsBadge.classList.remove("bump");
    void el.friendsBadge.offsetWidth;
    el.friendsBadge.classList.add("bump");
  }
}

/* живий таймер гри */

function syncFriendTicker() {
  const on = S.tab === "friends" && !el.app.hidden && !document.hidden;
  if (on && !tickerTimer) tickerTimer = setInterval(tickTimers, 1000);
  else if (!on && tickerTimer) {
    clearInterval(tickerTimer);
    tickerTimer = 0;
  }
}

/** Поточний час сервера: server_now зі знімка + скільки минуло локально (без розсинхрону годинників). */
function serverNowMs() {
  const base = Date.parse(S.social?.server_now || "");
  return (Number.isNaN(base) ? Date.now() : base) + (performance.now() - S.socialAt);
}

function playTime(sec) {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h) return t("time.hm", { h, m });
  if (m) return t("time.m", { m });
  return t("time.s", { s: sec });
}

function stopwatch(sec) {
  const h = Math.floor(sec / 3600);
  const m = String(Math.floor((sec % 3600) / 60)).padStart(2, "0");
  const s = String(sec % 60).padStart(2, "0");
  return h ? `${h}:${m}:${s}` : `${m}:${s}`;
}

function agoText(iso) {
  const at = Date.parse(iso || "");
  if (Number.isNaN(at)) return "";
  const min = Math.max(0, Math.floor((serverNowMs() - at) / 60000));
  if (min < 1) return t("time.justNow");
  if (min < 60) return t("time.minAgo", { n: min });
  if (min < 1440) return t("time.hourAgo", { n: Math.floor(min / 60) });
  return t("time.dayAgo", { n: Math.floor(min / 1440) });
}

function tickTimers() {
  const now = serverNowMs();
  for (const node of el.friendList.querySelectorAll("[data-since]")) {
    const sec = Math.max(0, Math.floor((now - Number(node.dataset.since)) / 1000));
    node.querySelector("[data-timer-text]").textContent = t("friends.timer", { time: playTime(sec) });
    node.querySelector("[data-timer-clock]").textContent = stopwatch(sec);
  }
}

/* дії зі збірками друзів */

function localShared(packId) {
  return S.view?.instances.find((i) => i.custom && i.shared_from === packId) || null;
}

function sameProject(code) {
  return Boolean(S.binding && code && S.binding.code.toLowerCase() === String(code).toLowerCase());
}

/** Що можна зробити зі спільною збіркою друга (з урахуванням доступу та вже завантаженої копії). */
function sharedAction(packId, access) {
  const local = localShared(packId);
  if (local) return { act: "open-local", local: local.id, label: t("friends.openPack"), icon: "play", primary: true };
  if (access === "public" || access === "approved") return { act: "download", label: t("friends.download"), icon: "download", primary: true };
  if (access === "pending") return { act: "pending", label: t("friends.requestSent"), icon: "clock", disabled: true };
  if (access === "denied") return { act: "request", label: t("friends.denied"), icon: "hand" };
  return { act: "request", label: t("friends.requestAccess"), icon: "hand" };
}

/** Кнопка під статусом друга, який зараз у грі. */
function friendAction(f) {
  if (f.status !== "playing") return null;
  if (f.pack_type === "project") {
    if (!f.project_code) return null;
    if (sameProject(f.project_code)) return { act: "open-project", label: t("friends.openPack"), icon: "play", primary: true };
    return { act: "join", label: t("friends.joinProject"), icon: "link", primary: true };
  }
  if (!f.shared_pack_id || f.pack_type === "custom_private") return null;
  return sharedAction(f.shared_pack_id, f.access);
}

function actionButton(action) {
  if (!action) return "";
  const cls = action.primary ? "btn-primary" : "btn-ghost";
  const local = action.local ? ` data-local="${esc(action.local)}"` : "";
  return `<button class="btn ${cls} btn-sm fr-act" type="button" data-fact="${action.act}"${local}${action.disabled ? " disabled" : ""}><span class="btn-spin" aria-hidden="true"></span><span class="btn-text">${ICONS[action.icon] || ""}${esc(action.label)}</span></button>`;
}

function activityText(f) {
  if (f.pack_type === "project") return t("friends.playingProject", { pack: f.pack_name || "—", project: f.project_name || f.project_code || "—" });
  if (f.pack_type === "custom_private" || !f.pack_name) return t("friends.playingPrivate");
  return t("friends.playingPack", { pack: f.pack_name });
}

function friendRow(f, index, animate) {
  const status = ["playing", "online"].includes(f.status) ? f.status : "offline";
  let line;
  if (status === "playing") {
    line = esc(activityText(f)) + (f.character ? ` <span class="fr-as">${esc(t("friends.playingAs", { nick: f.character }))}</span>` : "");
  } else if (status === "online") line = esc(t("st.online"));
  else line = esc(t("st.offline")) + (f.last_seen ? " · " + esc(t("friends.lastSeen", { ago: agoText(f.last_seen) })) : "");
  const since = status === "playing" ? Date.parse(f.started_at || "") : NaN;
  const timer = Number.isNaN(since)
    ? ""
    : `<span class="fr-timer" data-since="${since}"><i aria-hidden="true">${ICONS.clock}</i><b data-timer-text></b><span class="mono" data-timer-clock></span></span>`;
  const packs = f.packs || [];
  const packsBtn = packs.length
    ? `<button class="icon-sm fr-packs" type="button" data-fact="packs" title="${esc(t("friends.packs", { n: packs.length }))}" aria-label="${esc(t("friends.packs", { n: packs.length }))}">${ICONS.layers}<b>${packs.length}</b></button>`
    : "";
  return `<div class="friend-row st-${status}${animate ? " enter" : ""}" data-friend="${esc(f.user_id)}" style="animation-delay:${Math.min(index, 8) * 30}ms">
    <span class="fr-avatar"><canvas class="head" data-fhead="${esc(f.username)}" width="40" height="40" aria-hidden="true"></canvas><i class="fr-dot" aria-hidden="true"></i></span>
    <span class="fr-body">
      <span class="fr-name"><b>@${esc(f.username)}</b><span class="fr-pill">${esc(t("st." + status))}</span></span>
      <small class="fr-line">${line}</small>
      ${timer}
    </span>
    <span class="row-actions">${actionButton(friendAction(f))}${packsBtn}<button class="icon-sm danger" type="button" data-fact="remove" title="${esc(t("friends.remove"))}" aria-label="${esc(t("friends.remove"))}">${ICONS.trash}</button></span>
  </div>`;
}

function personRow(attrs, username, sub, buttons) {
  return `<div class="fr-req" ${attrs}><canvas class="head" data-fhead="${esc(username)}" width="32" height="32" aria-hidden="true"></canvas><span class="fr-req-text"><b>@${esc(username)}</b><small>${sub}</small></span><span class="row-actions">${buttons}</span></div>`;
}

function smallButton(attr, value, label, kind = "ghost") {
  return `<button class="btn btn-${kind} btn-sm" type="button" ${attr}="${value}"><span class="btn-spin" aria-hidden="true"></span><span class="btn-text">${esc(label)}</span></button>`;
}

function paintHeads(root) {
  for (const canvas of root.querySelectorAll("canvas[data-fhead]")) drawHead(canvas, null, canvas.dataset.fhead, canvas.width);
}

function friendsState(message, retry) {
  return `<div class="fr-empty">${ICONS.users}<p>${esc(message)}</p>${retry ? `<button class="btn btn-ghost btn-sm" type="button" data-fact="retry">${esc(t("friends.retry"))}</button>` : ""}</div>`;
}

function renderFriends() {
  renderCloudChip();
  const data = S.social;
  const synced = Boolean(S.account?.synced);
  el.friendQuery.disabled = !synced;
  el.friendAdd.disabled = !synced;
  if (!data) {
    for (const block of [el.incomingBlock, el.accessBlock, el.outgoingBlock]) block.hidden = true;
    el.friendsCount.textContent = "0";
    friendsSig = "";
    el.friendList.innerHTML = !synced
      ? friendsState(t("friends.pendingState", { user: S.account?.username || "—" }), false)
      : S.cloudOnline === false
        ? friendsState(t("friends.offlineState"), true)
        : `<div class="fr-empty fr-loading"><span class="spinner"></span><p>${esc(t("friends.loading"))}</p></div>`;
    return;
  }
  const incoming = data.incoming || [];
  const access = data.pack_requests || [];
  const outgoing = data.outgoing || [];
  const friends = data.friends || [];

  el.incomingBlock.hidden = !incoming.length;
  el.incomingCount.textContent = String(incoming.length);
  el.incomingList.innerHTML = incoming
    .map((r) => personRow(`data-req="${esc(r.id)}"`, r.username, esc(t("friends.wantsFriend")), smallButton("data-ract", "accept", t("friends.accept"), "primary") + smallButton("data-ract", "decline", t("friends.decline"))))
    .join("");
  el.accessBlock.hidden = !access.length;
  el.accessCount.textContent = String(access.length);
  el.accessList.innerHTML = access
    .map((r) => personRow(`data-preq="${esc(r.id)}" data-pack-name="${esc(r.pack_name)}" data-user="${esc(r.username)}"`, r.username, esc(t("friends.wantsAccess", { pack: r.pack_name })), smallButton("data-pact", "allow", t("friends.allow"), "primary") + smallButton("data-pact", "deny", t("friends.deny"))))
    .join("");
  el.outgoingBlock.hidden = !outgoing.length;
  el.outgoingList.innerHTML = outgoing
    .map((r) => personRow(`data-friend="${esc(r.user_id)}" data-user="${esc(r.username)}"`, r.username, esc(t("friends.waiting")), smallButton("data-fact", "cancel", t("friends.cancel"))))
    .join("");
  for (const list of [el.incomingList, el.accessList, el.outgoingList]) paintHeads(list);

  el.friendsCount.textContent = String(friends.length);
  // Перебудовуємо список лише коли щось змінилося (таймери тікають окремо, без перемальовування).
  const local = (S.view?.instances || []).filter((i) => i.shared_from).map((i) => i.shared_from);
  const sig = JSON.stringify([lang, S.binding?.code || "", local, friends.map((f) => [f.user_id, f.status, f.pack_name, f.pack_type, f.instance_id, f.project_code, f.shared_pack_id, f.started_at, f.access, f.character, (f.packs || []).map((p) => p.id + p.access), f.status === "offline" ? agoText(f.last_seen) : ""])]);
  if (sig !== friendsSig) {
    const animate = !friendsSig;
    friendsSig = sig;
    el.friendList.innerHTML = friends.length ? friends.map((f, i) => friendRow(f, i, animate)).join("") : friendsState(t("friends.empty"), false);
    paintHeads(el.friendList);
  }
  el.friendList.classList.toggle("stale", S.cloudOnline === false);
  tickTimers();
}

function friendById(id) {
  return (S.social?.friends || []).find((f) => f.user_id === id) || null;
}

/** Друг грає у збірку проєкту, який уже підключено: відкрити саме її. */
async function openFriendInstance(id) {
  let found = id && S.view?.instances.some((i) => i.id === id);
  if (id && !found) {
    setIsland("busy", t("friends.packMissing"));
    await loadProject({ quiet: true });
    found = S.view?.instances.some((i) => i.id === id);
  }
  switchTab("home");
  if (found) selectInstance(id);
}

/** «Увійти в проєкт»: підтвердження → connect_project(project_code) → відкрити збірку друга. */
/** Код проєкту від друга — лише безпечні символи (сервер перевіряє те саме). */
const PROJECT_CODE_RE = /^[A-Za-z0-9_./-]{1,120}$/;

async function joinFriendProject(f) {
  if (!PROJECT_CODE_RE.test(f.project_code || "")) {
    setIsland("error", t("friends.badProject"));
    return;
  }
  if (sameProject(f.project_code)) {
    openFriendInstance(f.instance_id);
    return;
  }
  const name = f.project_name || f.project_code;
  const replace = S.binding && !sameProject(f.project_code) ? `<p>${esc(t("friends.joinReplace", { cur: S.binding.name || S.binding.code }))}</p>` : "";
  const ok = await confirmDialog({
    icon: "link",
    title: t("friends.joinTitle", { name }),
    html: `<p>${t("friends.joinText", { user: esc(f.username), pack: esc(f.pack_name || "—"), name: esc(name), code: esc(f.project_code) })}</p>${replace}`,
    ok: t("friends.joinGo")
  });
  if (!ok) return;
  setIsland("busy", t("ob.checking"));
  try {
    S.binding = await invoke("connect_project", { code: f.project_code });
    S.changelog = null;
    S.pickedProject = "";
    await loadProject();
    renderTop();
    setIsland("ok", t("friends.joined", { name: S.binding.name || name }));
    openFriendInstance(f.instance_id);
  } catch (error) {
    setIsland("error", errHuman(error));
    showError(error);
  }
}

/** «Завантажити збірку» друга: доступ перевіряє сервер, встановлення — як готової збірки. */
async function downloadFriendPack(packId, name, owner, button) {
  if (S.busy || S.updating) {
    setIsland("warn", t("play.wait"));
    return;
  }
  // Збірка друга може містити моди не з Modrinth/CurseForge (файли в архіві) — лише від тих, кому довіряєш.
  S.busy = "confirm";
  const ok = await confirmDialog({
    icon: "download",
    title: t("friends.trustTitle", { name: name || "…", user: owner }),
    html: esc(t("friends.trustText", { user: owner })),
    ok: t("friends.download")
  });
  S.busy = null;
  if (!ok || S.busy || S.updating) return;
  S.busy = "modpack";
  S.modpackName = name || "…";
  if (button) setLoading(button, true);
  renderPlay();
  setIsland("busy", t("friends.downloading", { name: S.modpackName }));
  try {
    const result = await invoke("download_friend_pack", { packId });
    addCustomInstance(result.instance);
    setIsland("ok", t("friends.downloaded", { name: result.report.name, user: owner }));
    switchTab("home");
    selectInstance(result.instance.id, { save: false });
  } catch (error) {
    setIsland("error", errHuman(error));
    showError(error);
  } finally {
    S.busy = null;
    S.modpackName = "";
    if (button?.isConnected) setLoading(button, false);
    renderList();
    renderHero();
    friendsSig = "";
    if (S.tab === "friends") renderFriends();
  }
}

async function requestPackAccess(packId, name, button) {
  if (button) setLoading(button, true);
  try {
    await invoke("request_pack_access", { packId });
    setIsland("ok", t("friends.accessAsked", { pack: name || "…" }));
    await loadSocial({ quiet: true });
  } catch (error) {
    setIsland("error", errHuman(error));
    if (button?.isConnected) setLoading(button, false);
  }
}

/** Дія зі спільною збіркою (рядок друга або вікно «Збірки друга»). */
function runPackAction(act, { packId, name, owner, local, button }) {
  if (act === "open-local") {
    switchTab("home");
    selectInstance(local);
  } else if (act === "download") downloadFriendPack(packId, name, owner, button);
  else if (act === "request") requestPackAccess(packId, name, button);
}

function openFriendPacks(f) {
  const rows = (f.packs || [])
    .map((p) => {
      const meta = [p.minecraft_version ? "Minecraft " + p.minecraft_version : "", loaderName(p.loader), t("vis." + p.visibility), p.size_bytes ? formatSize(p.size_bytes) : ""].filter(Boolean).join(" · ");
      return `<div class="fp-row" data-pack="${esc(p.id)}" data-name="${esc(p.name)}"><span class="fp-ico">${ICONS[p.visibility === "public" ? "globe" : "hand"]}</span><span class="fp-text"><b>${esc(p.name)}</b><small>${esc(meta)}</small></span>${actionButton(sharedAction(p.id, p.access))}</div>`;
    })
    .join("");
  openModal({
    icon: "layers",
    title: t("friends.packsTitle", { user: f.username }),
    html: `<p>${esc(t("friends.packsLead"))}</p><div class="fp-list" id="fpList">${rows}</div>`,
    wide: true,
    buttons: [{ id: "close", label: t("common.close"), kind: "primary" }]
  });
  $("fpList")?.addEventListener("click", (event) => {
    const button = event.target.closest("[data-fact]");
    const row = event.target.closest("[data-pack]");
    if (!button || !row || button.disabled) return;
    const act = button.dataset.fact;
    const options = { packId: row.dataset.pack, name: row.dataset.name, owner: f.username, local: button.dataset.local, button };
    if (act === "request") {
      runPackAction(act, options);
      button.disabled = true;
      button.querySelector(".btn-text").textContent = t("friends.requestSent");
      return;
    }
    closeModal("close");
    setTimeout(() => runPackAction(act, { ...options, button: null }), 200);
  });
}

async function onFriendsClick(event) {
  const target = event.target;
  if (target.closest('[data-fact="retry"]')) {
    S.cloudOnline = null;
    renderFriends();
    loadSocial();
    return;
  }
  // Заявки в друзі.
  const req = target.closest("[data-req]");
  const ract = target.closest("[data-ract]");
  if (req && ract && !ract.disabled) {
    for (const node of req.querySelectorAll("button")) node.disabled = true;
    setLoading(ract, true);
    const accept = ract.dataset.ract === "accept";
    try {
      await invoke("respond_friend_request", { requestId: req.dataset.req, accept });
      const user = req.querySelector("b")?.textContent.replace(/^@/, "") || "";
      setIsland("ok", accept ? t("friends.accepted", { user }) : t("friends.declined"));
    } catch (error) {
      setIsland("error", errHuman(error));
    }
    loadSocial({ quiet: true });
    return;
  }
  // Запити на доступ до моїх збірок.
  const preq = target.closest("[data-preq]");
  const pact = target.closest("[data-pact]");
  if (preq && pact && !pact.disabled) {
    for (const node of preq.querySelectorAll("button")) node.disabled = true;
    setLoading(pact, true);
    const approve = pact.dataset.pact === "allow";
    try {
      await invoke("respond_pack_request", { requestId: preq.dataset.preq, approve });
      setIsland("ok", approve ? t("friends.accessGranted", { user: preq.dataset.user, pack: preq.dataset.packName }) : t("friends.accessDenied", { user: preq.dataset.user }));
    } catch (error) {
      setIsland("error", errHuman(error));
    }
    loadSocial({ quiet: true });
    return;
  }
  const button = target.closest("[data-fact]");
  const row = target.closest("[data-friend]");
  if (!button || !row || button.disabled) return;
  const act = button.dataset.fact;
  if (act === "cancel") {
    setLoading(button, true);
    try {
      await invoke("remove_friend", { friendId: row.dataset.friend });
    } catch (error) {
      setIsland("error", errHuman(error));
    }
    loadSocial({ quiet: true });
    return;
  }
  const f = friendById(row.dataset.friend);
  if (!f) return;
  if (act === "remove") {
    const ok = await confirmDialog({ icon: "trash", tone: "danger", title: t("friends.removeTitle", { user: f.username }), html: esc(t("friends.removeText")), ok: t("friends.remove"), danger: true });
    if (!ok) return;
    try {
      await invoke("remove_friend", { friendId: f.user_id });
      setIsland("ok", t("friends.removed", { user: f.username }));
    } catch (error) {
      setIsland("error", errHuman(error));
    }
    loadSocial({ quiet: true });
  } else if (act === "packs") openFriendPacks(f);
  else if (act === "join") joinFriendProject(f);
  else if (act === "open-project") openFriendInstance(f.instance_id);
  else runPackAction(act, { packId: f.shared_pack_id, name: f.pack_name, owner: f.username, local: button.dataset.local, button });
}

/* пошук і додавання друзів */

function hideFriendSearch() {
  searchSeq++;
  el.friendSearch.hidden = true;
  el.friendSearch.innerHTML = "";
}

function cleanQuery(value) {
  return String(value || "").trim().replace(/^@/, "");
}

function onFriendQueryInput() {
  clearTimeout(searchTimer);
  el.friendQuery.classList.remove("invalid");
  el.friendHint.classList.remove("error");
  el.friendHint.textContent = t("friends.addHint");
  const query = cleanQuery(el.friendQuery.value);
  if (query.length < 2 || !/^[A-Za-z0-9_]+$/.test(query)) {
    hideFriendSearch();
    return;
  }
  searchTimer = setTimeout(() => runFriendSearch(query), 280);
}

async function runFriendSearch(query) {
  const my = ++searchSeq;
  let list;
  try {
    list = await invoke("search_users", { query });
  } catch {
    list = null;
  }
  if (my !== searchSeq || cleanQuery(el.friendQuery.value) !== query) return;
  if (!Array.isArray(list)) {
    hideFriendSearch();
    return;
  }
  el.friendSearch.innerHTML = list.length
    ? list
        .map((u) => {
          const addable = u.relation === "none" || u.relation === "incoming";
          const tag = u.relation === "none" ? `<i class="fs-add">${ICONS.userPlus}</i>` : `<span class="tag">${esc(t("rel." + u.relation))}</span>`;
          return `<button type="button" class="fs-item" data-user="${esc(u.username)}"${addable ? "" : " disabled"}><canvas class="head" data-fhead="${esc(u.username)}" width="28" height="28" aria-hidden="true"></canvas><b>@${esc(u.username)}</b>${tag}</button>`;
        })
        .join("")
    : `<div class="fs-empty">${esc(t("friends.noResults"))}</div>`;
  paintHeads(el.friendSearch);
  el.friendSearch.hidden = false;
}

let sendingFriend = false;

async function sendFriend(username) {
  // Enter у полі та клік по кнопці / підказці — лише один запит за раз.
  if (sendingFriend) return;
  if (!USER_RE.test(username)) {
    el.friendQuery.classList.add("invalid");
    el.friendHint.classList.add("error");
    el.friendHint.textContent = t("ob.userBad");
    el.friendQuery.focus();
    return;
  }
  hideFriendSearch();
  sendingFriend = true;
  setLoading(el.friendAdd, true);
  try {
    const result = await invoke("send_friend_request", { username });
    const key = { sent: "friends.sent", accepted: "friends.accepted", already_friends: "friends.already", already_sent: "friends.alreadySent" }[result?.status] || "friends.sent";
    setIsland("ok", t(key, { user: username }));
    el.friendQuery.value = "";
    await loadSocial({ quiet: true });
  } catch (error) {
    el.friendQuery.classList.add("invalid");
    el.friendHint.classList.add("error");
    el.friendHint.textContent = errHuman(error);
  } finally {
    sendingFriend = false;
    setLoading(el.friendAdd, false);
    el.friendAdd.disabled = !S.account?.synced;
  }
}

/* ═══════════════════════════ ВИДИМІСТЬ ВЛАСНИХ ЗБІРОК ═══════════════════════════ */

const VISIBILITIES = ["private", "public", "on_request"];
const VIS_ICON = { private: "lock", public: "globe", on_request: "hand" };

/** Компактний перемикач «Приватна / Публічна для друзів / За проханням» + підказка. */
/** Сегменти видимості: акцентний «повзунок» їде під обрану кнопку (transform), а три підказки
 *  лежать в одній клітинці сітки — висота блоку дорівнює найдовшій, тож перемикання нічого не штовхає. */
function visibilitySeg(current) {
  const index = Math.max(0, VISIBILITIES.indexOf(current));
  const buttons = VISIBILITIES.map((v) => {
    const on = v === current;
    return `<button type="button" role="radio" aria-checked="${on}" class="${on ? "on" : ""}" data-vis="${v}">${ICONS[VIS_ICON[v]]}<span>${esc(t("vis." + v))}</span></button>`;
  }).join("");
  const hints = VISIBILITIES.map((v) => `<p class="hint vis-hint${v === current ? " on" : ""}" data-hint="${v}"${v === current ? "" : ' aria-hidden="true"'}>${esc(t("vis." + v + "Hint"))}</p>`).join("");
  return `<div class="vis-seg" role="radiogroup" style="--i:${index}"><span class="vis-thumb" aria-hidden="true"></span>${buttons}</div><div class="vis-hints">${hints}</div>`;
}

function bindVisibilitySeg(root, onChange) {
  root?.addEventListener("click", (event) => {
    const button = event.target.closest("[data-vis]");
    if (!button) return;
    for (const node of root.querySelectorAll("[data-vis]")) {
      const on = node === button;
      node.classList.toggle("on", on);
      node.setAttribute("aria-checked", String(on));
    }
    const seg = root.querySelector(".vis-seg");
    if (seg) seg.style.setProperty("--i", String(Math.max(0, VISIBILITIES.indexOf(button.dataset.vis))));
    for (const hint of root.querySelectorAll(".vis-hint")) {
      const on = hint.dataset.hint === button.dataset.vis;
      hint.classList.toggle("on", on);
      if (on) hint.removeAttribute("aria-hidden");
      else hint.setAttribute("aria-hidden", "true");
    }
    onChange(button.dataset.vis);
  });
}

/** Зберігає видимість; для спільних збірок — пакування й публікація копії для друзів. */
async function applyVisibility(id, visibility) {
  const name = S.view?.instances.find((i) => i.id === id)?.name || "";
  if (visibility !== "private") setIsland("busy", t("vis.publishing", { name }));
  try {
    const outcome = await invoke("set_pack_visibility", { instanceId: id, visibility });
    mergeInstance(outcome.instance);
    if (outcome.error) {
      setIsland("error", t("vis.failed"));
      showError(outcome.error);
    } else if (outcome.pending) setIsland("warn", t("vis.pending"));
    else if (outcome.published) {
      const report = outcome.report || { cdn: 0, bundled: 0, skipped: [] };
      setIsland("ok", t("vis.published", { name, cdn: report.cdn, bundled: report.bundled }));
      if (report.skipped?.length) {
        const files = report.skipped.slice(0, 6).join(", ") + (report.skipped.length > 6 ? "…" : "");
        toast({ icon: "alert", tone: "warn", title: t("vis.skipped", { files }), timeout: 12000 });
      }
    } else setIsland("ok", t("vis.privateDone", { name }));
  } catch (error) {
    setIsland("error", errHuman(error));
  }
}

/* ── іконка власної збірки ── */

const PACK_ICON_SIZE = 128;

/** Обрізає картинку до квадрата по центру й зменшує до 128×128 PNG (піксель-арт — без розмиття). */
async function makePackIcon(buffer) {
  let bitmap;
  try {
    bitmap = await createImageBitmap(new Blob([buffer]));
  } catch {
    throw new Error(t("icon.bad"));
  }
  const side = Math.min(bitmap.width, bitmap.height);
  if (!side) throw new Error(t("icon.bad"));
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = PACK_ICON_SIZE;
  const ctx = canvas.getContext("2d");
  ctx.imageSmoothingEnabled = side > 64;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(bitmap, (bitmap.width - side) / 2, (bitmap.height - side) / 2, side, side, 0, 0, PACK_ICON_SIZE, PACK_ICON_SIZE);
  bitmap.close?.();
  const png = await new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!png) throw new Error(t("icon.bad"));
  return { bytes: new Uint8Array(await png.arrayBuffer()), url: canvas.toDataURL("image/png") };
}

/** Рядок вибору іконки: прев'ю 42×42 (з «✕» на куті, коли картинку обрано), підпис і «Обрати файл…». */
function packIconPicker(id, url, letter, label) {
  const has = isIconUrl(url);
  return `<div class="pi${has ? " has-img" : ""}" id="${id}">
    <span class="pi-thumb">
      <span class="pi-preview" aria-hidden="true"><span class="pi-letter">${esc(letter)}</span><img alt="" draggable="false"${has ? ` src="${esc(url)}"` : ""}></span>
      <button type="button" class="pi-clear" data-pi="clear" aria-label="${esc(t("icon.clear"))}" title="${esc(t("icon.clear"))}"${has ? "" : ' tabindex="-1" aria-hidden="true"'}>${ICONS.close}</button>
    </span>
    <span class="pi-main">
      <span class="label">${esc(label)}</span>
      <span class="pi-acts">
        <button type="button" class="btn btn-ghost btn-sm pi-pick" data-pi="pick" title="${esc(t("icon.hintFull"))}">${esc(t("icon.pick"))}</button>
        <span class="hint pi-hint" title="${esc(t("icon.hintFull"))}">${esc(t("icon.hint"))}</span>
      </span>
    </span>
  </div>`;
}

function setPackIconLetter(root, letter) {
  const node = root?.querySelector(".pi-letter");
  if (node && node.textContent !== letter) node.textContent = letter;
}

function bindPackIconPicker(root, { onChange, onError }) {
  if (!root) return;
  let picking = false;
  const clear = root.querySelector('[data-pi="clear"]');
  const show = (icon) => {
    const img = root.querySelector(".pi-preview img");
    if (icon) img.src = icon.url;
    else img.removeAttribute("src");
    root.classList.toggle("has-img", Boolean(icon));
    if (icon) {
      clear.removeAttribute("tabindex");
      clear.removeAttribute("aria-hidden");
    } else {
      clear.setAttribute("tabindex", "-1");
      clear.setAttribute("aria-hidden", "true");
    }
  };
  root.addEventListener("click", async (event) => {
    const button = event.target.closest("[data-pi]");
    if (!button) return;
    if (button.dataset.pi === "clear") {
      if (!root.classList.contains("has-img")) return;
      show(null);
      onChange(null);
      root.querySelector('[data-pi="pick"]')?.focus();
      return;
    }
    if (picking) return;
    picking = true;
    button.disabled = true;
    try {
      const raw = await invoke("pick_pack_icon");
      const buffer = raw instanceof ArrayBuffer ? raw : ArrayBuffer.isView(raw) ? raw.buffer : Array.isArray(raw) ? new Uint8Array(raw).buffer : null;
      if (!buffer || !buffer.byteLength) return;
      const icon = await makePackIcon(buffer);
      show(icon);
      onChange(icon);
    } catch (error) {
      onError?.(error instanceof Error ? error.message : errHuman(error));
    } finally {
      picking = false;
      button.disabled = false;
    }
  });
}

/** Меню збірки → «Іконка збірки…»: змінити або прибрати іконку вже створеної власної збірки. */
function openIconDialog(id) {
  const inst = S.view?.instances.find((i) => i.id === id);
  if (!inst?.custom) return;
  let picked = undefined; // undefined — без змін, null — прибрати, {bytes,url} — нова
  const showErr = (text) => {
    const node = $("iconError");
    if (!node) return;
    node.textContent = text || "";
    node.hidden = !text;
  };
  const choice = openModal({
    icon: "image",
    title: t("icon.title", { name: inst.name }),
    html: `<p>${esc(t("icon.lead"))}</p><div class="pi-box">${packIconPicker("iconBox", inst.icon, initial(inst.name), t("icon.label"))}</div><p class="form-error" id="iconError" hidden></p>`,
    kind: "icon",
    buttons: [
      { id: "cancel", label: t("common.cancel"), kind: "ghost" },
      {
        id: "ok",
        label: t("icon.save"),
        kind: "primary",
        onClick: async (button) => {
          if (picked === undefined) return;
          button.disabled = true;
          try {
            mergeInstance(await invoke("set_custom_icon", { instanceId: id, bytes: picked ? Array.from(picked.bytes) : null }));
          } catch (error) {
            showErr(errHuman(error));
            button.disabled = false;
            return false;
          }
        }
      }
    ]
  });
  bindPackIconPicker($("iconBox"), {
    onChange: (icon) => {
      picked = icon;
      showErr("");
    },
    onError: showErr
  });
  choice.then((result) => {
    if (result === "ok" && picked !== undefined) setIsland("ok", t("icon.saved", { name: inst.name }));
  });
}

/** Меню збірки → «Видимість для друзів…». */
function openVisibilityDialog(id) {
  const inst = S.view?.instances.find((i) => i.id === id);
  if (!inst?.custom) return;
  let picked = VISIBILITIES.includes(inst.visibility) ? inst.visibility : "private";
  const saveLabel = () => (picked !== "private" && picked === inst.visibility && inst.published ? t("vis.update") : t("vis.save"));
  const choice = openModal({
    icon: "users",
    title: t("vis.title", { name: inst.name }),
    html: `<p>${esc(t("vis.lead"))}</p><div class="vis-box" id="visBox">${visibilitySeg(picked)}</div>`,
    wide: true,
    kind: "visibility",
    buttons: [
      { id: "cancel", label: t("common.cancel"), kind: "ghost" },
      { id: "ok", label: saveLabel(), kind: "primary" }
    ]
  });
  const okButton = el.modalActions.lastElementChild;
  bindVisibilitySeg($("visBox"), (value) => {
    picked = value;
    okButton.textContent = saveLabel();
  });
  choice.then((result) => {
    if (result !== "ok") return;
    if (picked === inst.visibility && picked === "private") return;
    applyVisibility(id, picked);
  });
}

/* ═══════════════════════════ DANGER ZONE ═══════════════════════════ */

/** Підтвердження незворотної дії: кнопка активується лише після введення слова-підтвердження. */
async function strictConfirm({ title, html, ok }) {
  const word = t("danger.word");
  const matches = () => ($("dzWord")?.value || "").trim().toLocaleUpperCase() === word.toLocaleUpperCase();
  const choice = openModal({
    icon: "alert",
    tone: "danger",
    title,
    kind: "danger",
    html: `${html}<label class="dz-confirm"><span>${t("danger.typeHint", { word: esc(word) })}</span><input type="text" id="dzWord" spellcheck="false" autocomplete="off" maxlength="32"></label>`,
    buttons: [
      { id: "cancel", label: t("common.cancel"), kind: "ghost" },
      {
        id: "ok",
        label: ok,
        kind: "danger",
        onClick: () => {
          if (matches()) return;
          $("dzWord")?.classList.add("invalid");
          $("dzWord")?.focus();
          return false;
        }
      }
    ]
  });
  const input = $("dzWord");
  const confirmButton = el.modalActions.querySelector(".btn-danger-solid");
  if (confirmButton) confirmButton.disabled = true;
  input?.addEventListener("input", () => {
    input.classList.remove("invalid");
    if (confirmButton) confirmButton.disabled = !matches();
  });
  input?.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && matches()) {
      event.preventDefault();
      confirmButton?.click();
    }
  });
  input?.focus();
  return (await choice) === "ok";
}

function dangerBlocked() {
  if (S.running) {
    setIsland("warn", t("delete.running"));
    return true;
  }
  if (S.busy || S.updating) {
    setIsland("warn", t("play.wait"));
    return true;
  }
  return false;
}

async function onDangerReset() {
  if (dangerBlocked()) return;
  const ok = await strictConfirm({ title: t("danger.resetConfirmTitle"), html: t("danger.resetConfirmText"), ok: t("danger.resetGo").replace("…", "") });
  if (!ok || dangerBlocked()) return;
  S.busy = "danger";
  renderPlay();
  renderDanger();
  setIsland("busy", t("danger.resetting"));
  try {
    await invoke("reset_launcher_data");
    setIsland("ok", t("danger.resetDone"));
  } catch (error) {
    setIsland("error", errHuman(error));
    showError(error);
  } finally {
    // Стан беремо з бекенда заново: налаштування скинуто, збірок на диску більше немає.
    try {
      const info = await invoke("get_init_info");
      S.settings = info.settings;
      el.ram.value = String(info.settings.ram_mb);
    } catch {
      /* лишаємо попередні значення */
    }
    S.busy = null;
    S.disk = [];
    hideProgress();
    await loadProject({ quiet: true });
    renderPlay();
    if (S.tab === "settings") refreshSettings();
  }
}

async function onDangerUninstall() {
  if (dangerBlocked()) return;
  const ok = await strictConfirm({ title: t("danger.uninstallConfirmTitle"), html: t("danger.uninstallConfirmText"), ok: t("danger.uninstallGo").replace("…", "") });
  if (!ok || dangerBlocked()) return;
  S.busy = "uninstall";
  renderPlay();
  renderDanger();
  el.bootText.textContent = t("danger.uninstalling");
  el.boot.classList.remove("gone");
  try {
    await invoke("uninstall_launcher");
    // Бекенд завершить процес за мить — інтерфейс більше нічого не робить.
    el.bootText.textContent = t("danger.bye");
  } catch (error) {
    el.boot.classList.add("gone");
    S.busy = null;
    renderPlay();
    renderDanger();
    showError(error);
  }
}

function renderDanger() {
  const locked = Boolean(S.busy) || Boolean(S.running) || S.updating;
  el.dzReset.disabled = locked;
  el.dzUninstall.disabled = locked;
}

/* ═══════════════════════════ МОВА ═══════════════════════════ */

function setLang(next) {
  if (!DICT[next] || next === lang) return;
  lang = next;
  if (S.settings) S.settings.ui_lang = next;
  Dropdown.closeAll();
  hideTip();
  applyI18n();
  // Сповіщення про оновлення — новою мовою (якщо зараз нічого не завантажується).
  const updateToast = $("toast-update");
  if (updateToast && S.update?.release && S.busy !== "update") {
    dismissToast(updateToast, true);
    showUpdateToast(S.update);
  }
  if (!el.onboarding.hidden) {
    renderProjectList();
    renderThemeGrid(el.obThemes, OB.theme);
    if (OB.step === 3) paintMigration();
  }
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
  if (S.tab === "changelog") paintChangelog();
  if (S.tab === "friends") {
    friendsSig = "";
    renderFriends();
  }
  renderHeads();
  renderSteps();
  renderRamNote();
  idleIsland();
  const active = el.nav.querySelector(".nav-item.on");
  requestAnimationFrame(() => moveNavIndicator(active, false));
}

/* ═══════════════════════════ ПОДІЇ ═══════════════════════════ */

async function registerListeners() {
  await listen("launcher-status", (event) => {
    if (S.busy || S.updating) setIsland("busy", String(event.payload));
    onLauncherStatus(String(event.payload));
  });
  await listen("play-step", (event) => setStep(String(event.payload)));
  await listen("sync-progress", (event) => {
    // Оновлення лаунчера показуємо у його сповіщенні, готову збірку — в «острові» й каталозі,
    // а не в панелі збірки.
    const stage = event.payload?.stage;
    if (stage === "launcher-update") onUpdateProgress(event.payload);
    else if (stage === "modpack") onModpackProgress(event.payload);
    else setProgress(event.payload);
  });
  await listen("play-phase", (event) => {
    S.phase = String(event.payload);
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
    renderPresenceDot();
    if (S.tab === "profiles") renderAccountCard();
  });
  // Акаунт змінився у фоні: офлайн-створений @username зареєстровано або він виявився зайнятим.
  await listen("account-changed", (event) => {
    S.account = event.payload;
    renderHeads();
    if (S.tab === "profiles") renderAccountCard();
    if (S.account?.needs_username && !el.app.hidden) showMigration();
    else if (!el.app.hidden) loadSocial({ quiet: true });
  });
  await listen("cloud-status", (event) => {
    const online = Boolean(event.payload?.online);
    const changed = S.cloudOnline !== online;
    S.cloudOnline = online;
    renderPresenceDot();
    if (S.tab === "profiles") renderAccountCard();
    if (S.tab === "friends") renderFriends();
    if (changed && online && !el.app.hidden) loadSocial({ quiet: true });
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

function bindEvents() {
  el.topSync.addEventListener("click", onSync);
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
    if (!el.accountPop.hidden && !event.target.closest("#accountPop") && !event.target.closest("#topProfile")) closeAccountPop();
    if (!el.friendSearch.hidden && !event.target.closest("#friendSearch") && !event.target.closest("#friendForm")) hideFriendSearch();
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
    if (event.target.closest('[data-act="create"]')) {
      openCreateDialog();
      return;
    }
    if (event.target.closest('[data-act="import-new"]')) {
      onImport("");
      return;
    }
    if (event.target.closest('[data-act="modpacks"]')) {
      openContentDialog("", "modpack");
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
      const cards = [...el.instList.querySelectorAll(".inst-card")];
      const index = cards.indexOf(card) + (event.key === "ArrowDown" ? 1 : -1);
      cards[index]?.focus();
    }
  });

  el.instMenu.addEventListener("click", (event) => {
    const button = event.target.closest("[data-act]");
    if (!button || button.disabled) return;
    const id = S.menuFor;
    closeMenu();
    const act = button.dataset.act;
    if (act === "folder") openFolder(id);
    else if (act === "mods") openContentDialog(id);
    else if (act === "repair") onRepair(id);
    else if (act === "debug") onDebug(id);
    else if (act === "import") onImport(id);
    else if (act === "visibility") openVisibilityDialog(id);
    else if (act === "icon") openIconDialog(id);
    else if (act === "delete") onDelete(id);
  });

  el.play.addEventListener("click", onPlay);
  el.stopBtn.addEventListener("click", onStopToggle);
  el.stopYes.addEventListener("click", onStopConfirm);
  el.stopNo.addEventListener("click", onStopCancel);
  el.repairBtn.addEventListener("click", () => onRepair());
  el.modsBtn.addEventListener("click", () => openContentDialog());
  el.folderBtn.addEventListener("click", () => openFolder());
  el.importBtn.addEventListener("click", () => onImport(S.activeId));
  el.deleteBtn.addEventListener("click", () => onDelete(S.activeId));

  el.obStep1.addEventListener("submit", onObStep1);
  el.obThemes.addEventListener("click", onObTheme);
  el.obBack.addEventListener("click", () => !OB.busy && setObStep(1));
  el.obSkip.addEventListener("click", onObSkip);
  el.obForm.addEventListener("submit", onOnboardingSubmit);
  el.obCode.addEventListener("input", () => {
    el.obCode.classList.remove("invalid");
    el.obError.hidden = true;
    if (el.obCode.value.trim() && S.pickedProject) {
      S.pickedProject = "";
      markPicked();
    }
  });
  el.obProjects.addEventListener("click", (event) => {
    el.obError.hidden = true;
    onProjectPick(event, el.obCode);
  });
  el.projPicker.addEventListener("click", (event) => {
    el.switchError.hidden = true;
    onProjectPick(event, el.switchCode);
  });
  el.obNick.addEventListener("input", () => {
    el.obError1.hidden = true;
    OB.nickTouched = el.obNick.value.trim().length > 0;
    nickState(el.obNick, el.obNickHint, "ob.nickHint");
  });
  // Перший ігровий нік за замовчуванням збігається з @username (поки гравець не змінив його сам).
  el.obUser.addEventListener("input", () => {
    el.obError1.hidden = true;
    userState(el.obUser, el.obUserHint, "ob.userHint");
    if (!OB.nickTouched) {
      el.obNick.value = el.obUser.value.trim().replace(/^@/, "").slice(0, 16);
      nickState(el.obNick, el.obNickHint, "ob.nickHint");
    }
  });
  el.obMigrate.addEventListener("submit", onMigrate);
  el.migUser.addEventListener("input", () => {
    el.migError.hidden = true;
    userState(el.migUser, el.migHint, "mig.hint");
  });

  el.topProfile.addEventListener("click", (event) => {
    event.stopPropagation();
    openAccountPop();
  });
  el.accountPop.addEventListener("click", onAccountPopClick);
  el.logoutBtn.addEventListener("click", onLogout);
  el.friendForm.addEventListener("submit", (event) => {
    event.preventDefault();
    sendFriend(cleanQuery(el.friendQuery.value));
  });
  el.friendQuery.addEventListener("input", onFriendQueryInput);
  el.friendQuery.addEventListener("focus", onFriendQueryInput);
  el.friendSearch.addEventListener("click", (event) => {
    const item = event.target.closest("[data-user]");
    if (item && !item.disabled) sendFriend(item.dataset.user);
  });
  document.querySelector('[data-view="friends"]').addEventListener("click", onFriendsClick);
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
  el.switchForm.addEventListener("submit", onSwitchProject);
  el.switchCode.addEventListener("input", () => {
    el.switchCode.classList.remove("invalid");
    el.switchError.hidden = true;
    if (el.switchCode.value.trim() && S.pickedProject) {
      S.pickedProject = "";
      markPicked();
    }
  });
  el.leaveBtn.addEventListener("click", onLeave);
  el.clWrap.addEventListener("click", onChangelogClick);
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
  el.autoUpdate.addEventListener("change", async () => {
    const on = el.autoUpdate.checked;
    try {
      await saveSettingsPatch({ auto_update_launcher: on });
      setIsland("ok", t(on ? "update.autoOn" : "update.autoOff"));
      // Увімкнули автооновлення, а нова версія вже відома — ставимо її одразу.
      if (on && S.update?.available && !S.busy && !S.running) startUpdateInstall();
    } catch (error) {
      el.autoUpdate.checked = !on;
      setIsland("error", errText(error));
    }
  });
  el.checkUpdateBtn.addEventListener("click", async () => {
    setLoading(el.checkUpdateBtn, true);
    S.updateDismissed = false;
    await checkUpdate({ manual: true });
    setLoading(el.checkUpdateBtn, false);
  });
  el.cfForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    const key = el.cfKey.value.trim();
    if (!key) {
      el.cfKey.classList.add("invalid");
      el.cfKey.focus();
      return;
    }
    try {
      await saveSettingsPatch({ curseforge_api_key: key });
      el.cfKey.value = "";
      el.cfKey.classList.remove("invalid");
      modCategoryCache.delete("curseforge");
      renderCurseForge();
      setIsland("ok", t("settings.cfSaved"));
    } catch (error) {
      el.cfKey.classList.add("invalid");
      setIsland("error", errText(error));
    }
  });
  el.cfKey.addEventListener("input", () => el.cfKey.classList.remove("invalid"));
  el.cfClear.addEventListener("click", async () => {
    try {
      await saveSettingsPatch({ curseforge_api_key: "" });
      modCategoryCache.delete("curseforge");
      renderCurseForge();
      setIsland("ok", t("settings.cfCleared"));
    } catch (error) {
      setIsland("error", errText(error));
    }
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
  el.themeGrid.addEventListener("click", onThemePick);
  el.dzReset.addEventListener("click", onDangerReset);
  el.dzUninstall.addEventListener("click", onDangerUninstall);

  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (Dropdown.current) Dropdown.closeAll();
    else if (!el.modal.hidden) closeModal(null);
    else if (S.stopConfirm) onStopCancel();
    else if (!el.accountPop.hidden) {
      closeAccountPop();
      el.topProfile.focus();
    } else if (!el.friendSearch.hidden) hideFriendSearch();
    else closeMenu();
  });
  window.addEventListener("resize", () => {
    closeMenu();
    closeAccountPop();
    Dropdown.closeAll();
    hideTip();
    const active = el.nav.querySelector(".nav-item.on");
    requestAnimationFrame(() => moveNavIndicator(active, false));
  });
  el.instList.addEventListener("scroll", closeMenu, { passive: true });
  document.addEventListener("visibilitychange", () => {
    updateCalm();
    if (document.hidden) {
      Dropdown.closeAll();
      hideTip();
    }
    if (!document.hidden && S.tab === "home") pingServer();
    // Друзі: живий таймер — лише у видимому вікні; після повернення — свіжий знімок.
    syncFriendTicker();
    if (!document.hidden && S.tab === "friends") loadSocial({ quiet: true });
  });
}

/* ═══════════════════════════ СТАРТ ═══════════════════════════ */

/* ═══ оновлення лаунчера: сповіщення, Changelog, встановлення ═══ */

function showRestarting() {
  el.bootText.textContent = t("update.restarting");
  el.boot.classList.remove("gone");
}

/**
 * Перевірка при запуску (і кнопкою в Налаштуваннях). Автооновлення бекенд ставить сам;
 * інакше — сповіщення «v0.2.5 → v0.3.0» з кнопками «Пізніше», «Що нового», «Завантажити».
 */
async function checkUpdate({ manual = false } = {}) {
  S.updating = true;
  renderPlay();
  let result = null;
  try {
    result = await invoke("check_launcher_update");
    S.update = result;
    if (result.restarting) {
      showRestarting();
      return;
    }
    if (result.available && result.release && (manual || !S.updateDismissed)) showUpdateToast(result);
    else if (!result.available && !result.error) dismissToast($("toast-update"));
    if (result.error) {
      if (manual && !result.available) setIsland("warn", t("update.checkFailed"));
      else if (!el.app.hidden && !S.busy) setIsland("warn", result.available ? t("update.failed") : t("status.updateFailed"));
    } else if (manual && !result.available) setIsland("ok", t("update.upToDate", { v: S.launcherVersion }));
  } catch (error) {
    if (manual) setIsland("error", errText(error));
  }
  S.updating = false;
  renderPlay();
  if (!manual && !el.app.hidden && S.view && !S.busy && !S.running && !result?.error) idleIsland();
}

/* ═══ кнопка «Синхронізувати» у верхній панелі ═══ */

/** Перелік через кому з «та / and» перед останнім пунктом. */
function joinList(items) {
  if (items.length <= 1) return items.join("");
  return items.slice(0, -1).join(", ") + t("sync.and") + items[items.length - 1];
}

function renderSyncButton() {
  el.topSync.classList.toggle("is-spinning", S.syncing);
  el.topSync.disabled = S.syncing;
  el.topSync.setAttribute("aria-busy", String(S.syncing));
}

/** Застосовує свіжий ProjectView із синхронізації, не ламаючи збірку, яку зараз встановлюють чи запускають. */
function applySyncedView(view) {
  if (S.busy && S.view) {
    const live = S.view.instances.find((i) => i.id === S.activeId);
    const index = view.instances.findIndex((i) => i.id === S.activeId);
    if (live && index >= 0) view.instances[index] = live;
  }
  const keep = view.instances.some((i) => i.id === S.activeId);
  S.view = view;
  S.viewError = null;
  if (!keep) S.activeId = view.active_instance;
  if (S.binding && view.name) S.binding.name = view.name;
  prefetchAssets();
  renderTop();
  renderList();
  renderHero();
  if (S.tab === "project") renderProject();
}

/**
 * Комплексна перевірка без кешу: нова версія лаунчера (сповіщення «Пізніше / Changelog / Завантажити»),
 * нові збірки проєкту та нові версії manifest.json встановлених збірок (кнопка «ОНОВИТИ»).
 */
async function onSync() {
  if (S.syncing) return;
  S.syncing = true;
  renderSyncButton();
  if (!S.busy) setIsland("busy", t("sync.running"));
  // Поки йшла синхронізація, гравець міг підключити інший проєкт чи вийти з нього.
  const bindingKey = S.binding?.index_url || "";
  const seq = loadSeq;
  try {
    const result = await invoke("sync_all");
    const launcher = result.launcher;
    if (launcher) S.update = launcher;
    if (result.project && bindingKey === (S.binding?.index_url || "") && seq === loadSeq) applySyncedView(result.project);

    const found = [];
    if (launcher?.available && launcher.release) {
      found.push(t("sync.launcher", { v: launcher.release.version }));
      S.updateDismissed = false;
      showUpdateToast(launcher);
    }
    const fresh = result.new_instances || [];
    const updates = result.updates || [];
    if (fresh.length) found.push(plural(fresh.length, "plural.newPacks"));
    if (updates.length) found.push(plural(updates.length, "plural.packUpdates"));

    const notes = [];
    if (fresh.length) notes.push(t("sync.newList", { names: fresh.join(", ") }));
    if (updates.length) notes.push(t("sync.updList", { names: updates.join(", ") }));
    if (launcher?.error) notes.push(t("sync.launcherFailed"));
    if (result.project_error) notes.push(t("sync.projectFailed", { error: result.project_error }));
    else if (result.project?.offline) notes.push(t("sync.offline"));
    if (result.manifest_errors) notes.push(t("sync.manifestFailed", { n: result.manifest_errors }));

    const failedAll = launcher?.error && (result.project_error || result.project?.offline);
    const summary = found.length ? t("sync.found", { list: joinList(found) }) : failedAll ? t("sync.failed") : t("sync.upToDate");
    const tone = found.length ? "accent" : notes.length ? "warn" : "";
    toast({
      id: "sync",
      icon: found.length ? "sparkle" : notes.length ? "alert" : "check",
      tone,
      title: t("sync.title"),
      text: summary,
      html: notes.length ? `<p class="toast-text">${notes.map(esc).join("<br>")}</p>` : "",
      timeout: found.length ? 9000 : 6000
    });
    if (!S.busy) setIsland(found.length ? "ok" : notes.length ? "warn" : "ok", summary);
  } catch (error) {
    if (!S.busy) setIsland("error", errHuman(error));
    toast({ id: "sync", icon: "alert", tone: "warn", title: t("sync.title"), text: errHuman(error), timeout: 9000 });
  } finally {
    S.syncing = false;
    renderSyncButton();
  }
}

function updateVersionLine(check) {
  const release = check.release;
  const date = release.date ? changelogDate(release.date) : "";
  return `<p class="toast-ver"><span class="mono">v${esc(check.current)}</span><span class="toast-arrow" aria-hidden="true">→</span><span class="mono toast-new">v${esc(release.version)}</span>${
    date ? `<span class="toast-date">${esc(t("update.released", { date }))}</span>` : ""
  }</p>`;
}

function showUpdateToast(check) {
  const existing = $("toast-update");
  if (existing && !existing.classList.contains("leaving") && existing.dataset.version === check.release.version) return existing;
  const node = toast({
    id: "update",
    icon: "sparkle",
    tone: "accent",
    title: t("update.title"),
    html:
      updateVersionLine(check) +
      `<div class="toast-progress" hidden><div class="progress-track"><div class="progress-fill"></div></div><small class="toast-progress-text"></small></div><p class="toast-err" hidden></p>`,
    actions: [
      { id: "later", label: t("update.later"), kind: "plain", onClick: () => void (S.updateDismissed = true) },
      { id: "changelog", label: t("update.changelog"), kind: "ghost", onClick: () => (openUpdateChangelog(check), false) },
      { id: "download", label: t("update.download"), kind: "primary", onClick: () => (startUpdateInstall(), false) }
    ]
  });
  node.classList.add("toast-update");
  node.dataset.version = check.release.version;
  node.querySelector(".toast-x").addEventListener("click", () => (S.updateDismissed = true));
  return node;
}

/** Повний форматований опис релізу з кнопками «Закрити» та «Завантажити оновлення». */
function openUpdateChangelog(check) {
  const release = check.release;
  const date = release.date ? changelogDate(release.date) : "";
  const meta = [`v${check.current} → v${release.version}`, date ? t("update.released", { date }) : "", release.name && release.name !== release.tag ? release.name : ""]
    .filter(Boolean)
    .join(" · ");
  openModal({
    icon: "sparkle",
    title: t("update.modalTitle", { v: release.version }),
    html: `<p class="cl-meta">${esc(meta)}</p><div class="md md-release selectable">${release.notes ? renderMarkdown(release.notes) : `<p>${esc(t("update.noNotes"))}</p>`}</div>`,
    xl: true,
    buttons: [
      { id: "close", label: t("common.close"), kind: "ghost" },
      { id: "download", label: t("update.downloadFull"), kind: "primary", onClick: () => void setTimeout(startUpdateInstall, 0) }
    ]
  });
}

function ensureUpdateToast() {
  const existing = $("toast-update");
  if (existing && !existing.classList.contains("leaving")) return existing;
  if (S.update?.release) return showUpdateToast(S.update);
  const node = toast({
    id: "update",
    icon: "sparkle",
    tone: "accent",
    title: t("update.downloading"),
    html: `<div class="toast-progress" hidden><div class="progress-track"><div class="progress-fill"></div></div><small class="toast-progress-text"></small></div><p class="toast-err" hidden></p>`
  });
  node.classList.add("toast-update");
  return node;
}

function setUpdateToastState(node, downloading, error = "") {
  node.classList.toggle("busy", downloading);
  const actions = node.querySelector(".toast-actions");
  if (actions) actions.hidden = downloading;
  node.querySelector(".toast-progress").hidden = !downloading;
  const err = node.querySelector(".toast-err");
  err.hidden = !error;
  err.textContent = error;
  if (downloading) {
    node.querySelector(".progress-track").classList.add("indeterminate");
    node.querySelector(".toast-progress-text").textContent = t("update.downloading");
  }
}

function onUpdateProgress(payload) {
  const node = ensureUpdateToast();
  if (!node.classList.contains("busy")) setUpdateToastState(node, true);
  const percent = Math.max(0, Math.min(100, Number(payload.percent) || 0));
  const track = node.querySelector(".progress-track");
  track.classList.remove("indeterminate");
  node.querySelector(".progress-fill").style.transform = `scaleX(${percent / 100})`;
  const amount = payload.total > 1 ? ` · ${megabytes(payload.current)} / ${megabytes(payload.total)} ${t("unit.mb")}` : "";
  node.querySelector(".toast-progress-text").textContent = `${t("update.downloading")} ${Math.floor(percent)}%${amount}`;
}

async function startUpdateInstall() {
  if (S.busy === "update") return;
  if (S.running) {
    setIsland("warn", t("update.gameRunning"));
    return;
  }
  if (S.busy) {
    setIsland("warn", t("play.wait"));
    return;
  }
  S.busy = "update";
  renderPlay();
  const node = ensureUpdateToast();
  setUpdateToastState(node, true);
  try {
    const result = await invoke("install_launcher_update");
    if (result.restarting) {
      showRestarting();
      return;
    }
    dismissToast(node);
    setIsland("ok", t("update.upToDate", { v: S.launcherVersion }));
  } catch (error) {
    setUpdateToastState(node, false, errHuman(error));
    setIsland("error", t("update.failed"));
  } finally {
    if (S.busy === "update") {
      S.busy = null;
      renderPlay();
    }
  }
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
  S.account = info.account;
  lang = info.settings.ui_lang === "en" ? "en" : "uk";
  applyTheme(info.settings.theme);

  el.ram.min = "1024";
  el.ram.max = String(Math.max(1280, info.max_ram_mb));
  el.ram.step = "256";
  el.ram.value = String(info.settings.ram_mb);
  applyI18n();
  updateCalm();
  // Шаблони дзеркал — щоб фони й іконки збірок вантажились і тоді, коли GitHub заблоковано.
  invoke("mirrors_info")
    .then((mirrors) => {
      S.mirrors = mirrors;
    })
    .catch(() => {});

  // Новий гравець — повний майстер; гравець v0.4.0 (є ніки, немає @username) — лише вікно
  // вибору @username, після якого одразу звичний інтерфейс з усіма збірками й ніками.
  if (!info.onboarded) showOnboarding();
  else if (info.account?.needs_username) showMigration();
  else showApp();
  checkUpdate();
}

init().catch((error) => {
  el.boot.classList.add("gone");
  el.app.hidden = false;
  setIsland("error", errText(error));
});
