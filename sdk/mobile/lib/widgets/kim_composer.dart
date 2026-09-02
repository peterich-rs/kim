library;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../copy.dart';
import '../core/haptics.dart';
import '../theme/kim_theme.dart';
import '../theme/motion.dart';
import 'kim_hairline.dart';

/// Telegram-style composer: capsule field, send/plus crossfade, media panel.
class KimComposer extends StatefulWidget {
  const KimComposer({
    super.key,
    required this.onSend,
    required this.onPickAlbum,
    required this.onTakePhoto,
    this.hintText,
  });

  final ValueChanged<String> onSend;
  final VoidCallback onPickAlbum;
  final VoidCallback onTakePhoto;
  final String? hintText;

  @override
  KimComposerState createState() => KimComposerState();
}

class KimComposerState extends State<KimComposer> {
  final _controller = TextEditingController();
  final _focus = FocusNode();
  var _panel = false;
  var _hasText = false;
  var _pressed = false;

  @override
  void initState() {
    super.initState();
    _controller.addListener(_onText);
    _focus.addListener(_onFocus);
  }

  @override
  void dispose() {
    _controller.removeListener(_onText);
    _focus.removeListener(_onFocus);
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  void quote(String text) {
    final clipped = text.trim();
    if (clipped.isEmpty) {
      return;
    }
    final prefix = '「$clipped」\n';
    final next = _controller.text;
    final already = next.startsWith(prefix) ? next : '$prefix$next';
    _controller.value = TextEditingValue(
      text: already,
      selection: TextSelection.collapsed(offset: already.length),
    );
    _focus.requestFocus();
  }

  void _onText() {
    final next = _controller.text.trim().isNotEmpty;
    if (next == _hasText) {
      return;
    }
    setState(() => _hasText = next);
  }

  void _onFocus() {
    if (_focus.hasFocus && _panel) {
      setState(() => _panel = false);
    }
  }

  void _togglePanel() {
    _focus.unfocus();
    setState(() => _panel = !_panel);
  }

  void _submit() {
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
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final raised = KimTheme.raisedOf(context);
    final capsule = BorderRadius.circular(22);
    return Material(
      color: raised,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const KimHairline(),
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 8, 8, 8),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Expanded(
                  child: TextField(
                    controller: _controller,
                    focusNode: _focus,
                    minLines: 1,
                    maxLines: 4,
                    style: const TextStyle(fontSize: KimTheme.fontBody),
                    textInputAction: TextInputAction.send,
                    onSubmitted: (_) => _submit(),
                    decoration: InputDecoration(
                      hintText: widget.hintText ?? Copy.messagePlaceholder,
                      filled: true,
                      fillColor: scheme.surface,
                      isDense: true,
                      contentPadding: const EdgeInsets.fromLTRB(16, 10, 16, 10),
                      border: OutlineInputBorder(
                        borderRadius: capsule,
                        borderSide: BorderSide(
                          color: KimTheme.hairlineOf(context),
                        ),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: capsule,
                        borderSide: BorderSide(
                          color: KimTheme.hairlineOf(context),
                        ),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: capsule,
                        borderSide: BorderSide(color: scheme.primary),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 4),
                Listener(
                  onPointerDown: (_) => setState(() => _pressed = true),
                  onPointerUp: (_) => setState(() => _pressed = false),
                  onPointerCancel: (_) => setState(() => _pressed = false),
                  child: AnimatedScale(
                    scale: _pressed ? 0.9 : 1,
                    duration: KimTheme.motionFast,
                    curve: KimTheme.motionEmphasized,
                    child: AnimatedSwitcher(
                      duration: KimTheme.motionFast,
                      switchInCurve: KimTheme.motionEmphasized,
                      switchOutCurve: KimTheme.motionEmphasized,
                      transitionBuilder: (child, animation) {
                        return FadeTransition(
                          opacity: animation,
                          child: ScaleTransition(
                            scale: animation,
                            child: child,
                          ),
                        );
                      },
                      child: _hasText
                          ? IconButton(
                              key: const Key('composer-send'),
                              tooltip: Copy.send,
                              onPressed: _submit,
                              style: IconButton.styleFrom(
                                backgroundColor: scheme.primary,
                                foregroundColor: scheme.onPrimary,
                              ),
                              icon: const Icon(LucideIcons.send, size: 18),
                            )
                          : IconButton(
                              key: const Key('composer-plus'),
                              tooltip: Copy.plusPanel,
                              onPressed: _togglePanel,
                              icon: Icon(
                                _panel ? LucideIcons.x : LucideIcons.plus,
                                size: 22,
                                color: scheme.onSurfaceVariant,
                              ),
                            ),
                    ),
                  ),
                ),
              ],
            ),
          ),
          AnimatedSize(
            duration: KimMotion.medium,
            curve: KimMotion.standard,
            alignment: Alignment.topCenter,
            child: _panel
                ? _MediaPanel(
                    onPickAlbum: () {
                      setState(() => _panel = false);
                      widget.onPickAlbum();
                    },
                    onTakePhoto: () {
                      setState(() => _panel = false);
                      widget.onTakePhoto();
                    },
                  )
                : const SizedBox(width: double.infinity, height: 0),
          ),
          SizedBox(height: MediaQuery.paddingOf(context).bottom),
        ],
      ),
    );
  }
}

class _MediaPanel extends StatelessWidget {
  const _MediaPanel({required this.onPickAlbum, required this.onTakePhoto});

  final VoidCallback onPickAlbum;
  final VoidCallback onTakePhoto;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 8, 20, 16),
      child: Row(
        children: [
          _ActionTile(
            key: const Key('composer-album'),
            icon: LucideIcons.image,
            label: Copy.album,
            onTap: onPickAlbum,
          ),
          const SizedBox(width: 24),
          _ActionTile(
            key: const Key('composer-camera'),
            icon: LucideIcons.camera,
            label: Copy.camera,
            onTap: onTakePhoto,
          ),
        ],
      ),
    );
  }
}

class _ActionTile extends StatelessWidget {
  const _ActionTile({
    super.key,
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 56,
            height: 56,
            decoration: BoxDecoration(
              color: scheme.surface,
              borderRadius: BorderRadius.circular(KimTheme.radiusField),
              border: Border.all(color: KimTheme.hairlineOf(context)),
            ),
            child: Icon(icon, size: 26, color: scheme.onSurface),
          ),
          const SizedBox(height: 8),
          Text(
            label,
            style: theme.textTheme.labelSmall?.copyWith(
              color: scheme.onSurfaceVariant,
              fontSize: KimTheme.fontMeta,
            ),
          ),
        ],
      ),
    );
  }
}
