// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get brand => 'KIM';

  @override
  String get brandSub => 'Messenger';

  @override
  String get brandPitch => 'Instant messaging';

  @override
  String get loginTitle => 'Sign in';

  @override
  String get registerTitle => 'Create account';

  @override
  String get account => 'Account';

  @override
  String get password => 'Password';

  @override
  String get confirmPassword => 'Confirm password';

  @override
  String get accountPlaceholder => 'Account';

  @override
  String get passwordPlaceholder => 'Password';

  @override
  String get confirmPlaceholder => 'Confirm password';

  @override
  String get accountHint => '3–32 letters, digits, or underscore';

  @override
  String get passwordHint => '8–128 characters';

  @override
  String get showPassword => 'Show password';

  @override
  String get hidePassword => 'Hide password';

  @override
  String get loginAction => 'Sign in';

  @override
  String get registerAction => 'Register';

  @override
  String get submittingLogin => 'Signing in…';

  @override
  String get submittingRegister => 'Registering…';

  @override
  String get noAccount => 'No account?';

  @override
  String get goRegister => 'Register';

  @override
  String get hasAccount => 'Already have an account?';

  @override
  String get goLogin => 'Sign in';

  @override
  String get mismatch => 'Passwords do not match';

  @override
  String get invalidAccount =>
      'Account must be 3–32 letters, digits, or underscore';

  @override
  String get invalidPassword => 'Password must be 8–128 characters';

  @override
  String get badCredentials => 'Wrong account or password';

  @override
  String get accountExists => 'Account already exists';

  @override
  String get network => 'Network error, try again later';

  @override
  String get unavailable => 'Service unavailable, try again later';

  @override
  String get authFailed => 'Sign-in failed, try again later';

  @override
  String get insecureAuthOrigin =>
      'Use HTTPS in production (http://127.0.0.1 is allowed locally)';

  @override
  String get sessionExpired => 'Session expired, please sign in again';

  @override
  String get sessionPersistFailed =>
      'Signed in but the session could not be saved. Check Keychain access.';

  @override
  String get timeout => 'Timed out, try again later';

  @override
  String get required => 'Please fill in every field';

  @override
  String get conversations => 'Chats';

  @override
  String get contacts => 'Contacts';

  @override
  String get me => 'Me';

  @override
  String get searchPlaceholder => 'Search';

  @override
  String get searchChats => 'Search chats';

  @override
  String get newChat => 'New chat';

  @override
  String get startChat => 'Start chat';

  @override
  String get noConversations => 'No conversations yet';

  @override
  String get noConversationsHint =>
      'Add a friend, then start a chat from Contacts';

  @override
  String get noMatch => 'No matching chats';

  @override
  String get noMessages => 'No messages yet';

  @override
  String get noMessagesHint => 'Send a message to start the conversation';

  @override
  String get messagePlaceholder => 'Message';

  @override
  String get send => 'Send';

  @override
  String get sendFailed => 'Send failed';

  @override
  String get album => 'Photos';

  @override
  String get camera => 'Camera';

  @override
  String get imageMessage => '[Image]';

  @override
  String get videoMessage => '[Video]';

  @override
  String get viewImage => 'View image';

  @override
  String get closeViewer => 'Close';

  @override
  String get imageFailed => 'Image failed to send';

  @override
  String get imageTooLarge => 'Images must be 5MB or smaller';

  @override
  String get imageUnsupported => 'JPEG, PNG, WebP, and GIF only';

  @override
  String get mediaFailed => 'Could not open camera or photos';

  @override
  String get mediaPermission => 'Camera or photo access is required';

  @override
  String get plusPanel => 'More';

  @override
  String get more => 'More';

  @override
  String get notConnected => 'Not connected yet, try again shortly';

  @override
  String get notFriends => 'You are not friends yet';

  @override
  String get botSocialDenied => 'The assistant is already a friend';

  @override
  String get blocked => 'You cannot interact with this user';

  @override
  String get userNotFound => 'User not found';

  @override
  String get cannotAddSelf => 'You cannot add yourself';

  @override
  String get waitingAccept => 'Request sent. You can chat after they accept.';

  @override
  String get addFriendToChat => 'Add as a friend to send messages';

  @override
  String get requestSent => 'Friend request sent';

  @override
  String get friendAccepted => 'You are now friends';

  @override
  String get addFriend => 'Add friend';

  @override
  String get incoming => 'New friends';

  @override
  String get accept => 'Accept';

  @override
  String get reject => 'Decline';

  @override
  String get requested => 'Requested';

  @override
  String get chatAction => 'Message';

  @override
  String get searchPeople => 'Search account or nickname';

  @override
  String get searchEmpty => 'No matching users';

  @override
  String get noIncoming => 'No friend requests';

  @override
  String get friendRequestToast => 'sent a friend request';

  @override
  String get retry => 'Retry';

  @override
  String get delete => 'Delete';

  @override
  String get cancel => 'Cancel';

  @override
  String get copy => 'Copy';

  @override
  String get readReceipt => 'Read';

  @override
  String get copied => 'Copied';

  @override
  String get quote => 'Quote';

  @override
  String get unreadBelow => 'Unread below';

  @override
  String nNewMessages(int count) {
    return '$count new messages';
  }

  @override
  String get peerAccount => 'Their account';

  @override
  String get peerPlaceholder => 'Enter their account';

  @override
  String get cannotChatSelf => 'You cannot choose your own account';

  @override
  String get openChat => 'Start chat';

  @override
  String get privateChat => 'Direct message';

  @override
  String get groupChat => 'Group';

  @override
  String get you => 'You';

  @override
  String get back => 'Back';

  @override
  String get online => 'Online';

  @override
  String get connecting => 'Connecting';

  @override
  String get reconnecting => 'Reconnecting';

  @override
  String get offline => 'Offline';

  @override
  String get kicked => 'Signed in on another device';

  @override
  String get logout => 'Sign out';

  @override
  String get loggingOut => 'Signing out…';

  @override
  String get yesterday => 'Yesterday';

  @override
  String get today => 'Today';

  @override
  String get offlineBanner =>
      'You\'re offline. Messages will send when you\'re back.';

  @override
  String get loopbackUnreachable =>
      'This device cannot reach 127.0.0.1. Switch to production or use your computer\'s LAN IP.';

  @override
  String get profile => 'Profile';

  @override
  String get changeAvatar => 'Change avatar';

  @override
  String get takePhoto => 'Take photo';

  @override
  String get pickFromAlbum => 'Choose from photos';

  @override
  String get avatarUpdated => 'Avatar updated';

  @override
  String get avatarFailed => 'Could not update avatar';

  @override
  String get avatarRelogin => 'Sign in again to change your avatar';

  @override
  String get avatarExportFailed => 'Could not read that photo. Try another.';

  @override
  String get avatarUnsupportedType => 'JPEG, PNG, WebP, and GIF only';

  @override
  String get uploading => 'Uploading…';

  @override
  String get changePassword => 'Change password';

  @override
  String get oldPassword => 'Current password';

  @override
  String get newPassword => 'New password';

  @override
  String get passwordChanged => 'Password updated';

  @override
  String get save => 'Save';

  @override
  String get localServer => 'Local';

  @override
  String get prodServer => 'Production';

  @override
  String get server => 'Server';

  @override
  String get about => 'About';

  @override
  String get version => 'Version';

  @override
  String get signedInAs => 'Signed in as';

  @override
  String get connection => 'Connection';

  @override
  String get environment => 'Environment';

  @override
  String get accountSection => 'Account';

  @override
  String get generalSection => 'General';

  @override
  String get noFriends => 'No friends yet';

  @override
  String get noFriendsHint => 'Search an account and send a friend request';

  @override
  String get addByAccount => 'Add friend';

  @override
  String get recentContacts => 'Friends';

  @override
  String get agentLocalSection => 'Local agent';

  @override
  String get agentLocalSubtitle => 'On-device Goose · never hits the server';

  @override
  String get agentLocalOnly =>
      'This is a local assistant and is not sent to the server';

  @override
  String get agentComposerDirect => 'Message the assistant';

  @override
  String get agentSettings => 'Agent settings';

  @override
  String get agentMode => 'Protocol';

  @override
  String get agentProvider => 'Goose provider';

  @override
  String get agentProviderOpenAi => 'OpenAI';

  @override
  String get agentProviderAnthropic => 'Anthropic';

  @override
  String get agentBaseUrl => 'Base URL';

  @override
  String get agentModel => 'Model';

  @override
  String get agentApiKey => 'API key';

  @override
  String get agentApiKeyHint =>
      'Stored in the system keychain, not in plain preferences';

  @override
  String get agentSaved => 'Agent settings saved';

  @override
  String get agentKeyMissing => 'API key required';

  @override
  String get agentGooseHint =>
      'The assistant in Contacts and Chats is on-device Goose. Open it to talk, or @助手 in other threads.';

  @override
  String get agentProviderCompatible => 'OpenAI compatible';

  @override
  String get agentFetchModels => 'Fetch models';

  @override
  String get agentReasoning => 'Reasoning';

  @override
  String get agentReasoningOff => 'Off';

  @override
  String get agentReasoningLow => 'Low';

  @override
  String get agentReasoningMedium => 'Med';

  @override
  String get agentReasoningHigh => 'High';

  @override
  String get agentReasoningMax => 'Max';

  @override
  String get agentFsLater => 'Workspace files (later)';

  @override
  String get agentFsReadonly => 'Workspace read-only files';

  @override
  String get agentBashLater => 'Shell (later)';

  @override
  String get agentBashDanger => 'Shell commands (dangerous, default off)';

  @override
  String get agentMcp => 'MCP extensions';

  @override
  String get agentMcpHint =>
      'Advanced. One per line: name command arg… Default empty. Tools require confirmation.';

  @override
  String get agentAllow => 'Allow';

  @override
  String get agentAlwaysAllow => 'Always';

  @override
  String get agentDeny => 'Deny';

  @override
  String get agentPermissions => 'Tool permissions';

  @override
  String get agentPermissionAlways => 'Always';

  @override
  String get agentPermissionAsk => 'Ask';

  @override
  String get agentPermissionNever => 'Never';

  @override
  String get agentToolSendMessage => 'Send message';

  @override
  String get agentToolClipboard => 'Clipboard';

  @override
  String get agentToolSearchContacts => 'Search contacts';

  @override
  String get agentToolSearchMessages => 'Search messages';

  @override
  String get agentMoreComing => 'More agents coming soon';

  @override
  String get agentMultiProfile => 'Show multiple local agents';

  @override
  String get agentServerIdentity => 'Sync assistant chats to other devices';

  @override
  String get agentServerIdentityHint =>
      'When on, desktop registers as soon as it is online. A b_ account in settings means it worked; only messages after that reach the phone. Old on-device history is not uploaded.';

  @override
  String get agentRegisterFailed =>
      'Could not register the assistant on the server. If you just deployed, confirm Chat was rolled to an image that has chat.bot.';

  @override
  String get agentDuplicate => 'Duplicate';

  @override
  String get agentDelete => 'Delete';

  @override
  String get agentNeedsEnv => 'desktop / needs env';

  @override
  String get agentComposerHint => 'Send a message, or @助手';

  @override
  String get emptyChatTitle => 'Select a conversation';

  @override
  String get emptyChatSubtitle =>
      'Open a chat from the list, or start a new one';

  @override
  String get routeNotFound => 'Page not found';

  @override
  String get goHome => 'Back to chats';
}
