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
  });

  final TextEditingController controller;
  final String label;
  final bool obscureable;
  final VoidCallback? onEditingComplete;
  final TextInputType? keyboardType;
  final TextInputAction textInputAction;

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
        enableSuggestions: !widget.obscureable,
        autocorrect: !widget.obscureable,
        keyboardType: widget.keyboardType,
        textInputAction: widget.textInputAction,
        onEditingComplete: widget.onEditingComplete,
        decoration: InputDecoration(
          labelText: widget.label,
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
