library;

import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../copy.dart';
import '../core/haptics.dart';
import '../theme/kim_theme.dart';
import '../theme/motion.dart';

/// Floating composer chrome: circular +, stadium field, circular send (↑).
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
  var _sendPressed = false;
  var _plusPressed = false;

  static const double _btn = 40;
  static const double _fieldRadius = 22;

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
    final bottom = MediaQuery.paddingOf(context).bottom;
    final frost = KimTheme.frostFillOf(context);
    final stadium = BorderRadius.circular(_fieldRadius);

    return Material(
      color: Colors.transparent,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: EdgeInsets.fromLTRB(12, 6, 12, _panel ? 4 : 8 + bottom),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                _FrostCircle(
                  key: const Key('composer-plus'),
                  size: _btn,
                  pressed: _plusPressed,
                  onPressedChanged: (v) => setState(() => _plusPressed = v),
                  onTap: _togglePanel,
                  tooltip: Copy.plusPanel,
                  child: Icon(
                    _panel ? LucideIcons.x : LucideIcons.plus,
                    size: 22,
                    color: scheme.onSurface,
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: ClipRRect(
                    borderRadius: stadium,
                    child: BackdropFilter(
                      filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
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
                          fillColor: frost,
                          isDense: true,
                          contentPadding: const EdgeInsets.fromLTRB(
                            16,
                            10,
                            16,
                            10,
                          ),
                          border: OutlineInputBorder(
                            borderRadius: stadium,
                            borderSide: BorderSide.none,
                          ),
                          enabledBorder: OutlineInputBorder(
                            borderRadius: stadium,
                            borderSide: BorderSide.none,
                          ),
                          focusedBorder: OutlineInputBorder(
                            borderRadius: stadium,
                            borderSide: BorderSide(
                              color: scheme.outline.withValues(alpha: 0.35),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                _SolidCircle(
                  key: const Key('composer-send'),
                  size: _btn,
                  color: scheme.onSurface,
                  pressed: _sendPressed,
                  enabled: _hasText,
                  onPressedChanged: (v) => setState(() => _sendPressed = v),
                  onTap: _submit,
                  tooltip: Copy.send,
                  child: Icon(
                    LucideIcons.arrowUp,
                    size: 20,
                    color: scheme.surface,
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
                ? Padding(
                    padding: EdgeInsets.only(bottom: 8 + bottom),
                    child: _MediaPanel(
                      onPickAlbum: () {
                        setState(() => _panel = false);
                        widget.onPickAlbum();
                      },
                      onTakePhoto: () {
                        setState(() => _panel = false);
                        widget.onTakePhoto();
                      },
                    ),
                  )
                : const SizedBox(width: double.infinity, height: 0),
          ),
        ],
      ),
    );
  }
}

class _FrostCircle extends StatelessWidget {
  const _FrostCircle({
    super.key,
    required this.size,
    required this.child,
    required this.onTap,
    required this.pressed,
    required this.onPressedChanged,
    this.tooltip,
  });

  final double size;
  final Widget child;
  final VoidCallback onTap;
  final bool pressed;
  final ValueChanged<bool> onPressedChanged;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final button = Listener(
      onPointerDown: (_) => onPressedChanged(true),
      onPointerUp: (_) => onPressedChanged(false),
      onPointerCancel: (_) => onPressedChanged(false),
      child: AnimatedScale(
        scale: pressed ? 0.9 : 1,
        duration: KimTheme.motionFast,
        curve: KimTheme.motionEmphasized,
        child: ClipOval(
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
            child: Material(
              color: KimTheme.frostFillOf(context),
              shape: const CircleBorder(),
              child: InkWell(
                customBorder: const CircleBorder(),
                onTap: onTap,
                child: SizedBox(
                  width: size,
                  height: size,
                  child: Center(child: child),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    if (tooltip == null) {
      return button;
    }
    return Tooltip(message: tooltip!, child: button);
  }
}

class _SolidCircle extends StatelessWidget {
  const _SolidCircle({
    super.key,
    required this.size,
    required this.color,
    required this.child,
    required this.onTap,
    required this.pressed,
    required this.onPressedChanged,
    this.enabled = true,
    this.tooltip,
  });

  final double size;
  final Color color;
  final Widget child;
  final VoidCallback onTap;
  final bool pressed;
  final ValueChanged<bool> onPressedChanged;
  final bool enabled;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final button = Listener(
      onPointerDown: enabled ? (_) => onPressedChanged(true) : null,
      onPointerUp: (_) => onPressedChanged(false),
      onPointerCancel: (_) => onPressedChanged(false),
      child: AnimatedScale(
        scale: pressed ? 0.9 : 1,
        duration: KimTheme.motionFast,
        curve: KimTheme.motionEmphasized,
        child: AnimatedOpacity(
          opacity: enabled ? 1 : 0.38,
          duration: KimTheme.motionFast,
          child: Material(
            color: color,
            shape: const CircleBorder(),
            clipBehavior: Clip.antiAlias,
            child: InkWell(
              customBorder: const CircleBorder(),
              onTap: enabled ? onTap : null,
              child: SizedBox(
                width: size,
                height: size,
                child: Center(child: child),
              ),
            ),
          ),
        ),
      ),
    );
    if (tooltip == null) {
      return button;
    }
    return Tooltip(message: tooltip!, child: button);
  }
}

class _MediaPanel extends StatelessWidget {
  const _MediaPanel({required this.onPickAlbum, required this.onTakePhoto});

  final VoidCallback onPickAlbum;
  final VoidCallback onTakePhoto;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 4, 20, 8),
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
              color: scheme.surfaceContainerHigh,
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
