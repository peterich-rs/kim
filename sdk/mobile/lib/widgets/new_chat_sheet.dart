library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:wolt_modal_sheet/wolt_modal_sheet.dart';

import '../copy.dart';
import '../core/haptics.dart';
import '../core/validation.dart';
import '../models/models.dart';
import '../state/inbox.dart';
import '../state/session.dart';
import 'kim_text_field.dart';

Future<void> openNewChatSheet(BuildContext context) {
  return WoltModalSheet.show<void>(
    context: context,
    showDragHandle: true,
    pageListBuilder: (context) => [
      WoltModalSheetPage(
        hasSabGradient: false,
        navBarHeight: 56,
        pageTitle: const Padding(
          padding: EdgeInsets.fromLTRB(24, 8, 24, 0),
          child: Text(Copy.newChat),
        ),
        child: const Padding(
          padding: EdgeInsets.fromLTRB(8, 0, 8, 28),
          child: _NewChatBody(),
        ),
      ),
    ],
  );
}

class _NewChatBody extends ConsumerStatefulWidget {
  const _NewChatBody();

  @override
  ConsumerState<_NewChatBody> createState() => _NewChatBodyState();
}

class _NewChatBodyState extends ConsumerState<_NewChatBody> {
  late final TextEditingController _peer;
  String? _error;

  @override
  void initState() {
    super.initState();
    _peer = TextEditingController();
  }

  @override
  void dispose() {
    _peer.dispose();
    super.dispose();
  }

  void _open() {
    final dest = _peer.text.trim();
    final accountErr = validateAccount(dest);
    final me = ref.read(sessionProvider).account;
    setState(() {
      if (accountErr != null) {
        _error = accountErr;
      } else if (dest == me) {
        _error = Copy.cannotChatSelf;
      } else {
        _error = null;
      }
    });
    if (_error != null) {
      return;
    }
    final thread = ref.read(inboxProvider.notifier).ensureThread(
      id: dest,
      kind: ThreadKind.user,
      title: dest,
    );
    KimHaptics.selection();
    Navigator.of(context).pop();
    context.push('/chat/${thread.id}', extra: thread);
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        KimTextField(
          controller: _peer,
          label: Copy.peerAccount,
          hintText: Copy.peerPlaceholder,
          helperText: Copy.accountHint,
          errorText: _error,
          maxLength: 32,
          autofocus: true,
          prefixIcon: LucideIcons.user,
          autocorrect: false,
          enableSuggestions: false,
          textInputAction: TextInputAction.done,
          onEditingComplete: _open,
        ),
        const Gap(16),
        FilledButton(
          onPressed: _open,
          child: const Text(Copy.openChat),
        ),
      ],
    );
  }
}
