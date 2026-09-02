/// Reverse chat list: bottom-anchored newest messages, keyboard-safe, paginated.
library;

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import '../../copy.dart';
import '../../models/models.dart';
import '../../theme/kim_theme.dart';

class ChatListController {
  _ChatListState? _state;

  bool get atBottomEdge => _state?._atBottom ?? true;

  void jumpToMessage(String key, {double alignment = 0.5}) {
    _state?._jumpTo(key, alignment: alignment);
  }

  Future<void> scrollToBottom({required bool animated}) {
    return _state?._scrollToBottom(animated: animated) ?? Future<void>.value();
  }

  void _attach(_ChatListState state) => _state = state;

  void _detach(_ChatListState state) {
    if (_state == state) {
      _state = null;
    }
  }
}

class ChatList extends StatefulWidget {
  const ChatList({
    super.key,
    required this.items,
    required this.itemBuilder,
    this.controller,
    this.onLoadOlder,
    this.loadingOlder = false,
    this.hasMore = true,
    this.empty,
    this.padding = const EdgeInsets.only(bottom: 8),
  });

  final List<KimChatMsg> items;
  final Widget Function(BuildContext context, KimChatMsg msg, int index)
  itemBuilder;
  final ChatListController? controller;
  final VoidCallback? onLoadOlder;
  final bool loadingOlder;
  final bool hasMore;
  final Widget? empty;
  final EdgeInsets padding;

  @override
  State<ChatList> createState() => _ChatListState();
}

class _ChatListState extends State<ChatList> {
  static const _bottomSlop = 48.0;
  static const _topSlop = 96.0;

  final _scroll = ScrollController();
  final _keys = <String, GlobalKey>{};
  var _atBottom = true;
  var _unseen = 0;
  String? _anchorKey;
  double _anchorDy = 0;
  var _loadArmed = true;

  @override
  void initState() {
    super.initState();
    widget.controller?._attach(this);
    _scroll.addListener(_onScroll);
  }

  @override
  void didUpdateWidget(covariant ChatList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller?._detach(this);
      widget.controller?._attach(this);
    }
    _reconcileNew(oldWidget.items);
    _schedulePrependRestore(oldWidget.items);
  }

  @override
  void dispose() {
    widget.controller?._detach(this);
    _scroll.removeListener(_onScroll);
    _scroll.dispose();
    super.dispose();
  }

  GlobalKey _keyFor(String id) => _keys.putIfAbsent(id, GlobalKey.new);

  bool get _stuckToBottom {
    if (!_scroll.hasClients) {
      return true;
    }
    return _scroll.position.pixels <= _bottomSlop;
  }

  void _onScroll() {
    if (!_scroll.hasClients) {
      return;
    }
    final next = _stuckToBottom;
    var dirty = false;
    if (next != _atBottom) {
      _atBottom = next;
      dirty = true;
    }
    if (next && _unseen != 0) {
      _unseen = 0;
      dirty = true;
    }
    if (dirty && mounted) {
      setState(() {});
    }
    _maybeLoadOlder();
  }

  void _maybeLoadOlder() {
    if (!_scroll.hasClients || !widget.hasMore || widget.loadingOlder) {
      return;
    }
    if (!_loadArmed) {
      return;
    }
    if (_scroll.position.pixels >=
        _scroll.position.maxScrollExtent - _topSlop) {
      _loadArmed = false;
      widget.onLoadOlder?.call();
    }
  }

  void _reconcileNew(List<KimChatMsg> previous) {
    if (widget.items.isEmpty) {
      return;
    }
    if (previous.isEmpty) {
      return;
    }
    final oldLast = previous.last.key;
    final newLast = widget.items.last.key;
    if (oldLast == newLast) {
      if (!widget.loadingOlder) {
        _loadArmed = true;
      }
      return;
    }
    final grown = widget.items.length >= previous.length;
    if (!grown) {
      return;
    }
    if (_stuckToBottom) {
      _atBottom = true;
      _unseen = 0;
      return;
    }
    final delta = widget.items.length - previous.length;
    _unseen += delta > 0 ? delta : 1;
  }

  void _schedulePrependRestore(List<KimChatMsg> previous) {
    if (previous.isEmpty || widget.items.isEmpty) {
      if (!widget.loadingOlder) {
        _loadArmed = true;
      }
      return;
    }
    final prepended =
        widget.items.length > previous.length &&
        widget.items.last.key == previous.last.key &&
        widget.items.first.key != previous.first.key;
    if (!prepended) {
      if (!widget.loadingOlder) {
        _loadArmed = true;
      }
      return;
    }
    _captureAnchor();
    SchedulerBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      _restoreAnchor();
      _loadArmed = true;
    });
  }

  void _captureAnchor() {
    if (_stuckToBottom) {
      _anchorKey = null;
      return;
    }
    final box = context.findRenderObject() as RenderBox?;
    if (box == null || !box.hasSize) {
      return;
    }
    final top = box.localToGlobal(Offset.zero).dy;
    final bottom = top + box.size.height;
    for (final msg in widget.items) {
      final ctx = _keys[msg.key]?.currentContext;
      if (ctx == null) {
        continue;
      }
      final item = ctx.findRenderObject() as RenderBox?;
      if (item == null || !item.hasSize) {
        continue;
      }
      final dy = item.localToGlobal(Offset.zero).dy;
      if (dy >= top - 1 && dy <= bottom) {
        _anchorKey = msg.key;
        _anchorDy = dy;
        return;
      }
    }
  }

  void _restoreAnchor() {
    final key = _anchorKey;
    if (key == null || !_scroll.hasClients) {
      return;
    }
    final ctx = _keys[key]?.currentContext;
    if (ctx == null) {
      return;
    }
    final item = ctx.findRenderObject() as RenderBox?;
    if (item == null || !item.hasSize) {
      return;
    }
    final dy = item.localToGlobal(Offset.zero).dy;
    final delta = dy - _anchorDy;
    if (delta.abs() < 0.5) {
      return;
    }
    final next = (_scroll.position.pixels - delta).clamp(
      0.0,
      _scroll.position.maxScrollExtent,
    );
    _scroll.jumpTo(next);
  }

  void _onViewport(double height, double previous) {
    if (previous <= 0 || (height - previous).abs() < 1) {
      return;
    }
    if (_stuckToBottom) {
      return;
    }
    _captureAnchor();
    SchedulerBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _stuckToBottom) {
        return;
      }
      _restoreAnchor();
    });
  }

  void _jumpTo(String key, {double alignment = 0.5}) {
    final ctx = _keys[key]?.currentContext;
    if (ctx == null) {
      return;
    }
    Scrollable.ensureVisible(
      ctx,
      alignment: alignment,
      duration: KimTheme.motionBase,
      curve: KimTheme.motionEmphasized,
    );
  }

  Future<void> _scrollToBottom({required bool animated}) async {
    if (!_scroll.hasClients) {
      return;
    }
    setState(() {
      _unseen = 0;
      _atBottom = true;
    });
    if (animated) {
      await _scroll.animateTo(
        0,
        duration: KimTheme.motionBase,
        curve: KimTheme.motionEmphasized,
      );
    } else {
      _scroll.jumpTo(0);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.items.isEmpty) {
      return widget.empty ?? const SizedBox.expand();
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        return _ViewportProbe(
          height: constraints.maxHeight,
          onChange: _onViewport,
          child: Stack(
            children: [
              NotificationListener<ScrollNotification>(
                onNotification: (notification) {
                  if (notification.metrics.axis != Axis.vertical) {
                    return false;
                  }
                  _maybeLoadOlder();
                  return false;
                },
                child: CustomScrollView(
                  key: const Key('chat-list'),
                  reverse: true,
                  controller: _scroll,
                  physics: const AlwaysScrollableScrollPhysics(
                    parent: BouncingScrollPhysics(),
                  ),
                  slivers: [
                    SliverPadding(
                      padding: widget.padding,
                      sliver: SliverList(
                        delegate: SliverChildBuilderDelegate((context, index) {
                          final chrono = widget.items.length - 1 - index;
                          final msg = widget.items[chrono];
                          return KeyedSubtree(
                            key: ValueKey(msg.key),
                            child: KeyedSubtree(
                              key: _keyFor(msg.key),
                              child: widget.itemBuilder(context, msg, chrono),
                            ),
                          );
                        }, childCount: widget.items.length),
                      ),
                    ),
                    if (widget.loadingOlder)
                      const SliverToBoxAdapter(
                        child: Padding(
                          padding: EdgeInsets.symmetric(vertical: 12),
                          child: Center(
                            child: SizedBox(
                              width: 18,
                              height: 18,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
              if (_unseen > 0 && !_atBottom)
                Positioned(
                  right: KimTheme.spaceUnit * 4,
                  bottom: KimTheme.spaceUnit * 4,
                  child: _NewMessagesPill(
                    count: _unseen,
                    onTap: () => _scrollToBottom(animated: true),
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}

class _ViewportProbe extends StatefulWidget {
  const _ViewportProbe({
    required this.height,
    required this.onChange,
    required this.child,
  });

  final double height;
  final void Function(double height, double previous) onChange;
  final Widget child;

  @override
  State<_ViewportProbe> createState() => _ViewportProbeState();
}

class _ViewportProbeState extends State<_ViewportProbe> {
  @override
  void didUpdateWidget(covariant _ViewportProbe oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.height != widget.height) {
      widget.onChange(widget.height, oldWidget.height);
    }
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

class _NewMessagesPill extends StatelessWidget {
  const _NewMessagesPill({required this.count, required this.onTap});

  final int count;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      key: const Key('new-messages-pill'),
      color: scheme.primary,
      elevation: 2,
      borderRadius: BorderRadius.circular(20),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(20),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 8, 14, 8),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(LucideIcons.arrowDown, size: 14, color: scheme.onPrimary),
              const SizedBox(width: 6),
              Text(
                '$count ${Copy.newMessages}',
                style: TextStyle(
                  color: scheme.onPrimary,
                  fontSize: KimTheme.fontMeta,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
