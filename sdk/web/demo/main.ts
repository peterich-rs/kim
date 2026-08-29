import {
  Content,
  KIMClient,
  KIMEvent,
  KeyValueStore,
  Message,
  State,
} from "../src/index.ts";
import { mintToken } from "./mint.ts";

const $ = <T extends HTMLElement>(id: string): T => {
  const el = document.getElementById(id);
  if (!el) {
    throw new Error(`#${id} missing`);
  }
  return el as T;
};

const accountEl = $<HTMLInputElement>("account");
const wsurlEl = $<HTMLInputElement>("wsurl");
const destEl = $<HTMLInputElement>("dest");
const kindEl = $<HTMLSelectElement>("kind");
const bodyEl = $<HTMLInputElement>("body");
const membersEl = $<HTMLInputElement>("members");
const connectBtn = $<HTMLButtonElement>("connect");
const sendBtn = $<HTMLButtonElement>("send");
const mkgroupBtn = $<HTMLButtonElement>("mkgroup");
const statusEl = $("status");
const logEl = $<HTMLOListElement>("log");
const threadsEl = $<HTMLUListElement>("threads");
const composer = $<HTMLFormElement>("composer");
const hintEl = $("hint");

const params = new URLSearchParams(location.search);
accountEl.value = params.get("acc") ?? "alice";
wsurlEl.value = params.get("ws") ?? "ws://127.0.0.1:8001/";
destEl.value = params.get("dest") ?? "bob";
membersEl.value = "alice,bob";

type Row =
  | { kind: "msg"; dest: string; msg: Message }
  | { kind: "sys"; dest: string; text: string };

const rows: Row[] = [];
const threadNames = new Set<string>();
const groupIds = new Set<string>();
let cli: KIMClient | undefined;

function setStatus(text: string, cls: "ok" | "bad" | "" = ""): void {
  statusEl.textContent = text;
  statusEl.className = `status ${cls}`.trim();
}

function currentDest(): string {
  return destEl.value.trim();
}

function threadOf(msg: Message, me: string): string {
  if (msg.group) {
    return msg.group;
  }
  if (msg.sender === me) {
    return msg.receiver;
  }
  return msg.sender;
}

function remember(dest: string): void {
  if (!dest) {
    return;
  }
  threadNames.add(dest);
  renderThreads();
}

function renderThreads(): void {
  const active = currentDest();
  threadsEl.replaceChildren();
  for (const name of threadNames) {
    const li = document.createElement("li");
    li.textContent = name;
    if (name === active) {
      li.classList.add("active");
    }
    li.addEventListener("click", () => {
      destEl.value = name;
      kindEl.value = groupIds.has(name) ? "group" : "user";
      renderLog();
      renderThreads();
    });
    threadsEl.append(li);
  }
}

function renderLog(): void {
  const dest = currentDest();
  const me = cli?.account ?? accountEl.value.trim();
  logEl.replaceChildren();
  for (const row of rows) {
    if (row.dest && row.dest !== dest && row.kind === "msg") {
      continue;
    }
    if (row.kind === "sys" && row.dest && row.dest !== dest) {
      continue;
    }
    const li = document.createElement("li");
    if (row.kind === "sys") {
      li.className = "row sys";
      li.textContent = row.text;
    } else {
      const mine = row.msg.sender === me;
      li.className = `row${mine ? " me" : ""}`;
      const meta = document.createElement("span");
      meta.className = "meta";
      meta.textContent = row.msg.sender || (mine ? me : row.dest);
      li.append(meta, document.createTextNode(row.msg.body));
    }
    logEl.append(li);
  }
  logEl.scrollTop = logEl.scrollHeight;
}

function pushSys(text: string, dest = currentDest()): void {
  rows.push({ kind: "sys", dest, text });
  renderLog();
}

function pushMsg(msg: Message, dest: string): void {
  if (msg.group) {
    groupIds.add(msg.group);
  }
  remember(dest);
  rows.push({ kind: "msg", dest, msg });
  renderLog();
}

function bindClient(next: KIMClient): void {
  next.register(
    [KIMEvent.Closed, KIMEvent.Kickout, KIMEvent.Reconnecting, KIMEvent.Reconnected],
    (evt) => {
      if (evt === KIMEvent.Kickout) {
        setStatus("被踢下线（同账号另一端登录）", "bad");
      } else if (evt === KIMEvent.Reconnecting) {
        setStatus("重连中…");
      } else if (evt === KIMEvent.Reconnected) {
        setStatus(`已重连 ${next.channelId}`, "ok");
      } else {
        setStatus("已断开");
        setConnected(false);
      }
      pushSys(`事件 ${evt}`);
    },
  );
  next.onmessage((m) => {
    pushMsg(m, threadOf(m, next.account));
  });
  next.onofflinemessage((om) => {
    const users = om.listUsers();
    const groups = om.listGroups();
    if (users.length === 0 && groups.length === 0) {
      return;
    }
    pushSys(`离线：用户 ${users.join(",") || "无"}；群 ${groups.join(",") || "无"}`);
    void (async () => {
      for (const u of users) {
        remember(u);
        const page = await om.loadUser(u, 1);
        for (const m of page) {
          m.sender = m.sender || u;
          pushMsg(m, u);
        }
      }
      for (const g of groups) {
        groupIds.add(g);
        remember(g);
        const page = await om.loadGroup(g, 1);
        for (const m of page) {
          m.group = g;
          pushMsg(m, g);
        }
      }
    })();
  });
  next.ongroupcreate((groupId, members) => {
    groupIds.add(groupId);
    remember(groupId);
    pushSys(`群 ${groupId} 成员 ${members.join(",")}`, groupId);
  });
}

function setConnected(on: boolean): void {
  connectBtn.textContent = on ? "断开" : "连接";
  sendBtn.disabled = !on;
  mkgroupBtn.disabled = !on;
  accountEl.disabled = on;
  wsurlEl.disabled = on;
}

async function connect(): Promise<void> {
  const account = accountEl.value.trim();
  const url = wsurlEl.value.trim();
  if (!account || !url) {
    setStatus("填账号和网关", "bad");
    return;
  }
  const q = new URLSearchParams({ acc: account, dest: currentDest() });
  history.replaceState(null, "", `?${q.toString()}`);
  setStatus("连接中…");
  const token = await mintToken(account);
  const next = new KIMClient(url, { token }, {
    store: new KeyValueStore(localStorage, `kim_${account}`),
    reconnect: true,
  });
  bindClient(next);
  const { success, err } = await next.login();
  if (!success) {
    const raw = err?.message ?? "登录失败";
    const hint =
      raw.includes("timeout") || raw.includes("closed") || raw.includes("unreachable")
        ? `${raw}。先停掉假进程，按 chat → gateway 顺序再起。`
        : raw;
    setStatus(hint, "bad");
    return;
  }
  cli = next;
  remember(currentDest());
  setConnected(true);
  setStatus(`已连接 ${next.channelId}`, "ok");
}

async function disconnect(): Promise<void> {
  const cur = cli;
  cli = undefined;
  await cur?.logout();
  setConnected(false);
  setStatus("已断开");
}

connectBtn.addEventListener("click", () => {
  if (cli && (cli.state === State.CONNECTED || cli.state === State.CONNECTING)) {
    void disconnect();
    return;
  }
  void connect().catch((err: unknown) => {
    setStatus(err instanceof Error ? err.message : String(err), "bad");
  });
});

composer.addEventListener("submit", (ev) => {
  ev.preventDefault();
  void send();
});

destEl.addEventListener("change", () => {
  remember(currentDest());
  renderLog();
});

async function send(): Promise<void> {
  if (!cli || cli.state !== State.CONNECTED) {
    return;
  }
  const dest = currentDest();
  const text = bodyEl.value.trim();
  if (!dest || !text) {
    return;
  }
  const groupish = kindEl.value === "group";
  const { status, resp, err } = groupish
    ? await cli.talkToGroup(dest, new Content(text))
    : await cli.talkToUser(dest, new Content(text));
  if (status !== 0) {
    pushSys(`发送失败 ${status} ${err?.message ?? ""}`);
    return;
  }
  const msg = new Message(resp?.messageId ?? 0n, resp?.sendTime ?? 0n);
  msg.sender = cli.account;
  msg.receiver = dest;
  msg.group = groupish ? dest : "";
  msg.type = 1;
  msg.body = text;
  msg.contentLoaded = true;
  if (groupish) {
    groupIds.add(dest);
  }
  pushMsg(msg, dest);
  bodyEl.value = "";
  bodyEl.focus();
}

mkgroupBtn.addEventListener("click", () => {
  void (async () => {
    if (!cli) {
      return;
    }
    const members = membersEl.value
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    if (!members.includes(cli.account)) {
      members.unshift(cli.account);
    }
    const { status, groupId, err } = await cli.createGroup({
      name: "demo",
      members,
    });
    if (status !== 0 || !groupId) {
      pushSys(`建群失败 ${status} ${err?.message ?? ""}`);
      return;
    }
    destEl.value = groupId;
    kindEl.value = "group";
    groupIds.add(groupId);
    remember(groupId);
    pushSys(`已建群 ${groupId}`, groupId);
    renderThreads();
  })();
});

hintEl.innerHTML =
  `先起网关。本页 <a href="?acc=alice&dest=bob">alice</a>，另开 <a href="?acc=bob&dest=alice">bob</a>。`;

setConnected(false);
renderThreads();
