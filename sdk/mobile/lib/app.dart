/// MaterialApp shell: M3 light/dark, connectivity banner, tap-outside dismiss.
library;

import 'package:flutter/material.dart';

import 'core/runtime.dart';
import 'screens/shell_page.dart';
import 'theme/kim_theme.dart';
import 'widgets/kim_offline_banner.dart';

class KimApp extends StatelessWidget {
  const KimApp({super.key, required this.runtime});

  final KimRuntime runtime;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'KIM',
      debugShowCheckedModeBanner: false,
      theme: KimTheme.light(),
      darkTheme: KimTheme.dark(),
      themeMode: ThemeMode.system,
      builder: (context, child) {
        return GestureDetector(
          onTap: () => FocusManager.instance.primaryFocus?.unfocus(),
          behavior: HitTestBehavior.translucent,
          child: KimOfflineBanner(
            connectivity: runtime.connectivity,
            child: child ?? const SizedBox.shrink(),
          ),
        );
      },
      initialRoute: '/',
      routes: {'/': (_) => ShellPage(runtime: runtime)},
    );
  }
}
