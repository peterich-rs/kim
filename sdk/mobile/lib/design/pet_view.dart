library;

import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:kim_mobile/design/kim_avatar.dart';
import 'package:kim_mobile/design/pet_atlas_painter.dart';
import 'package:kim_mobile/features/agent/agent_presence.dart';
import 'package:kim_mobile/features/agent/pet_pack.dart';
import 'package:kim_mobile/features/session/typing.dart';

class PetPackScope extends InheritedWidget {
  const PetPackScope({super.key, required this.pack, required super.child});

  final PetPack pack;

  static PetPack? maybeOf(BuildContext context) {
    return context.dependOnInheritedWidgetOfExactType<PetPackScope>()?.pack;
  }

  @override
  bool updateShouldNotify(PetPackScope old) => old.pack != pack;
}

class PetView extends ConsumerStatefulWidget {
  const PetView({
    super.key,
    required this.dest,
    this.size = KimAvatarSize.sm,
    this.shape = KimAvatarShape.squircle,
    this.fallbackName = '',
    this.fallbackUrl = '',
    this.debugPack,
  });

  final String dest;
  final KimAvatarSize size;
  final KimAvatarShape shape;
  final String fallbackName;
  final String fallbackUrl;
  final PetPack? debugPack;

  @override
  ConsumerState<PetView> createState() => _PetViewState();
}

class _PetViewState extends ConsumerState<PetView>
    with SingleTickerProviderStateMixin {
  late final AnimationController _ctrl;
  PetPack? _pack;
  ui.Image? _image;
  var _failed = false;
  var _startedLoad = false;
  PetPhase? _phase;

  double get _px => switch (widget.size) {
    KimAvatarSize.sm => 36,
    KimAvatarSize.md => 48,
    KimAvatarSize.lg => 72,
  };

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 750),
    );
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_startedLoad) {
      _startedLoad = true;
      unawaited(_load());
    }
  }

  @override
  void didUpdateWidget(PetView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.debugPack != widget.debugPack) {
      _pack = null;
      _image?.dispose();
      _image = null;
      _failed = false;
      _phase = null;
      unawaited(_load());
      return;
    }
    if (oldWidget.dest != widget.dest) {
      _phase = null;
      if (_pack != null) {
        _applyPhase(ref.read(agentPresenceProvider(widget.dest)).phase);
      }
    }
  }

  @override
  void dispose() {
    _ctrl.dispose();
    _image?.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final injected = widget.debugPack ?? PetPackScope.maybeOf(context);
      final pack =
          injected ?? await PetPack.loadAsset(DefaultAssetBundle.of(context));
      final codec = await ui.instantiateImageCodec(pack.imageBytes);
      final frame = await codec.getNextFrame();
      if (!mounted) {
        frame.image.dispose();
        return;
      }
      _image?.dispose();
      setState(() {
        _pack = pack;
        _image = frame.image;
        _failed = false;
      });
      _applyPhase(ref.read(agentPresenceProvider(widget.dest)).phase);
    } catch (_) {
      if (mounted) {
        setState(() => _failed = true);
      }
    }
  }

  void _applyPhase(PetPhase phase) {
    final pack = _pack;
    if (pack == null) {
      return;
    }
    _phase = phase;
    final clip = pack.clipFor(phase);
    final duration = clip.duration;
    _ctrl.duration = duration.inMilliseconds <= 0
        ? const Duration(milliseconds: 1)
        : duration;
    _ctrl.reset();
    if (clip.loop) {
      unawaited(_ctrl.repeat());
    } else {
      unawaited(
        _ctrl.forward().then((_) {
          if (!mounted || _phase != phase) {
            return;
          }
          _settleAfterOneShot();
        }),
      );
    }
  }

  void _settleAfterOneShot() {
    final dest = widget.dest;
    ref.read(agentRunStatusProvider.notifier).consumeOneShot(dest);
    final status = ref.read(agentRunStatusProvider);
    final typing = ref.read(peerTypingProvider(dest));
    _applyPhase(
      typing || status.running.contains(dest)
          ? PetPhase.running
          : PetPhase.idle,
    );
  }

  @override
  Widget build(BuildContext context) {
    final presence = ref.watch(agentPresenceProvider(widget.dest));
    ref.listen<AgentPresence>(agentPresenceProvider(widget.dest), (prev, next) {
      if (next.phase != _phase) {
        _applyPhase(next.phase);
      }
    });

    final image = _image;
    final pack = _pack;
    if (_failed || image == null || pack == null) {
      return KimAvatar(
        name: widget.fallbackName,
        url: widget.fallbackUrl,
        size: widget.size,
        shape: widget.shape,
      );
    }

    return SizedBox(
      width: _px,
      height: _px,
      child: AnimatedBuilder(
        animation: _ctrl,
        builder: (context, _) {
          final phase = _phase ?? presence.phase;
          final clip = pack.clipFor(phase);
          final frames = clip.frames <= 0 ? 1 : clip.frames;
          final frame = (_ctrl.value * frames).floor().clamp(0, frames - 1);
          return CustomPaint(
            size: Size(_px, _px),
            painter: PetAtlasPainter(
              image: image,
              src: pack.srcRect(phase, frame),
              shape: widget.shape,
            ),
          );
        },
      ),
    );
  }
}
