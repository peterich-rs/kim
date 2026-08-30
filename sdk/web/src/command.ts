export const Command = {
  SignIn: "login.signin",
  SignOut: "login.signout",
  Renew: "login.renew",
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
  UserProfile: "chat.user.profile",
  UserUpdate: "chat.user.update",
  UserSearch: "chat.user.search",
  FriendRequest: "chat.friend.request",
  FriendAccept: "chat.friend.accept",
  FriendReject: "chat.friend.reject",
  FriendRemove: "chat.friend.remove",
  FriendList: "chat.friend.list",
  FriendIncoming: "chat.friend.incoming",
  BlockAdd: "chat.block.add",
  BlockRemove: "chat.block.remove",
  BlockList: "chat.block.list",
  InboxList: "chat.inbox.list",
  InboxRead: "chat.inbox.read",
  History: "chat.history",
} as const;

export const InboxKind = {
  User: 0,
  Group: 1,
} as const;

export type CommandName = (typeof Command)[keyof typeof Command];

export const MessageType = {
  Text: 1,
  Image: 2,
  Voice: 3,
  Video: 4,
} as const;
