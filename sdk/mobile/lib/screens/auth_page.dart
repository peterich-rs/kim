library;

import 'package:flutter/material.dart';

import '../copy.dart';
import '../core/errors.dart';
import '../core/haptics.dart';
import '../core/runtime.dart';
import '../core/settings.dart';
import '../core/user_agent.dart';
import '../core/validation.dart';
import '../kim_bridge.dart';
import '../theme/motion.dart';
import '../widgets/kim_text_field.dart';

class AuthPage extends StatefulWidget {
  const AuthPage({
    super.key,
    required this.runtime,
    required this.auth,
    required this.onSignedIn,
  });

  final KimRuntime runtime;
  final KimAuthPort auth;
  final VoidCallback onSignedIn;

  @override
  State<AuthPage> createState() => _AuthPageState();
}

class _AuthPageState extends State<AuthPage> {
  late final TextEditingController _account;
  late final TextEditingController _password;
  late final TextEditingController _confirm;
  bool _register = false;
  bool _busy = false;
  String _error = '';
  String? _accountErr;
  String? _passwordErr;
  String? _confirmErr;

  SettingsStore get _settings => widget.runtime.settings;

  @override
  void initState() {
    super.initState();
    _account = TextEditingController();
    _password = TextEditingController();
    _confirm = TextEditingController();
  }

  @override
  void dispose() {
    _account.dispose();
    _password.dispose();
    _confirm.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_busy) {
      return;
    }
    final account = _account.text.trim();
    final password = _password.text;
    final accountErr = validateAccount(account);
    final passwordErr = validatePassword(password);
    final confirmErr = _register ? validateConfirm(password, _confirm.text) : null;
    setState(() {
      _accountErr = accountErr;
      _passwordErr = passwordErr;
      _confirmErr = confirmErr;
      _error = '';
    });
    if (accountErr != null || passwordErr != null || confirmErr != null) {
      return;
    }
    setState(() => _busy = true);
    try {
      final ua = kimUserAgent(widget.runtime);
      final origin = _settings.httpOrigin;
      final session = _register
          ? await widget.auth.register(
              origin: origin,
              userAgent: ua,
              account: account,
              password: password,
            )
          : await widget.auth.login(
              origin: origin,
              userAgent: ua,
              account: account,
              password: password,
            );
      await _settings.saveSession(token: session.token, account: session.account);
      await KimHaptics.success();
      widget.onSignedIn();
    } catch (err) {
      await KimHaptics.error();
      if (mounted) {
        setState(() => _error = mapUserError(err));
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(24, 32, 24, 24),
          children: [
            Text(Copy.brand, style: theme.textTheme.headlineMedium),
            const SizedBox(height: 4),
            Text(Copy.brandSub, style: theme.textTheme.bodyMedium),
            const SizedBox(height: 28),
            Text(
              _register ? Copy.registerTitle : Copy.loginTitle,
              style: theme.textTheme.titleLarge,
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              children: [
                TextButton(
                  onPressed: _busy
                      ? null
                      : () async {
                          await _settings.useLocal();
                          setState(() {});
                        },
                  child: const Text(Copy.localServer),
                ),
                TextButton(
                  onPressed: _busy
                      ? null
                      : () async {
                          await _settings.useProd();
                          setState(() {});
                        },
                  child: const Text(Copy.prodServer),
                ),
              ],
            ),
            Text(_settings.httpOrigin, style: theme.textTheme.bodySmall),
            KimTextField(
              controller: _account,
              label: Copy.account,
              helperText: Copy.accountHint,
              errorText: _accountErr,
              maxLength: 32,
              autofocus: true,
              keyboardType: TextInputType.visiblePassword,
              autocorrect: false,
              enableSuggestions: false,
              autofillHints: const [AutofillHints.username],
            ),
            KimTextField(
              controller: _password,
              label: Copy.password,
              helperText: _register ? Copy.passwordHint : null,
              errorText: _passwordErr,
              obscureable: true,
              maxLength: 128,
              autofillHints: [
                _register ? AutofillHints.newPassword : AutofillHints.password,
              ],
            ),
            if (_register)
              KimTextField(
                controller: _confirm,
                label: Copy.confirmPassword,
                errorText: _confirmErr,
                obscureable: true,
                maxLength: 128,
                textInputAction: TextInputAction.done,
                onEditingComplete: () => _submit(),
                autofillHints: const [AutofillHints.newPassword],
              ),
            if (_error.isNotEmpty) ...[
              const SizedBox(height: 12),
              Text(
                _error,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
            ],
            const SizedBox(height: 20),
            FilledButton(
              key: const Key('auth-submit'),
              onPressed: _busy ? null : _submit,
              child: _busy
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Text(_register ? Copy.registerAction : Copy.loginAction),
            ),
            const SizedBox(height: 12),
            TextButton(
              key: const Key('auth-toggle'),
              onPressed: _busy
                  ? null
                  : () {
                      setState(() {
                        _register = !_register;
                        _error = '';
                        _confirmErr = null;
                      });
                    },
              child: Text(_register ? '${Copy.hasAccount} ${Copy.goLogin}' : '${Copy.noAccount} ${Copy.goRegister}'),
            ),
            AnimatedOpacity(
              duration: KimMotion.short,
              opacity: _busy ? 0.5 : 1,
              child: const SizedBox.shrink(),
            ),
          ],
        ),
      ),
    );
  }
}
