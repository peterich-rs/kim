library;

import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';
import 'package:flutter_riverpod/experimental/mutation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';

import '../copy.dart';
import '../core/errors.dart';
import '../core/haptics.dart';
import '../core/validation.dart';
import '../state/auth.dart';
import '../state/mutations.dart';
import '../state/providers.dart';
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
  String? _accountErr;
  String? _passwordErr;
  String? _confirmErr;

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

  Mutation<void> get _mutation =>
      widget.register ? registerMutation : signInMutation;

  Future<void> _submit() async {
    final account = _account.text.trim();
    final password = _password.text;
    final accountErr = validateAccount(account);
    final passwordErr = validatePassword(password);
    final confirmErr = widget.register
        ? validateConfirm(password, _confirm.text)
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
            .signIn(
              register: widget.register,
              account: account,
              password: password,
            );
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
    final isRegister = widget.register;
    final mut = ref.watch(_mutation);
    final busy = mut is MutationPending;
    final error = switch (mut) {
      MutationError(:final error) => mapUserError(error),
      _ => '',
    };

    return Scaffold(
      body: DecoratedBox(
        decoration: BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
            colors: [
              scheme.primary.withValues(alpha: 0.16),
              scheme.surface,
              scheme.tertiary.withValues(alpha: 0.10),
            ],
          ),
        ),
        child: SafeArea(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(20, 28, 20, 24),
            children: [
              const KimMark()
                  .animate()
                  .fadeIn(duration: 400.ms)
                  .scale(
                    begin: const Offset(0.92, 0.92),
                    curve: Curves.easeOut,
                  ),
              const Gap(18),
              Text(
                Copy.brand,
                style: theme.textTheme.headlineLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                  letterSpacing: -0.6,
                ),
              ),
              const Gap(4),
              Text(
                Copy.brandPitch,
                style: theme.textTheme.bodyLarge?.copyWith(
                  color: scheme.onSurfaceVariant,
                ),
              ),
              const Gap(28),
              DecoratedBox(
                    decoration: BoxDecoration(
                      color: scheme.surface.withValues(alpha: 0.92),
                      borderRadius: BorderRadius.circular(24),
                      border: Border.all(
                        color: scheme.outlineVariant.withValues(alpha: 0.5),
                      ),
                      boxShadow: [
                        BoxShadow(
                          color: scheme.shadow.withValues(alpha: 0.06),
                          blurRadius: 24,
                          offset: const Offset(0, 10),
                        ),
                      ],
                    ),
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(18, 20, 18, 22),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Text(
                            isRegister ? Copy.registerTitle : Copy.loginTitle,
                            style: theme.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                          KimTextField(
                            controller: _account,
                            label: Copy.account,
                            hintText: Copy.accountPlaceholder,
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
                            hintText: Copy.passwordPlaceholder,
                            helperText: isRegister ? Copy.passwordHint : null,
                            errorText: _passwordErr,
                            obscureable: true,
                            maxLength: 128,
                            autofillHints: [
                              isRegister
                                  ? AutofillHints.newPassword
                                  : AutofillHints.password,
                            ],
                          ),
                          if (isRegister)
                            KimTextField(
                              controller: _confirm,
                              label: Copy.confirmPassword,
                              hintText: Copy.confirmPlaceholder,
                              errorText: _confirmErr,
                              obscureable: true,
                              maxLength: 128,
                              textInputAction: TextInputAction.done,
                              onEditingComplete: _submit,
                              autofillHints: const [AutofillHints.newPassword],
                            ),
                          if (error.isNotEmpty) ...[
                            const Gap(12),
                            DecoratedBox(
                              decoration: BoxDecoration(
                                color: scheme.error.withValues(alpha: 0.1),
                                borderRadius: BorderRadius.circular(12),
                              ),
                              child: Padding(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 12,
                                  vertical: 10,
                                ),
                                child: Text(
                                  error,
                                  style: theme.textTheme.bodyMedium?.copyWith(
                                    color: scheme.error,
                                  ),
                                ),
                              ),
                            ),
                          ],
                          const Gap(20),
                          FilledButton(
                            key: const Key('auth-submit'),
                            onPressed: busy ? null : _submit,
                            style: FilledButton.styleFrom(
                              minimumSize: const Size.fromHeight(52),
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
                                : Text(
                                    isRegister
                                        ? Copy.registerAction
                                        : Copy.loginAction,
                                  ),
                          ),
                        ],
                      ),
                    ),
                  )
                  .animate()
                  .fadeIn(duration: 420.ms, delay: 80.ms)
                  .slideY(begin: 0.05, curve: Curves.easeOutCubic),
              const Gap(16),
              TextButton(
                key: const Key('auth-toggle'),
                onPressed: busy
                    ? null
                    : () => context.go(isRegister ? '/login' : '/register'),
                child: Text(
                  isRegister
                      ? '${Copy.hasAccount} ${Copy.goLogin}'
                      : '${Copy.noAccount} ${Copy.goRegister}',
                ),
              ),
              const Gap(12),
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  TextButton(
                    onPressed: busy
                        ? null
                        : () async {
                            await settings.useLocal();
                            setState(() {});
                          },
                    child: Text(
                      Copy.localServer,
                      style: TextStyle(
                        color: settings.httpOrigin.contains('127.0.0.1')
                            ? scheme.primary
                            : scheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                  TextButton(
                    onPressed: busy
                        ? null
                        : () async {
                            await settings.useProd();
                            setState(() {});
                          },
                    child: Text(
                      Copy.prodServer,
                      style: TextStyle(
                        color: settings.httpOrigin.contains('127.0.0.1')
                            ? scheme.onSurfaceVariant
                            : scheme.primary,
                      ),
                    ),
                  ),
                ],
              ),
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
