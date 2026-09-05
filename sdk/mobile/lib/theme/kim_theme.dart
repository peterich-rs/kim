/// Material 3 theme via flex_color_scheme, overlaid with KIM product tokens.
library;

import 'package:flex_color_scheme/flex_color_scheme.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';

abstract final class KimTheme {
  static const Color seed = Color(0xFF0F766E);
  static const Color outgoing = Color(0xFF0D9488);
  static const Color bubbleOwnStart = Color(0xFF14B8A6);
  static const Color bubbleOwnEnd = Color(0xFF0D9488);

  static const double fontTitle = 17;
  static const double fontBody = 15.5;
  static const double fontMeta = 12.5;

  static const double radiusControl = 8;
  static const double radiusField = 12;
  static const double radiusBubble = 14;
  static const double radiusBubbleTail = 6;
  static const double radiusCard = 16;
  static const double radiusSheet = 24;

  static const double spaceUnit = 4;

  static const Duration motionFast = Duration(milliseconds: 180);
  static const Duration motionBase = Duration(milliseconds: 260);
  static const Curve motionEmphasized = Curves.easeOutCubic;

  static const Color _chatCanvasLight = Color(0xFFF2F5F8);
  static const Color _chatCanvasDark = Color(0xFF0E1621);

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

  /// Semi-transparent frosted chip fill (floating chrome over chat).
  static Color frostFillOf(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return (dark ? const Color(0xFF000000) : const Color(0xFFFFFFFF))
        .withValues(alpha: dark ? 0.45 : 0.65);
  }

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
    final text = base.textTheme;
    return base.copyWith(
      scaffoldBackgroundColor: scheme.surfaceContainerLowest,
      pageTransitionsTheme: const PageTransitionsTheme(
        builders: {
          TargetPlatform.android: PredictiveBackPageTransitionsBuilder(),
          TargetPlatform.iOS: CupertinoPageTransitionsBuilder(),
          TargetPlatform.macOS: CupertinoPageTransitionsBuilder(),
        },
      ),
      textTheme: text.copyWith(
        titleMedium: (text.titleMedium ?? const TextStyle()).copyWith(
          fontSize: fontTitle,
          fontWeight: FontWeight.w600,
          height: 1.25,
          letterSpacing: -0.2,
        ),
        bodyMedium: (text.bodyMedium ?? const TextStyle()).copyWith(
          fontSize: fontBody,
          height: 1.35,
        ),
        bodySmall: (text.bodySmall ?? const TextStyle()).copyWith(
          fontSize: fontMeta,
          height: 1.3,
        ),
        labelSmall: (text.labelSmall ?? const TextStyle()).copyWith(
          fontSize: fontMeta,
          height: 1.2,
        ),
      ),
      appBarTheme: base.appBarTheme.copyWith(
        centerTitle: false,
        backgroundColor: scheme.surface,
        foregroundColor: scheme.onSurface,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        shape: Border(bottom: BorderSide(color: hairline, width: 0.5)),
        titleTextStyle: TextStyle(
          fontSize: fontTitle,
          fontWeight: FontWeight.w600,
          color: scheme.onSurface,
          height: 1.2,
        ),
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
}
