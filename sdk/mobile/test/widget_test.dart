import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/main.dart';

void main() {
  testWidgets('shell shows WGateway fields', (tester) async {
    await tester.pumpWidget(const KimApp());
    expect(find.text('KIM (shell)'), findsOneWidget);
    expect(find.text('connect'), findsOneWidget);
    expect(find.text('login'), findsOneWidget);
  });
}
