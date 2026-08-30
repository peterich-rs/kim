/// Material 3 theme via flex_color_scheme. Layered surfaces, not iMessage bubbles.
library;

import 'package:flex_color_scheme/flex_color_scheme.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart';

abstract final class KimTheme {
  static const Color seed = Color(0xFF0F766E);
  static const Color outgoing = Color(0xFF0D9488);

  static const double radiusControl = 8;
  static const double radiusField = 12;
  static const double radiusCard = 16;
  static const double radiusSheet = 24;

  static const Color _chatCanvasLight = Color(0xFFF4F5F7);
  static const Color _chatCanvasDark = Color(0xFF121417);

  static Color canvasOf(BuildContext context) =>
      Theme.of(context).colorScheme.surfaceContainerLowest;

  static Color chromeOf(BuildContext context) =>
      Theme.of(context).colorScheme.surface;

  static Color raisedOf(BuildContext context) =>
      Theme.of(context).colorScheme.surfaceContainerLow;

  static Color chatCanvasOf(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return brightness == Brightness.dark ? _chatCanvasDark : _chatCanvasLight;
  }

  static Color hairlineOf(BuildContext context) =>
      Theme.of(context).colorScheme.outlineVariant.withValues(alpha: 0.72);

  static ThemeData light() => _from(Brightness.light);

  static ThemeData dark() => _from(Brightness.dark);

  static ThemeData _from(Brightness brightness) {
    final isLight = brightness == Brightness.light;
    final colors = FlexSchemeColor.from(primary: seed);
    final base = isLight
        ? FlexThemeData.light(
            colors: colors,
            useMaterial3: true,
            visualDensity: FlexColorScheme.comfortablePlatformDensity,
            subThemesData: _subThemes,
            appBarStyle: FlexAppBarStyle.surface,
            appBarElevation: 0,
            surfaceMode: FlexSurfaceMode.levelSurfacesLowScaffold,
            blendLevel: 8,
          )
        : FlexThemeData.dark(
            colors: colors,
            useMaterial3: true,
            visualDensity: FlexColorScheme.comfortablePlatformDensity,
            subThemesData: _subThemes,
            appBarStyle: FlexAppBarStyle.surface,
            appBarElevation: 0,
            darkIsTrueBlack: true,
            surfaceMode: FlexSurfaceMode.levelSurfacesLowScaffold,
            blendLevel: 8,
          );
    final scheme = base.colorScheme;
    final hairline = scheme.outlineVariant.withValues(alpha: 0.72);
    return base.copyWith(
      scaffoldBackgroundColor: scheme.surfaceContainerLowest,
      pageTransitionsTheme: const PageTransitionsTheme(
        builders: {
          TargetPlatform.android: PredictiveBackPageTransitionsBuilder(),
          TargetPlatform.iOS: CupertinoPageTransitionsBuilder(),
          TargetPlatform.macOS: CupertinoPageTransitionsBuilder(),
        },
      ),
      appBarTheme: base.appBarTheme.copyWith(
        centerTitle: false,
        backgroundColor: scheme.surface,
        foregroundColor: scheme.onSurface,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        shape: Border(bottom: BorderSide(color: hairline, width: 0.5)),
      ),
      navigationBarTheme: base.navigationBarTheme.copyWith(
        backgroundColor: scheme.surface,
        elevation: 0,
        height: 56,
        shadowColor: Colors.transparent,
        surfaceTintColor: Colors.transparent,
        indicatorColor: scheme.primary.withValues(alpha: 0.14),
        labelTextStyle: WidgetStateProperty.resolveWith((states) {
          final selected = states.contains(WidgetState.selected);
          return TextStyle(
            fontSize: 11,
            fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
            color: selected ? scheme.primary : scheme.onSurfaceVariant,
          );
        }),
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusField),
        ),
      ),
      dividerTheme: DividerThemeData(
        color: hairline,
        space: 0.5,
        thickness: 0.5,
      ),
      searchBarTheme: base.searchBarTheme.copyWith(
        elevation: const WidgetStatePropertyAll(0),
        backgroundColor: WidgetStatePropertyAll(scheme.surfaceContainerLow),
        shadowColor: const WidgetStatePropertyAll(Colors.transparent),
        overlayColor: const WidgetStatePropertyAll(Colors.transparent),
        side: WidgetStatePropertyAll(BorderSide(color: hairline)),
        shape: WidgetStatePropertyAll(
          RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(radiusField),
          ),
        ),
      ),
    );
  }

  static const FlexSubThemesData _subThemes = FlexSubThemesData(
    interactionEffects: true,
    tintedDisabledControls: true,
    blendOnLevel: 10,
    useM2StyleDividerInM3: false,
    defaultRadius: radiusField,
    inputDecoratorRadius: radiusField,
    inputDecoratorBorderType: FlexInputBorderType.outline,
    filledButtonRadius: radiusField,
    elevatedButtonRadius: radiusField,
    outlinedButtonRadius: radiusField,
    filledButtonSchemeColor: SchemeColor.primary,
    navigationBarHeight: 56,
    navigationBarIndicatorRadius: 18,
    navigationBarLabelBehavior: NavigationDestinationLabelBehavior.alwaysShow,
    cardRadius: radiusCard,
    bottomSheetRadius: radiusSheet,
    alignedDropdown: true,
  );

  static ChatTheme chat(ThemeData theme) {
    final scheme = theme.colorScheme;
    return ChatTheme.fromThemeData(theme).copyWith(
      colors: ChatColors(
        primary: scheme.primary,
        onPrimary: scheme.onPrimary,
        surface: chatCanvasColor(scheme),
        onSurface: scheme.onSurface,
        surfaceContainer: scheme.surfaceContainerLow,
        surfaceContainerLow: scheme.surfaceContainerLow,
        surfaceContainerHigh: scheme.surfaceContainerHigh,
      ),
      typography: ChatTypography.fromThemeData(theme).copyWith(
        bodyMedium: (theme.textTheme.bodyMedium ?? const TextStyle()).copyWith(
          fontSize: 15,
          height: 1.375,
        ),
        labelSmall: (theme.textTheme.labelSmall ?? const TextStyle()).copyWith(
          fontSize: 11,
          height: 1.2,
        ),
      ),
    );
  }

  static Color chatCanvasColor(ColorScheme scheme) {
    return scheme.brightness == Brightness.dark
        ? _chatCanvasDark
        : _chatCanvasLight;
  }
}
