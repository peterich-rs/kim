library;

import 'package:flutter/material.dart';

/// Material 3 filled dense field used across the shell.
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
    this.maxLength,
    this.autofocus = false,
    this.enabled = true,
    this.autocorrect,
    this.enableSuggestions,
    this.autofillHints,
  });

  final TextEditingController controller;
  final String label;
  final bool obscureable;
  final VoidCallback? onEditingComplete;
  final TextInputType? keyboardType;
  final TextInputAction textInputAction;
  final String? errorText;
  final String? helperText;
  final int? maxLength;
  final bool autofocus;
  final bool enabled;
  final bool? autocorrect;
  final bool? enableSuggestions;
  final Iterable<String>? autofillHints;

  @override
  State<KimTextField> createState() => _KimTextFieldState();
}

class _KimTextFieldState extends State<KimTextField> {
  late bool _obscured = widget.obscureable;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 8),
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
        decoration: InputDecoration(
          labelText: widget.label,
          errorText: widget.errorText,
          helperText: widget.helperText,
          counterText: widget.maxLength == null ? null : '',
          suffixIcon: widget.obscureable
              ? IconButton(
                  tooltip: _obscured ? 'Show' : 'Hide',
                  onPressed: () => setState(() => _obscured = !_obscured),
                  icon: Icon(
                    _obscured ? Icons.visibility : Icons.visibility_off,
                  ),
                )
              : null,
        ),
      ),
    );
  }
}
