library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/experimental/mutation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/errors.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/core/validation.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/auth/auth_form.dart';
import 'package:kim_mobile/features/session/mutations.dart';
import 'package:kim_mobile/design/kim_theme.dart';
import 'package:kim_mobile/design/motion.dart';
import 'package:kim_mobile/design/kim_mark.dart';
import 'package:kim_mobile/design/kim_text_field.dart';

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
  var _sending = false;

  NotifierProvider<AuthDraftNotifier, AuthDraft> get _draft =>
      authDraftProvider(widget.register);

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

  Mutation<void> _mutationFor(bool register) =>
      register ? registerMutation : signInMutation;

  Future<void> _submit() async {
    if (_sending) {
      return;
    }
    _sending = true;
    try {
      FocusManager.instance.primaryFocus?.unfocus();
      await WidgetsBinding.instance.endOfFrame;
      if (!mounted) {
        return;
      }
      final account = _account.text.trim();
      final password = sanitizePassword(_password.text);
      final accountErr = validateAccount(account);
      final passwordErr = validatePassword(password);
      final register = ref.read(_draft).register;
      final confirmErr = register
          ? validateConfirm(password, sanitizePassword(_confirm.text))
          : null;
      ref
          .read(_draft.notifier)
          .showErrors(
            account: accountErr,
            password: passwordErr,
            confirm: confirmErr,
          );
      if (accountErr != null || passwordErr != null || confirmErr != null) {
        return;
      }
      final mutation = _mutationFor(register);
      mutation.reset(ref);
      try {
        await mutation.run(ref, (tsx) async {
          await tsx
              .get(authProvider.notifier)
              .signIn(register: register, account: account, password: password);
        });
        _password.clear();
        _confirm.clear();
      } catch (err) {
        await KimHaptics.error();
      }
    } finally {
      if (mounted) {
        _sending = false;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final draft = ref.watch(_draft);
    final isRegister = draft.register;
    final mut = ref.watch(_mutationFor(isRegister));
    final busy = mut is MutationPending;
    final notice = ref.watch(authProvider).notice ?? '';
    final error = switch (mut) {
      MutationError(:final error) => mapUserError(error),
      _ => notice,
    };

    return Scaffold(
      backgroundColor: KimTheme.canvasOf(context),
      body: SafeArea(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final wide = constraints.maxWidth >= kKimCompactBreakpoint;
            final form = _AuthForm(
              isRegister: isRegister,
              busy: busy,
              error: error,
              account: _account,
              password: _password,
              confirm: _confirm,
              accountErr: draft.accountErr,
              passwordErr: draft.passwordErr,
              confirmErr: draft.confirmErr,
              onSubmit: _submit,
              onToggle: busy
                  ? null
                  : () {
                      _mutationFor(isRegister).reset(ref);
                      ref.read(_draft.notifier).toggleRegister();
                    },
            );
            return SingleChildScrollView(
              padding: EdgeInsets.symmetric(
                horizontal: wide ? 24 : 28,
                vertical: wide ? 40 : 24,
              ),
              child: ConstrainedBox(
                constraints: BoxConstraints(
                  minHeight: (constraints.maxHeight - (wide ? 80 : 48)).clamp(
                    0,
                    double.infinity,
                  ),
                ),
                child: Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(
                      maxWidth: kKimFormMaxWidth,
                    ),
                    child: wide
                        ? DecoratedBox(
                            decoration: BoxDecoration(
                              color: KimTheme.raisedOf(context),
                              borderRadius: BorderRadius.circular(
                                KimTheme.radiusCard,
                              ),
                              border: Border.all(
                                color: KimTheme.hairlineOf(context),
                              ),
                            ),
                            child: Padding(
                              padding: const EdgeInsets.fromLTRB(
                                32,
                                36,
                                32,
                                28,
                              ),
                              child: form,
                            ),
                          )
                        : form,
                  ),
                ),
              ),
            );
          },
        ),
      ),
    );
  }
}

class _AuthForm extends StatelessWidget {
  const _AuthForm({
    required this.isRegister,
    required this.busy,
    required this.error,
    required this.account,
    required this.password,
    required this.confirm,
    required this.accountErr,
    required this.passwordErr,
    required this.confirmErr,
    required this.onSubmit,
    required this.onToggle,
  });

  final bool isRegister;
  final bool busy;
  final String error;
  final TextEditingController account;
  final TextEditingController password;
  final TextEditingController confirm;
  final String? accountErr;
  final String? passwordErr;
  final String? confirmErr;
  final VoidCallback onSubmit;
  final VoidCallback? onToggle;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const KimMark(size: 48),
        const Gap(18),
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
        const Gap(28),
        KimTextField(
          controller: account,
          label: Copy.account,
          errorText: accountErr,
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
          controller: password,
          label: Copy.password,
          errorText: passwordErr,
          obscureable: true,
          keyboardType: TextInputType.visiblePassword,
          maxLength: 128,
          textInputAction: isRegister
              ? TextInputAction.next
              : TextInputAction.done,
          onEditingComplete: isRegister ? null : onSubmit,
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
                  controller: confirm,
                  label: Copy.confirmPassword,
                  errorText: confirmErr,
                  obscureable: true,
                  keyboardType: TextInputType.visiblePassword,
                  maxLength: 128,
                  textInputAction: TextInputAction.done,
                  onEditingComplete: onSubmit,
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
        const Gap(24),
        FilledButton(
          key: const Key('auth-submit'),
          onPressed: busy ? null : onSubmit,
          style: FilledButton.styleFrom(
            minimumSize: const Size.fromHeight(48),
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
        const Gap(4),
        Align(
          alignment: Alignment.center,
          child: TextButton(
            key: const Key('auth-toggle'),
            onPressed: onToggle,
            style: TextButton.styleFrom(
              foregroundColor: scheme.primary,
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            ),
            child: Text(
              isRegister
                  ? '${Copy.hasAccount}${Copy.goLogin}'
                  : '${Copy.noAccount}${Copy.goRegister}',
            ),
          ),
        ),
      ],
    );
  }
}
