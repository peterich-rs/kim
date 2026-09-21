library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/experimental/mutation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:toastification/toastification.dart';

import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/errors.dart';
import 'package:kim_mobile/core/haptics.dart';
import 'package:kim_mobile/core/layout.dart';
import 'package:kim_mobile/core/validation.dart';
import 'package:kim_mobile/design/kim_header.dart';
import 'package:kim_mobile/features/auth/auth.dart';
import 'package:kim_mobile/features/auth/auth_form.dart';
import 'package:kim_mobile/features/session/mutations.dart';
import 'package:kim_mobile/design/kim_text_field.dart';

class PasswordPage extends ConsumerStatefulWidget {
  const PasswordPage({super.key});

  @override
  ConsumerState<PasswordPage> createState() => _PasswordPageState();
}

class _PasswordPageState extends ConsumerState<PasswordPage> {
  late final TextEditingController _old;
  late final TextEditingController _next;
  late final TextEditingController _confirm;

  @override
  void initState() {
    super.initState();
    _old = TextEditingController();
    _next = TextEditingController();
    _confirm = TextEditingController();
  }

  @override
  void dispose() {
    _old.dispose();
    _next.dispose();
    _confirm.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final oldPassword = sanitizePassword(_old.text);
    final newPassword = sanitizePassword(_next.text);
    final oldErr = validatePassword(oldPassword);
    final nextErr = validatePassword(newPassword);
    final confirmErr = validateConfirm(
      newPassword,
      sanitizePassword(_confirm.text),
    );
    ref
        .read(passwordDraftProvider.notifier)
        .showErrors(old: oldErr, next: nextErr, confirm: confirmErr);
    if (oldErr != null || nextErr != null || confirmErr != null) {
      return;
    }
    changePasswordMutation.reset(ref);
    try {
      await changePasswordMutation.run(ref, (tsx) async {
        await tsx
            .get(authProvider.notifier)
            .changePassword(oldPassword: oldPassword, newPassword: newPassword);
      });
      if (!mounted) {
        return;
      }
      _old.clear();
      _next.clear();
      _confirm.clear();
      toastification.show(
        context: context,
        type: ToastificationType.success,
        style: ToastificationStyle.flatColored,
        title: Text(Copy.passwordChanged),
        autoCloseDuration: const Duration(seconds: 2),
        alignment: Alignment.topCenter,
      );
      context.pop();
    } catch (err) {
      await KimHaptics.error();
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final draft = ref.watch(passwordDraftProvider);
    final mut = ref.watch(changePasswordMutation);
    final busy = mut is MutationPending;
    final error = switch (mut) {
      MutationError(:final error) => mapUserError(error),
      _ => '',
    };
    return Scaffold(
      appBar: AppBar(
        automaticallyImplyLeading: false,
        leading: kimHeaderLeading(context),
        title: Text(Copy.changePassword),
      ),
      body: kimConstrainedForm(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(28, 12, 28, 28),
          children: [
            KimTextField(
              controller: _old,
              label: Copy.oldPassword,
              errorText: draft.oldErr,
              obscureable: true,
              keyboardType: TextInputType.visiblePassword,
              maxLength: 128,
              autofocus: true,
              autofillHints: const [AutofillHints.password],
            ),
            KimTextField(
              controller: _next,
              label: Copy.newPassword,
              errorText: draft.nextErr,
              obscureable: true,
              keyboardType: TextInputType.visiblePassword,
              maxLength: 128,
              autofillHints: const [AutofillHints.newPassword],
            ),
            KimTextField(
              controller: _confirm,
              label: Copy.confirmPassword,
              errorText: draft.confirmErr,
              obscureable: true,
              keyboardType: TextInputType.visiblePassword,
              maxLength: 128,
              textInputAction: TextInputAction.done,
              onEditingComplete: _save,
              autofillHints: const [AutofillHints.newPassword],
            ),
            if (error.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 12),
                child: Text(error, style: TextStyle(color: scheme.error)),
              ),
            const Gap(24),
            FilledButton(
              onPressed: busy ? null : _save,
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(52),
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
                  : Text(Copy.save),
            ),
          ],
        ),
      ),
    );
  }
}
