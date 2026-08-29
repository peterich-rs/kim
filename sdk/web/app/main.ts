import type { Message } from "../src/index.ts";
import { login, logout, register } from "./auth.ts";
import { ChatSession, type Kind } from "./chat.ts";
import { clearSession, loadSession, saveSession } from "./session.ts";

const DEFAULT_WS = "ws://127.0.0.1:8001/";

const $ = <T extends HTMLElement>(id: string): T => {
  const el = document.getElementById(id);
  if (!el) {
    throw new Error(`#${id} missing`);
  }
  return el as T;
};

const authView = $("auth");
const shell = $("shell");
const authForm = $<HTMLFormElement>("auth-form");
const accountEl = $<HTMLInputElement>("auth-account");
const passwordEl = $<HTMLInputElement>("auth-password");
const confirmEl = $<HTMLInputElement>("auth-confirm");
const confirmWrap = $("confirm-wrap");
const authError = $("auth-error");
const authSubmit = $<HTMLButtonElement>("auth-submit");
const tabLogin = $("tab-login");
const tabRegister = $("tab-register");
const threadsEl = $<HTMLUListElement>("threads");
const logEl = $<HTMLOListElement>("log");
const emptyEl = $("empty");
const bodyEl = $<HTMLInputElement>("body");
const sendBtn = $<HTMLButtonElement>("send");
const composer = $<HTMLFormElement>("composer");
const meName = $("me-name");
const meStatus = $("me-status");
const meAvatar = $("me-avatar");
const chatTitle = $("chat-title");
const chatSub = $("chat-sub");
const sideTitle = $("side-title");
const railGroups = $("rail-groups");
const membersPanel = $("members-panel");
const membersEl = $<HTMLUListElement>("members");
const banner = $("banner");
const groupModal = $("group-modal");
const groupForm = $<HTMLFormElement>("group-form");
const groupNameEl = $<HTMLInputElement>("group-name");
const groupMembersEl = $<HTMLInputElement>("group-members");
const groupError = $("group-error");
const newDmAcc = $<HTMLInputElement>("new-dm-acc");

type Row =
  | { kind: "msg"; dest: string; msg: Message }
  | { kind: "sys"; dest: string; text: string };

const threads = new Map<string, { kind: Kind; title: string }>();
const groupIds = new Set<string>();
const rows: Row[] = [];

let mode: "login" | "register" = "login";
let session: ChatSession | undefined;
let active = "";
let activeKind: Kind = "user";
let railMode: "home" | "group" = "home";

function hue(name: string): string {
  let h = 0;
  for (const c of name) {
    h = (h + c.charCodeAt(0) * 17) % 360;
  }
  return `hsl(${h} 35% 38%)`;
}

function initial(name: string): string {
  return (name.trim().charAt(0) || "?").toUpperCase();
}

function setMode(next: "login" | "register"): void {
  mode = next;
  tabLogin.classList.toggle("on", next === "login");
  tabRegister.classList.toggle("on", next === "register");
  confirmWrap.classList.toggle("hidden", next === "login");
  authSubmit.textContent = next === "login" ? "登录" : "注册";
  passwordEl.autocomplete = next === "login" ? "current-password" : "new-password";
}

function showAuthError(text: string): void {
  authError.hidden = !text;
  authError.textContent = text;
}

function setMeStatus(text: string, cls: "ok" | "bad" | "" = ""): void {
  meStatus.textContent = text;
  meStatus.className = cls;
}

function showBanner(text: string): void {
  banner.textContent = text;
  banner.classList.toggle("hidden", !text);
}

function showShell(on: boolean): void {
  authView.classList.toggle("hidden", on);
  shell.classList.toggle("hidden", !on);
}

function avatarEl(name: string): HTMLSpanElement {
  const span = document.createElement("span");
  span.className = "avatar";
  span.textContent = initial(name);
  span.style.background = hue(name);
  return span;
}

function remember(id: string, kind: Kind, title = id): void {
  if (!id) {
    return;
  }
  threads.set(id, { kind, title });
  if (kind === "group") {
    groupIds.add(id);
  }
  renderRail();
  renderThreads();
}

function visibleThreads(): [string, { kind: Kind; title: string }][] {
  const list = [...threads.entries()];
  if (railMode === "home") {
    return list.filter(([, t]) => t.kind === "user");
  }
  return list.filter(([id]) => id === active && groupIds.has(id));
}

function renderRail(): void {
  railGroups.replaceChildren();
  for (const id of groupIds) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `pill${railMode === "group" && active === id ? " on" : ""}`;
    btn.title = threads.get(id)?.title ?? id;
    btn.textContent = initial(threads.get(id)?.title ?? id);
    btn.addEventListener("click", () => {
      railMode = "group";
      openThread(id, "group");
      $("home-pill").classList.remove("on");
      renderRail();
    });
    railGroups.append(btn);
  }
}

function renderThreads(): void {
  threadsEl.replaceChildren();
  for (const [id, meta] of visibleThreads()) {
    const li = document.createElement("li");
    if (id === active) {
      li.classList.add("active");
    }
    li.append(avatarEl(meta.title), document.createTextNode(meta.title));
    li.addEventListener("click", () => openThread(id, meta.kind));
    threadsEl.append(li);
  }
}

function renderLog(): void {
  const me = session?.account ?? "";
  logEl.replaceChildren();
  let last = "";
  let any = false;
  for (const row of rows) {
    if (row.dest !== active) {
      continue;
    }
    any = true;
    const li = document.createElement("li");
    if (row.kind === "sys") {
      li.className = "row sys";
      li.textContent = row.text;
    } else {
      const mine = row.msg.sender === me;
      const cont = last === row.msg.sender;
      li.className = `row${mine ? " me" : ""}${cont ? " cont" : ""}`;
      if (!cont) {
        const meta = document.createElement("span");
        meta.className = "meta";
        meta.textContent = row.msg.sender || (mine ? me : active);
        li.append(meta);
      }
      li.append(document.createTextNode(row.msg.body));
      last = row.msg.sender;
    }
    logEl.append(li);
  }
  emptyEl.classList.toggle("hidden", any || !active);
  logEl.scrollTop = logEl.scrollHeight;
}

async function renderMembers(): Promise<void> {
  const group = activeKind === "group";
  membersPanel.hidden = !group;
  shell.classList.toggle("no-members", !group);
  membersEl.replaceChildren();
  if (!group || !session) {
    return;
  }
  const list = await session.members(active);
  for (const name of list) {
    const li = document.createElement("li");
    li.append(avatarEl(name), document.createTextNode(name));
    membersEl.append(li);
  }
}

function openThread(id: string, kind: Kind): void {
  active = id;
  activeKind = kind;
  remember(id, kind);
  chatTitle.textContent = threads.get(id)?.title ?? id;
  chatSub.textContent = kind === "group" ? "群聊" : "私信";
  sendBtn.disabled = !session;
  shell.classList.add("chat-open");
  sideTitle.textContent = railMode === "home" ? "私信" : "群";
  renderThreads();
  renderLog();
  void renderMembers();
}

async function enter(token: string, account: string, exp: number, ws: string): Promise<void> {
  saveSession({ token, account, exp, ws });
  meName.textContent = account;
  meAvatar.textContent = initial(account);
  meAvatar.style.background = hue(account);
  showShell(true);
  setMeStatus("连接中…");
  const chat = new ChatSession(account, {
    onStatus: setMeStatus,
    onEvent: (evt) => {
      rows.push({ kind: "sys", dest: active, text: evt });
      renderLog();
    },
    onMessage: (msg, dest) => {
      const kind: Kind = msg.group ? "group" : "user";
      remember(dest, kind, dest);
      rows.push({ kind: "msg", dest, msg });
      if (!active) {
        openThread(dest, kind);
      } else {
        renderLog();
      }
    },
    onKick: () => {
      showBanner("已在其他设备登录");
      void leave(false);
    },
    onGroup: (groupId, members) => {
      remember(groupId, "group", groupId);
      rows.push({
        kind: "sys",
        dest: groupId,
        text: `群成员 ${members.join("、")}`,
      });
      renderLog();
    },
  });
  session = chat;
  await chat.connect(ws, token);
  sendBtn.disabled = !active;
}

async function leave(callLogout: boolean): Promise<void> {
  const token = loadSession()?.token;
  const chat = session;
  session = undefined;
  if (callLogout && token) {
    try {
      await logout(token);
    } catch {
      /* still clear locally */
    }
  }
  await chat?.disconnect();
  clearSession();
  showShell(false);
  showBanner("");
  rows.length = 0;
  threads.clear();
  groupIds.clear();
  active = "";
  setMeStatus("未连接");
}

tabLogin.addEventListener("click", () => setMode("login"));
tabRegister.addEventListener("click", () => setMode("register"));

$("toggle-pw").addEventListener("click", () => {
  const show = passwordEl.type === "password";
  passwordEl.type = show ? "text" : "password";
  confirmEl.type = passwordEl.type;
  $("toggle-pw").textContent = show ? "隐藏" : "显示";
});

authForm.addEventListener("submit", (ev) => {
  ev.preventDefault();
  void (async () => {
    showAuthError("");
    const account = accountEl.value.trim();
    const password = passwordEl.value;
    if (mode === "register" && password !== confirmEl.value) {
      showAuthError("两次密码不一致");
      return;
    }
    authSubmit.disabled = true;
    try {
      const body =
        mode === "register"
          ? await register(account, password)
          : await login(account, password);
      await enter(body.token, body.account, body.exp, DEFAULT_WS);
    } catch (err) {
      showAuthError(err instanceof Error ? err.message : String(err));
    } finally {
      authSubmit.disabled = false;
    }
  })();
});

$("logout").addEventListener("click", () => {
  void leave(true);
});

$("home-pill").addEventListener("click", () => {
  railMode = "home";
  $("home-pill").classList.add("on");
  sideTitle.textContent = "私信";
  renderRail();
  renderThreads();
  shell.classList.remove("chat-open");
});

$("new-dm").addEventListener("submit", (ev) => {
  ev.preventDefault();
  const dest = newDmAcc.value.trim();
  if (!dest) {
    return;
  }
  railMode = "home";
  openThread(dest, "user");
  newDmAcc.value = "";
});

composer.addEventListener("submit", (ev) => {
  ev.preventDefault();
  void (async () => {
    const text = bodyEl.value.trim();
    if (!session || !active || !text) {
      return;
    }
    try {
      const msg = await session.send(active, activeKind, text);
      rows.push({ kind: "msg", dest: active, msg });
      bodyEl.value = "";
      renderLog();
    } catch (err) {
      rows.push({
        kind: "sys",
        dest: active,
        text: err instanceof Error ? err.message : String(err),
      });
      renderLog();
    }
  })();
});

$("new-group").addEventListener("click", () => {
  groupModal.classList.remove("hidden");
  groupError.hidden = true;
});

$("group-cancel").addEventListener("click", () => {
  groupModal.classList.add("hidden");
});

groupForm.addEventListener("submit", (ev) => {
  ev.preventDefault();
  void (async () => {
    if (!session) {
      return;
    }
    const name = groupNameEl.value.trim() || "群";
    const members = groupMembersEl.value
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    try {
      const id = await session.createGroup(name, members);
      groupModal.classList.add("hidden");
      remember(id, "group", name);
      railMode = "group";
      openThread(id, "group");
      renderRail();
    } catch (err) {
      groupError.hidden = false;
      groupError.textContent = err instanceof Error ? err.message : String(err);
    }
  })();
});

$("back").addEventListener("click", () => {
  shell.classList.remove("chat-open");
});

$("members-toggle").addEventListener("click", () => {
  membersPanel.hidden = !membersPanel.hidden;
});

const existing = loadSession();
if (existing) {
  void enter(existing.token, existing.account, existing.exp, existing.ws).catch((err: unknown) => {
    clearSession();
    showShell(false);
    showAuthError(err instanceof Error ? err.message : String(err));
  });
}

setMode("login");
