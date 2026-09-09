/// User-facing copy. Source of truth is ARB (`lib/l10n/app_*.arb`).
///
/// Widgets with a [BuildContext] should prefer [AppLocalizations.of].
/// Notifiers, formatters, and tests use [Copy] / [kimL10n] against `zh`.
library;

import 'package:flutter/widgets.dart';

import 'l10n/app_localizations.dart';

export 'l10n/app_localizations.dart';

AppLocalizations kimL10n([BuildContext? context]) {
  if (context != null) {
    return AppLocalizations.of(context);
  }
  return lookupAppLocalizations(const Locale('zh'));
}

/// Generated l10n for the default product locale.
// ignore: non_constant_identifier_names
AppLocalizations get Copy => kimL10n();
