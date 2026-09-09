library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/experimental/mutation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';

import '../copy.dart';
import '../core/errors.dart';
import '../core/haptics.dart';
import '../core/layout.dart';
import '../core/validation.dart';
import '../state/auth.dart';
import '../state/mutations.dart';
import '../state/providers.dart';
import '../theme/kim_theme.dart';
import '../theme/motion.dart';
import '../widgets/kim_mark.dart';
import '../widgets/kim_text_field.dart';

class AuthPage extends ConsumerStatefulWidget {
  const AuthPage({super.key, required this.register});

  final bool register;

  @override
  ConsumerState<AuthPage> createState() => _AuthPageState();
}

class _AuthPageState extends ConsumerState<AuthPage> {
  late final TextEditingController _account;
  late final TextEditingController _password;
  late final TextEditingController _confirm;
  late bool _register;
  String? _accountErr;
  String? _passwordErr;
  String? _confirmErr;

  @override
  void initState() {
    super.initState();
    _register = widget.register;
    _account = TextEditingController();
    _password = TextEditingController();
    _confirm = TextEditingController();
  }

  @override
  void didUpdateWidget(covariant AuthPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.register != widget.register) {
      _register = widget.register;
    }
  }

  @override
  void dispose() {
    _account.dispose();
    _password.dispose();
    _confirm.dispose();
    super.dispose();
  }

  Mutation<void> get _mutation => _register ? registerMutation : signInMutation;

  Future<void> _submit() async {
    FocusManager.instance.primaryFocus?.unfocus();
    await WidgetsBinding.instance.endOfFrame;
    final account = _account.text.trim();
    final password = sanitizePassword(_password.text);
    final accountErr = validateAccount(account);
    final passwordErr = validatePassword(password);
    final confirmErr = _register
        ? validateConfirm(password, sanitizePassword(_confirm.text))
        : null;
    setState(() {
      _accountErr = accountErr;
      _passwordErr = passwordErr;
      _confirmErr = confirmErr;
    });
    if (accountErr != null || passwordErr != null || confirmErr != null) {
      return;
    }
    _mutation.reset(ref);
    try {
      await _mutation.run(ref, (tsx) async {
        await tsx
            .get(authProvider.notifier)
            .signIn(register: _register, account: account, password: password);
      });
    } catch (err) {
      await KimHaptics.error();
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final settings = ref.watch(runtimeProvider).settings;
    final isRegister = _register;
    final mut = ref.watch(_mutation);
    final busy = mut is MutationPending;
    final notice = ref.watch(authProvider).notice ?? '';
    final error = switch (mut) {
      MutationError(:final error) => mapUserError(error),
      _ => notice,
    };
    final local = settings.httpOrigin.contains('127.0.0.1');

    return Scaffold(
      backgroundColor: KimTheme.canvasOf(context),
      body: SafeArea(
        child: kimConstrainedForm(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(28, 56, 28, 28),
            children: [
              const KimMark(size: 48),
              const Gap(20),
              Text(
                Copy.brand,
                style: theme.textTheme.headlineMedium?.copyWith(
                  fontWeight: FontWeight.w700,
                  letterSpacing: -0.5,
                  height: 1.15,
                ),
              ),
              const Gap(6),
              Text(
                isRegister ? Copy.registerTitle : Copy.loginTitle,
                style: theme.textTheme.titleMedium?.copyWith(
                  color: scheme.onSurfaceVariant,
                  fontWeight: FontWeight.w500,
                ),
              ),
              const Gap(36),
              KimTextField(
                controller: _account,
                label: Copy.account,
                errorText: _accountErr,
                maxLength: 32,
                autofocus: true,
                keyboardType: (Platform.isIOS || Platform.isAndroid)
                    ? TextInputType.visiblePassword
                    : TextInputType.text,
                autocorrect: false,
                enableSuggestions: false,
                autofillHints: Platform.isMacOS
                    ? null
                    : const [AutofillHints.username],
              ),
              KimTextField(
                controller: _password,
                label: Copy.password,
                errorText: _passwordErr,
                obscureable: true,
                maxLength: 128,
                textInputAction: isRegister
                    ? TextInputAction.next
                    : TextInputAction.done,
                onEditingComplete: isRegister ? null : _submit,
                autofillHints: Platform.isMacOS
                    ? null
                    : [
                        isRegister
                            ? AutofillHints.newPassword
                            : AutofillHints.password,
                      ],
              ),
              AnimatedSize(
                duration: KimMotion.medium,
                curve: KimMotion.standard,
                alignment: Alignment.topCenter,
                child: isRegister
                    ? KimTextField(
                        controller: _confirm,
                        label: Copy.confirmPassword,
                        errorText: _confirmErr,
                        obscureable: true,
                        maxLength: 128,
                        textInputAction: TextInputAction.done,
                        onEditingComplete: _submit,
                        autofillHints: Platform.isMacOS
                            ? null
                            : const [AutofillHints.newPassword],
                      )
                    : const SizedBox.shrink(),
              ),
              if (error.isNotEmpty) ...[
                const Gap(16),
                Text(
                  error,
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: scheme.error,
                    height: 1.35,
                  ),
                ),
              ],
              const Gap(28),
              FilledButton(
                key: const Key('auth-submit'),
                onPressed: busy ? null : _submit,
                style: FilledButton.styleFrom(
                  minimumSize: const Size.fromHeight(50),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(KimTheme.radiusField),
                  ),
                  textStyle: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                child: busy
                    ? SizedBox(
                        width: 20,
                        height: 20,
                        child: CircularProgressIndicator(
                          strokeWidth: 2.2,
                          color: scheme.onPrimary,
                        ),
                      )
                    : Text(isRegister ? Copy.registerAction : Copy.loginAction),
              ),
              const Gap(8),
              Center(
                child: TextButton(
                  key: const Key('auth-toggle'),
                  onPressed: busy
                      ? null
                      : () {
                          _mutation.reset(ref);
                          setState(() {
                            _register = !_register;
                            _confirmErr = null;
                            _passwordErr = null;
                            _accountErr = null;
                          });
                        },
                  style: TextButton.styleFrom(
                    foregroundColor: scheme.primary,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 10,
                    ),
                  ),
                  child: Text(
                    isRegister
                        ? '${Copy.hasAccount}${Copy.goLogin}'
                        : '${Copy.noAccount}${Copy.goRegister}',
                  ),
                ),
              ),
              const Gap(40),
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  _EnvChip(
                    label: Copy.localServer,
                    selected: local,
                    enabled: !busy,
                    onTap: () async {
                      await settings.useLocal();
                      setState(() {});
                    },
                  ),
                  const Gap(8),
                  _EnvChip(
                    label: Copy.prodServer,
                    selected: !local,
                    enabled: !busy,
                    onTap: () async {
                      await settings.useProd();
                      setState(() {});
                    },
                  ),
                ],
              ),
              const Gap(8),
              Text(
                settings.httpOrigin,
                textAlign: TextAlign.center,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: scheme.outline,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _EnvChip extends StatelessWidget {
  const _EnvChip({
    required this.label,
    required this.selected,
    required this.enabled,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final bool enabled;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return TextButton(
      onPressed: enabled ? onTap : null,
      style: TextButton.styleFrom(
        foregroundColor: selected ? scheme.primary : scheme.onSurfaceVariant,
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
        minimumSize: Size.zero,
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
        textStyle: const TextStyle(fontSize: 13, fontWeight: FontWeight.w500),
      ),
      child: Text(label),
    );
  }
}
