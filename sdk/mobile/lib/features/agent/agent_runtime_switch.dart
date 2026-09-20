import 'package:flutter/material.dart';
import 'package:gap/gap.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:kim_mobile/design/kim_group.dart';
import 'package:kim_mobile/features/agent/host_support.dart';

const agentRuntimePref = 'agent.runtime';

/// Session-open switch. Default stays Goose. The next message uses the
/// selected runtime; an in-flight Goose session is not migrated.
class AgentRuntimeSwitch extends StatefulWidget {
  const AgentRuntimeSwitch({super.key});

  @override
  State<AgentRuntimeSwitch> createState() => _AgentRuntimeSwitchState();
}

class _AgentRuntimeSwitchState extends State<AgentRuntimeSwitch> {
  var _codex = false;
  var _ready = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final prefs = await SharedPreferences.getInstance();
    if (!mounted) {
      return;
    }
    setState(() {
      _codex = prefs.getString(agentRuntimePref) == 'codex';
      _ready = true;
    });
  }

  Future<void> _set(bool codex) async {
    setState(() => _codex = codex);
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(agentRuntimePref, codex ? 'codex' : 'goose');
  }

  @override
  Widget build(BuildContext context) {
    if (!agentHostSupported) {
      return const SizedBox.shrink();
    }
    final zh = Localizations.localeOf(context).languageCode.startsWith('zh');
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          zh ? '运行时' : 'Runtime',
          style: theme.textTheme.titleSmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const Gap(8),
        KimGroupCard(
          children: [
            SwitchListTile(
              key: const Key('agent-runtime-codex'),
              title: Text(zh ? '用 Codex 跑这个助手' : 'Run this assistant on Codex'),
              subtitle: Text(
                zh
                    ? '默认仍是 Goose。从下一条消息生效，旧会话不会迁过去。'
                    : 'Goose stays the default. Applies on the next message. Existing sessions are not migrated.',
              ),
              value: _codex,
              onChanged: _ready ? (value) => _set(value) : null,
            ),
          ],
        ),
        const Gap(18),
      ],
    );
  }
}
