library;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../core/haptics.dart';
import '../../theme/kim_theme.dart';

/// Agent composer: Send, or Stop while busy (no dual Send).
class AgentComposer extends StatefulWidget {
  const AgentComposer({
    super.key,
    required this.busy,
    required this.onSend,
    required this.onStop,
  });

  final bool busy;
  final ValueChanged<String> onSend;
  final VoidCallback onStop;

  @override
  State<AgentComposer> createState() => _AgentComposerState();
}

class _AgentComposerState extends State<AgentComposer> {
  final _controller = TextEditingController();
  final _focus = FocusNode();
  var _hasText = false;

  @override
  void initState() {
    super.initState();
    _controller.addListener(() {
      final next = _controller.text.trim().isNotEmpty;
      if (next != _hasText) {
        setState(() => _hasText = next);
      }
    });
  }

  @override
  void dispose() {
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _submit() {
    if (widget.busy) {
      KimHaptics.selection();
      widget.onStop();
      return;
    }
    final text = _controller.text.trim();
    if (text.isEmpty) {
      return;
    }
    KimHaptics.selection();
    widget.onSend(text);
    _controller.clear();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final bottom = MediaQuery.paddingOf(context).bottom;
    final canTap = widget.busy || _hasText;
    return Material(
      color: KimTheme.chromeOf(context),
      child: Padding(
        padding: EdgeInsets.fromLTRB(12, 8, 12, 8 + bottom),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Expanded(
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: KimTheme.raisedOf(context),
                  borderRadius: BorderRadius.circular(22),
                  border: Border.all(color: KimTheme.hairlineOf(context)),
                ),
                child: TextField(
                  controller: _controller,
                  focusNode: _focus,
                  minLines: 1,
                  maxLines: 6,
                  enabled: !widget.busy,
                  textInputAction: TextInputAction.newline,
                  decoration: const InputDecoration(
                    hintText: Copy.agentPromptHint,
                    border: InputBorder.none,
                    contentPadding: EdgeInsets.fromLTRB(16, 12, 16, 12),
                  ),
                  onSubmitted: (_) {
                    if (!widget.busy) {
                      _submit();
                    }
                  },
                ),
              ),
            ),
            const SizedBox(width: 8),
            SizedBox(
              width: 44,
              height: 44,
              child: FilledButton(
                onPressed: canTap ? _submit : null,
                style: FilledButton.styleFrom(
                  padding: EdgeInsets.zero,
                  shape: const CircleBorder(),
                  backgroundColor: widget.busy ? scheme.error : scheme.primary,
                ),
                child: Icon(
                  widget.busy ? LucideIcons.square : LucideIcons.arrowUp,
                  size: 18,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
