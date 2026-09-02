import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { changePassword as postPassword, login, logout, register } from "../lib/auth.ts";
import { ChatSession } from "../lib/chat.ts";
import { mapUserError } from "../lib/errors.ts";
import { sendTimeMs } from "../lib/format.ts";
import {
  ACCEPT_IMAGE,
  encodeImageExtra,
  isImageMessage,
  MAX_IMAGE_BYTES,
  parseImageExtra,
  previewBody,
  readImageSize,
} from "../lib/image.ts";
import { InboxKind, MessageType } from "../../src/command.ts";
import { gatewayUrl } from "../lib/gateway.ts";
import { uploadImage } from "../../src/media.ts";
import { Content, type Message } from "../../src/index.ts";
import { clearSession, loadSession, saveSession, type StoredSession } from "../lib/session.ts";
import {
  loadThreads,
  saveThreads,
  type Kind,
  type Thread,
} from "../lib/threads.ts";

export type ConnStatus = "connecting" | "online" | "reconnecting" | "offline";
export type MsgStatus = "sending" | "sent" | "failed";

export interface Person {
  account: string;
  nickname: string;
}

export interface ChatMsg {
  key: string;
  dest: string;
  sender: string;
  body: string;
  at: number;
  sys: boolean;
  type: number;
  extra: string;
  width: number;
  height: number;
  status: MsgStatus;
}

const MAX_MESSAGES = 800;
const HISTORY_PAGE = 50;

interface ChatState {
  status: ConnStatus;
  threads: Thread[];
  messages: Record<string, ChatMsg[]>;
  activeId: string | null;
  members: string[];
  membersOpen: boolean;
  connectError: string | null;
  hasMore: Record<string, boolean>;
  loadingOlder: Record<string, boolean>;
}

type Action =
  | { type: "reset" }
  | { type: "hydrate"; threads: Thread[] }
  | { type: "status"; status: ConnStatus }
  | { type: "connectError"; error: string | null }
  | {
      type: "upsertThread";
      thread: Partial<Thread> & { id: string; kind: Kind };
    }
  | { type: "message"; dest: string; msg: ChatMsg; kind: Kind; title?: string; fromSelf: boolean }
  | { type: "active"; id: string | null }
  | { type: "members"; members: string[] }
  | { type: "membersOpen"; open: boolean }
  | {
      type: "hydrateThread";
      dest: string;
      kind: Kind;
      title?: string;
      msgs: ChatMsg[];
      lastBody: string;
      lastAt: number;
      hasMore: boolean;
    }
  | { type: "prepend"; dest: string; msgs: ChatMsg[]; hasMore: boolean }
  | { type: "replaceMessage"; dest: string; key: string; msg: ChatMsg }
  | { type: "patchMessage"; dest: string; key: string; patch: Partial<ChatMsg> }
  | { type: "mute"; id: string; muted: boolean }
  | { type: "loadingOlder"; dest: string; loading: boolean }
  | { type: "deleteMessage"; dest: string; key: string };

const empty: ChatState = {
  status: "offline",
  threads: [],
  messages: {},
  activeId: null,
  members: [],
  membersOpen: false,
  connectError: null,
  hasMore: {},
  loadingOlder: {},
};

function upsertThread(list: Thread[], patch: Partial<Thread> & { id: string; kind: Kind }): Thread[] {
  const idx = list.findIndex((t) => t.id === patch.id);
  const prev: Thread =
    idx >= 0
      ? list[idx]!
      : {
          id: patch.id,
          kind: patch.kind,
          title: patch.title ?? patch.id,
          lastBody: "",
          lastAt: 0,
          unread: 0,
          muted: false,
        };
  const next: Thread = {
    ...prev,
    ...patch,
    title: patch.title ?? prev.title,
    lastBody: patch.lastBody ?? prev.lastBody,
    lastAt: patch.lastAt ?? prev.lastAt,
    unread: patch.unread ?? prev.unread,
    muted: patch.muted ?? prev.muted,
  };
  const rest = idx >= 0 ? list.filter((_, i) => i !== idx) : list;
  return [next, ...rest].sort((a, b) => b.lastAt - a.lastAt || a.title.localeCompare(b.title, "zh-CN"));
}

function mergeByKey(prev: ChatMsg[], extra: ChatMsg[]): ChatMsg[] {
  const map = new Map<string, ChatMsg>();
  for (const m of extra) {
    map.set(m.key, m);
  }
  for (const m of prev) {
    if (!map.has(m.key)) {
      map.set(m.key, m);
    }
  }
  return [...map.values()]
    .sort((a, b) => a.at - b.at || a.key.localeCompare(b.key))
    .slice(-MAX_MESSAGES);
}

function reducer(state: ChatState, action: Action): ChatState {
  switch (action.type) {
    case "reset":
      return empty;
    case "hydrate":
      return { ...empty, threads: action.threads, status: "connecting" };
    case "status":
      return { ...state, status: action.status, connectError: action.status === "online" ? null : state.connectError };
    case "connectError":
      return { ...state, connectError: action.error };
    case "upsertThread":
      return { ...state, threads: upsertThread(state.threads, action.thread) };
    case "message": {
      const prev = state.messages[action.dest] ?? [];
      if (prev.some((m) => m.key === action.msg.key)) {
        return state;
      }
      const clipped = [...prev, action.msg].slice(-MAX_MESSAGES);
      const active = state.activeId === action.dest;
      const unreadDelta = !action.fromSelf && !action.msg.sys && !active ? 1 : 0;
      const existing = state.threads.find((t) => t.id === action.dest);
      return {
        ...state,
        messages: { ...state.messages, [action.dest]: clipped },
        threads: upsertThread(state.threads, {
          id: action.dest,
          kind: action.kind,
          title: action.title ?? existing?.title,
          lastBody: action.msg.sys
            ? (existing?.lastBody ?? "")
            : previewBody(action.msg.type, action.msg.body, action.msg.extra),
          lastAt: action.msg.at,
          unread: (existing?.unread ?? 0) + unreadDelta,
        }),
      };
    }
    case "active": {
      if (!action.id) {
        return { ...state, activeId: null, members: [], membersOpen: false };
      }
      return {
        ...state,
        activeId: action.id,
        threads: state.threads.map((t) => (t.id === action.id ? { ...t, unread: 0 } : t)),
      };
    }
    case "members":
      return { ...state, members: action.members };
    case "membersOpen":
      return { ...state, membersOpen: action.open };
    case "hydrateThread": {
      const existing = state.threads.find((t) => t.id === action.dest);
      const prev = state.messages[action.dest] ?? [];
      const merged = mergeByKey(prev, action.msgs);
      const last = merged[merged.length - 1];
      return {
        ...state,
        messages: { ...state.messages, [action.dest]: merged },
        hasMore: { ...state.hasMore, [action.dest]: action.hasMore },
        threads: upsertThread(state.threads, {
          id: action.dest,
          kind: action.kind,
          title: action.title ?? existing?.title,
          lastBody:
            action.lastBody ||
            (last && !last.sys ? previewBody(last.type, last.body, last.extra) : existing?.lastBody),
          lastAt: Math.max(action.lastAt, last?.at ?? 0, existing?.lastAt ?? 0),
          unread: 0,
        }),
      };
    }
    case "prepend": {
      const prev = state.messages[action.dest] ?? [];
      const seen = new Set(prev.map((m) => m.key));
      const extra = action.msgs.filter((m) => !seen.has(m.key));
      return {
        ...state,
        messages: {
          ...state.messages,
          [action.dest]: [...extra, ...prev].slice(0, MAX_MESSAGES),
        },
        hasMore: { ...state.hasMore, [action.dest]: action.hasMore },
      };
    }
    case "replaceMessage": {
      const prev = state.messages[action.dest] ?? [];
      if (!prev.some((m) => m.key === action.key)) {
        return {
          ...state,
          messages: { ...state.messages, [action.dest]: [...prev, action.msg].slice(-MAX_MESSAGES) },
        };
      }
      return {
        ...state,
        messages: {
          ...state.messages,
          [action.dest]: prev.map((m) => (m.key === action.key ? action.msg : m)),
        },
      };
    }
    case "patchMessage": {
      const prev = state.messages[action.dest] ?? [];
      return {
        ...state,
        messages: {
          ...state.messages,
          [action.dest]: prev.map((m) => (m.key === action.key ? { ...m, ...action.patch } : m)),
        },
      };
    }
    case "mute":
      return {
        ...state,
        threads: state.threads.map((t) => (t.id === action.id ? { ...t, muted: action.muted } : t)),
      };
    case "loadingOlder":
      return {
        ...state,
        loadingOlder: { ...state.loadingOlder, [action.dest]: action.loading },
      };
    case "deleteMessage": {
      const prev = state.messages[action.dest] ?? [];
      return {
        ...state,
        messages: {
          ...state.messages,
          [action.dest]: prev.filter((m) => m.key !== action.key),
        },
      };
    }
    default:
      return state;
  }
}

function toChatMsg(msg: Message, dest: string, me: string): ChatMsg {
  const extra = msg.extra ?? "";
  const type = msg.type || (isImageMessage(0, msg.body, extra) ? MessageType.Image : MessageType.Text);
  const size = parseImageExtra(extra);
  return {
    key: msg.messageId === 0n ? `local-${msg.arrivalTime}-${msg.body}` : msg.messageId.toString(),
    dest,
    sender: msg.sender || me,
    body: msg.body,
    at: sendTimeMs(msg.sendTime, msg.arrivalTime),
    sys: false,
    type,
    extra,
    width: size?.w ?? 0,
    height: size?.h ?? 0,
    status: "sent",
  };
}

interface ChatContextValue {
  account: string | undefined;
  status: ConnStatus;
  threads: Thread[];
  messages: Record<string, ChatMsg[]>;
  activeId: string | null;
  active: Thread | undefined;
  members: string[];
  membersOpen: boolean;
  connectError: string | null;
  inboxReady: boolean;
  kicked: boolean;
  hasMore: Record<string, boolean>;
  loadingOlder: Record<string, boolean>;
  signIn: (mode: "login" | "register", account: string, password: string) => Promise<void>;
  signOut: (notice?: string) => Promise<void>;
  dismissKick: () => void;
  connect: () => Promise<void>;
  openThread: (id: string, kind: Kind, title?: string) => void;
  closeThread: () => void;
  send: (text: string) => Promise<void>;
  sendImage: (file: File) => Promise<void>;
  retryMessage: (key: string) => Promise<void>;
  loadOlder: (dest: string) => Promise<number>;
  muteThread: (id: string, muted: boolean) => void;
  createGroup: (name: string, members: string[]) => Promise<void>;
  toggleMembers: () => void;
  nickname: string;
  incomingCount: number;
  people: Person[];
  incomingPeople: Person[];
  outgoing: string[];
  socialReady: boolean;
  isFriend: (account: string) => boolean;
  isOutgoing: (account: string) => boolean;
  isIncoming: (account: string) => boolean;
  refreshSocial: () => Promise<void>;
  searchUsers: (query: string) => Promise<Person[]>;
  friends: () => Promise<Person[]>;
  incoming: () => Promise<Person[]>;
  requestFriend: (account: string) => Promise<void>;
  acceptFriend: (account: string) => Promise<void>;
  rejectFriend: (account: string) => Promise<void>;
  updateProfile: (nickname: string, bio: string) => Promise<void>;
  changePassword: (oldPassword: string, newPassword: string) => Promise<void>;
}

const ChatContext = createContext<ChatContextValue | undefined>(undefined);

type PendingPayload = { kind: "text"; text: string } | { kind: "image"; file: File };

export function ChatProvider({ children }: { children: ReactNode }) {
  const initial = loadSession();
  const [auth, setAuth] = useReducer(
    (_prev: StoredSession | undefined, next: StoredSession | undefined) => next,
    initial,
  );
  const [state, dispatch] = useReducer(reducer, empty);
  const sessionRef = useRef<ChatSession | undefined>(undefined);
  const stateRef = useRef(state);
  stateRef.current = state;
  const pendingRef = useRef(new Map<string, PendingPayload>());
  const [nickname, setNickname] = useState(initial?.account ?? "");
  const [incomingCount, setIncomingCount] = useState(0);
  const [people, setPeople] = useState<Person[]>([]);
  const [incomingPeople, setIncomingPeople] = useState<Person[]>([]);
  const [outgoing, setOutgoing] = useState<string[]>([]);
  const [socialReady, setSocialReady] = useState(false);
  const [inboxReady, setInboxReady] = useState(false);
  const [kicked, setKicked] = useState(false);

  const persistAccount = auth?.account;

  useEffect(() => {
    if (!persistAccount) {
      dispatch({ type: "reset" });
      setInboxReady(false);
      return;
    }
    dispatch({ type: "hydrate", threads: loadThreads(persistAccount) });
    setInboxReady(false);
  }, [persistAccount]);

  useEffect(() => {
    if (persistAccount) {
      saveThreads(persistAccount, state.threads);
    }
  }, [persistAccount, state.threads]);

  const pushMessage = useCallback((msg: Message, dest: string, me: string) => {
    const kind: Kind = msg.group ? "group" : "user";
    dispatch({
      type: "message",
      dest,
      msg: toChatMsg(msg, dest, me),
      kind,
      fromSelf: msg.sender === me,
    });
  }, []);

  const refreshGroup = useCallback(async (groupId: string, session: ChatSession) => {
    const title = await session.groupTitle(groupId);
    if (title && session.alive) {
      dispatch({ type: "upsertThread", thread: { id: groupId, kind: "group", title } });
    }
    if (stateRef.current.activeId === groupId && session.alive) {
      const members = await session.members(groupId);
      if (session.alive) {
        dispatch({ type: "members", members });
      }
    }
  }, []);

  const signOut = useCallback(async (notice?: string) => {
    const token = loadSession()?.token;
    sessionRef.current?.dispose();
    sessionRef.current = undefined;
    if (token) {
      try {
        await logout(token);
      } catch {
        /* still clear locally */
      }
    }
    clearSession();
    setAuth(undefined);
    setNickname("");
    setIncomingCount(0);
    setPeople([]);
    setIncomingPeople([]);
    setOutgoing([]);
    setSocialReady(false);
    setInboxReady(false);
    dispatch({ type: "reset" });
    if (notice && notice !== COPY.kicked) {
      toast.error(notice);
    }
  }, []);

  const attach = useCallback(
    (account: string, ws: string, token: string): ChatSession => {
      sessionRef.current?.dispose();
      const session = new ChatSession(account, {
        onStatus: (status) => dispatch({ type: "status", status }),
        onMessage: (msg, dest) => {
          pushMessage(msg, dest, account);
          if (msg.group) {
            void refreshGroup(msg.group, session);
          }
        },
        onKick: () => {
          setKicked(true);
          void signOut(COPY.kicked);
        },
        onToken: (token, exp) => {
          const cur = loadSession();
          if (!cur) {
            return;
          }
          saveSession({ ...cur, token, exp });
        },
        onGroup: (groupId) => {
          dispatch({
            type: "upsertThread",
            thread: { id: groupId, kind: "group", title: groupId },
          });
          void refreshGroup(groupId, session);
        },
        onFriend: (from, nick) => {
          setIncomingPeople((cur) => {
            if (cur.some((p) => p.account === from)) {
              return cur;
            }
            return [{ account: from, nickname: nick || from }, ...cur];
          });
          setIncomingCount((n) => n + 1);
          toast.message(`${nick || from} ${COPY.friendRequestToast}`);
        },
      });
      sessionRef.current = session;
      void (async () => {
        try {
          await session.connect(ws, token);
        } catch (err) {
          if (!session.alive) {
            return;
          }
          dispatch({ type: "status", status: "offline" });
          dispatch({ type: "connectError", error: mapUserError(err) });
          setInboxReady(true);
          return;
        }
        if (!session.alive) {
          return;
        }
        try {
          const me = await session.me();
          if (me?.nickname) {
            setNickname(me.nickname);
          }
          const pending = await session.incoming();
          const friendRows = await session.friends();
          if (session.alive) {
            setIncomingPeople(
              pending.map((p) => ({ account: p.account, nickname: p.nickname || p.account })),
            );
            setIncomingCount(pending.length);
            setPeople(
              friendRows.map((p) => ({ account: p.account, nickname: p.nickname || p.account })),
            );
          }
          const items = await session.inbox();
          if (!session.alive) {
            return;
          }
          for (const item of items) {
            dispatch({
              type: "upsertThread",
              thread: {
                id: item.dest,
                kind: item.kind === InboxKind.Group ? "group" : "user",
                title: item.title || item.dest,
                lastBody: previewBody(0, item.lastBody),
                lastAt: sendTimeMs(item.lastSendTime),
                unread: item.unread,
              },
            });
          }
        } catch {
          /* WS is already up; social/inbox must not fail the handshake */
        } finally {
          if (session.alive) {
            setSocialReady(true);
            setInboxReady(true);
          }
        }
      })();
      return session;
    },
    [pushMessage, refreshGroup, signOut],
  );

  useEffect(() => {
    if (!auth) {
      sessionRef.current?.dispose();
      sessionRef.current = undefined;
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled) {
        return;
      }
      attach(auth.account, gatewayUrl(), auth.token);
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      sessionRef.current?.dispose();
      sessionRef.current = undefined;
    };
  }, [auth, attach]);

  const signIn = useCallback(async (mode: "login" | "register", account: string, password: string) => {
    const body = mode === "register" ? await register(account, password) : await login(account, password);
    const stored: StoredSession = { ...body, ws: gatewayUrl() };
    saveSession(stored);
    setKicked(false);
    setAuth(stored);
  }, []);

  const connect = useCallback(async () => {
    if (!auth) {
      return;
    }
    dispatch({ type: "connectError", error: null });
    attach(auth.account, gatewayUrl(), auth.token);
  }, [auth, attach]);

  const dismissKick = useCallback(() => {
    setKicked(false);
  }, []);

  const openThread = useCallback((id: string, kind: Kind, title?: string) => {
    dispatch({ type: "upsertThread", thread: { id, kind, title: title ?? id } });
    dispatch({ type: "active", id });
    const session = sessionRef.current;
    if (kind === "group" && session) {
      dispatch({ type: "membersOpen", open: true });
      void (async () => {
        const members = await session.members(id);
        if (session.alive) {
          dispatch({ type: "members", members });
        }
        await refreshGroup(id, session);
      })();
    } else {
      dispatch({ type: "members", members: [] });
      dispatch({ type: "membersOpen", open: false });
    }
    if (session) {
      void (async () => {
        const rows = await session.history(id, kind);
        if (!session.alive) {
          return;
        }
        const msgs = rows
          .slice()
          .reverse()
          .map((msg) => toChatMsg(msg, id, session.account));
        const last = msgs[msgs.length - 1];
        dispatch({
          type: "hydrateThread",
          dest: id,
          kind,
          title,
          msgs,
          lastBody: last && !last.sys ? previewBody(last.type, last.body, last.extra) : "",
          lastAt: last?.at ?? 0,
          hasMore: rows.length >= HISTORY_PAGE,
        });
        const newest = rows[0];
        if (newest) {
          await session.markRead(id, kind, newest.messageId);
        }
      })();
    }
  }, [refreshGroup]);

  const closeThread = useCallback(() => {
    dispatch({ type: "active", id: null });
  }, []);

  const localKey = () => `local-${Date.now()}-${Math.random().toString(16).slice(2)}`;

  const send = useCallback(async (text: string) => {
    const session = sessionRef.current;
    const active = stateRef.current.threads.find((t) => t.id === stateRef.current.activeId);
    if (!session || !active) {
      throw new Error(COPY.notConnected);
    }
    if (
      active.kind === "user" &&
      socialReady &&
      !people.some((p) => p.account === active.id)
    ) {
      throw new Error(COPY.notFriends);
    }
    const key = localKey();
    const optimistic: ChatMsg = {
      key,
      dest: active.id,
      sender: session.account,
      body: text,
      at: Date.now(),
      sys: false,
      type: MessageType.Text,
      extra: "",
      width: 0,
      height: 0,
      status: "sending",
    };
    pendingRef.current.set(key, { kind: "text", text });
    dispatch({ type: "message", dest: active.id, msg: optimistic, kind: active.kind, fromSelf: true });
    try {
      const msg = await session.send(active.id, active.kind, text);
      const real = toChatMsg(msg, active.id, session.account);
      dispatch({ type: "replaceMessage", dest: active.id, key, msg: real });
      pendingRef.current.delete(key);
    } catch (err) {
      dispatch({ type: "patchMessage", dest: active.id, key, patch: { status: "failed" } });
      throw err;
    }
  }, [people, socialReady]);

  const sendImage = useCallback(async (file: File) => {
    const session = sessionRef.current;
    const active = stateRef.current.threads.find((t) => t.id === stateRef.current.activeId);
    const token = loadSession()?.token;
    if (!session || !active || !token) {
      throw new Error(COPY.notConnected);
    }
    if (
      active.kind === "user" &&
      socialReady &&
      !people.some((p) => p.account === active.id)
    ) {
      throw new Error(COPY.notFriends);
    }
    const allowed = ACCEPT_IMAGE.split(",");
    if (file.type && !allowed.includes(file.type) && file.type !== "image/jpg") {
      throw new Error(COPY.imageUnsupported);
    }
    if (file.size > MAX_IMAGE_BYTES) {
      throw new Error(COPY.imageTooLarge);
    }
    const key = localKey();
    const preview = URL.createObjectURL(file);
    const optimistic: ChatMsg = {
      key,
      dest: active.id,
      sender: session.account,
      body: preview,
      at: Date.now(),
      sys: false,
      type: MessageType.Image,
      extra: "",
      width: 0,
      height: 0,
      status: "sending",
    };
    pendingRef.current.set(key, { kind: "image", file });
    dispatch({ type: "message", dest: active.id, msg: optimistic, kind: active.kind, fromSelf: true });
    try {
      const [uploaded, size] = await Promise.all([
        uploadImage(token, file, { contentType: file.type || "image/jpeg" }),
        readImageSize(file).catch(() => ({ w: 0, h: 0 })),
      ]);
      const extra = encodeImageExtra(size.w, size.h);
      const msg = await session.sendContent(
        active.id,
        active.kind,
        new Content(uploaded.url, MessageType.Image, extra),
      );
      const real = toChatMsg(msg, active.id, session.account);
      dispatch({ type: "replaceMessage", dest: active.id, key, msg: real });
      pendingRef.current.delete(key);
      URL.revokeObjectURL(preview);
    } catch (err) {
      dispatch({ type: "patchMessage", dest: active.id, key, patch: { status: "failed" } });
      throw err;
    }
  }, [people, socialReady]);

  const retryMessage = useCallback(async (key: string) => {
    const dest = stateRef.current.activeId;
    if (!dest) {
      return;
    }
    const pending = pendingRef.current.get(key);
    if (!pending) {
      return;
    }
    dispatch({ type: "patchMessage", dest, key, patch: { status: "sending" } });
    try {
      if (pending.kind === "text") {
        const session = sessionRef.current;
        const active = stateRef.current.threads.find((t) => t.id === dest);
        if (!session || !active) {
          throw new Error(COPY.notConnected);
        }
        const msg = await session.send(active.id, active.kind, pending.text);
        const real = toChatMsg(msg, active.id, session.account);
        dispatch({ type: "replaceMessage", dest, key, msg: real });
        pendingRef.current.delete(key);
      } else {
        dispatch({ type: "deleteMessage", dest, key });
        pendingRef.current.delete(key);
        await sendImage(pending.file);
      }
    } catch (err) {
      dispatch({ type: "patchMessage", dest, key, patch: { status: "failed" } });
      throw err;
    }
  }, [sendImage]);

  const loadOlder = useCallback(async (dest: string): Promise<number> => {
    const session = sessionRef.current;
    const thread = stateRef.current.threads.find((t) => t.id === dest);
    if (!session || !thread) {
      return 0;
    }
    if (stateRef.current.hasMore[dest] === false) {
      return 0;
    }
    if (stateRef.current.loadingOlder[dest]) {
      return 0;
    }
    dispatch({ type: "loadingOlder", dest, loading: true });
    const msgs = stateRef.current.messages[dest] ?? [];
    const oldest = msgs.find((m) => /^\d+$/.test(m.key));
    const beforeId = oldest ? BigInt(oldest.key) : 0n;
    try {
      const rows = await session.history(dest, thread.kind, beforeId);
      if (!session.alive) {
        return 0;
      }
      const mapped = rows
        .slice()
        .reverse()
        .map((msg) => toChatMsg(msg, dest, session.account));
      const seen = new Set(msgs.map((m) => m.key));
      const fresh = mapped.filter((m) => !seen.has(m.key));
      dispatch({
        type: "prepend",
        dest,
        msgs: mapped,
        hasMore: rows.length >= HISTORY_PAGE,
      });
      return fresh.length;
    } finally {
      dispatch({ type: "loadingOlder", dest, loading: false });
    }
  }, []);

  const muteThread = useCallback((id: string, muted: boolean) => {
    dispatch({ type: "mute", id, muted });
  }, []);

  const createGroup = useCallback(async (name: string, members: string[]) => {
    const session = sessionRef.current;
    if (!session) {
      throw new Error(COPY.notConnected);
    }
    const id = await session.createGroup(name, members);
    openThread(id, "group", name);
    toast.success(COPY.groupCreated);
  }, [openThread]);

  const toggleMembers = useCallback(() => {
    dispatch({ type: "membersOpen", open: !stateRef.current.membersOpen });
  }, []);

  const asPeople = (rows: { account: string; nickname?: string }[]): Person[] =>
    rows.map((p) => ({ account: p.account, nickname: p.nickname || p.account }));

  const refreshSocial = useCallback(async () => {
    const session = sessionRef.current;
    if (!session) {
      return;
    }
    const [friendRows, pending] = await Promise.all([session.friends(), session.incoming()]);
    if (!session.alive) {
      return;
    }
    const nextFriends = asPeople(friendRows);
    const nextIncoming = asPeople(pending);
    setPeople(nextFriends);
    setIncomingPeople(nextIncoming);
    setIncomingCount(nextIncoming.length);
    setOutgoing((cur) => cur.filter((id) => !nextFriends.some((p) => p.account === id)));
    setSocialReady(true);
  }, []);

  const isFriend = useCallback(
    (account: string) => people.some((p) => p.account === account),
    [people],
  );
  const isOutgoing = useCallback((account: string) => outgoing.includes(account), [outgoing]);
  const isIncoming = useCallback(
    (account: string) => incomingPeople.some((p) => p.account === account),
    [incomingPeople],
  );

  const searchUsers = useCallback(async (query: string) => {
    const session = sessionRef.current;
    if (!session) {
      return [];
    }
    const rows = await session.searchUsers(query);
    return asPeople(rows);
  }, []);

  const friends = useCallback(async () => {
    const session = sessionRef.current;
    if (!session) {
      return [];
    }
    const rows = await session.friends();
    const next = asPeople(rows);
    setPeople(next);
    return next;
  }, []);

  const incoming = useCallback(async () => {
    const session = sessionRef.current;
    if (!session) {
      return [];
    }
    const rows = await session.incoming();
    const next = asPeople(rows);
    setIncomingPeople(next);
    setIncomingCount(next.length);
    return next;
  }, []);

  const requestFriend = useCallback(async (account: string) => {
    const session = sessionRef.current;
    if (!session) {
      throw new Error(COPY.notConnected);
    }
    await session.requestFriend(account);
    const friendRows = asPeople(await session.friends());
    const pending = asPeople(await session.incoming());
    if (!session.alive) {
      return;
    }
    setPeople(friendRows);
    setIncomingPeople(pending);
    setIncomingCount(pending.length);
    if (friendRows.some((p) => p.account === account)) {
      setOutgoing((cur) => cur.filter((id) => id !== account));
      toast.success(COPY.friendAccepted);
      openThread(account, "user", account);
      return;
    }
    setOutgoing((cur) => (cur.includes(account) ? cur : [...cur, account]));
    toast.success(COPY.requestSent);
  }, [openThread]);

  const acceptFriend = useCallback(async (account: string) => {
    const session = sessionRef.current;
    if (!session) {
      throw new Error(COPY.notConnected);
    }
    await session.acceptFriend(account);
    await refreshSocial();
    toast.success(COPY.friendAccepted);
    const title = incomingPeople.find((p) => p.account === account)?.nickname ?? account;
    openThread(account, "user", title);
  }, [incomingPeople, openThread, refreshSocial]);

  const rejectFriend = useCallback(async (account: string) => {
    const session = sessionRef.current;
    if (!session) {
      throw new Error(COPY.notConnected);
    }
    await session.rejectFriend(account);
    await refreshSocial();
  }, [refreshSocial]);

  const updateProfile = useCallback(async (name: string, bio: string) => {
    const session = sessionRef.current;
    if (!session) {
      throw new Error(COPY.notConnected);
    }
    const profile = await session.updateProfile(name, bio);
    if (profile?.nickname) {
      setNickname(profile.nickname);
    }
    toast.success(COPY.saved);
  }, []);

  const changePasswordFn = useCallback(async (oldPassword: string, newPassword: string) => {
    const token = loadSession()?.token;
    if (!token) {
      throw new Error(COPY.notConnected);
    }
    await postPassword(token, oldPassword, newPassword);
    toast.success(COPY.passwordChanged);
  }, []);

  const active = state.threads.find((t) => t.id === state.activeId);

  const value = useMemo<ChatContextValue>(
    () => ({
      account: auth?.account,
      status: state.status,
      threads: state.threads,
      messages: state.messages,
      activeId: state.activeId,
      active,
      members: state.members,
      membersOpen: state.membersOpen,
      connectError: state.connectError,
      inboxReady,
      kicked,
      hasMore: state.hasMore,
      loadingOlder: state.loadingOlder,
      signIn,
      signOut,
      dismissKick,
      connect,
      openThread,
      closeThread,
      send,
      sendImage,
      retryMessage,
      loadOlder,
      muteThread,
      createGroup,
      toggleMembers,
      nickname: nickname || auth?.account || "",
      incomingCount,
      people,
      incomingPeople,
      outgoing,
      socialReady,
      isFriend,
      isOutgoing,
      isIncoming,
      refreshSocial,
      searchUsers,
      friends,
      incoming,
      requestFriend,
      acceptFriend,
      rejectFriend,
      updateProfile,
      changePassword: changePasswordFn,
    }),
    [
      auth?.account,
      state.status,
      state.threads,
      state.messages,
      state.activeId,
      state.members,
      state.membersOpen,
      state.connectError,
      state.hasMore,
      state.loadingOlder,
      inboxReady,
      kicked,
      active,
      signIn,
      signOut,
      dismissKick,
      connect,
      openThread,
      closeThread,
      send,
      sendImage,
      retryMessage,
      loadOlder,
      muteThread,
      createGroup,
      toggleMembers,
      nickname,
      incomingCount,
      people,
      incomingPeople,
      outgoing,
      socialReady,
      isFriend,
      isOutgoing,
      isIncoming,
      refreshSocial,
      searchUsers,
      friends,
      incoming,
      requestFriend,
      acceptFriend,
      rejectFriend,
      updateProfile,
      changePasswordFn,
    ],
  );

  return <ChatContext.Provider value={value}>{children}</ChatContext.Provider>;
}

export function useChat(): ChatContextValue {
  const ctx = useContext(ChatContext);
  if (!ctx) {
    throw new Error("ChatProvider missing");
  }
  return ctx;
}
