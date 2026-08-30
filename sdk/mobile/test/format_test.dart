import 'package:flutter_test/flutter_test.dart';
import 'package:kim_mobile/copy.dart';
import 'package:kim_mobile/core/format.dart';

void main() {
  final stamp = DateTime(2026, 8, 30, 14, 32);
  final ts = stamp.millisecondsSinceEpoch;

  test('formatMessageStamp uses today / yesterday / month-day / year', () {
    expect(
      formatMessageStamp(ts, now: DateTime(2026, 8, 30, 18)),
      '${Copy.today} 14:32',
    );
    expect(
      formatMessageStamp(ts, now: DateTime(2026, 8, 31, 9)),
      '${Copy.yesterday} 14:32',
    );
    expect(formatMessageStamp(ts, now: DateTime(2026, 9, 2)), '8月30日 14:32');
    expect(
      formatMessageStamp(ts, now: DateTime(2027, 1, 1)),
      '2026年8月30日 14:32',
    );
  });

  test('formatDateDivider drops the clock', () {
    expect(formatDateDivider(ts, now: DateTime(2026, 8, 30)), Copy.today);
    expect(formatDateDivider(ts, now: DateTime(2026, 8, 31)), Copy.yesterday);
    expect(formatDateDivider(ts, now: DateTime(2026, 9, 2)), '8月30日');
    expect(formatDateDivider(ts, now: DateTime(2027, 1, 1)), '2026年8月30日');
  });

  test('sameCalendarDay ignores time of day', () {
    expect(
      sameCalendarDay(DateTime(2026, 8, 30, 1), DateTime(2026, 8, 30, 23)),
      isTrue,
    );
    expect(
      sameCalendarDay(DateTime(2026, 8, 30), DateTime(2026, 8, 31)),
      isFalse,
    );
  });
}
