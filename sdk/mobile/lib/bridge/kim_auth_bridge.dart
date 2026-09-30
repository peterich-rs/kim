/// Auth port adapter over [KimBridgeBase]. Origin and User-Agent are
/// Rust-derived from the settings table + platform bootstrap.
library;

import 'package:kim_mobile/bridge/kim_bridge_base.dart';
import 'package:kim_mobile/bridge/kim_ports.dart';
import 'package:kim_mobile/src/rust/api/auth.dart' as rust_auth;

mixin KimAuthBridge on KimBridgeBase implements KimAuthPort {
  KimAuthSession authSession(rust_auth.AuthSession s) {
    return KimAuthSession(
      token: s.token,
      exp: s.exp.toInt(),
      account: s.account,
    );
  }

  Future<rust_auth.KimAuth> _client() async {
    await ensure();
    return requireApi().auth();
  }

  @override
  Future<KimAuthSession> login({
    required String account,
    required String password,
  }) async {
    final client = await _client();
    return authSession(
      await client.login(account: account, password: password),
    );
  }

  @override
  Future<KimAuthSession> register({
    required String account,
    required String password,
  }) async {
    final client = await _client();
    return authSession(
      await client.register(account: account, password: password),
    );
  }
}
