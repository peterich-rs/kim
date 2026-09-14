import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/core/env.dart';

void main() {
  test('default KIM_ENV is prod; debug still enables DevPanel', () {
    expect(kimEnv, KimEnv.prod);
    expect(kimDevPanelEnabled, isTrue);
  });
}
