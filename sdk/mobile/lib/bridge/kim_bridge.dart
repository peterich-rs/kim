/// Production FFI facade: auth + session client + media.
library;

import 'package:kim_mobile/bridge/kim_auth_bridge.dart';
import 'package:kim_mobile/bridge/kim_bridge_base.dart';
import 'package:kim_mobile/bridge/kim_client_bridge.dart';
import 'package:kim_mobile/bridge/kim_media_bridge.dart';
import 'package:kim_mobile/bridge/kim_ports.dart';
import 'package:kim_mobile/core/media.dart';

export 'package:kim_mobile/bridge/kim_ports.dart';

class KimBridge extends KimBridgeBase
    with KimAuthBridge, KimClientBridge, KimMediaBridge
    implements KimAuthPort, KimClientPort, KimMediaPort {}
