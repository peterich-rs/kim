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
  String get agentLocalSubtitle => '本机运行 · 消息走会话';

  @override
  String get agentLocalOnly => '这是本机助手，不会发到服务器';

  @override
  String get agentComposerDirect => '给助手发消息';

  @override
  String get agentSettings => 'Agent 设置';

  @override
  String get agentMode => '协议';

  @override
  String get agentProvider => 'Provider';

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
  String get agentSaveFailed => '无法保存 Agent 设置';

  @override
  String get agentKeyMissing => '需要 API Key';

  @override
  String get agentGooseHint => '通讯录和会话列表里的「助手」是本机 Goose。点开即可对话。其它会话里也可以 @助手。';

  @override
  String get agentProviderCompatible => 'OpenAI 兼容';

  @override
  String get agentFetchModels => '拉取模型';

  @override
  String agentFetchModelsOk(int count) {
    return '已拉取 $count 个模型';
  }

  @override
  String agentFetchModelsFailed(String error) {
    return '拉取失败：$error';
  }

  @override
  String get agentPickModel => '从列表选择';

  @override
  String get agentModelHint => '保存以输入框为准。可手填，或拉取后从列表选入。';

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
  String get agentReasoningNone => '无';

  @override
  String get agentReasoningAlwaysOn => '始终开启';

  @override
  String get agentReasoningMinimal => '最低';

  @override
  String get agentReasoningXhigh => '极高';

  @override
  String get agentReasoningDropped => '已忽略不受支持的推理参数';

  @override
  String get agentAdvancedJson => '高级 JSON';

  @override
  String get agentVendorPrimary => '常用';

  @override
  String get agentVendorGateway => '网关';

  @override
  String get agentVendorOther => '其他';

  @override
  String get agentAltUrl => '备选地址';

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
  String get agentToolGetConversationContext => '会话上下文';

  @override
  String get agentToolListProfiles => '列出人设';

  @override
  String get agentCapabilitiesEntry => '能力';

  @override
  String get agentCapabilitiesTitle => '能力';

  @override
  String get agentCapabilitiesSubtitleEmpty => '未启用工具 · 未分配技能';

  @override
  String agentCapabilitiesSubtitleTools(int count) {
    return '$count 个工具';
  }

  @override
  String agentCapabilitiesSubtitleSkills(int count) {
    return '$count 个技能';
  }

  @override
  String get agentCapabilitiesPreview => '装配预览';

  @override
  String get agentCapabilitiesPreviewEmpty => '当前无工具（纯对话）';

  @override
  String agentCapabilitiesPreviewLocal(String tools) {
    return '主机预览不可用 · $tools';
  }

  @override
  String get agentCapabilitiesImSection => '即时通讯';

  @override
  String get agentCapabilitiesFsSection => '文件与命令行';

  @override
  String get agentCapabilitiesMcpSection => 'MCP';

  @override
  String get agentCapabilitiesMcpSave => '保存 MCP 行';

  @override
  String get agentCapabilitiesSkillsSection => '技能';

  @override
  String get agentMoreComing => '打开「多个本地 Agent」后可管理列表';

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
  String get agentListTitle => 'Agent';

  @override
  String get agentAccounts => '厂商账号';

  @override
  String get agentDisplayName => '名称';

  @override
  String get agentAliases => '别名';

  @override
  String get agentAliasesHint => '逗号分隔';

  @override
  String get agentCapReached => '最多 20 个已注册助手';

  @override
  String get agentAccountInUse => '仍有 Agent 使用此账号';

  @override
  String get agentAddAccount => '添加账号';

  @override
  String get agentNew => '新建';

  @override
  String get agentInvalidUrl => '请使用 https 地址（本机可用 http://127.0.0.1）';

  @override
  String get agentNeedsEnv => '桌面/需环境变量';

  @override
  String get agentComposerHint => '发消息，或 @助手';

  @override
  String get agentEmptyTitle => '还没有 Agent';

  @override
  String get agentEmptyHint => '创建一个助手，出现在通讯录和会话列表里';

  @override
  String get agentCreate => '创建 Agent';

  @override
  String get agentPrompt => '系统提示';

  @override
  String get agentPromptHint => '留空则使用该默认';

  @override
  String get agentNewProvider => '新建 Provider…';

  @override
  String get agentNeedProvider => '先添加一个厂商账号';

  @override
  String get agentEmptyProviders => '还没有厂商账号';

  @override
  String get agentEmptyProvidersHint => '先添加厂商和 API 密钥，再回来创建 Agent';

  @override
  String get agentKeepKeyHint => '留空则保留已保存的密钥';

  @override
  String get agentModelOther => '其他…';

  @override
  String agentModelFallback(String model) {
    return '模型不在新账号列表中，已改用 $model';
  }

  @override
  String get agentListHint => '点进通讯录里的 Agent 即可对话。密钥在厂商账号里。';

  @override
  String get agentDeletedReadOnly => '此 Agent 已删除，记录只读';

  @override
  String get agentProviderKeyMissing => '未配置 API Key。打开「我 → Agent → 厂商账号」补全密钥。';

  @override
  String get composerHint => '发消息';

  @override
  String get agentNameHint => '给 Agent 起个名字';

  @override
  String get agentAdvanced => '高级';

  @override
  String get agentWorkspaceEntry => '工作区与能力';

  @override
  String get agentWorkspaceTitle => '工作区与能力';

  @override
  String get agentWorkspaceSubtitleOff => '应用内沙箱 · 未开读写';

  @override
  String get agentWorkspaceSubtitleFs => '应用内沙箱 · 只读';

  @override
  String get agentWorkspaceSubtitleWrite => '应用内沙箱 · 读·写';

  @override
  String get agentWorkspaceSubtitleBash => '应用内沙箱 · 终端';

  @override
  String get agentWorkspaceSubtitleFsBash => '应用内沙箱 · 只读·终端';

  @override
  String get agentWorkspaceSubtitleWriteBash => '应用内沙箱 · 读·写·终端';

  @override
  String get agentWorkspaceCapsOff => '未开读写';

  @override
  String get agentWorkspaceCapsFs => '只读';

  @override
  String get agentWorkspaceCapsWrite => '读·写';

  @override
  String get agentWorkspaceCapsBash => '终端';

  @override
  String get agentWorkspaceCapsFsBash => '只读·终端';

  @override
  String get agentWorkspaceCapsWriteBash => '读·写·终端';

  @override
  String get agentWorkspacePath => '当前工作目录';

  @override
  String get agentWorkspaceKindSandbox => '应用内沙箱';

  @override
  String get agentWorkspaceKindRepo => '本地仓库';

  @override
  String get agentWorkspaceUseRepo => '使用本地仓库';

  @override
  String get agentWorkspaceUseRepoHint => '把读写与终端指向你选的文件夹（需系统授权）';

  @override
  String get agentWorkspacePickRepo => '选择本地仓库…';

  @override
  String get agentWorkspaceRepoRequired => '编码工作区需要先选择仓库';

  @override
  String get agentWorkspaceRepoReselect => '仓库授权已失效，请重新选择';

  @override
  String get agentFsWrite => '工作区写入';

  @override
  String get agentFsWriteHint => '允许创建与修改沙箱内文件（如 MEMORY.md）';

  @override
  String get agentWorkspacePresetKnowledge => '知识';

  @override
  String get agentWorkspacePresetCoding => '编码';

  @override
  String get agentWorkspacePresetHint => '预设只改本页开关，不换人设';

  @override
  String get agentSkillsEntry => '技能';

  @override
  String get agentSkillsTitle => '技能';

  @override
  String get agentSkillsNone => '未分配';

  @override
  String get agentSkillsEmptyTitle => '还没有可配置的技能';

  @override
  String get agentSkillsEmptyHint => '之后可在这里分配 KIM 技能，或从广场发现生态技能';

  @override
  String get agentSkillsOpenPlaza => '去广场看看';

  @override
  String get agentSkillsAppSection => 'KIM 技能';

  @override
  String get agentSkillsAppEmpty => '暂无内置技能';

  @override
  String get agentSkillsAppEmptyHint => '需要桌面端 Agent 运行时才能列出 kim-*';

  @override
  String get agentSkillsPortableSection => '生态技能（发现 / 屏蔽）';

  @override
  String get agentSkillsPortableEmpty => '未发现生态技能';

  @override
  String get agentSkillsPortableEmptyHint =>
      '打开读写或选用本地仓库后，会扫描 ~/.agents 与项目 .agents';

  @override
  String get agentSkillsPortableScanOff => '当前不扫描生态技能';

  @override
  String get agentSkillsPortableScanOffHint =>
      '沙箱且未开读写时，不会把 ~/.agents 里的技能塞进 catalog';

  @override
  String get agentSkillsPortableMuteHint => '打开开关 = 从 catalog 屏蔽';

  @override
  String get agentSkillsNeedsToolsTitle => '需要打开工具';

  @override
  String agentSkillsNeedsToolsBody(String skillId, String tools) {
    return '分配 $skillId 需要：$tools。不会静默改开关。';
  }

  @override
  String get agentSkillsNeedsToolsEnable => '打开这些开关';

  @override
  String get agentSkillsNeedsToolsCancel => '取消';

  @override
  String get agentSkillsNeedsToolsToast => '未打开所需工具，技能未分配';

  @override
  String get agentToolsEntry => '权限与扩展';

  @override
  String get agentToolsTitle => '权限与扩展';

  @override
  String get agentToolsSubtitleDefault => '默认 · 无 MCP';

  @override
  String agentToolsSubtitleMcp(int count) {
    return '默认 · MCP $count';
  }

  @override
  String get agentPlazaTitle => '技能广场';

  @override
  String get agentPlazaEmptyTitle => '广场即将到来';

  @override
  String get agentPlazaEmptyHint => '之后可在这里浏览生态技能与 KIM 内置技能';

  @override
  String get agentPlazaKimSection => 'KIM 架';

  @override
  String get agentPlazaEcoSection => '生态架';

  @override
  String get agentPlazaEcoEmpty => '生态架为空';

  @override
  String get agentPlazaEcoEmptyHint => '真实用户家目录 ~/.agents/skills 里尚无 SKILL.md';

  @override
  String get agentPlazaAssign => '分配';

  @override
  String agentPlazaAssigningTo(String name) {
    return '正在为人设「$name」挑选';
  }

  @override
  String get agentPlazaPickAgentFirst => '请从人设的技能页进入广场再分配';

  @override
  String agentPlazaAssigned(String id) {
    return '已分配 $id';
  }

  @override
  String get agentPlazaImport => '导入文件夹到 ~/.agents…';

  @override
  String agentPlazaImported(String id) {
    return '已导入 $id';
  }

  @override
  String get agentPlazaImportFailed => '导入失败（需要含 SKILL.md 的目录）';

  @override
  String get agentPlazaImportNoHome => '找不到真实 ~/.agents/skills';

  @override
  String get agentOpenChat => '去聊天';

  @override
  String get emptyChatTitle => '选择一个会话';

  @override
  String get emptyChatSubtitle => '从左侧列表打开聊天，或发起新会话';

  @override
  String get routeNotFound => '页面不存在';

  @override
  String get goHome => '回到消息';

  @override
  String get bio => '简介';

  @override
  String get botBadge => '助手';

  @override
  String get deletePeer => '删除';

  @override
  String get deletePeerTitle => '删除联系人？';

  @override
  String get deletePeerBody => '将解除关系，并清除本机会话。人类好友的服务端历史会保留。';

  @override
  String get peerDeleted => '已删除';
}
