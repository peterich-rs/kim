import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('zh'),
  ];

  /// No description provided for @brand.
  ///
  /// In zh, this message translates to:
  /// **'KIM'**
  String get brand;

  /// No description provided for @brandSub.
  ///
  /// In zh, this message translates to:
  /// **'即时通讯'**
  String get brandSub;

  /// No description provided for @brandPitch.
  ///
  /// In zh, this message translates to:
  /// **'即时通讯'**
  String get brandPitch;

  /// No description provided for @loginTitle.
  ///
  /// In zh, this message translates to:
  /// **'登录'**
  String get loginTitle;

  /// No description provided for @registerTitle.
  ///
  /// In zh, this message translates to:
  /// **'注册账号'**
  String get registerTitle;

  /// No description provided for @account.
  ///
  /// In zh, this message translates to:
  /// **'账号'**
  String get account;

  /// No description provided for @password.
  ///
  /// In zh, this message translates to:
  /// **'密码'**
  String get password;

  /// No description provided for @confirmPassword.
  ///
  /// In zh, this message translates to:
  /// **'确认密码'**
  String get confirmPassword;

  /// No description provided for @accountPlaceholder.
  ///
  /// In zh, this message translates to:
  /// **'账号'**
  String get accountPlaceholder;

  /// No description provided for @passwordPlaceholder.
  ///
  /// In zh, this message translates to:
  /// **'密码'**
  String get passwordPlaceholder;

  /// No description provided for @confirmPlaceholder.
  ///
  /// In zh, this message translates to:
  /// **'确认密码'**
  String get confirmPlaceholder;

  /// No description provided for @accountHint.
  ///
  /// In zh, this message translates to:
  /// **'3–32 位字母、数字或下划线'**
  String get accountHint;

  /// No description provided for @passwordHint.
  ///
  /// In zh, this message translates to:
  /// **'8–128 位'**
  String get passwordHint;

  /// No description provided for @showPassword.
  ///
  /// In zh, this message translates to:
  /// **'显示密码'**
  String get showPassword;

  /// No description provided for @hidePassword.
  ///
  /// In zh, this message translates to:
  /// **'隐藏密码'**
  String get hidePassword;

  /// No description provided for @loginAction.
  ///
  /// In zh, this message translates to:
  /// **'登录'**
  String get loginAction;

  /// No description provided for @registerAction.
  ///
  /// In zh, this message translates to:
  /// **'注册'**
  String get registerAction;

  /// No description provided for @submittingLogin.
  ///
  /// In zh, this message translates to:
  /// **'登录中…'**
  String get submittingLogin;

  /// No description provided for @submittingRegister.
  ///
  /// In zh, this message translates to:
  /// **'注册中…'**
  String get submittingRegister;

  /// No description provided for @noAccount.
  ///
  /// In zh, this message translates to:
  /// **'没有账号？'**
  String get noAccount;

  /// No description provided for @goRegister.
  ///
  /// In zh, this message translates to:
  /// **'注册'**
  String get goRegister;

  /// No description provided for @hasAccount.
  ///
  /// In zh, this message translates to:
  /// **'已有账号？'**
  String get hasAccount;

  /// No description provided for @goLogin.
  ///
  /// In zh, this message translates to:
  /// **'去登录'**
  String get goLogin;

  /// No description provided for @mismatch.
  ///
  /// In zh, this message translates to:
  /// **'两次输入的密码不一致'**
  String get mismatch;

  /// No description provided for @invalidAccount.
  ///
  /// In zh, this message translates to:
  /// **'账号需为 3–32 位字母、数字或下划线'**
  String get invalidAccount;

  /// No description provided for @invalidPassword.
  ///
  /// In zh, this message translates to:
  /// **'密码需为 8–128 位'**
  String get invalidPassword;

  /// No description provided for @badCredentials.
  ///
  /// In zh, this message translates to:
  /// **'账号或密码错误'**
  String get badCredentials;

  /// No description provided for @accountExists.
  ///
  /// In zh, this message translates to:
  /// **'账号已存在'**
  String get accountExists;

  /// No description provided for @network.
  ///
  /// In zh, this message translates to:
  /// **'网络异常，请稍后重试'**
  String get network;

  /// No description provided for @unavailable.
  ///
  /// In zh, this message translates to:
  /// **'服务暂时不可用，请稍后重试'**
  String get unavailable;

  /// No description provided for @authFailed.
  ///
  /// In zh, this message translates to:
  /// **'登录失败，请稍后重试'**
  String get authFailed;

  /// No description provided for @insecureAuthOrigin.
  ///
  /// In zh, this message translates to:
  /// **'生产环境请使用 HTTPS（本地可用 http://127.0.0.1）'**
  String get insecureAuthOrigin;

  /// No description provided for @sessionExpired.
  ///
  /// In zh, this message translates to:
  /// **'登录已过期，请重新登录'**
  String get sessionExpired;

  /// No description provided for @sessionPersistFailed.
  ///
  /// In zh, this message translates to:
  /// **'登录成功但无法保存会话，请检查 Keychain 权限'**
  String get sessionPersistFailed;

  /// No description provided for @timeout.
  ///
  /// In zh, this message translates to:
  /// **'连接超时，请稍后重试'**
  String get timeout;

  /// No description provided for @required.
  ///
  /// In zh, this message translates to:
  /// **'请填写完整信息'**
  String get required;

  /// No description provided for @conversations.
  ///
  /// In zh, this message translates to:
  /// **'消息'**
  String get conversations;

  /// No description provided for @contacts.
  ///
  /// In zh, this message translates to:
  /// **'通讯录'**
  String get contacts;

  /// No description provided for @me.
  ///
  /// In zh, this message translates to:
  /// **'我'**
  String get me;

  /// No description provided for @searchPlaceholder.
  ///
  /// In zh, this message translates to:
  /// **'搜索'**
  String get searchPlaceholder;

  /// No description provided for @searchChats.
  ///
  /// In zh, this message translates to:
  /// **'搜索会话'**
  String get searchChats;

  /// No description provided for @newChat.
  ///
  /// In zh, this message translates to:
  /// **'发起聊天'**
  String get newChat;

  /// No description provided for @startChat.
  ///
  /// In zh, this message translates to:
  /// **'开始聊天'**
  String get startChat;

  /// No description provided for @noConversations.
  ///
  /// In zh, this message translates to:
  /// **'还没有会话'**
  String get noConversations;

  /// No description provided for @noConversationsHint.
  ///
  /// In zh, this message translates to:
  /// **'添加好友后，从通讯录开始聊天'**
  String get noConversationsHint;

  /// No description provided for @noMatch.
  ///
  /// In zh, this message translates to:
  /// **'没有匹配的会话'**
  String get noMatch;

  /// No description provided for @noMessages.
  ///
  /// In zh, this message translates to:
  /// **'暂无消息'**
  String get noMessages;

  /// No description provided for @noMessagesHint.
  ///
  /// In zh, this message translates to:
  /// **'发一条消息，开始对话'**
  String get noMessagesHint;

  /// No description provided for @messagePlaceholder.
  ///
  /// In zh, this message translates to:
  /// **'发消息'**
  String get messagePlaceholder;

  /// No description provided for @send.
  ///
  /// In zh, this message translates to:
  /// **'发送'**
  String get send;

  /// No description provided for @sendFailed.
  ///
  /// In zh, this message translates to:
  /// **'发送失败'**
  String get sendFailed;

  /// No description provided for @album.
  ///
  /// In zh, this message translates to:
  /// **'相册'**
  String get album;

  /// No description provided for @camera.
  ///
  /// In zh, this message translates to:
  /// **'拍摄'**
  String get camera;

  /// No description provided for @imageMessage.
  ///
  /// In zh, this message translates to:
  /// **'[图片]'**
  String get imageMessage;

  /// No description provided for @videoMessage.
  ///
  /// In zh, this message translates to:
  /// **'[视频]'**
  String get videoMessage;

  /// No description provided for @viewImage.
  ///
  /// In zh, this message translates to:
  /// **'查看图片'**
  String get viewImage;

  /// No description provided for @closeViewer.
  ///
  /// In zh, this message translates to:
  /// **'关闭'**
  String get closeViewer;

  /// No description provided for @imageFailed.
  ///
  /// In zh, this message translates to:
  /// **'图片发送失败'**
  String get imageFailed;

  /// No description provided for @imageTooLarge.
  ///
  /// In zh, this message translates to:
  /// **'图片不能超过 5MB'**
  String get imageTooLarge;

  /// No description provided for @imageUnsupported.
  ///
  /// In zh, this message translates to:
  /// **'仅支持 JPEG、PNG、WebP、GIF'**
  String get imageUnsupported;

  /// No description provided for @mediaFailed.
  ///
  /// In zh, this message translates to:
  /// **'无法打开相机或相册'**
  String get mediaFailed;

  /// No description provided for @mediaPermission.
  ///
  /// In zh, this message translates to:
  /// **'需要相机或相册权限才能继续'**
  String get mediaPermission;

  /// No description provided for @plusPanel.
  ///
  /// In zh, this message translates to:
  /// **'更多'**
  String get plusPanel;

  /// No description provided for @more.
  ///
  /// In zh, this message translates to:
  /// **'更多'**
  String get more;

  /// No description provided for @notConnected.
  ///
  /// In zh, this message translates to:
  /// **'尚未连接，请稍后重试'**
  String get notConnected;

  /// No description provided for @notFriends.
  ///
  /// In zh, this message translates to:
  /// **'对方还不是你好友'**
  String get notFriends;

  /// No description provided for @botSocialDenied.
  ///
  /// In zh, this message translates to:
  /// **'助手已是好友，不能再发申请'**
  String get botSocialDenied;

  /// No description provided for @blocked.
  ///
  /// In zh, this message translates to:
  /// **'无法与该用户互动'**
  String get blocked;

  /// No description provided for @userNotFound.
  ///
  /// In zh, this message translates to:
  /// **'找不到该用户'**
  String get userNotFound;

  /// No description provided for @cannotAddSelf.
  ///
  /// In zh, this message translates to:
  /// **'不能添加自己'**
  String get cannotAddSelf;

  /// No description provided for @waitingAccept.
  ///
  /// In zh, this message translates to:
  /// **'已发送申请，通过后即可聊天'**
  String get waitingAccept;

  /// No description provided for @addFriendToChat.
  ///
  /// In zh, this message translates to:
  /// **'加为好友后即可发送消息'**
  String get addFriendToChat;

  /// No description provided for @requestSent.
  ///
  /// In zh, this message translates to:
  /// **'已发送好友申请'**
  String get requestSent;

  /// No description provided for @friendAccepted.
  ///
  /// In zh, this message translates to:
  /// **'已成为好友'**
  String get friendAccepted;

  /// No description provided for @addFriend.
  ///
  /// In zh, this message translates to:
  /// **'添加好友'**
  String get addFriend;

  /// No description provided for @incoming.
  ///
  /// In zh, this message translates to:
  /// **'新的朋友'**
  String get incoming;

  /// No description provided for @accept.
  ///
  /// In zh, this message translates to:
  /// **'同意'**
  String get accept;

  /// No description provided for @reject.
  ///
  /// In zh, this message translates to:
  /// **'拒绝'**
  String get reject;

  /// No description provided for @requested.
  ///
  /// In zh, this message translates to:
  /// **'已申请'**
  String get requested;

  /// No description provided for @chatAction.
  ///
  /// In zh, this message translates to:
  /// **'发消息'**
  String get chatAction;

  /// No description provided for @searchPeople.
  ///
  /// In zh, this message translates to:
  /// **'搜索账号或昵称'**
  String get searchPeople;

  /// No description provided for @searchEmpty.
  ///
  /// In zh, this message translates to:
  /// **'没有找到相关用户'**
  String get searchEmpty;

  /// No description provided for @noIncoming.
  ///
  /// In zh, this message translates to:
  /// **'暂无好友申请'**
  String get noIncoming;

  /// No description provided for @friendRequestToast.
  ///
  /// In zh, this message translates to:
  /// **'发来好友申请'**
  String get friendRequestToast;

  /// No description provided for @retry.
  ///
  /// In zh, this message translates to:
  /// **'重试'**
  String get retry;

  /// No description provided for @delete.
  ///
  /// In zh, this message translates to:
  /// **'删除'**
  String get delete;

  /// No description provided for @cancel.
  ///
  /// In zh, this message translates to:
  /// **'取消'**
  String get cancel;

  /// No description provided for @copy.
  ///
  /// In zh, this message translates to:
  /// **'复制'**
  String get copy;

  /// No description provided for @readReceipt.
  ///
  /// In zh, this message translates to:
  /// **'已读'**
  String get readReceipt;

  /// No description provided for @copied.
  ///
  /// In zh, this message translates to:
  /// **'已复制'**
  String get copied;

  /// No description provided for @quote.
  ///
  /// In zh, this message translates to:
  /// **'引用'**
  String get quote;

  /// No description provided for @unreadBelow.
  ///
  /// In zh, this message translates to:
  /// **'以下未读'**
  String get unreadBelow;

  /// No description provided for @nNewMessages.
  ///
  /// In zh, this message translates to:
  /// **'{count} 条新消息'**
  String nNewMessages(int count);

  /// No description provided for @peerAccount.
  ///
  /// In zh, this message translates to:
  /// **'对方账号'**
  String get peerAccount;

  /// No description provided for @peerPlaceholder.
  ///
  /// In zh, this message translates to:
  /// **'输入对方账号'**
  String get peerPlaceholder;

  /// No description provided for @cannotChatSelf.
  ///
  /// In zh, this message translates to:
  /// **'不能选择自己的账号'**
  String get cannotChatSelf;

  /// No description provided for @openChat.
  ///
  /// In zh, this message translates to:
  /// **'开始聊天'**
  String get openChat;

  /// No description provided for @privateChat.
  ///
  /// In zh, this message translates to:
  /// **'私聊'**
  String get privateChat;

  /// No description provided for @groupChat.
  ///
  /// In zh, this message translates to:
  /// **'群聊'**
  String get groupChat;

  /// No description provided for @you.
  ///
  /// In zh, this message translates to:
  /// **'你'**
  String get you;

  /// No description provided for @back.
  ///
  /// In zh, this message translates to:
  /// **'返回'**
  String get back;

  /// No description provided for @online.
  ///
  /// In zh, this message translates to:
  /// **'在线'**
  String get online;

  /// No description provided for @connecting.
  ///
  /// In zh, this message translates to:
  /// **'连接中'**
  String get connecting;

  /// No description provided for @reconnecting.
  ///
  /// In zh, this message translates to:
  /// **'重连中'**
  String get reconnecting;

  /// No description provided for @offline.
  ///
  /// In zh, this message translates to:
  /// **'未连接'**
  String get offline;

  /// No description provided for @kicked.
  ///
  /// In zh, this message translates to:
  /// **'账号已在其他设备登录'**
  String get kicked;

  /// No description provided for @logout.
  ///
  /// In zh, this message translates to:
  /// **'退出登录'**
  String get logout;

  /// No description provided for @loggingOut.
  ///
  /// In zh, this message translates to:
  /// **'退出中…'**
  String get loggingOut;

  /// No description provided for @yesterday.
  ///
  /// In zh, this message translates to:
  /// **'昨天'**
  String get yesterday;

  /// No description provided for @today.
  ///
  /// In zh, this message translates to:
  /// **'今天'**
  String get today;

  /// No description provided for @offlineBanner.
  ///
  /// In zh, this message translates to:
  /// **'当前无网络，消息将在恢复后发送'**
  String get offlineBanner;

  /// No description provided for @loopbackUnreachable.
  ///
  /// In zh, this message translates to:
  /// **'真机连不上 127.0.0.1，请切到「生产」或改成电脑的局域网 IP'**
  String get loopbackUnreachable;

  /// No description provided for @profile.
  ///
  /// In zh, this message translates to:
  /// **'个人资料'**
  String get profile;

  /// No description provided for @changeAvatar.
  ///
  /// In zh, this message translates to:
  /// **'更换头像'**
  String get changeAvatar;

  /// No description provided for @takePhoto.
  ///
  /// In zh, this message translates to:
  /// **'拍照'**
  String get takePhoto;

  /// No description provided for @pickFromAlbum.
  ///
  /// In zh, this message translates to:
  /// **'从相册选择'**
  String get pickFromAlbum;

  /// No description provided for @avatarUpdated.
  ///
  /// In zh, this message translates to:
  /// **'头像已更新'**
  String get avatarUpdated;

  /// No description provided for @avatarFailed.
  ///
  /// In zh, this message translates to:
  /// **'头像更新失败'**
  String get avatarFailed;

  /// No description provided for @avatarRelogin.
  ///
  /// In zh, this message translates to:
  /// **'请重新登录后再换头像'**
  String get avatarRelogin;

  /// No description provided for @avatarExportFailed.
  ///
  /// In zh, this message translates to:
  /// **'无法读取这张照片，请换一张再试'**
  String get avatarExportFailed;

  /// No description provided for @avatarUnsupportedType.
  ///
  /// In zh, this message translates to:
  /// **'仅支持 JPEG、PNG、WebP、GIF'**
  String get avatarUnsupportedType;

  /// No description provided for @uploading.
  ///
  /// In zh, this message translates to:
  /// **'上传中…'**
  String get uploading;

  /// No description provided for @changePassword.
  ///
  /// In zh, this message translates to:
  /// **'修改密码'**
  String get changePassword;

  /// No description provided for @oldPassword.
  ///
  /// In zh, this message translates to:
  /// **'当前密码'**
  String get oldPassword;

  /// No description provided for @newPassword.
  ///
  /// In zh, this message translates to:
  /// **'新密码'**
  String get newPassword;

  /// No description provided for @passwordChanged.
  ///
  /// In zh, this message translates to:
  /// **'密码已更新'**
  String get passwordChanged;

  /// No description provided for @save.
  ///
  /// In zh, this message translates to:
  /// **'保存'**
  String get save;

  /// No description provided for @localServer.
  ///
  /// In zh, this message translates to:
  /// **'本地'**
  String get localServer;

  /// No description provided for @prodServer.
  ///
  /// In zh, this message translates to:
  /// **'正式'**
  String get prodServer;

  /// No description provided for @server.
  ///
  /// In zh, this message translates to:
  /// **'服务器'**
  String get server;

  /// No description provided for @about.
  ///
  /// In zh, this message translates to:
  /// **'关于'**
  String get about;

  /// No description provided for @version.
  ///
  /// In zh, this message translates to:
  /// **'版本'**
  String get version;

  /// No description provided for @signedInAs.
  ///
  /// In zh, this message translates to:
  /// **'当前账号'**
  String get signedInAs;

  /// No description provided for @connection.
  ///
  /// In zh, this message translates to:
  /// **'连接状态'**
  String get connection;

  /// No description provided for @environment.
  ///
  /// In zh, this message translates to:
  /// **'运行环境'**
  String get environment;

  /// No description provided for @accountSection.
  ///
  /// In zh, this message translates to:
  /// **'账号'**
  String get accountSection;

  /// No description provided for @generalSection.
  ///
  /// In zh, this message translates to:
  /// **'通用'**
  String get generalSection;

  /// No description provided for @noFriends.
  ///
  /// In zh, this message translates to:
  /// **'还没有好友'**
  String get noFriends;

  /// No description provided for @noFriendsHint.
  ///
  /// In zh, this message translates to:
  /// **'搜索账号，发送好友申请'**
  String get noFriendsHint;

  /// No description provided for @addByAccount.
  ///
  /// In zh, this message translates to:
  /// **'添加好友'**
  String get addByAccount;

  /// No description provided for @recentContacts.
  ///
  /// In zh, this message translates to:
  /// **'好友'**
  String get recentContacts;

  /// No description provided for @agentLocalSection.
  ///
  /// In zh, this message translates to:
  /// **'本地 Agent'**
  String get agentLocalSection;

  /// No description provided for @agentLocalSubtitle.
  ///
  /// In zh, this message translates to:
  /// **'本机 Goose · 不经过服务器'**
  String get agentLocalSubtitle;

  /// No description provided for @agentLocalOnly.
  ///
  /// In zh, this message translates to:
  /// **'这是本机助手，不会发到服务器'**
  String get agentLocalOnly;

  /// No description provided for @agentComposerDirect.
  ///
  /// In zh, this message translates to:
  /// **'给助手发消息'**
  String get agentComposerDirect;

  /// No description provided for @agentSettings.
  ///
  /// In zh, this message translates to:
  /// **'Agent 设置'**
  String get agentSettings;

  /// No description provided for @agentMode.
  ///
  /// In zh, this message translates to:
  /// **'协议'**
  String get agentMode;

  /// No description provided for @agentProvider.
  ///
  /// In zh, this message translates to:
  /// **'Goose Provider'**
  String get agentProvider;

  /// No description provided for @agentProviderOpenAi.
  ///
  /// In zh, this message translates to:
  /// **'OpenAI'**
  String get agentProviderOpenAi;

  /// No description provided for @agentProviderAnthropic.
  ///
  /// In zh, this message translates to:
  /// **'Anthropic'**
  String get agentProviderAnthropic;

  /// No description provided for @agentBaseUrl.
  ///
  /// In zh, this message translates to:
  /// **'Base URL'**
  String get agentBaseUrl;

  /// No description provided for @agentModel.
  ///
  /// In zh, this message translates to:
  /// **'模型'**
  String get agentModel;

  /// No description provided for @agentApiKey.
  ///
  /// In zh, this message translates to:
  /// **'API Key'**
  String get agentApiKey;

  /// No description provided for @agentApiKeyHint.
  ///
  /// In zh, this message translates to:
  /// **'保存在系统安全存储，不会写入普通偏好'**
  String get agentApiKeyHint;

  /// No description provided for @agentSaved.
  ///
  /// In zh, this message translates to:
  /// **'Agent 设置已保存'**
  String get agentSaved;

  /// No description provided for @agentKeyMissing.
  ///
  /// In zh, this message translates to:
  /// **'需要 API Key'**
  String get agentKeyMissing;

  /// No description provided for @agentGooseHint.
  ///
  /// In zh, this message translates to:
  /// **'通讯录和会话列表里的「助手」是本机 Goose。点开即可对话。其它会话里也可以 @助手。'**
  String get agentGooseHint;

  /// No description provided for @agentProviderCompatible.
  ///
  /// In zh, this message translates to:
  /// **'OpenAI 兼容'**
  String get agentProviderCompatible;

  /// No description provided for @agentFetchModels.
  ///
  /// In zh, this message translates to:
  /// **'拉取模型'**
  String get agentFetchModels;

  /// No description provided for @agentReasoning.
  ///
  /// In zh, this message translates to:
  /// **'推理强度'**
  String get agentReasoning;

  /// No description provided for @agentReasoningOff.
  ///
  /// In zh, this message translates to:
  /// **'关'**
  String get agentReasoningOff;

  /// No description provided for @agentReasoningLow.
  ///
  /// In zh, this message translates to:
  /// **'低'**
  String get agentReasoningLow;

  /// No description provided for @agentReasoningMedium.
  ///
  /// In zh, this message translates to:
  /// **'中'**
  String get agentReasoningMedium;

  /// No description provided for @agentReasoningHigh.
  ///
  /// In zh, this message translates to:
  /// **'高'**
  String get agentReasoningHigh;

  /// No description provided for @agentReasoningMax.
  ///
  /// In zh, this message translates to:
  /// **'最高'**
  String get agentReasoningMax;

  /// No description provided for @agentReasoningNone.
  ///
  /// In zh, this message translates to:
  /// **'无'**
  String get agentReasoningNone;

  /// No description provided for @agentReasoningAlwaysOn.
  ///
  /// In zh, this message translates to:
  /// **'始终开启'**
  String get agentReasoningAlwaysOn;

  /// No description provided for @agentReasoningMinimal.
  ///
  /// In zh, this message translates to:
  /// **'最低'**
  String get agentReasoningMinimal;

  /// No description provided for @agentReasoningXhigh.
  ///
  /// In zh, this message translates to:
  /// **'极高'**
  String get agentReasoningXhigh;

  /// No description provided for @agentReasoningDropped.
  ///
  /// In zh, this message translates to:
  /// **'已忽略不受支持的推理参数'**
  String get agentReasoningDropped;

  /// No description provided for @agentAdvancedJson.
  ///
  /// In zh, this message translates to:
  /// **'高级 JSON'**
  String get agentAdvancedJson;

  /// No description provided for @agentVendorPrimary.
  ///
  /// In zh, this message translates to:
  /// **'常用'**
  String get agentVendorPrimary;

  /// No description provided for @agentVendorGateway.
  ///
  /// In zh, this message translates to:
  /// **'网关'**
  String get agentVendorGateway;

  /// No description provided for @agentVendorOther.
  ///
  /// In zh, this message translates to:
  /// **'其他'**
  String get agentVendorOther;

  /// No description provided for @agentAltUrl.
  ///
  /// In zh, this message translates to:
  /// **'备选地址'**
  String get agentAltUrl;

  /// No description provided for @agentFsLater.
  ///
  /// In zh, this message translates to:
  /// **'工作区文件（后续版本）'**
  String get agentFsLater;

  /// No description provided for @agentFsReadonly.
  ///
  /// In zh, this message translates to:
  /// **'工作区只读文件'**
  String get agentFsReadonly;

  /// No description provided for @agentBashLater.
  ///
  /// In zh, this message translates to:
  /// **'命令行（后续版本）'**
  String get agentBashLater;

  /// No description provided for @agentBashDanger.
  ///
  /// In zh, this message translates to:
  /// **'命令行（危险，默认关闭）'**
  String get agentBashDanger;

  /// No description provided for @agentMcp.
  ///
  /// In zh, this message translates to:
  /// **'MCP 扩展'**
  String get agentMcp;

  /// No description provided for @agentMcpHint.
  ///
  /// In zh, this message translates to:
  /// **'高级。每行：名称 命令 参数… 默认空。工具必须确认后才执行。'**
  String get agentMcpHint;

  /// No description provided for @agentAllow.
  ///
  /// In zh, this message translates to:
  /// **'允许'**
  String get agentAllow;

  /// No description provided for @agentAlwaysAllow.
  ///
  /// In zh, this message translates to:
  /// **'总是允许'**
  String get agentAlwaysAllow;

  /// No description provided for @agentDeny.
  ///
  /// In zh, this message translates to:
  /// **'拒绝'**
  String get agentDeny;

  /// No description provided for @agentPermissions.
  ///
  /// In zh, this message translates to:
  /// **'工具权限'**
  String get agentPermissions;

  /// No description provided for @agentPermissionAlways.
  ///
  /// In zh, this message translates to:
  /// **'总是'**
  String get agentPermissionAlways;

  /// No description provided for @agentPermissionAsk.
  ///
  /// In zh, this message translates to:
  /// **'询问'**
  String get agentPermissionAsk;

  /// No description provided for @agentPermissionNever.
  ///
  /// In zh, this message translates to:
  /// **'禁止'**
  String get agentPermissionNever;

  /// No description provided for @agentToolSendMessage.
  ///
  /// In zh, this message translates to:
  /// **'代发消息'**
  String get agentToolSendMessage;

  /// No description provided for @agentToolClipboard.
  ///
  /// In zh, this message translates to:
  /// **'剪贴板'**
  String get agentToolClipboard;

  /// No description provided for @agentToolSearchContacts.
  ///
  /// In zh, this message translates to:
  /// **'搜索联系人'**
  String get agentToolSearchContacts;

  /// No description provided for @agentToolSearchMessages.
  ///
  /// In zh, this message translates to:
  /// **'搜索消息'**
  String get agentToolSearchMessages;

  /// No description provided for @agentMoreComing.
  ///
  /// In zh, this message translates to:
  /// **'打开「多个本地 Agent」后可管理列表'**
  String get agentMoreComing;

  /// No description provided for @agentMultiProfile.
  ///
  /// In zh, this message translates to:
  /// **'在通讯录展示多个本地 Agent'**
  String get agentMultiProfile;

  /// No description provided for @agentServerIdentity.
  ///
  /// In zh, this message translates to:
  /// **'同步助手会话到其它设备'**
  String get agentServerIdentity;

  /// No description provided for @agentServerIdentityHint.
  ///
  /// In zh, this message translates to:
  /// **'打开后桌面一上线就会向服务器注册。设置里出现 b_ 账号即成功；之后发的消息才会进手机。本机旧历史不会上传。'**
  String get agentServerIdentityHint;

  /// No description provided for @agentRegisterFailed.
  ///
  /// In zh, this message translates to:
  /// **'无法在服务器注册助手。若刚部署，确认 Chat 已滚动到含 chat.bot 的镜像。'**
  String get agentRegisterFailed;

  /// No description provided for @agentDuplicate.
  ///
  /// In zh, this message translates to:
  /// **'复制'**
  String get agentDuplicate;

  /// No description provided for @agentDelete.
  ///
  /// In zh, this message translates to:
  /// **'删除'**
  String get agentDelete;

  /// No description provided for @agentListTitle.
  ///
  /// In zh, this message translates to:
  /// **'Agent'**
  String get agentListTitle;

  /// No description provided for @agentAccounts.
  ///
  /// In zh, this message translates to:
  /// **'厂商账号'**
  String get agentAccounts;

  /// No description provided for @agentDisplayName.
  ///
  /// In zh, this message translates to:
  /// **'名称'**
  String get agentDisplayName;

  /// No description provided for @agentAliases.
  ///
  /// In zh, this message translates to:
  /// **'别名'**
  String get agentAliases;

  /// No description provided for @agentAliasesHint.
  ///
  /// In zh, this message translates to:
  /// **'逗号分隔'**
  String get agentAliasesHint;

  /// No description provided for @agentCapReached.
  ///
  /// In zh, this message translates to:
  /// **'最多 20 个已注册助手'**
  String get agentCapReached;

  /// No description provided for @agentAccountInUse.
  ///
  /// In zh, this message translates to:
  /// **'仍有 Agent 使用此账号'**
  String get agentAccountInUse;

  /// No description provided for @agentAddAccount.
  ///
  /// In zh, this message translates to:
  /// **'添加账号'**
  String get agentAddAccount;

  /// No description provided for @agentNew.
  ///
  /// In zh, this message translates to:
  /// **'新建'**
  String get agentNew;

  /// No description provided for @agentWizardBlank.
  ///
  /// In zh, this message translates to:
  /// **'空白'**
  String get agentWizardBlank;

  /// No description provided for @agentWizardTranslator.
  ///
  /// In zh, this message translates to:
  /// **'译者'**
  String get agentWizardTranslator;

  /// No description provided for @agentWizardCoder.
  ///
  /// In zh, this message translates to:
  /// **'编码'**
  String get agentWizardCoder;

  /// No description provided for @agentInvalidUrl.
  ///
  /// In zh, this message translates to:
  /// **'请使用 https 地址（本机可用 http://127.0.0.1）'**
  String get agentInvalidUrl;

  /// No description provided for @agentNeedsEnv.
  ///
  /// In zh, this message translates to:
  /// **'桌面/需环境变量'**
  String get agentNeedsEnv;

  /// No description provided for @agentComposerHint.
  ///
  /// In zh, this message translates to:
  /// **'发消息，或 @助手'**
  String get agentComposerHint;

  /// No description provided for @emptyChatTitle.
  ///
  /// In zh, this message translates to:
  /// **'选择一个会话'**
  String get emptyChatTitle;

  /// No description provided for @emptyChatSubtitle.
  ///
  /// In zh, this message translates to:
  /// **'从左侧列表打开聊天，或发起新会话'**
  String get emptyChatSubtitle;

  /// No description provided for @routeNotFound.
  ///
  /// In zh, this message translates to:
  /// **'页面不存在'**
  String get routeNotFound;

  /// No description provided for @goHome.
  ///
  /// In zh, this message translates to:
  /// **'回到消息'**
  String get goHome;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'zh'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
