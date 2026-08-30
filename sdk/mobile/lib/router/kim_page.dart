library;

import 'dart:math';

import 'package:flutter/cupertino.dart';
import 'package:flutter/gestures.dart';

/// Left-edge width that starts the interactive pop.
///
/// iOS [CupertinoPage] uses 20px, which sits in the curved bezel and loses
/// the arena to a vertical chat list. 80px is still an edge swipe.
const double kKimBackGestureWidth = 80;

/// Push page with a follow-through back gesture on every platform.
///
/// Wider than [CupertinoPage]'s 20px edge. Android keeps an in-app swipe
/// instead of relying only on system predictive back.
Page<void> kimPushPage({
  required LocalKey key,
  required Widget child,
  String? name,
  Object? arguments,
}) {
  return KimSwipePage(key: key, name: name, arguments: arguments, child: child);
}

/// Page that slides in from the right and pops from a wide left-edge drag.
class KimSwipePage extends Page<void> {
  const KimSwipePage({
    required this.child,
    super.key,
    super.name,
    super.arguments,
    super.restorationId,
  });

  final Widget child;

  @override
  Route<void> createRoute(BuildContext context) {
    return _KimSwipePageRoute<void>(page: this);
  }
}

class _KimSwipePageRoute<T> extends PageRoute<T>
    with CupertinoRouteTransitionMixin<T> {
  _KimSwipePageRoute({required KimSwipePage page}) : super(settings: page);

  KimSwipePage get _page => settings as KimSwipePage;

  @override
  Widget buildContent(BuildContext context) => _page.child;

  @override
  String? get title => null;

  @override
  bool get maintainState => true;

  @override
  bool get fullscreenDialog => false;

  @override
  DelegatedTransitionBuilder? get delegatedTransition =>
      CupertinoPageTransition.delegatedTransition;

  @override
  Widget buildTransitions(
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
    Widget child,
  ) {
    return CupertinoPageTransition(
      primaryRouteAnimation: animation,
      secondaryRouteAnimation: secondaryAnimation,
      linearTransition: popGestureInProgress,
      child: _KimBackGestureDetector<T>(
        enabledCallback: () => popGestureEnabled,
        onStartPopGesture: () {
          return _KimBackGestureController<T>(
            navigator: navigator!,
            controller: controller!,
            getIsCurrent: () => isCurrent,
            getIsActive: () => isActive,
          );
        },
        child: child,
      ),
    );
  }
}

class _KimBackGestureDetector<T> extends StatefulWidget {
  const _KimBackGestureDetector({
    required this.enabledCallback,
    required this.onStartPopGesture,
    required this.child,
  });

  final Widget child;
  final ValueGetter<bool> enabledCallback;
  final ValueGetter<_KimBackGestureController<T>> onStartPopGesture;

  @override
  State<_KimBackGestureDetector<T>> createState() =>
      _KimBackGestureDetectorState<T>();
}

class _KimBackGestureDetectorState<T>
    extends State<_KimBackGestureDetector<T>> {
  _KimBackGestureController<T>? _backGestureController;
  late HorizontalDragGestureRecognizer _recognizer;

  @override
  void initState() {
    super.initState();
    _recognizer = HorizontalDragGestureRecognizer(debugOwner: this)
      ..onStart = _handleDragStart
      ..onUpdate = _handleDragUpdate
      ..onEnd = _handleDragEnd
      ..onCancel = _handleDragCancel;
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _recognizer.gestureSettings = MediaQuery.maybeGestureSettingsOf(context);
  }

  @override
  void dispose() {
    _recognizer.dispose();
    if (_backGestureController != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (_backGestureController?.navigator.mounted ?? false) {
          _backGestureController?.navigator.didStopUserGesture();
        }
        _backGestureController = null;
      });
    }
    super.dispose();
  }

  void _handleDragStart(DragStartDetails details) {
    assert(mounted);
    assert(_backGestureController == null);
    _backGestureController = widget.onStartPopGesture();
  }

  void _handleDragUpdate(DragUpdateDetails details) {
    assert(mounted);
    assert(_backGestureController != null);
    _backGestureController!.dragUpdate(
      _convertToLogical(details.primaryDelta! / context.size!.width),
    );
  }

  void _handleDragEnd(DragEndDetails details) {
    assert(mounted);
    assert(_backGestureController != null);
    _backGestureController!.dragEnd(
      _convertToLogical(
        details.velocity.pixelsPerSecond.dx / context.size!.width,
      ),
    );
    _backGestureController = null;
  }

  void _handleDragCancel() {
    assert(mounted);
    _backGestureController?.dragEnd(0.0);
    _backGestureController = null;
  }

  void _handlePointerDown(PointerDownEvent event) {
    if (widget.enabledCallback()) {
      _recognizer.addPointer(event);
    }
  }

  double _convertToLogical(double value) {
    return switch (Directionality.of(context)) {
      TextDirection.rtl => -value,
      TextDirection.ltr => value,
    };
  }

  double _gestureWidth(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    final pad = switch (Directionality.of(context)) {
      TextDirection.rtl => MediaQuery.paddingOf(context).right,
      TextDirection.ltr => MediaQuery.paddingOf(context).left,
    };
    final fraction = min(size.width * 0.22, 96.0);
    return max(pad, max(kKimBackGestureWidth, fraction));
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      fit: StackFit.passthrough,
      children: [
        widget.child,
        PositionedDirectional(
          start: 0,
          width: _gestureWidth(context),
          top: 0,
          bottom: 0,
          child: Listener(
            onPointerDown: _handlePointerDown,
            behavior: HitTestBehavior.translucent,
          ),
        ),
      ],
    );
  }
}

/// Drives the route animation while the user swipes.
///
/// 0.0 is dismissed, 1.0 is fully on screen (same as Cupertino).
class _KimBackGestureController<T> {
  _KimBackGestureController({
    required this.navigator,
    required this.controller,
    required this.getIsCurrent,
    required this.getIsActive,
  }) {
    navigator.didStartUserGesture();
  }

  final AnimationController controller;
  final NavigatorState navigator;
  final ValueGetter<bool> getIsCurrent;
  final ValueGetter<bool> getIsActive;

  void dragUpdate(double delta) {
    controller.value -= delta;
  }

  void dragEnd(double velocity) {
    const curve = Curves.fastEaseInToSlowEaseOut;
    const duration = Duration(milliseconds: 350);
    final isCurrent = getIsCurrent();
    final bool animateForward;
    if (!isCurrent) {
      animateForward = getIsActive();
    } else if (velocity.abs() >= 1.0) {
      animateForward = velocity <= 0;
    } else {
      animateForward = controller.value > 0.5;
    }

    if (animateForward) {
      controller.animateTo(1.0, duration: duration, curve: curve);
    } else {
      if (isCurrent) {
        navigator.pop();
      }
      if (controller.isAnimating) {
        controller.animateBack(0.0, duration: duration, curve: curve);
      }
    }

    if (controller.isAnimating) {
      late final AnimationStatusListener listener;
      listener = (status) {
        navigator.didStopUserGesture();
        controller.removeStatusListener(listener);
      };
      controller.addStatusListener(listener);
    } else {
      navigator.didStopUserGesture();
    }
  }
}
