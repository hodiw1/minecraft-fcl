const tauri = window.__TAURI__;
const invoke = tauri.core.invoke;
const listen = tauri.event.listen;

const NICK_RE = /^[A-Za-z0-9_]{3,16}$/;

const STAGES = {
  "launcher-update": "Оновлення лаунчера",
  verify: "Перевірка файлів збірки",
  download: "Завантаження збірки",
  java: "Завантаження Java",
  client: "Клієнт Minecraft",
  libraries: "Бібліотеки",
  assets: "Ресурси гри"
};

const LOADERS = {
  vanilla: "Vanilla",
  fabric: "Fabric",
  forge: "Forge",
  neoforge: "NeoForge"
};

const el = {
  island: document.getElementById("island"),
  status: document.getElementById("statusText"),
  nickname: document.getElementById("nickname"),
  nickHint: document.getElementById("nickHint"),
  ram: document.getElementById("ram"),
  ramValue: document.getElementById("ramValue"),
  ramHint: document.getElementById("ramHint"),
  stage: document.getElementById("progressStage"),
  percent: document.getElementById("progressPercent"),
  fill: document.getElementById("progressFill"),
  file: document.getElementById("progressFile"),
  play: document.getElementById("play"),
  playLabel: document.getElementById("playLabel"),
  version: document.getElementById("launcherVersion"),
  packName: document.getElementById("packName"),
  packVersion: document.getElementById("packVersion"),
  mcBadge: document.getElementById("mcBadge"),
  loaderBadge: document.getElementById("loaderBadge")
};

const state = {
  ready: false,
  busy: false,
  running: false,
  recommendedRam: null
};

function setIsland(kind, text) {
  el.island.className = "island is-" + kind;
  el.status.textContent = text;
  el.island.title = text;
  void el.island.offsetWidth;
  el.island.classList.add("pop");
}

function formatRam(mb) {
  const gb = mb / 1024;
  const text = Number.isInteger(gb) ? String(gb) : gb.toFixed(2).replace(/0$/, "");
  return text + " ГБ";
}

function refreshRam() {
  const min = Number(el.ram.min);
  const max = Number(el.ram.max);
  const value = Number(el.ram.value);
  const fill = max > min ? ((value - min) / (max - min)) * 100 : 0;
  el.ram.style.setProperty("--fill", fill + "%");
  el.ramValue.textContent = formatRam(value) + " · " + value + " МБ";
  if (state.recommendedRam) {
    el.ramHint.textContent = "Рекомендовано для збірки: " + formatRam(state.recommendedRam);
  } else {
    el.ramHint.textContent = "Доступно в системі до " + formatRam(max);
  }
}

function setProgress(payload) {
  const percent = Math.max(0, Math.min(100, Number(payload.percent) || 0));
  el.fill.style.width = percent + "%";
  el.percent.textContent = Math.round(percent) + "%";
  el.stage.textContent = STAGES[payload.stage] || "Робота";
  el.file.textContent = payload.file || " ";
}

function resetProgress(label) {
  el.fill.style.width = "0%";
  el.percent.textContent = "0%";
  el.stage.textContent = label || "Очікування";
  el.file.textContent = " ";
}

function applyPack(pack) {
  if (!pack) {
    return;
  }
  el.packName.textContent = pack.name || "Збірка";
  el.packVersion.textContent = pack.version ? "Версія збірки " + pack.version : "Версія збірки не вказана";
  el.mcBadge.textContent = "Minecraft " + pack.minecraft_version;
  const loader = LOADERS[String(pack.loader).toLowerCase()] || pack.loader;
  el.loaderBadge.textContent = pack.loader_version ? loader + " " + pack.loader_version : loader;
  state.recommendedRam = pack.recommended_ram_mb || null;
  refreshRam();
}

function updatePlayButton() {
  if (state.running) {
    el.playLabel.textContent = "Гра запущена";
    el.play.disabled = true;
  } else if (state.busy) {
    el.playLabel.textContent = "Зачекайте…";
    el.play.disabled = true;
  } else if (!state.ready) {
    el.playLabel.textContent = "Повторити";
    el.play.disabled = false;
  } else {
    el.playLabel.textContent = "Грати";
    el.play.disabled = false;
  }
}

function nicknameValid() {
  return NICK_RE.test(el.nickname.value.trim());
}

function showNickState() {
  const value = el.nickname.value.trim();
  const invalid = value.length > 0 && !NICK_RE.test(value);
  el.nickname.classList.toggle("invalid", invalid);
  el.nickHint.classList.toggle("error", invalid);
  el.nickHint.textContent = invalid
    ? "Лише латиниця, цифри та _ (3–16 символів)"
    : "3–16 символів: латиниця, цифри, _";
}

function persistSettings() {
  if (!nicknameValid()) {
    return;
  }
  invoke("save_settings", {
    nickname: el.nickname.value.trim(),
    ramMb: Number(el.ram.value)
  }).catch(() => {});
}

async function runBootstrap() {
  state.busy = true;
  state.ready = false;
  updatePlayButton();
  resetProgress("Підготовка");
  let restarting = false;
  try {
    const result = await invoke("bootstrap");
    if (result.restarting) {
      restarting = true;
      setIsland("busy", "Перезапуск лаунчера…");
      return;
    }
    state.ready = true;
    applyPack(result.pack);
    setProgress({ stage: "verify", percent: 100, file: "" });
    el.stage.textContent = "Збірка актуальна";
    if (result.offline) {
      setIsland("warn", "Офлайн-режим: використовується локальна збірка");
    } else {
      setIsland("ok", "Готово до запуску");
    }
  } catch (error) {
    state.ready = false;
    setIsland("error", String(error));
    resetProgress("Помилка");
  } finally {
    if (!restarting) {
      state.busy = false;
    }
    updatePlayButton();
  }
}

async function onPlay() {
  if (!state.ready) {
    await runBootstrap();
    return;
  }
  if (!nicknameValid()) {
    el.nickname.classList.add("invalid");
    el.nickHint.classList.add("error");
    el.nickHint.textContent = "Введи коректний нікнейм: латиниця, цифри, _ (3–16)";
    el.nickname.focus();
    return;
  }
  state.busy = true;
  updatePlayButton();
  resetProgress("Підготовка до запуску");
  try {
    await invoke("play", {
      nickname: el.nickname.value.trim(),
      ramMb: Number(el.ram.value)
    });
    resetProgress("Гра запущена");
  } catch (error) {
    setIsland("error", String(error));
    resetProgress("Помилка запуску");
  } finally {
    state.busy = false;
    updatePlayButton();
  }
}

async function registerListeners() {
  await listen("launcher-status", (event) => {
    setIsland("busy", String(event.payload));
  });
  await listen("sync-progress", (event) => {
    setProgress(event.payload);
  });
  await listen("game-state", (event) => {
    const payload = event.payload;
    if (payload.state === "running") {
      state.running = true;
      setIsland("ok", "Гра запущена");
    } else {
      state.running = false;
      resetProgress("Очікування");
      if (payload.code !== null && payload.code !== 0) {
        setIsland("error", "Гра завершилась з кодом " + payload.code + ". Лог: game_output.log");
      } else {
        setIsland("ok", "Гра закрита");
      }
    }
    updatePlayButton();
  });
}

async function init() {
  el.nickname.addEventListener("input", showNickState);
  el.nickname.addEventListener("change", persistSettings);
  el.nickname.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !el.play.disabled) {
      onPlay();
    }
  });
  el.ram.addEventListener("input", refreshRam);
  el.ram.addEventListener("change", persistSettings);
  el.play.addEventListener("click", onPlay);

  await registerListeners();

  const info = await invoke("get_init_info");
  el.version.textContent = "Лаунчер v" + info.launcher_version;
  el.nickname.value = info.nickname || "";
  el.ram.min = "1024";
  el.ram.max = String(Math.max(info.max_ram_mb, 1280));
  el.ram.step = "256";
  el.ram.value = String(info.ram_mb);
  applyPack(info.pack);
  refreshRam();
  showNickState();
  updatePlayButton();

  await runBootstrap();
}

init().catch((error) => {
  setIsland("error", String(error));
});
