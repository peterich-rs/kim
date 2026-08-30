/// MaterialApp shell: M3 light/dark, connectivity banner, tap-outside dismiss.
library;

import 'package:animations/animations.dart';
import 'package:flutter/material.dart';

import 'core/runtime.dart';
import 'kim_bridge.dart';
import 'screens/auth_page.dart';
import 'screens/shell_page.dart';
import 'theme/kim_theme.dart';
import 'widgets/kim_offline_banner.dart';

class KimApp extends StatefulWidget {
  const KimApp({super.key, required this.runtime, this.bridge, this.auth});

  final KimRuntime runtime;
  final KimBridge? bridge;
  final KimAuthPort? auth;

  @override
  State<KimApp> createState() => _KimAppState();
}

class _KimAppState extends State<KimApp> {
  late final KimBridge _bridge;
  late final KimAuthPort _auth;
  late bool _signedIn;

  @override
  void initState() {
    super.initState();
    _bridge = widget.bridge ?? KimBridge();
    _auth = widget.auth ?? _bridge;
    _signedIn = widget.runtime.settings.token.isNotEmpty;
  }

  void _signedInNow() {
    setState(() => _signedIn = true);
  }

  void _signedOutNow() {
    setState(() => _signedIn = false);
  }

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
            connectivity: widget.runtime.connectivity,
            child: child ?? const SizedBox.shrink(),
          ),
        );
      },
      home: PageTransitionSwitcher(
        duration: KimTheme.pageDuration,
        transitionBuilder: (child, animation, secondaryAnimation) {
          return SharedAxisTransition(
            animation: animation,
            secondaryAnimation: secondaryAnimation,
            transitionType: SharedAxisTransitionType.horizontal,
            child: child,
          );
        },
        child: _signedIn
            ? ShellPage(
                key: const ValueKey('shell'),
                runtime: widget.runtime,
                bridge: _bridge,
                auth: _auth,
                onSignedOut: _signedOutNow,
              )
            : AuthPage(
                key: const ValueKey('auth'),
                runtime: widget.runtime,
                auth: _auth,
                onSignedIn: _signedInNow,
              ),
      ),
    );
  }
}
