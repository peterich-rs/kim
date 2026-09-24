/// Central path constants and helpers for GoRouter navigation.
///
/// Keep string paths here so call sites stay typed and redirects stay in sync.
library;

abstract final class AppRoutes {
  static const home = '/';
  static const login = '/login';
  static const register = '/register';
  static const contacts = '/contacts';
  static const me = '/me';
  static const password = '/password';
  static const dev = '/dev';
  static const agent = '/agent';
  static const agentNew = '/agent/new';
  static const agentAccounts = '/agent/accounts';
  static const agentAccountNew = '/agent/accounts/new';
  static const agentPlaza = '/agent/plaza';

  static const chatPrefix = '/chat/';
  static const peerPrefix = '/peer/';
  static const agentPrefix = '/agent';

  static String chat(String id) => '$chatPrefix$id';

  static String peer(String account, {String? title}) {
    if (title == null || title.isEmpty) {
      return '$peerPrefix$account';
    }
    return '$peerPrefix$account?title=${Uri.encodeQueryComponent(title)}';
  }

  static String agentProfile(String id) => '$agentPrefix/$id';

  static String agentCapabilities(String id, {String? section}) {
    final base = '${agentProfile(id)}/capabilities';
    if (section == null || section.isEmpty) {
      return base;
    }
    return '$base?section=${Uri.encodeQueryComponent(section)}';
  }

  static String agentAccount(String accountId) => '$agentAccounts/$accountId';

  static String agentPlazaAssign(String profileId) =>
      '$agentPlaza?assignTo=${Uri.encodeQueryComponent(profileId)}';

  static bool isAuth(String path) => path == login || path == register;

  static bool isChat(String path) => path.startsWith(chatPrefix);

  static bool isOverlay(String path) =>
      path.startsWith(agentPrefix) ||
      path.startsWith(password) ||
      path.startsWith(dev) ||
      path.startsWith(peerPrefix);

  static String? chatIdFromPath(String path) {
    if (!path.startsWith(chatPrefix)) {
      return null;
    }
    final rest = path.substring(chatPrefix.length);
    final slash = rest.indexOf('/');
    final raw = slash == -1 ? rest : rest.substring(0, slash);
    if (raw.isEmpty) {
      return null;
    }
    return Uri.decodeComponent(raw);
  }
}
