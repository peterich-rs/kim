import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  type ReactNode,
} from "react";
import { toast } from "sonner";

import { COPY } from "../copy.ts";
import { login, logout, register } from "../lib/auth.ts";
import { ChatSession } from "../lib/chat.ts";
import { mapUserError } from "../lib/errors.ts";
import { sendTimeMs, truncate } from "../lib/format.ts";
import { gatewayUrl } from "../lib/gateway.ts";
import { clearSession, loadSession, saveSession, type StoredSession } from "../lib/session.ts";
import {
  loadThreads,
  saveThreads,
  type Kind,
  type Thread,
} from "../lib/threads.ts";
import type { Message } from "../../src/index.ts";

export type ConnStatus = "connecting" | "online" | "reconnecting" | "offline";

export interface ChatMsg {
  key: string;
  dest: string;
  sender: string;
  body: string;
  at: number;
  sys: boolean;
}

const MAX_MESSAGES = 400;

interface ChatState {
  status: ConnStatus;
  threads: Thread[];
  messages: Record<string, ChatMsg[]>;
  activeId: string | null;
  members: string[];
  membersOpen: boolean;
  connectError: string | null;
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
  | { type: "membersOpen"; open: boolean };

const empty: ChatState = {
  status: "offline",
  threads: [],
  messages: {},
  activeId: null,
  members: [],
  membersOpen: false,
  connectError: null,
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
        };
  const next: Thread = {
    ...prev,
    ...patch,
    title: patch.title ?? prev.title,
    lastBody: patch.lastBody ?? prev.lastBody,
    lastAt: patch.lastAt ?? prev.lastAt,
    unread: patch.unread ?? prev.unread,
  };
  const rest = idx >= 0 ? list.filter((_, i) => i !== idx) : list;
  return [next, ...rest].sort((a, b) => b.lastAt - a.lastAt || a.title.localeCompare(b.title, "zh-CN"));
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
          lastBody: action.msg.sys ? existing?.lastBody ?? "" : truncate(action.msg.body),
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
    default:
      return state;
  }
}

function toChatMsg(msg: Message, dest: string, me: string): ChatMsg {
  return {
    key: msg.messageId === 0n ? `local-${msg.arrivalTime}-${msg.body}` : msg.messageId.toString(),
    dest,
    sender: msg.sender || me,
    body: msg.body,
    at: sendTimeMs(msg.sendTime, msg.arrivalTime),
    sys: false,
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
  signIn: (mode: "login" | "register", account: string, password: string) => Promise<void>;
  signOut: (notice?: string) => Promise<void>;
  connect: () => Promise<void>;
  openThread: (id: string, kind: Kind, title?: string) => void;
  closeThread: () => void;
  send: (text: string) => Promise<void>;
  createGroup: (name: string, members: string[]) => Promise<void>;
  toggleMembers: () => void;
}

const ChatContext = createContext<ChatContextValue | undefined>(undefined);

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

  const persistAccount = auth?.account;

  useEffect(() => {
    if (!persistAccount) {
      dispatch({ type: "reset" });
      return;
    }
    dispatch({ type: "hydrate", threads: loadThreads(persistAccount) });
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
    dispatch({ type: "reset" });
    if (notice) {
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
    setAuth(stored);
  }, []);

  const connect = useCallback(async () => {
    if (!auth) {
      return;
    }
    dispatch({ type: "connectError", error: null });
    attach(auth.account, gatewayUrl(), auth.token);
  }, [auth, attach]);

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
  }, [refreshGroup]);

  const closeThread = useCallback(() => {
    dispatch({ type: "active", id: null });
  }, []);

  const send = useCallback(async (text: string) => {
    const session = sessionRef.current;
    const active = stateRef.current.threads.find((t) => t.id === stateRef.current.activeId);
    if (!session || !active) {
      throw new Error(COPY.notConnected);
    }
    const msg = await session.send(active.id, active.kind, text);
    pushMessage(msg, active.id, session.account);
  }, [pushMessage]);

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
      signIn,
      signOut,
      connect,
      openThread,
      closeThread,
      send,
      createGroup,
      toggleMembers,
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
      active,
      signIn,
      signOut,
      connect,
      openThread,
      closeThread,
      send,
      createGroup,
      toggleMembers,
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
