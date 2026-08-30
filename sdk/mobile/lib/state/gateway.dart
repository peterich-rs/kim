library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/user_agent.dart';
import '../models/models.dart';
import 'auth.dart';
import 'providers.dart';
import 'retry.dart';

/// WGateway session. Connect failures throw so [kimRetry] backs off.
/// Radio-down and signed-out return [ConnStatus.offline] without throwing.
class GatewayNotifier extends AsyncNotifier<ConnStatus> {
  @override
  Future<ConnStatus> build() async {
    final signedIn = ref.watch(authProvider.select((s) => s.signedIn));
    final client = ref.read(clientPortProvider);
    if (!signedIn) {
      try {
        await client.disconnect();
      } catch (_) {}
      return ConnStatus.offline;
    }
    final radio = ref.watch(radioOnlineProvider);
    if (!radio) {
      try {
        await client.disconnect();
      } catch (_) {}
      return ConnStatus.offline;
    }
    final runtime = ref.read(runtimeProvider);
    final token = runtime.settings.token;
    if (token.isEmpty) {
      return ConnStatus.offline;
    }
    await client.connect(
      runtime.settings.url,
      token,
      userAgent: kimUserAgent(runtime),
    );
    if (!ref.mounted) {
      return ConnStatus.offline;
    }
    await client.loginWs();
    if (!ref.mounted) {
      return ConnStatus.offline;
    }
    return ConnStatus.online;
  }

  Future<void> drop() async {
    try {
      await ref.read(clientPortProvider).disconnect();
    } catch (_) {}
    if (!ref.mounted) {
      return;
    }
    state = const AsyncData(ConnStatus.offline);
  }
}

final gatewayProvider = AsyncNotifierProvider<GatewayNotifier, ConnStatus>(
  GatewayNotifier.new,
  retry: kimRetry,
);
