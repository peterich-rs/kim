import 'package:flutter/widgets.dart';

import 'app.dart';
import 'core/runtime.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final runtime = await KimRuntime.bootstrap();
  runApp(KimApp(runtime: runtime));
}
