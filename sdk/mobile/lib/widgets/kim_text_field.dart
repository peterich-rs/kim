library;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../copy.dart';

class KimTextField extends StatefulWidget {
  const KimTextField({
    super.key,
    required this.controller,
    required this.label,
    this.obscureable = false,
    this.onEditingComplete,
    this.keyboardType,
    this.textInputAction = TextInputAction.next,
    this.errorText,
    this.helperText,
    this.hintText,
    this.maxLength,
    this.autofocus = false,
    this.enabled = true,
    this.autocorrect,
    this.enableSuggestions,
    this.autofillHints,
    this.prefixIcon,
  });

  final TextEditingController controller;
  final String label;
  final bool obscureable;
  final VoidCallback? onEditingComplete;
  final TextInputType? keyboardType;
  final TextInputAction textInputAction;
  final String? errorText;
  final String? helperText;
  final String? hintText;
  final int? maxLength;
  final bool autofocus;
  final bool enabled;
  final bool? autocorrect;
  final bool? enableSuggestions;
  final Iterable<String>? autofillHints;
  final IconData? prefixIcon;

  @override
  State<KimTextField> createState() => _KimTextFieldState();
}

class _KimTextFieldState extends State<KimTextField> {
  late bool _obscured = widget.obscureable;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: TextField(
        controller: widget.controller,
        obscureText: _obscured,
        enableSuggestions: widget.enableSuggestions ?? !widget.obscureable,
        autocorrect: widget.autocorrect ?? !widget.obscureable,
        keyboardType: widget.keyboardType,
        textInputAction: widget.textInputAction,
        onEditingComplete: widget.onEditingComplete,
        autofocus: widget.autofocus,
        enabled: widget.enabled,
        maxLength: widget.maxLength,
        autofillHints: widget.autofillHints,
        style: const TextStyle(fontSize: 16, height: 1.3),
        decoration: InputDecoration(
          labelText: widget.label,
          hintText: widget.hintText,
          errorText: widget.errorText,
          helperText: widget.helperText,
          counterText: widget.maxLength == null ? null : '',
          filled: true,
          prefixIcon: widget.prefixIcon == null
              ? null
              : Icon(widget.prefixIcon, size: 18),
          suffixIcon: widget.obscureable
              ? IconButton(
                  tooltip: _obscured ? Copy.showPassword : Copy.hidePassword,
                  onPressed: () => setState(() => _obscured = !_obscured),
                  icon: Icon(
                    _obscured ? LucideIcons.eye : LucideIcons.eyeOff,
                    size: 18,
                  ),
                )
              : null,
        ),
      ),
    );
  }
}
