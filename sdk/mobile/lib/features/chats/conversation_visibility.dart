library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/core/logger.dart';
import 'package:kim_mobile/features/chats/inbox.dart';
import 'package:kim_mobile/features/session/location.dart';
import 'package:kim_mobile/features/session/providers.dart';
import 'package:kim_mobile/models/models.dart';

class ConversationVisibilityState {
  const ConversationVisibilityState({
    this.generation = 0,
    this.foreground = false,
    this.dest,
    this.kind = ThreadKind.user,
  });

  final int generation;
  final bool foreground;
  final String? dest;
  final ThreadKind kind;

  ConversationVisibilityState copyWith({
    int? generation,
    bool? foreground,
    Object? dest = _unset,
    ThreadKind? kind,
  }) {
    return ConversationVisibilityState(
      generation: generation ?? this.generation,
      foreground: foreground ?? this.foreground,
      dest: identical(dest, _unset) ? this.dest : dest as String?,
      kind: kind ?? this.kind,
    );
  }
}

const _unset = Object();

class ConversationVisibilityNotifier
    extends Notifier<ConversationVisibilityState>
    with WidgetsBindingObserver {
  var _bound = false;
  var _generation = 0;
  var _resumed = true;

  @override
  ConversationVisibilityState build() {
    if (!_bound) {
      _bound = true;
      WidgetsBinding.instance.addObserver(this);
      ref.onDispose(() {
        WidgetsBinding.instance.removeObserver(this);
        _bound = false;
      });
    }
    final path = ref.watch(locationProvider);
    final dest = chatIdFromPath(path);
    final kind = dest == null
        ? ThreadKind.user
        : (ref.read(threadsProvider).thread(dest)?.kind ?? ThreadKind.user);
    final next = ConversationVisibilityState(
      generation: _generation + 1,
      foreground: _resumed,
      dest: dest,
      kind: kind,
    );
    _generation = next.generation;
    Future<void>.microtask(() => _push(next));
    return next;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    final resumed = state == AppLifecycleState.resumed;
    if (resumed == _resumed) {
      return;
    }
    _resumed = resumed;
    _publish();
  }

  void _publish() {
    final path = ref.read(locationProvider);
    final dest = chatIdFromPath(path);
    final kind = dest == null
        ? ThreadKind.user
        : (ref.read(threadsProvider).thread(dest)?.kind ?? ThreadKind.user);
    final next = ConversationVisibilityState(
      generation: _generation + 1,
      foreground: _resumed,
      dest: dest,
      kind: kind,
    );
    _generation = next.generation;
    state = next;
    unawaited(_push(next));
  }

  Future<void> _push(ConversationVisibilityState next) async {
    try {
      await ref
          .read(clientPortProvider)
          .setConversationVisibility(
            generation: next.generation,
            foreground: next.foreground,
            dest: next.dest ?? '',
            kind: next.kind,
          );
    } catch (e, st) {
      KimLogger.warn('setConversationVisibility', e, st);
    }
  }
}

final conversationVisibilityProvider =
    NotifierProvider<
      ConversationVisibilityNotifier,
      ConversationVisibilityState
    >(ConversationVisibilityNotifier.new);
