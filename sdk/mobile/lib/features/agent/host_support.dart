/// Local Goose host is a desktop IM process (bash/fs/MCP on the machine).
library;

import 'package:flutter/foundation.dart';

/// Phone clients are IM only. Do not compile or init `kim_agent_ffi` on
/// iOS / Android / web.
bool get agentHostSupported {
  if (kIsWeb) {
    return false;
  }
  return switch (defaultTargetPlatform) {
    TargetPlatform.macOS ||
    TargetPlatform.windows ||
    TargetPlatform.linux => true,
    _ => false,
  };
}
