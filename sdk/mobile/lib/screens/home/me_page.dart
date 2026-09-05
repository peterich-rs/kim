library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/experimental/mutation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:gap/gap.dart';
import 'package:go_router/go_router.dart';
import 'package:kim_media_picker/kim_media_picker.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:toastification/toastification.dart';

import '../../copy.dart';
import '../../core/ota_info.dart';
import '../../core/haptics.dart';
import '../../models/models.dart';
import '../../state/auth.dart';
import '../../state/link.dart';
import '../../state/mutations.dart';
import '../../state/profile.dart';
import '../../state/providers.dart';
import '../../state/session.dart';
import '../../theme/kim_theme.dart';
import '../../widgets/kim_avatar.dart';
import '../../widgets/kim_group.dart';
import '../../widgets/kim_header.dart';
import '../../widgets/status_chip.dart';

class MePage extends ConsumerWidget {
  const MePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final session = ref.watch(sessionProvider);
    final me = ref.watch(profileProvider);
    final runtime = ref.watch(runtimeProvider);
    final logout = ref.watch(signOutMutation);
    final settings = runtime.settings;
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final local = settings.httpOrigin.contains('127.0.0.1');
    final loggingOut = logout is MutationPending;

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          const KimSliverHeader(title: Copy.me),
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: KimTheme.raisedOf(context),
                  borderRadius: BorderRadius.circular(KimTheme.radiusCard),
                  border: Border.all(color: KimTheme.hairlineOf(context)),
                ),
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(18, 20, 18, 20),
                  child: Row(
                    children: [
                      _AvatarButton(
                        name: session.account.isEmpty ? '?' : session.account,
                        url: me.avatar,
                      ),
                      const Gap(16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              session.account,
                              style: theme.textTheme.headlineSmall?.copyWith(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            const Gap(6),
                            Row(
                              children: [
                                StatusDot(status: session.status),
                                const Gap(6),
                                Text(
                                  session.statusLabel,
                                  style: theme.textTheme.bodyMedium?.copyWith(
                                    color: scheme.onSurfaceVariant,
                                  ),
                                ),
                              ],
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
          SliverPadding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            sliver: SliverList.list(
              children: [
                _SectionLabel(Copy.accountSection),
                KimGroupCard(
                  children: [
                    ListTile(
                      leading: const Icon(LucideIcons.lock),
                      title: const Text(Copy.changePassword),
                      trailing: const Icon(LucideIcons.chevronRight, size: 18),
                      onTap: () => context.push('/password'),
                    ),
                    const Divider(indent: 56),
                    ListTile(
                      key: const Key('logout'),
                      leading: Icon(LucideIcons.logOut, color: scheme.error),
                      title: Text(
                        Copy.logout,
                        style: TextStyle(color: scheme.error),
                      ),
                      onTap: loggingOut
                          ? null
                          : () {
                              signOutMutation.run(ref, (tsx) async {
                                await tsx.get(authProvider.notifier).signOut();
                              });
                            },
                    ),
                  ],
                ),
                const Gap(18),
                _SectionLabel(Copy.generalSection),
                KimGroupCard(
                  children: [
                    ListTile(
                      leading: const Icon(LucideIcons.server),
                      title: const Text(Copy.environment),
                      subtitle: Text(
                        local ? Copy.localServer : Copy.prodServer,
                      ),
                    ),
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
                      child: SegmentedButton<bool>(
                        segments: const [
                          ButtonSegment(
                            value: false,
                            label: Text(Copy.prodServer),
                          ),
                          ButtonSegment(
                            value: true,
                            label: Text(Copy.localServer),
                          ),
                        ],
                        selected: {local},
                        onSelectionChanged: (next) async {
                          KimHaptics.selection();
                          if (next.first) {
                            await settings.useLocal();
                          } else {
                            await settings.useProd();
                          }
                          ref.read(linkProvider.notifier).retry();
                        },
                      ),
                    ),
                    const Divider(indent: 56),
                    ListTile(
                      leading: const Icon(LucideIcons.info),
                      title: const Text(Copy.about),
                      subtitle: Text('${Copy.brand} ${runtime.versionLabel}'),
                    ),
                    if (Platform.isAndroid) ...[
                      const Divider(indent: 56),
                      FutureBuilder<OtaInfo>(
                        future: OtaBridge.status(),
                        builder: (context, snap) {
                          final info = snap.data ?? OtaInfo.none;
                          return ListTile(
                            leading: const Icon(LucideIcons.package2),
                            title: const Text('Logic OTA'),
                            subtitle: Text(info.debugLabel),
                          );
                        },
                      ),
                    ],
                    if (session.status != ConnStatus.online)
                      ListTile(
                        leading: const Icon(LucideIcons.refreshCw),
                        title: const Text(Copy.retry),
                        subtitle: Text(session.connectError ?? Copy.offline),
                        onTap: () => ref.read(linkProvider.notifier).retry(),
                      ),
                  ],
                ),
                const Gap(48),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _AvatarButton extends ConsumerWidget {
  const _AvatarButton({required this.name, required this.url});

  final String name;
  final String url;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(avatarMutation) is MutationPending;
    return GestureDetector(
      onTap: busy ? null : () => _pick(context, ref),
      child: Stack(
        alignment: Alignment.center,
        children: [
          KimAvatar(name: name, url: url, size: KimAvatarSize.lg),
          if (busy)
            const SizedBox(
              width: 72,
              height: 72,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
        ],
      ),
    );
  }

  Future<void> _pick(BuildContext context, WidgetRef ref) async {
    final choice = await showModalBottomSheet<String>(
      context: context,
      builder: (ctx) {
        return SafeArea(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              ListTile(
                leading: const Icon(LucideIcons.camera),
                title: const Text(Copy.takePhoto),
                onTap: () => Navigator.pop(ctx, 'camera'),
              ),
              ListTile(
                leading: const Icon(LucideIcons.image),
                title: const Text(Copy.pickFromAlbum),
                onTap: () => Navigator.pop(ctx, 'album'),
              ),
              ListTile(
                title: const Text(Copy.cancel),
                onTap: () => Navigator.pop(ctx),
              ),
            ],
          ),
        );
      },
    );
    if (choice == null || !context.mounted) {
      return;
    }
    await Future<void>.delayed(const Duration(milliseconds: 350));
    if (!context.mounted) {
      return;
    }
    try {
      final asset = choice == 'camera'
          ? await KimMediaPicker.instance.takePhoto()
          : await KimMediaPicker.instance.pickSingle();
      if (asset == null) {
        return;
      }
      final file = File(asset.path);
      if (!file.existsSync()) {
        throw StateError(Copy.avatarExportFailed);
      }
      await avatarMutation.run(ref, (tsx) async {
        final bytes = await file.readAsBytes();
        final uploaded = await tsx
            .get(mediaPortProvider)
            .uploadImage(
              token: tsx.get(runtimeProvider).settings.token,
              bytes: bytes,
              contentType: asset.mimeType.isEmpty
                  ? 'image/jpeg'
                  : asset.mimeType,
            );
        await tsx.get(profileProvider.notifier).applyAvatar(uploaded.url);
      });
      if (context.mounted) {
        toastification.show(
          context: context,
          type: ToastificationType.success,
          style: ToastificationStyle.flatColored,
          title: const Text(Copy.avatarUpdated),
          autoCloseDuration: const Duration(seconds: 2),
          alignment: Alignment.topCenter,
        );
      }
    } on MissingPluginException {
      return;
    } on KimMediaPickerException catch (err) {
      if (context.mounted) {
        _toastFail(context, err.message);
      }
    } catch (err) {
      if (context.mounted) {
        _toastFail(context, _avatarError(err));
      }
    }
  }

  void _toastFail(BuildContext context, String message) {
    if (!context.mounted) {
      return;
    }
    toastification.show(
      context: context,
      type: ToastificationType.error,
      style: ToastificationStyle.flatColored,
      title: Text(message.isEmpty ? Copy.avatarFailed : message),
      autoCloseDuration: const Duration(seconds: 3),
      alignment: Alignment.topCenter,
    );
  }

  String _avatarError(Object err) {
    final msg = err.toString();
    if (msg.contains(Copy.avatarExportFailed)) {
      return Copy.avatarExportFailed;
    }
    if (msg.contains('unsupported media type') || msg.contains('415')) {
      return Copy.avatarUnsupportedType;
    }
    if (msg.contains('401') || msg.contains('unauthorized')) {
      return Copy.avatarRelogin;
    }
    if (msg.contains('too large') || msg.contains('413')) {
      return Copy.avatarFailed;
    }
    if (msg.contains('connect first') || msg.contains(Copy.notConnected)) {
      return Copy.notConnected;
    }
    if (msg.contains('upload') ||
        msg.contains('Socket') ||
        msg.contains('network')) {
      return Copy.network;
    }
    return Copy.avatarFailed;
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(4, 0, 4, 8),
      child: Text(
        text,
        style: Theme.of(context).textTheme.labelLarge
            ?.copyWith(color: Theme.of(context).colorScheme.onSurfaceVariant),
      ),
    );
  }
}
