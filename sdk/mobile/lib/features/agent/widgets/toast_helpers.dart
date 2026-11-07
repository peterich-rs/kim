/// Toast helpers shared by agent editor pages.
library;

import 'package:flutter/material.dart';
import 'package:toastification/toastification.dart';

void showAgentToastInfo(BuildContext context, String message) {
  toastification.show(
    context: context,
    type: ToastificationType.info,
    title: Text(message),
    autoCloseDuration: const Duration(seconds: 3),
  );
}

void showAgentToastError(BuildContext context, String message) {
  toastification.show(
    context: context,
    type: ToastificationType.error,
    title: Text(message),
    autoCloseDuration: const Duration(seconds: 4),
  );
}
