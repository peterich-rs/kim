// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get brand => 'KIM';

  @override
  String get brandSub => '即时通讯';

  @override
  String get brandPitch => '即时通讯';

  @override
  String get loginTitle => '登录';

  @override
  String get registerTitle => '注册账号';

  @override
  String get account => '账号';

  @override
  String get password => '密码';

  @override
  String get confirmPassword => '确认密码';

  @override
  String get accountPlaceholder => '账号';

  @override
  String get passwordPlaceholder => '密码';

  @override
  String get confirmPlaceholder => '确认密码';

  @override
  String get accountHint => '3–32 位字母、数字或下划线';

  @override
  String get passwordHint => '8–128 位';

  @override
  String get showPassword => '显示密码';

  @override
  String get hidePassword => '隐藏密码';

  @override
  String get loginAction => '登录';

  @override
  String get registerAction => '注册';

  @override
  String get submittingLogin => '登录中…';

  @override
  String get submittingRegister => '注册中…';

  @override
  String get noAccount => '没有账号？';

  @override
  String get goRegister => '注册';

  @override
  String get hasAccount => '已有账号？';

  @override
  String get goLogin => '去登录';

  @override
  String get mismatch => '两次输入的密码不一致';

  @override
  String get invalidAccount => '账号需为 3–32 位字母、数字或下划线';

  @override
  String get invalidPassword => '密码需为 8–128 位';

  @override
  String get badCredentials => '账号或密码错误';

  @override
  String get accountExists => '账号已存在';

  @override
  String get network => '网络异常，请稍后重试';

  @override
  String get unavailable => '服务暂时不可用，请稍后重试';

  @override
  String get authFailed => '登录失败，请稍后重试';

  @override
  String get insecureAuthOrigin => '生产环境请使用 HTTPS（本地可用 http://127.0.0.1）';

  @override
  String get sessionExpired => '登录已过期，请重新登录';

  @override
  String get sessionPersistFailed => '登录成功但无法保存会话，请检查 Keychain 权限';

  @override
  String get timeout => '连接超时，请稍后重试';

  @override
  String get required => '请填写完整信息';

  @override
  String get conversations => '消息';

  @override
  String get contacts => '通讯录';

  @override
  String get me => '我';

  @override
  String get searchPlaceholder => '搜索';

  @override
  String get searchChats => '搜索会话';

  @override
  String get newChat => '发起聊天';

  @override
  String get startChat => '开始聊天';

  @override
  String get noConversations => '还没有会话';

  @override
  String get noConversationsHint => '添加好友后，从通讯录开始聊天';

  @override
  String get noMatch => '没有匹配的会话';

  @override
  String get noMessages => '暂无消息';

  @override
  String get noMessagesHint => '发一条消息，开始对话';

  @override
  String get messagePlaceholder => '发消息';

  @override
  String get send => '发送';

  @override
  String get sendFailed => '发送失败';

  @override
  String get album => '相册';

  @override
  String get camera => '拍摄';

  @override
  String get imageMessage => '[图片]';

  @override
  String get videoMessage => '[视频]';

  @override
  String get viewImage => '查看图片';

  @override
  String get closeViewer => '关闭';

  @override
  String get imageFailed => '图片发送失败';

  @override
  String get imageTooLarge => '图片不能超过 5MB';

  @override
  String get imageUnsupported => '仅支持 JPEG、PNG、WebP、GIF';

  @override
  String get mediaFailed => '无法打开相机或相册';

  @override
  String get mediaPermission => '需要相机或相册权限才能继续';

  @override
  String get plusPanel => '更多';

  @override
  String get more => '更多';

  @override
  String get notConnected => '尚未连接，请稍后重试';

  @override
  String get notFriends => '对方还不是你好友';

  @override
  String get botSocialDenied => '助手已是好友，不能再发申请';

  @override
  String get blocked => '无法与该用户互动';

  @override
  String get userNotFound => '找不到该用户';

  @override
  String get cannotAddSelf => '不能添加自己';

  @override
  String get waitingAccept => '已发送申请，通过后即可聊天';

  @override
  String get addFriendToChat => '加为好友后即可发送消息';

  @override
  String get requestSent => '已发送好友申请';

  @override
  String get friendAccepted => '已成为好友';

  @override
  String get addFriend => '添加好友';

  @override
  String get incoming => '新的朋友';

  @override
  String get accept => '同意';

  @override
  String get reject => '拒绝';

  @override
  String get requested => '已申请';

  @override
  String get chatAction => '发消息';

  @override
  String get searchPeople => '搜索账号或昵称';

  @override
  String get searchEmpty => '没有找到相关用户';

  @override
  String get noIncoming => '暂无好友申请';

  @override
  String get friendRequestToast => '发来好友申请';

  @override
  String get retry => '重试';

  @override
  String get delete => '删除';

  @override
  String get cancel => '取消';

  @override
  String get copy => '复制';

  @override
  String get readReceipt => '已读';

  @override
  String get copied => '已复制';

  @override
  String get quote => '引用';

  @override
  String get unreadBelow => '以下未读';

  @override
  String nNewMessages(int count) {
    return '$count 条新消息';
  }

  @override
  String get peerAccount => '对方账号';

  @override
  String get peerPlaceholder => '输入对方账号';

  @override
  String get cannotChatSelf => '不能选择自己的账号';

  @override
  String get openChat => '开始聊天';

  @override
  String get privateChat => '私聊';

  @override
  String get groupChat => '群聊';

  @override
  String get you => '你';

  @override
  String get back => '返回';

  @override
  String get online => '在线';

  @override
  String get connecting => '连接中';

  @override
  String get reconnecting => '重连中';

  @override
  String get offline => '未连接';

  @override
  String get kicked => '账号已在其他设备登录';

  @override
  String get logout => '退出登录';

  @override
  String get loggingOut => '退出中…';

  @override
  String get yesterday => '昨天';

  @override
  String get today => '今天';

  @override
  String get offlineBanner => '当前无网络，消息将在恢复后发送';

  @override
  String get loopbackUnreachable => '真机连不上 127.0.0.1，请切到「生产」或改成电脑的局域网 IP';

  @override
  String get profile => '个人资料';

  @override
  String get changeAvatar => '更换头像';

  @override
  String get takePhoto => '拍照';

  @override
  String get pickFromAlbum => '从相册选择';

  @override
  String get avatarUpdated => '头像已更新';

  @override
  String get avatarFailed => '头像更新失败';

  @override
  String get avatarRelogin => '请重新登录后再换头像';

  @override
  String get avatarExportFailed => '无法读取这张照片，请换一张再试';

  @override
  String get avatarUnsupportedType => '仅支持 JPEG、PNG、WebP、GIF';

  @override
  String get uploading => '上传中…';

  @override
  String get changePassword => '修改密码';

  @override
  String get oldPassword => '当前密码';

  @override
  String get newPassword => '新密码';

  @override
  String get passwordChanged => '密码已更新';

  @override
  String get save => '保存';

  @override
  String get localServer => '本地';

  @override
  String get prodServer => '正式';

  @override
  String get server => '服务器';

  @override
  String get about => '关于';

  @override
  String get version => '版本';

  @override
  String get signedInAs => '当前账号';

  @override
  String get connection => '连接状态';

  @override
  String get environment => '运行环境';

  @override
  String get accountSection => '账号';

  @override
  String get generalSection => '通用';

  @override
  String get noFriends => '还没有好友';

  @override
  String get noFriendsHint => '搜索账号，发送好友申请';

  @override
  String get addByAccount => '添加好友';

  @override
  String get recentContacts => '好友';

  @override
  String get agentLocalSection => '本地 Agent';

  @override
  String get agentLocalSubtitle => '本机 Goose · 不经过服务器';

  @override
  String get agentLocalOnly => '这是本机助手，不会发到服务器';

  @override
  String get agentComposerDirect => '给助手发消息';

  @override
  String get agentSettings => 'Agent 设置';

  @override
  String get agentMode => '协议';

  @override
  String get agentProvider => 'Goose Provider';

  @override
  String get agentProviderOpenAi => 'OpenAI';

  @override
  String get agentProviderAnthropic => 'Anthropic';

  @override
  String get agentBaseUrl => 'Base URL';

  @override
  String get agentModel => '模型';

  @override
  String get agentApiKey => 'API Key';

  @override
  String get agentApiKeyHint => '保存在系统安全存储，不会写入普通偏好';

  @override
  String get agentSaved => 'Agent 设置已保存';

  @override
  String get agentKeyMissing => '需要 API Key';

  @override
  String get agentGooseHint => '通讯录和会话列表里的「助手」是本机 Goose。点开即可对话。其它会话里也可以 @助手。';

  @override
  String get agentProviderCompatible => 'OpenAI 兼容';

  @override
  String get agentFetchModels => '拉取模型';

  @override
  String get agentReasoning => '推理强度';

  @override
  String get agentReasoningOff => '关';

  @override
  String get agentReasoningLow => '低';

  @override
  String get agentReasoningMedium => '中';

  @override
  String get agentReasoningHigh => '高';

  @override
  String get agentReasoningMax => '最高';

  @override
  String get agentFsLater => '工作区文件（后续版本）';

  @override
  String get agentFsReadonly => '工作区只读文件';

  @override
  String get agentBashLater => '命令行（后续版本）';

  @override
  String get agentBashDanger => '命令行（危险，默认关闭）';

  @override
  String get agentMcp => 'MCP 扩展';

  @override
  String get agentMcpHint => '高级。每行：名称 命令 参数… 默认空。工具必须确认后才执行。';

  @override
  String get agentAllow => '允许';

  @override
  String get agentAlwaysAllow => '总是允许';

  @override
  String get agentDeny => '拒绝';

  @override
  String get agentPermissions => '工具权限';

  @override
  String get agentPermissionAlways => '总是';

  @override
  String get agentPermissionAsk => '询问';

  @override
  String get agentPermissionNever => '禁止';

  @override
  String get agentToolSendMessage => '代发消息';

  @override
  String get agentToolClipboard => '剪贴板';

  @override
  String get agentToolSearchContacts => '搜索联系人';

  @override
  String get agentToolSearchMessages => '搜索消息';

  @override
  String get agentMoreComing => '更多 Agent（即将推出）';

  @override
  String get agentMultiProfile => '在通讯录展示多个本地 Agent';

  @override
  String get agentServerIdentity => '同步助手会话到其它设备';

  @override
  String get agentServerIdentityHint =>
      '打开后桌面一上线就会向服务器注册。设置里出现 b_ 账号即成功；之后发的消息才会进手机。本机旧历史不会上传。';

  @override
  String get agentRegisterFailed =>
      '无法在服务器注册助手。若刚部署，确认 Chat 已滚动到含 chat.bot 的镜像。';

  @override
  String get agentDuplicate => '复制';

  @override
  String get agentDelete => '删除';

  @override
  String get agentNeedsEnv => '桌面/需环境变量';

  @override
  String get agentComposerHint => '发消息，或 @助手';

  @override
  String get emptyChatTitle => '选择一个会话';

  @override
  String get emptyChatSubtitle => '从左侧列表打开聊天，或发起新会话';

  @override
  String get routeNotFound => '页面不存在';

  @override
  String get goHome => '回到消息';
}
