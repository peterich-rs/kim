library;

import 'package:flutter/material.dart';

import '../copy.dart';
import '../core/errors.dart';
import '../core/haptics.dart';
import '../core/runtime.dart';
import '../core/user_agent.dart';
import '../core/validation.dart';
import '../kim_bridge.dart';
import '../widgets/kim_text_field.dart';

class PasswordPage extends StatefulWidget {
  const PasswordPage({
    super.key,
    required this.runtime,
    required this.auth,
  });

  final KimRuntime runtime;
  final KimAuthPort auth;

  @override
  State<PasswordPage> createState() => _PasswordPageState();
}

class _PasswordPageState extends State<PasswordPage> {
  late final TextEditingController _old;
  late final TextEditingController _next;
  late final TextEditingController _confirm;
  bool _busy = false;
  String _error = '';
  String? _oldErr;
  String? _nextErr;
  String? _confirmErr;

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
    if (_busy) {
      return;
    }
    final oldErr = validatePassword(_old.text);
    final nextErr = validatePassword(_next.text);
    final confirmErr = validateConfirm(_next.text, _confirm.text);
    setState(() {
      _oldErr = oldErr;
      _nextErr = nextErr;
      _confirmErr = confirmErr;
      _error = '';
    });
    if (oldErr != null || nextErr != null || confirmErr != null) {
      return;
    }
    setState(() => _busy = true);
    try {
      await widget.auth.changePassword(
        origin: widget.runtime.settings.httpOrigin,
        userAgent: kimUserAgent(widget.runtime),
        token: widget.runtime.settings.token,
        oldPassword: _old.text,
        newPassword: _next.text,
      );
      await KimHaptics.success();
      if (mounted) {
        Navigator.of(context).pop(true);
      }
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
    return Scaffold(
      appBar: AppBar(title: const Text(Copy.changePassword)),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
        children: [
          KimTextField(
            controller: _old,
            label: Copy.oldPassword,
            errorText: _oldErr,
            obscureable: true,
            maxLength: 128,
            autofocus: true,
            autofillHints: const [AutofillHints.password],
          ),
          KimTextField(
            controller: _next,
            label: Copy.newPassword,
            helperText: Copy.passwordHint,
            errorText: _nextErr,
            obscureable: true,
            maxLength: 128,
            autofillHints: const [AutofillHints.newPassword],
          ),
          KimTextField(
            controller: _confirm,
            label: Copy.confirmPassword,
            errorText: _confirmErr,
            obscureable: true,
            maxLength: 128,
            textInputAction: TextInputAction.done,
            onEditingComplete: _save,
            autofillHints: const [AutofillHints.newPassword],
          ),
          if (_error.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Text(
                _error,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          const SizedBox(height: 20),
          FilledButton(
            onPressed: _busy ? null : _save,
            child: _busy
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text(Copy.save),
          ),
        ],
      ),
    );
  }
}
