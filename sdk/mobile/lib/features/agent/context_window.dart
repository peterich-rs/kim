/// Known model context windows. Unknown ids fall back to [kDefaultContextTokens].
library;

/// Used when the model id is not in the published table.
const kDefaultContextTokens = 256000;

/// Common picker sizes. The model's default is unioned in by [contextTokenOptions].
const kContextTokenPresets = <int>[
  32000,
  64000,
  128000,
  200000,
  256000,
  400000,
  500000,
  1000000,
];

/// Longest prefix first. Keep in sync with host `family_context_rules`.
const _kKnownContextPrefixes = <(String, int)>[
  ('claude-sonnet-4-5', 200000),
  ('claude-opus-4-5', 200000),
  ('claude-haiku-4-5', 200000),
  ('claude-sonnet-4-6', 1000000),
  ('claude-opus-4-6', 1000000),
  ('claude-opus-4-7', 1000000),
  ('claude-opus-4-8', 1000000),
  ('claude-sonnet-5', 1000000),
  ('claude-opus-5', 1000000),
  ('claude-haiku-5', 1000000),
  ('claude-fable', 1000000),
  ('claude-mythos', 1000000),
  ('claude-sonnet-4', 200000),
  ('claude-opus-4', 200000),
  ('claude-haiku-4', 200000),
  ('claude-', 200000),
  ('gpt-4.1', 1047576),
  ('gpt-4o', 128000),
  ('gpt-4-turbo', 128000),
  ('gpt-4-32k', 32768),
  ('gpt-4', 8192),
  ('gpt-5.4', 1050000),
  ('gpt-5.5', 1050000),
  ('gpt-5.6', 1050000),
  ('gpt-6', 1050000),
  ('gpt-5', 400000),
  ('o1', 200000),
  ('o3', 200000),
  ('o4', 200000),
  ('grok-4.20', 500000),
  ('grok-4.6', 500000),
  ('grok-4.5', 500000),
  ('grok-4.3', 1000000),
  ('grok-4', 256000),
  ('grok-3', 131072),
  ('grok', 256000),
  ('deepseek-v4', 1000000),
  ('deepseek-flash', 1000000),
  ('deepseek-chat', 128000),
  ('deepseek-reasoner', 128000),
  ('kimi-k2.7', 262144),
  ('kimi-k2.6', 262144),
  ('kimi-k2.5', 262144),
  ('kimi-k3', 1048576),
  ('kimi-k2', 262144),
  ('qwen3.8', 1000000),
  ('qwen3.7', 1000000),
  ('qwen3.6', 1000000),
  ('qwen3', 1000000),
  ('qwen-plus', 1000000),
  ('qwen-max', 1000000),
  ('qwen-long', 10000000),
  ('qwen-turbo', 128000),
  ('qwen-flash', 128000),
  ('glm-5', 1000000),
  ('glm-4.7', 200000),
  ('glm-4.6', 200000),
  ('glm-4.5', 128000),
  ('minimax-m3', 1000000),
  ('minimax-m2', 1000000),
  ('gemini-3', 1048576),
  ('gemini-2.5', 1048576),
  ('gemini-2.0', 1048576),
  ('gemini-1.5', 1048576),
];

String _modelBasename(String model) {
  final slash = model.lastIndexOf('/');
  final colon = model.lastIndexOf(':');
  final cut = slash > colon ? slash : colon;
  if (cut < 0 || cut + 1 >= model.length) {
    return model;
  }
  return model.substring(cut + 1);
}

bool _prefixMatches(String model, String prefix) {
  final id = model.toLowerCase();
  final pat = prefix.toLowerCase();
  if (!pat.contains('*')) {
    return id.startsWith(pat);
  }
  final parts = pat.split('*');
  var rest = id;
  if (parts.first.isNotEmpty) {
    if (!rest.startsWith(parts.first)) {
      return false;
    }
    rest = rest.substring(parts.first.length);
  }
  for (var i = 1; i < parts.length; i++) {
    final part = parts[i];
    if (part.isEmpty) {
      continue;
    }
    final idx = rest.indexOf(part);
    if (idx < 0) {
      return false;
    }
    rest = rest.substring(idx + part.length);
    if (i == parts.length - 1 && !pat.endsWith('*') && rest.isNotEmpty) {
      return false;
    }
  }
  return true;
}

/// Published window for [model], or [kDefaultContextTokens].
int defaultContextTokens(String model) {
  final full = model.trim();
  if (full.isEmpty) {
    return kDefaultContextTokens;
  }
  final base = _modelBasename(full);
  final ranked = [..._kKnownContextPrefixes]
    ..sort((a, b) => b.$1.length.compareTo(a.$1.length));
  for (final id in [full, base]) {
    for (final rule in ranked) {
      if (_prefixMatches(id, rule.$1)) {
        return rule.$2;
      }
    }
  }
  return kDefaultContextTokens;
}

/// Presets plus [modelDefault] and [current], unique, ascending.
List<int> contextTokenOptions({required int modelDefault, int? current}) {
  final seen = <int>{};
  final out = <int>[];
  for (final n in [...kContextTokenPresets, modelDefault, ?current]) {
    if (n <= 0 || !seen.add(n)) {
      continue;
    }
    out.add(n);
  }
  out.sort();
  return out;
}

String formatContextTokens(int tokens) {
  if (tokens <= 0) {
    return '0';
  }
  if (tokens == 1047576 || tokens == 1048576) {
    return '1M';
  }
  if (tokens == 1050000) {
    return '1.05M';
  }
  if (tokens == 262144) {
    return '256K';
  }
  if (tokens % 1000000 == 0) {
    return '${tokens ~/ 1000000}M';
  }
  if (tokens % 1000 == 0) {
    return '${tokens ~/ 1000}K';
  }
  return '$tokens';
}

/// Accepts `128000`, `128k`, `1m`, `1.05m`.
int? parseContextTokens(String raw) {
  var t = raw.trim().toLowerCase().replaceAll(',', '').replaceAll(' ', '');
  if (t.isEmpty) {
    return null;
  }
  var mult = 1.0;
  if (t.endsWith('m')) {
    mult = 1000000;
    t = t.substring(0, t.length - 1);
  } else if (t.endsWith('k')) {
    mult = 1000;
    t = t.substring(0, t.length - 1);
  }
  final value = double.tryParse(t);
  if (value == null || value <= 0) {
    return null;
  }
  final tokens = (value * mult).round();
  if (tokens < 1024 || tokens > 16000000) {
    return null;
  }
  return tokens;
}
