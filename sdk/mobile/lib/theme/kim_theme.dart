/// Material 3 theme via flex_color_scheme. Teal brand, iMessage-like radii.
library;

import 'package:flex_color_scheme/flex_color_scheme.dart';
import 'package:flutter/material.dart';
import 'package:flutter_chat_core/flutter_chat_core.dart';

abstract final class KimTheme {
  static const Color seed = Color(0xFF0F766E);
  static const Color outgoing = Color(0xFF0D9488);
  static const Color incomingLight = Color(0xFFE9E9EB);
  static const Color incomingDark = Color(0xFF2C2C2E);

  static const BorderRadius bubbleRadius = BorderRadius.all(Radius.circular(19));

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
    return base.copyWith(
      appBarTheme: base.appBarTheme.copyWith(
        centerTitle: false,
        backgroundColor: scheme.surface,
        foregroundColor: scheme.onSurface,
        surfaceTintColor: Colors.transparent,
      ),
      navigationBarTheme: base.navigationBarTheme.copyWith(
        backgroundColor: scheme.surface,
        elevation: 0,
        height: 68,
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
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
      dividerTheme: DividerThemeData(
        color: scheme.outlineVariant.withValues(alpha: 0.6),
        space: 0.5,
        thickness: 0.5,
      ),
    );
  }

  static const FlexSubThemesData _subThemes = FlexSubThemesData(
    interactionEffects: true,
    tintedDisabledControls: true,
    blendOnLevel: 10,
    useM2StyleDividerInM3: false,
    defaultRadius: 16,
    inputDecoratorRadius: 16,
    inputDecoratorBorderType: FlexInputBorderType.outline,
    filledButtonRadius: 16,
    elevatedButtonRadius: 16,
    outlinedButtonRadius: 16,
    filledButtonSchemeColor: SchemeColor.primary,
    navigationBarHeight: 68,
    navigationBarIndicatorRadius: 18,
    navigationBarLabelBehavior: NavigationDestinationLabelBehavior.alwaysShow,
    cardRadius: 20,
    bottomSheetRadius: 24,
    alignedDropdown: true,
  );

  static ChatTheme chat(ThemeData theme) {
    final scheme = theme.colorScheme;
    final isLight = scheme.brightness == Brightness.light;
    return ChatTheme.fromThemeData(theme).copyWith(
      shape: bubbleRadius,
      colors: ChatColors(
        primary: outgoing,
        onPrimary: Colors.white,
        surface: scheme.surface,
        onSurface: scheme.onSurface,
        surfaceContainer: isLight ? incomingLight : incomingDark,
        surfaceContainerLow: scheme.surfaceContainerLow,
        surfaceContainerHigh: scheme.surfaceContainerHigh,
      ),
      typography: ChatTypography.fromThemeData(theme).copyWith(
        bodyMedium: (theme.textTheme.bodyLarge ?? const TextStyle()).copyWith(
          fontSize: 17,
          height: 1.3,
        ),
      ),
    );
  }
}
