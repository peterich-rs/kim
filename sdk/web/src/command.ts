export const Command = {
  SignIn: "login.signin",
  SignOut: "login.signout",
  ChatUserTalk: "chat.user.talk",
  ChatGroupTalk: "chat.group.talk",
  ChatTalkAck: "chat.talk.ack",
  OfflineIndex: "chat.offline.index",
  OfflineContent: "chat.offline.content",
  GroupCreate: "chat.group.create",
  GroupJoin: "chat.group.join",
  GroupQuit: "chat.group.quit",
  GroupDetail: "chat.group.detail",
  GroupMembers: "chat.group.members",
} as const;

export type CommandName = (typeof Command)[keyof typeof Command];

export const MessageType = {
  Text: 1,
  Image: 2,
  Voice: 3,
  Video: 4,
} as const;
