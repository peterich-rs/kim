library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../agent_bridge.dart';

const _kMode = 'agent.llm_backend';
const _kBaseUrl = 'agent.base_url';
const _kModel = 'agent.model';
const _kFsTools = 'agent.enable_fs_tools';
const _kBash = 'agent.bash_enabled';
const _kApiKey = 'agent.api_key';

class AgentSettings {
  const AgentSettings({
    required this.llmBackend,
    required this.baseUrl,
    required this.model,
    required this.apiKey,
    required this.enableFsTools,
    required this.bashEnabled,
  });

  final String llmBackend;
  final String baseUrl;
  final String model;
  final String apiKey;
  final bool enableFsTools;
  final bool bashEnabled;

  bool get isLive =>
      llmBackend == 'responses_http' ||
      llmBackend == 'live' ||
      llmBackend == 'responses';

  AgentSettings copyWith({
    String? llmBackend,
    String? baseUrl,
    String? model,
    String? apiKey,
    bool? enableFsTools,
    bool? bashEnabled,
  }) {
    return AgentSettings(
      llmBackend: llmBackend ?? this.llmBackend,
      baseUrl: baseUrl ?? this.baseUrl,
      model: model ?? this.model,
      apiKey: apiKey ?? this.apiKey,
      enableFsTools: enableFsTools ?? this.enableFsTools,
      bashEnabled: bashEnabled ?? this.bashEnabled,
    );
  }

  SessionOpenOpts toOpts({bool resumeOnOpen = true}) {
    return SessionOpenOpts(
      model: model,
      llmBackend: llmBackend,
      resumeOnOpen: resumeOnOpen,
      baseUrl: baseUrl,
      apiKey: apiKey,
      enableFsTools: enableFsTools,
      bashEnabled: bashEnabled,
    );
  }

  static const defaults = AgentSettings(
    llmBackend: 'scripted',
    baseUrl: 'https://api.openai.com/v1',
    model: 'scripted',
    apiKey: '',
    enableFsTools: true,
    bashEnabled: true,
  );
}

class AgentSettingsNotifier extends Notifier<AgentSettings> {
  final _secure = const FlutterSecureStorage();

  @override
  AgentSettings build() {
    Future.microtask(reload);
    return AgentSettings.defaults;
  }

  Future<void> reload() async {
    final prefs = await SharedPreferences.getInstance();
    final key = await _secure.read(key: _kApiKey) ?? '';
    state = AgentSettings(
      llmBackend: prefs.getString(_kMode) ?? AgentSettings.defaults.llmBackend,
      baseUrl: prefs.getString(_kBaseUrl) ?? AgentSettings.defaults.baseUrl,
      model: prefs.getString(_kModel) ?? AgentSettings.defaults.model,
      apiKey: key,
      enableFsTools:
          prefs.getBool(_kFsTools) ?? AgentSettings.defaults.enableFsTools,
      bashEnabled: prefs.getBool(_kBash) ?? AgentSettings.defaults.bashEnabled,
    );
  }

  Future<void> save(AgentSettings next) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_kMode, next.llmBackend);
    await prefs.setString(_kBaseUrl, next.baseUrl);
    await prefs.setString(_kModel, next.model);
    await prefs.setBool(_kFsTools, next.enableFsTools);
    await prefs.setBool(_kBash, next.bashEnabled);
    if (next.apiKey.isEmpty) {
      await _secure.delete(key: _kApiKey);
    } else {
      await _secure.write(key: _kApiKey, value: next.apiKey);
    }
    state = next;
  }
}

final agentSettingsProvider =
    NotifierProvider<AgentSettingsNotifier, AgentSettings>(
      AgentSettingsNotifier.new,
    );
