/// Auth port adapter over [KimBridgeBase].
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

  @override
  Future<KimAuthSession> login({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  }) async {
    await ensure();
    return authSession(
      await authClient(
        origin,
        userAgent,
      ).login(account: account, password: password),
    );
  }

  @override
  Future<KimAuthSession> register({
    required String origin,
    required String userAgent,
    required String account,
    required String password,
  }) async {
    await ensure();
    return authSession(
      await authClient(
        origin,
        userAgent,
      ).register(account: account, password: password),
    );
  }

  @override
  Future<void> logout({
    required String origin,
    required String userAgent,
    required String token,
  }) async {
    await ensure();
    await authClient(origin, userAgent).logout(token: token);
  }

  @override
  Future<void> changePassword({
    required String origin,
    required String userAgent,
    required String token,
    required String oldPassword,
    required String newPassword,
  }) async {
    await ensure();
    await authClient(origin, userAgent).changePassword(
      token: token,
      oldPassword: oldPassword,
      newPassword: newPassword,
    );
  }

  @override
  String httpOriginFromWs(String wsUrl) {
    return rust_auth.httpOriginFromWs(wsUrl: wsUrl);
  }


}
